use super::segment::{MessageSegment, SegmentType};

/// One deliverable unit produced by a renderer.
///
/// Renderers stay platform-agnostic: they only decide *what* belongs
/// together, and each chat-platform adapter decides *how* a bundle is
/// presented natively — a QQ merged-forward, a Telegram media group, a
/// Discord embed batch. Adapters without a grouping concept can deliver the
/// bundled messages sequentially.
#[derive(Debug, Clone)]
pub enum RenderOutput {
    /// A standalone chat message.
    Message(MessageOutput),
    /// An ordered group of messages to present as one unit.
    Bundle(Vec<MessageOutput>),
}

impl RenderOutput {
    /// All contained messages in delivery order.
    pub fn messages(&self) -> &[MessageOutput] {
        match self {
            RenderOutput::Message(message) => std::slice::from_ref(message),
            RenderOutput::Bundle(messages) => messages,
        }
    }

    /// Mutable view of all contained messages in delivery order.
    pub fn messages_mut(&mut self) -> &mut [MessageOutput] {
        match self {
            RenderOutput::Message(message) => std::slice::from_mut(message),
            RenderOutput::Bundle(messages) => messages,
        }
    }
}

impl From<MessageOutput> for RenderOutput {
    fn from(message: MessageOutput) -> Self {
        RenderOutput::Message(message)
    }
}

/// A complete message containing multiple segments
#[derive(Debug, Clone, Default)]
pub struct MessageOutput {
    /// Segments in this message
    pub segments: Vec<MessageSegment>,
}

impl MessageOutput {
    /// Create a new empty message
    pub fn new() -> Self {
        Self {
            segments: Vec::new(),
        }
    }

    /// Create a message from a single text string
    pub fn from_text(text: impl Into<String>) -> Self {
        Self {
            segments: vec![MessageSegment::text(text)],
        }
    }

    /// Create a message from a single segment
    pub fn from_segment(segment: MessageSegment) -> Self {
        Self {
            segments: vec![segment],
        }
    }

    /// Add a segment to the message
    pub fn push(&mut self, segment: MessageSegment) {
        self.segments.push(segment);
    }

    /// Add a segment and return self (builder pattern)
    pub fn with_segment(mut self, segment: MessageSegment) -> Self {
        self.segments.push(segment);
        self
    }

    /// Check if the message is empty
    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }

    /// Get the number of segments
    pub fn len(&self) -> usize {
        self.segments.len()
    }

    /// Iterate over segments
    pub fn iter(&self) -> impl Iterator<Item = &MessageSegment> {
        self.segments.iter()
    }

    /// Get plain text content (concatenated text segments)
    pub fn plain_text(&self) -> String {
        self.segments
            .iter()
            .filter(|s| s.segment_type == SegmentType::Text)
            .filter_map(|s| s.text.as_deref())
            .fold(String::new(), |mut text, part| {
                text.push_str(part);
                text
            })
    }
}

impl IntoIterator for MessageOutput {
    type Item = MessageSegment;
    type IntoIter = std::vec::IntoIter<MessageSegment>;

    fn into_iter(self) -> Self::IntoIter {
        self.segments.into_iter()
    }
}

impl FromIterator<MessageSegment> for MessageOutput {
    fn from_iter<T: IntoIterator<Item = MessageSegment>>(iter: T) -> Self {
        Self {
            segments: iter.into_iter().collect(),
        }
    }
}
