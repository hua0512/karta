use std::collections::HashMap;

use serde::Serialize;

use crate::constants::Platform;

use super::{Content, VideoContent};

/// UTC offset cards display times in: Beijing time (UTC+8), the local time
/// of the bot's audience. China observes no DST, so a fixed offset is exact.
const DISPLAY_UTC_OFFSET_SECS: i32 = 8 * 60 * 60;

/// Convert a Unix timestamp to the card display time zone (UTC+8).
pub(crate) fn display_datetime(ts: i64) -> Option<chrono::DateTime<chrono::FixedOffset>> {
    let offset = chrono::FixedOffset::east_opt(DISPLAY_UTC_OFFSET_SECS)?;
    chrono::DateTime::from_timestamp(ts, 0).map(|dt| dt.with_timezone(&offset))
}

/// Author information
#[derive(Debug, Clone, Serialize)]
pub struct Author {
    /// Author's display name
    pub name: String,
    /// Author's unique ID on the platform
    pub uid: Option<String>,
    /// Author's avatar URL
    pub avatar: Option<String>,
    /// Author's profile URL
    pub url: Option<String>,
    /// Author's bio/description
    pub description: Option<String>,
}

impl Author {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            uid: None,
            avatar: None,
            url: None,
            description: None,
        }
    }

    pub fn with_uid(mut self, uid: impl Into<String>) -> Self {
        self.uid = Some(uid.into());
        self
    }

    pub fn with_avatar(mut self, avatar: impl Into<String>) -> Self {
        self.avatar = Some(avatar.into());
        self
    }

    pub fn with_url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());
        self
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}

/// Parse result containing all extracted information
#[derive(Debug, Clone, Serialize)]
pub struct ParseResult {
    /// Platform the content is from
    pub platform: Platform,
    /// Author information
    pub author: Option<Author>,
    /// Content title
    pub title: Option<String>,
    /// Content description/text
    pub text: Option<String>,
    /// Unix timestamp of the content
    pub timestamp: Option<i64>,
    /// Original URL
    pub url: Option<String>,
    /// Media contents (videos, images, audio)
    pub contents: Vec<Content>,
    /// Extra metadata
    pub extra: HashMap<String, String>,
    /// Reposted/quoted content
    pub repost: Option<Box<ParseResult>>,
}

impl ParseResult {
    /// Create a new ParseResult for a platform
    pub fn new(platform: Platform) -> Self {
        Self {
            platform,
            author: None,
            title: None,
            text: None,
            timestamp: None,
            url: None,
            contents: Vec::new(),
            extra: HashMap::new(),
            repost: None,
        }
    }

    pub fn with_author(mut self, author: Author) -> Self {
        self.author = Some(author);
        self
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        self.text = Some(text.into());
        self
    }

    pub fn with_timestamp(mut self, timestamp: i64) -> Self {
        self.timestamp = Some(timestamp);
        self
    }

    pub fn with_url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());
        self
    }

    pub fn with_content(mut self, content: Content) -> Self {
        self.contents.push(content);
        self
    }

    pub fn with_contents(mut self, contents: Vec<Content>) -> Self {
        self.contents = contents;
        self
    }

    pub fn with_extra(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.extra.insert(key.into(), value.into());
        self
    }

    pub fn with_repost(mut self, repost: ParseResult) -> Self {
        self.repost = Some(Box::new(repost));
        self
    }

    /// Add a content item
    pub fn add_content(&mut self, content: Content) {
        self.contents.push(content);
    }

    /// Add extra metadata
    pub fn add_extra(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.extra.insert(key.into(), value.into());
    }

    /// Get formatted timestamp string in Beijing time (UTC+8)
    pub fn formatted_time(&self) -> Option<String> {
        self.timestamp.map(|ts| {
            display_datetime(ts)
                .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_else(|| ts.to_string())
        })
    }

    /// Check if there are any media contents
    pub fn has_media(&self) -> bool {
        !self.contents.is_empty()
    }

    /// Get all video contents
    pub fn videos(&self) -> Vec<&super::VideoContent> {
        self.contents
            .iter()
            .filter_map(|c| match c {
                Content::Video(v) => Some(v),
                _ => None,
            })
            .collect()
    }

    /// Get all image contents
    pub fn images(&self) -> Vec<&super::ImageContent> {
        self.contents
            .iter()
            .filter_map(|c| match c {
                Content::Image(i) => Some(i),
                _ => None,
            })
            .collect()
    }

    /// Get all audio contents
    pub fn audios(&self) -> Vec<&super::AudioContent> {
        self.contents
            .iter()
            .filter_map(|c| match c {
                Content::Audio(a) => Some(a),
                _ => None,
            })
            .collect()
    }

    /// Get the first video content.
    pub fn video(&self) -> Option<&VideoContent> {
        self.contents.iter().find_map(|c| match c {
            Content::Video(v) => Some(v),
            _ => None,
        })
    }
}
