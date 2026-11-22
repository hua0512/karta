use std::sync::Arc;

use async_trait::async_trait;
use regex::{Regex, RegexSet};
use smallvec::SmallVec;

use crate::constants::Platform;
use crate::data::ParseResult;
use crate::error::{ParserError, Result};

/// Typed match result that decouples parsers from `regex::Captures`.
pub struct MatchResult<'a> {
    /// Pattern name (e.g. "bv", "b23", "douyin").
    pub pattern: &'a str,
    /// Entire matched text (group 0).
    pub full_match: &'a str,
    /// Capture groups (index 1+). `None` for non-participating groups.
    groups: SmallVec<[Option<&'a str>; 4]>,
}

impl<'a> MatchResult<'a> {
    /// Build from a pattern name and regex captures.
    pub fn from_captures(pattern: &'a str, captures: &regex::Captures<'a>) -> Self {
        let full_match = captures.get(0).map(|m| m.as_str()).unwrap_or("");
        let mut groups = SmallVec::new();
        // Store groups 1..N
        for i in 1..captures.len() {
            groups.push(captures.get(i).map(|m| m.as_str()));
        }
        Self {
            pattern,
            full_match,
            groups,
        }
    }

    /// Get capture group by index (1-based, matching regex convention).
    /// Returns `None` if the group doesn't exist or didn't participate.
    pub fn group(&self, index: usize) -> Option<&'a str> {
        if index == 0 {
            return Some(self.full_match);
        }
        self.groups.get(index - 1).copied().flatten()
    }

    /// Get a required capture group, returning an error if missing.
    pub fn require_group(&self, index: usize) -> Result<&'a str> {
        self.group(index).ok_or_else(|| {
            ParserError::Parse(format!(
                "Missing capture group {} in pattern '{}'",
                index, self.pattern
            ))
        })
    }
}

/// Parser trait for all platform parsers.
#[async_trait]
pub trait Parser: Send + Sync {
    /// Get the platform this parser handles.
    fn platform(&self) -> Platform;

    /// Patterns this parser recognizes in input text.
    fn patterns(&self) -> &[RegisteredPattern];

    /// Parse content from a match result.
    async fn parse(&self, match_result: MatchResult<'_>) -> Result<ParseResult>;
}

#[derive(Clone)]
struct PatternEntry {
    // Hold a clone of the Arc so search() can return it cheaply.
    parser: Arc<dyn Parser>,
    pattern_name: &'static str,
    // Regex::clone is cheap (Arc internally).
    regex: Regex,
}

/// Parser registry that holds all platform parsers
pub struct ParserRegistry {
    parsers: Vec<Arc<dyn Parser>>,
    // Fast first-pass matching across all patterns.
    pattern_set: RegexSet,
    // Flattened patterns in precedence order: parser order, then pattern
    // order within each parser.
    patterns: Vec<PatternEntry>,
}

impl ParserRegistry {
    /// Create a new parser registry from a pre-built list of parsers.
    pub fn new(parsers: Vec<Arc<dyn Parser>>) -> Self {
        // Build a flattened pattern list in precedence order and a RegexSet for fast filtering.
        let mut pattern_strs: Vec<&str> = Vec::new();
        let mut patterns: Vec<PatternEntry> = Vec::new();
        for parser in &parsers {
            for pat in parser.patterns() {
                pattern_strs.push(pat.regex.as_str());
                patterns.push(PatternEntry {
                    parser: parser.clone(),
                    pattern_name: pat.name,
                    regex: pat.regex.clone(),
                });
            }
        }

        let pattern_set = RegexSet::new(pattern_strs).expect("valid parser regex set");

        tracing::info!(
            parsers = parsers.len(),
            patterns = patterns.len(),
            "Parser registry initialized"
        );

        Self {
            parsers,
            pattern_set,
            patterns,
        }
    }

    /// Search for a matching parser and pattern in the text.
    ///
    /// Returns the parser and a `MatchResult` on success. The `MatchResult`
    /// borrows only from `text`, not from the registry.
    pub fn search<'t>(&self, text: &'t str) -> Option<(Arc<dyn Parser>, MatchResult<'t>)> {
        let matches = self.pattern_set.matches(text);
        for idx in matches.iter() {
            let entry = self.patterns.get(idx)?;
            if let Some(captures) = entry.regex.captures(text) {
                let match_result = MatchResult::from_captures(entry.pattern_name, &captures);
                return Some((entry.parser.clone(), match_result));
            }
        }
        tracing::trace!("No pattern matched");
        None
    }

    /// Check whether any registered pattern matches, without extracting captures.
    pub fn is_match(&self, text: &str) -> bool {
        self.pattern_set.is_match(text)
    }

    /// Identify the platform of the first matching pattern (in precedence
    /// order), without extracting captures.
    pub fn matched_platform(&self, text: &str) -> Option<Platform> {
        let idx = self.pattern_set.matches(text).iter().next()?;
        self.patterns.get(idx).map(|entry| entry.parser.platform())
    }

    /// Get all registered parsers
    pub fn parsers(&self) -> &[Arc<dyn Parser>] {
        &self.parsers
    }
}

/// Declare parser patterns concisely.
///
/// # Example
/// ```ignore
/// let patterns = karta_core::patterns![
///     "bv" => r"BV[0-9a-zA-Z]{10}",
///     "av" => r"av(\d+)",
/// ];
/// ```
#[macro_export]
macro_rules! patterns {
    ($($name:literal => $regex:literal),+ $(,)?) => {
        vec![$($crate::parser::RegisteredPattern::new_static($name, $regex)),+]
    };
}

/// A registered pattern
pub struct RegisteredPattern {
    pub name: &'static str,
    pub regex: Regex,
}

impl RegisteredPattern {
    /// Create a new pattern from static strings.
    ///
    /// Panics if the regex is invalid — this surfaces invalid regexes immediately.
    pub fn new_static(name: &'static str, pattern: &'static str) -> Self {
        let regex = Regex::new(pattern).expect("valid registered pattern regex");
        Self { name, regex }
    }
}

/// Try to match text against a parser's patterns (test helper).
///
/// Returns `(pattern_name, MatchResult)` on first match.
pub fn try_match_parser<'a>(parser: &'a dyn Parser, text: &'a str) -> Option<MatchResult<'a>> {
    for pat in parser.patterns() {
        if let Some(captures) = pat.regex.captures(text) {
            return Some(MatchResult::from_captures(pat.name, &captures));
        }
    }
    None
}
