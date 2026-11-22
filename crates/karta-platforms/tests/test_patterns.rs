//! Pattern matching integration test.
//!
//! Reads the URL catalog (`test_urls.md`) and checks every row against the
//! `ParserRegistry` built by `create_all_parsers`: the platform, the pattern
//! picked first (which pins cross-platform precedence), the capture groups and,
//! when given, the full match. The catalog format is described in its header.

use std::sync::Arc;

use karta_core::config::Config;
use karta_core::constants::Platform;
use karta_core::download::StreamDownloader;
use karta_core::parser::ParserRegistry;

const CATALOG: &str = include_str!("test_urls.md");
const ROW_HEADER: &str = "| input | pattern | captures | match |";
const NO_MATCH_HEADER: &str = "| input |";

#[derive(Debug)]
struct Row {
    line: usize,
    platform: Platform,
    input: String,
    pattern: String,
    /// Groups 1.. in order; `None` for a non-participating group.
    captures: Vec<Option<String>>,
    full_match: Option<String>,
}

#[derive(Default)]
struct Catalog {
    rows: Vec<Row>,
    no_match: Vec<(usize, String)>,
}

enum Section {
    Platform(Platform),
    NoMatch,
}

fn section_for(heading: &str) -> Option<Section> {
    let platform = match heading {
        "No match" => return Some(Section::NoMatch),
        "YouTube" => Platform::YouTube,
        "Kuaishou" => Platform::Kuaishou,
        "Hongguo" => Platform::Hongguo,
        "Bilibili" => Platform::Bilibili,
        "TikTok" => Platform::TikTok,
        "AcFun" => Platform::AcFun,
        "X / Twitter" => Platform::Twitter,
        "Douyin" => Platform::Douyin,
        "Xiaohongshu" => Platform::Xiaohongshu,
        "Weibo" => Platform::Weibo,
        "NGA" => Platform::Nga,
        "Xiaoheihe" => Platform::Xiaoheihe,
        "Instagram" => Platform::Instagram,
        "V2EX" => Platform::V2ex,
        "WeChat Article" => Platform::Wechat,
        "WeChat Channel" => Platform::WechatChannel,
        _ => return None,
    };
    Some(Section::Platform(platform))
}

/// Text between the backticks of a `` `value` `` cell.
fn code_span(cell: &str, line: usize) -> String {
    cell.strip_prefix('`')
        .and_then(|rest| rest.strip_suffix('`'))
        .filter(|value| !value.contains('`'))
        .unwrap_or_else(|| panic!("test_urls.md:{line}: expected a `code` cell, got {cell:?}"))
        .to_string()
}

/// A captures cell: `` `a` - `b` `` → `[Some(a), None, Some(b)]`.
fn parse_captures(cell: &str, line: usize) -> Vec<Option<String>> {
    let mut captures = Vec::new();
    let mut rest = cell.trim();
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix('-') {
            captures.push(None);
            rest = after.trim_start();
        } else if let Some(after) = rest.strip_prefix('`') {
            let end = after
                .find('`')
                .unwrap_or_else(|| panic!("test_urls.md:{line}: unclosed capture in {cell:?}"));
            captures.push(Some(after[..end].to_string()));
            rest = after[end + 1..].trim_start();
        } else {
            panic!("test_urls.md:{line}: captures must be `value` or -, got {cell:?}");
        }
    }
    captures
}

/// Parse `test_urls.md`. Panics on an unknown heading or a malformed row so a
/// typo cannot silently drop coverage.
fn load_catalog() -> Catalog {
    let mut catalog = Catalog::default();
    let mut section: Option<Section> = None;
    let mut header_seen = false;

    for (index, raw) in CATALOG.lines().enumerate() {
        let line = index + 1;
        let trimmed = raw.trim();

        if let Some(heading) = trimmed.strip_prefix("## ") {
            section = Some(
                section_for(heading.trim())
                    .unwrap_or_else(|| panic!("test_urls.md:{line}: unknown heading {heading:?}")),
            );
            header_seen = false;
            continue;
        }
        if trimmed.starts_with('#') {
            assert!(
                section.is_none(),
                "test_urls.md:{line}: only `## <Platform>` headings may follow the intro"
            );
            continue;
        }
        let Some(section) = &section else {
            continue;
        };
        // Prose lists are allowed in the intro only; a list item inside a
        // section would be a row the test never reads.
        assert!(
            !trimmed.starts_with("- "),
            "test_urls.md:{line}: rows must be table rows, got {trimmed:?}"
        );
        if !trimmed.starts_with('|') {
            continue;
        }

        let expected_header = match section {
            Section::Platform(_) => ROW_HEADER,
            Section::NoMatch => NO_MATCH_HEADER,
        };
        if !header_seen {
            assert_eq!(
                trimmed, expected_header,
                "test_urls.md:{line}: unexpected table header"
            );
            header_seen = true;
            continue;
        }
        if trimmed.trim_matches(|c| c == '|' || c == '-').is_empty() {
            continue;
        }

        let cells: Vec<&str> = trimmed
            .strip_prefix('|')
            .and_then(|row| row.strip_suffix('|'))
            .unwrap_or_else(|| panic!("test_urls.md:{line}: row must start and end with |"))
            .split('|')
            .map(str::trim)
            .collect();

        match section {
            Section::NoMatch => {
                assert_eq!(cells.len(), 1, "test_urls.md:{line}: expected 1 cell");
                catalog.no_match.push((line, code_span(cells[0], line)));
            }
            Section::Platform(platform) => {
                assert_eq!(cells.len(), 4, "test_urls.md:{line}: expected 4 cells");
                assert!(
                    !cells[1].is_empty(),
                    "test_urls.md:{line}: pattern is required"
                );
                catalog.rows.push(Row {
                    line,
                    platform: *platform,
                    input: code_span(cells[0], line),
                    pattern: cells[1].to_string(),
                    captures: parse_captures(cells[2], line),
                    full_match: (!cells[3].is_empty()).then(|| code_span(cells[3], line)),
                });
            }
        }
    }

    catalog
}

