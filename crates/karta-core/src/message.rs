//! Framework-agnostic message types
//!
//! These types represent messages that can be converted to any bot framework's format.

mod output;
mod segment;

pub use output::{MessageOutput, RenderOutput};
pub use segment::{MessageSegment, SegmentType};
