//! A throwaway data directory with a `config.json`, and engines built on it.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use karta_core::download::SharedDownloader;
use karta_core::parser::Parser;
use karta_core::{Config, ParserEngine};
use tempfile::TempDir;

/// Temp data path whose `config.json` is the default config with
/// `overrides` (a JSON object, top-level keys) applied. The config is loaded
/// back through [`Config::load`], exactly as production reads it.
pub struct TempApp {
    dir: TempDir,
    config: Config,
}

impl TempApp {
    /// Build the real platform parsers on a fake upstream, retaining their data directory.
    #[cfg(feature = "platforms")]
    pub fn with_platforms(
        fake: &crate::FakeUpstream,
        overrides: serde_json::Value,
    ) -> (Self, ParserEngine) {
        let app = Self::new(overrides);
        let engine = app.platform_engine(fake.downloader(app.config(), app.data_path()));
        (app, engine)
    }

    /// Panics if `overrides` is not an object or the result does not parse.
    #[track_caller]
    pub fn new(overrides: serde_json::Value) -> Self {
        let dir = tempfile::tempdir().expect("create temp data dir");
        let serde_json::Value::Object(overrides) = overrides else {
            panic!("config overrides must be a JSON object");
        };
        let mut config = serde_json::to_value(Config::default()).expect("serialize default config");
        let fields = config
            .as_object_mut()
            .expect("config serializes to an object");
        fields.extend(overrides);
        std::fs::write(
            dir.path().join("config.json"),
            serde_json::to_vec_pretty(&config).expect("serialize config"),
        )
        .expect("write config.json");

        let config = Config::load(dir.path());
        assert!(
            !dir.path().join("config.json.bad").exists(),
            "config.json with these overrides does not parse"
        );
        Self { dir, config }
    }

    pub fn data_path(&self) -> &Path {
        self.dir.path()
    }

    /// The loaded config.
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Sorted names of entries in the media cache.
    pub fn cache_entries(&self) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(Config::cache_dir(self.data_path()))
            .expect("read media cache")
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    /// An engine over `parsers` (e.g. [`crate::FakeParser`]s).
    pub fn engine(
        &self,
        downloader: SharedDownloader,
        parsers: Vec<Arc<dyn Parser>>,
    ) -> ParserEngine {
        ParserEngine::with_parsers(
            self.config.clone(),
            PathBuf::from(self.data_path()),
            downloader,
            parsers,
        )
    }

    /// An engine over the real platform parsers.
    #[cfg(feature = "platforms")]
    pub fn platform_engine(&self, downloader: SharedDownloader) -> ParserEngine {
        let parsers =
            karta_platforms::create_all_parsers(&self.config, downloader.clone(), self.data_path());
        self.engine(downloader, parsers)
    }
}
