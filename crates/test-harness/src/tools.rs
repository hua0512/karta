//! External tools in tests: fake `ffmpeg` / `ffprobe` / `yt-dlp` scripts,
//! and a gate for tests that need the real binary.
//!
//! Real-tool tests are `#[ignore]`d and start with
//! `if !require_tool("ffmpeg") { return; }`. Locally a missing tool skips the
//! test; with `KARTA_REQUIRE_TOOLS=1` (the CI tools job) it fails instead.
//!
//! ```ignore
//! let dir = tempfile::tempdir()?;
//! let ffmpeg = FakeTool::new()
//!     .writes(OutputArg::Last, b"merged".to_vec())
//!     .install(dir.path(), "ffmpeg");
//! let config = Config { ffmpeg_path: Some(ffmpeg.path_string()), ..Config::default() };
//! // ... run the code under test ...
//! assert_eq!(ffmpeg.invocations().len(), 1);
//! ```

use std::path::Path;
#[cfg(unix)]
use std::path::PathBuf;

/// Environment variable that turns a missing tool into a test failure.
pub const REQUIRE_TOOLS_ENV: &str = "KARTA_REQUIRE_TOOLS";

/// Whether `name` is on `PATH`. When it is not: panics if
/// [`REQUIRE_TOOLS_ENV`] is `1`, else prints a skip notice and returns
/// `false`.
#[track_caller]
pub fn require_tool(name: &str) -> bool {
    let found = std::env::var_os("PATH").is_some_and(|paths| {
        std::env::split_paths(&paths).any(|dir| is_executable(&dir.join(name)))
    });
    if found {
        return true;
    }
    if std::env::var(REQUIRE_TOOLS_ENV).as_deref() == Ok("1") {
        panic!("{name} is required ({REQUIRE_TOOLS_ENV}=1) but not on PATH");
    }
    eprintln!("skipping: {name} not on PATH (set {REQUIRE_TOOLS_ENV}=1 to fail instead)");
    false
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata()
        .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

/// Where a fake tool writes its scripted output file.
#[cfg(unix)]
#[derive(Clone, Debug)]
pub enum OutputArg {
    /// The last argument (ffmpeg's output path).
    Last,
    /// The argument following this flag (yt-dlp's `-o`). A yt-dlp
    /// `%(ext)s` placeholder in it becomes the `--audio-format` or
    /// `--merge-output-format` value, as yt-dlp would name the file.
    After(&'static str),
}

/// A fake executable: records its argv, prints scripted stdout/stderr,
/// optionally writes an output file, and exits with a scripted code.
#[cfg(unix)]
#[derive(Clone, Debug, Default)]
pub struct FakeTool {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    exit_code: i32,
    output: Option<(OutputArg, Vec<u8>)>,
    input_flag: Option<&'static str>,
}

#[cfg(unix)]
impl FakeTool {
    /// Exits 0 with no output.
    pub fn new() -> Self {
        Self::default()
    }

    pub fn stdout(mut self, stdout: impl Into<Vec<u8>>) -> Self {
        self.stdout = stdout.into();
        self
    }

    pub fn stderr(mut self, stderr: impl Into<Vec<u8>>) -> Self {
        self.stderr = stderr.into();
        self
    }

    pub fn exit_code(mut self, code: i32) -> Self {
        self.exit_code = code;
        self
    }

    /// Write `bytes` to the path named by `arg`.
    pub fn writes(mut self, arg: OutputArg, bytes: impl Into<Vec<u8>>) -> Self {
        self.output = Some((arg, bytes.into()));
        self
    }

    /// Fail when a file named after `flag` is missing when the tool runs.
    pub fn requires_files_after(mut self, flag: &'static str) -> Self {
        self.input_flag = Some(flag);
        self
    }

    /// Install as `dir/name` (sidecar files `dir/name.*` hold the script data).
    ///
    /// Returns once the script can be executed: a process forked by a
    /// concurrent test while the script was open for writing keeps it busy
    /// (`ETXTBSY`) until that child execs.
    pub fn install(&self, dir: &Path, name: &str) -> InstalledTool {
        use std::os::unix::fs::PermissionsExt;

        let path = dir.join(name);
        let sidecar = |ext: &str| dir.join(format!("{name}.{ext}"));
        let log = sidecar("argv");
        std::fs::write(sidecar("stdout"), &self.stdout).expect("write fake stdout");
        std::fs::write(sidecar("stderr"), &self.stderr).expect("write fake stderr");

        let output = match &self.output {
            None => String::new(),
            Some((arg, bytes)) => {
                std::fs::write(sidecar("output"), bytes).expect("write fake output");
                let target = match arg {
                    OutputArg::Last => "for a in \"$@\"; do target=$a; done\n".to_string(),
                    OutputArg::After(flag) => format!(
                        "prev=\n\
                         ext=\n\
                         for a in \"$@\"; do\n\
                         [ \"$prev\" = {flag} ] && target=$a\n\
                         case \"$prev\" in --audio-format|--merge-output-format) ext=$a ;; esac\n\
                         prev=$a\n\
                         done\n\
                         case \"$target\" in *'%(ext)s') target=\"${{target%'%(ext)s'}}${{ext:-mp4}}\" ;; esac\n",
                        flag = shell_quote(flag)
                    ),
                };
                // An option in last place (`ffmpeg -version`) is no output.
                format!(
                    "target=\n{target}case \"$target\" in -*) target= ;; esac\n\
                     [ -n \"$target\" ] && cp {src} \"$target\"\n",
                    src = shell_quote(&sidecar("output").to_string_lossy())
                )
            }
        };

        let check_inputs = self.input_flag.map_or(String::new(), |flag| {
            format!(
                "prev=\nfor arg in \"$@\"; do\n\
             if [ \"$prev\" = {flag} ] && [ ! -f \"$arg\" ]; then\n\
             echo \"missing input: $arg\" >&2; exit 1\nfi\nprev=$arg\ndone\n",
                flag = shell_quote(flag),
            )
        });

        // One line per invocation, arguments separated by U+001F.
        let script = format!(
            "#!/bin/sh\n\
             [ -n \"$FAKE_TOOL_PROBE\" ] && exit 0\n\
             {{ printf '%s\\037' \"$@\"; printf '\\n'; }} >> {log}\n\
             {check_inputs}\
             {output}\
             cat {stdout}\n\
             cat {stderr} >&2\n\
             exit {code}\n",
            log = shell_quote(&log.to_string_lossy()),
            stdout = shell_quote(&sidecar("stdout").to_string_lossy()),
            stderr = shell_quote(&sidecar("stderr").to_string_lossy()),
            code = self.exit_code,
        );
        std::fs::write(&path, script).expect("write fake tool");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .expect("chmod fake tool");
        wait_until_executable(&path);
        InstalledTool { path, log }
    }
}

/// A fake tool on disk.
#[cfg(unix)]
pub struct InstalledTool {
    path: PathBuf,
    log: PathBuf,
}

#[cfg(unix)]
impl InstalledTool {
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// `path()` as the `String` configs take.
    pub fn path_string(&self) -> String {
        self.path.to_string_lossy().into_owned()
    }

    /// The argv of every invocation so far.
    pub fn invocations(&self) -> Vec<Vec<String>> {
        let log = std::fs::read_to_string(&self.log).unwrap_or_default();
        log.lines()
            .map(|line| {
                line.split('\u{1f}')
                    .filter(|arg| !arg.is_empty())
                    .map(str::to_string)
                    .collect()
            })
            .collect()
    }
}

/// Run the script in probe mode (exits before doing anything) until it is no
/// longer busy.
#[cfg(unix)]
fn wait_until_executable(path: &Path) {
    for _ in 0..1000 {
        match std::process::Command::new(path)
            .env("FAKE_TOOL_PROBE", "1")
            .status()
        {
            Err(e) if e.kind() == std::io::ErrorKind::ExecutableFileBusy => {
                std::thread::yield_now();
            }
            result => {
                let status = result.expect("run fake tool");
                assert!(status.success(), "fake tool probe failed: {status}");
                return;
            }
        }
    }
    panic!("fake tool {} stayed busy", path.display());
}

#[cfg(unix)]
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}
