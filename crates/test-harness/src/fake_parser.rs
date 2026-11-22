//! A [`Parser`] with fixed patterns and a scripted outcome.

use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use karta_core::error::{ParserError, Result};
use karta_core::parser::{MatchResult, Parser, RegisteredPattern};
use karta_core::{ParseResult, Platform};

type Outcome = Box<dyn Fn() -> Result<ParseResult> + Send + Sync>;

/// Counts calls and records each matched text.
///
/// ```ignore
/// let parser = Arc::new(
///     FakeParser::new(Platform::Bilibili, &[("bv", r"BV[0-9A-Za-z]{10}")])
///         .returning(ParseResult::new(Platform::Bilibili).with_title("t")),
/// );
/// let engine = app.engine(downloader, vec![parser.clone()]);
/// ```
pub struct FakeParser {
    platform: Platform,
    patterns: Vec<RegisteredPattern>,
    outcome: Outcome,
    delay: Option<Duration>,
    hang: bool,
    calls: AtomicUsize,
    inputs: Mutex<Vec<String>>,
}

impl FakeParser {
    /// A parser for `platform` matching `(name, regex)` patterns in order.
    /// Until scripted it returns an empty result for `platform`.
    pub fn new(platform: Platform, patterns: &[(&'static str, &'static str)]) -> Self {
        Self {
            platform,
            patterns: patterns
                .iter()
                .map(|(name, regex)| RegisteredPattern::new_static(name, regex))
                .collect(),
            outcome: Box::new(move || Ok(ParseResult::new(platform))),
            delay: None,
            hang: false,
            calls: AtomicUsize::new(0),
            inputs: Mutex::new(Vec::new()),
        }
    }

    /// Return a clone of `result` on every call.
    pub fn returning(mut self, result: ParseResult) -> Self {
        self.outcome = Box::new(move || Ok(result.clone()));
        self
    }

    /// Fail with the error `make` builds on every call.
    pub fn failing(mut self, make: impl Fn() -> ParserError + Send + Sync + 'static) -> Self {
        self.outcome = Box::new(move || Err(make()));
        self
    }

    /// Sleep (tokio time) before answering.
    pub fn delayed(mut self, delay: Duration) -> Self {
        self.delay = Some(delay);
        self
    }

    /// Never answer.
    pub fn hanging(mut self) -> Self {
        self.hang = true;
        self
    }

    /// How many times `parse` was called.
    pub fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }

    /// The matched text of every call, in order.
    pub fn inputs(&self) -> Vec<String> {
        self.inputs.lock().unwrap().clone()
    }
}

#[async_trait]
impl Parser for FakeParser {
    fn platform(&self) -> Platform {
        self.platform
    }

    fn patterns(&self) -> &[RegisteredPattern] {
        &self.patterns
    }

    async fn parse(&self, match_result: MatchResult<'_>) -> Result<ParseResult> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.inputs
            .lock()
            .unwrap()
            .push(match_result.full_match.to_string());
        if self.hang {
            std::future::pending::<()>().await;
        }
        if let Some(delay) = self.delay {
            tokio::time::sleep(delay).await;
        }
        (self.outcome)()
    }
}
