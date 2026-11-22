//! Shared end-to-end test harness.
//!
//! Use it only from integration tests (`crates/*/tests/`). Unit tests inside
//! a crate's `src/` would link a second copy of that crate through this
//! harness, and its types would not match.
//!
//! - [`upstream`]: one local server impersonating every upstream host and CDN.
//! - [`app`]: temp data dir + `config.json` + engine construction.
//! - [`fake_parser`]: a scripted [`karta_core::parser::Parser`].
//! - [`tools`]: fake `ffmpeg`/`ffprobe`/`yt-dlp` scripts and the real-tool gate.
//! - `onebot` (feature `kovi`): a kovi bot on an in-process fake OneBot driver.
//! - [`wait`]: bounded waiting.

pub mod app;
pub mod fake_parser;
pub mod fixtures;
#[cfg(feature = "kovi")]
pub mod onebot;
pub mod tools;
pub mod upstream;
pub mod wait;

pub use app::TempApp;
pub use fake_parser::FakeParser;
pub use fixtures::Fixtures;
pub use upstream::{CapturedRequest, FakeUpstream, Reply, Route};
pub use wait::{WAIT, eventually};
