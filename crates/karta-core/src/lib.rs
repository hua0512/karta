//! Karta Core Library
//!
//! A framework-agnostic library for parsing social media URLs and rendering rich media content.
//! Supports 10+ platforms including Bilibili, Douyin, Twitter/X, YouTube, and more.
//!
//! ## Usage
//!
//! ```ignore
//! use std::sync::Arc;
//!
//! use karta_core::download::StreamDownloader;
//! use karta_core::{Config, ParserEngine};
//!
//! let config = Config::default();
//! let downloader = Arc::new(StreamDownloader::new(&config, &data_path)?);
//! // Platform parsers live in the `karta-platforms` crate
//! // (e.g. `karta_platforms::create_all_parsers`).
//! let engine = ParserEngine::with_parsers(config, data_path, downloader, parsers);
//!
//! if let Some(result) = engine.parse("https://www.bilibili.com/video/BV1xx411c7mD").await? {
//!     let messages = engine.render(&result).await?;
//!     // Send messages using your bot framework
//! }
//! ```

pub mod cache;
pub mod config;
pub mod constants;
pub mod data;
pub mod download;
mod engine;
pub mod error;
pub mod extract;
pub mod html;
pub mod http;
pub mod message;
pub mod parser;
pub mod render;

// Re-exports for convenience
pub use config::{BiliVideoCodec, BiliVideoQuality, Config, RenderType};
pub use constants::Platform;
pub use data::{Author, Content, ParseResult};
pub use engine::ParserEngine;
pub use error::{DownloadError, ParserError, RenderError, ResultExt};
pub use message::{MessageOutput, MessageSegment, RenderOutput, SegmentType};