/// Registry over the real parsers. Matching does no I/O; the temp data dir is
/// returned only to outlive the parsers.
fn make_registry(config: &Config) -> (tempfile::TempDir, ParserRegistry) {
    let data_dir = tempfile::tempdir().expect("temp data dir");
    let downloader = Arc::new(
        StreamDownloader::with_client(config, data_dir.path(), wreq::Client::new())
            .expect("create test downloader"),
    );
    let parsers = karta_platforms::create_all_parsers(config, downloader, data_dir.path());
    (data_dir, ParserRegistry::new(parsers))
}

/// Describe a mismatch between a row and what the registry matched, if any.
fn check_row(registry: &ParserRegistry, row: &Row) -> Option<String> {
    let Some((parser, matched)) = registry.search(&row.input) else {
        return Some(format!(
            "expected {:?} / {}, got no match",
            row.platform, row.pattern
        ));
    };
    if parser.platform() != row.platform || matched.pattern != row.pattern {
        return Some(format!(
            "expected {:?} / {}, got {:?} / {}",
            row.platform,
            row.pattern,
            parser.platform(),
            matched.pattern
        ));
    }

    // One past the listed groups must not participate either, so every
    // participating group has to be listed.
    let actual: Vec<Option<&str>> = (1..=row.captures.len() + 1)
        .map(|index| matched.group(index))
        .collect();
    let mut expected: Vec<Option<&str>> = row.captures.iter().map(Option::as_deref).collect();
    expected.push(None);
    if actual != expected {
        return Some(format!("captures: expected {expected:?}, got {actual:?}"));
    }

    if let Some(full_match) = &row.full_match
        && matched.full_match != full_match
    {
        return Some(format!(
            "full match: expected {full_match:?}, got {:?}",
            matched.full_match
        ));
    }
    None
}

#[test]
fn catalog_rows_match_platform_pattern_and_captures() {
    let (_data_dir, registry) = make_registry(&Config::default());
    let catalog = load_catalog();
    assert!(!catalog.rows.is_empty(), "test_urls.md should have rows");

    let failures: Vec<String> = catalog
        .rows
        .iter()
        .filter_map(|row| {
            check_row(&registry, row)
                .map(|why| format!("test_urls.md:{} {:?}\n  {why}", row.line, row.input))
        })
        .collect();

    assert!(
        failures.is_empty(),
        "\n{} of {} catalog rows failed:\n\n{}\n",
        failures.len(),
        catalog.rows.len(),
        failures.join("\n\n")
    );
}

#[test]
fn catalog_no_match_texts_are_not_claimed() {
    let (_data_dir, registry) = make_registry(&Config::default());
    let catalog = load_catalog();
    assert!(
        !catalog.no_match.is_empty(),
        "test_urls.md should have a No match section"
    );

    let failures: Vec<String> = catalog
        .no_match
        .iter()
        .filter_map(|(line, text)| {
            registry.search(text).map(|(parser, matched)| {
                format!(
                    "test_urls.md:{line} {text:?}\n  claimed by {:?} / {} ({:?})",
                    parser.platform(),
                    matched.pattern,
                    matched.full_match
                )
            })
        })
        .collect();

    assert!(
        failures.is_empty(),
        "\n{} no-match texts were claimed:\n\n{}\n",
        failures.len(),
        failures.join("\n\n")
    );
}

#[test]
fn catalog_covers_every_platform() {
    let catalog = load_catalog();
    for platform in Platform::ALL {
        assert!(
            catalog.rows.iter().any(|row| row.platform == *platform),
            "test_urls.md has no rows for {platform:?}"
        );
    }
}

#[test]
fn disabled_platforms_are_not_registered() {
    let disabled = [
        Platform::Bilibili,
        Platform::Douyin,
        Platform::WechatChannel,
    ];
    let config = Config {
        disabled_platforms: disabled.to_vec(),
        ..Config::default()
    };
    let (_data_dir, registry) = make_registry(&config);

    let registered: Vec<Platform> = registry.parsers().iter().map(|p| p.platform()).collect();
    for platform in Platform::ALL {
        assert_eq!(
            registered.contains(platform),
            !disabled.contains(platform),
            "registration of {platform:?}"
        );
    }
    assert_eq!(registered.len(), Platform::ALL.len() - disabled.len());

    for text in [
        "https://www.bilibili.com/video/BV1xx411c7mD",
        "https://www.douyin.com/video/7521023890996514083",
        "https://weixin.qq.com/sph/AbCd123",
    ] {
        assert!(
            registry.search(text).is_none(),
            "disabled platform still claims {text}"
        );
    }
    assert_eq!(
        registry.matched_platform("https://x.com/user/status/123"),
        Some(Platform::Twitter)
    );
}
