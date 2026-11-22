use async_trait::async_trait;

use crate::data::ParseResult;
use crate::download::SharedDownloader;
use crate::error::RenderResult;
use crate::message::RenderOutput;

use super::RenderOptions;
use super::Renderer;

mod forward;
mod segments;

/// Text-based renderer
pub struct TextRenderer {
    options: RenderOptions,
}

impl TextRenderer {
    pub fn new(options: RenderOptions) -> Self {
        Self { options }
    }

    /// Format the header line
    fn format_header(&self, result: &ParseResult) -> String {
        let mut parts = vec![format!("[{}]", result.platform)];

        if let Some(author) = &result.author {
            parts.push(format!("@{}", author.name));
        }

        if let Some(title) = &result.title {
            parts.push(format!("| {}", title));
        }

        parts.join(" ")
    }

    /// Format extra info
    fn format_extra(&self, result: &ParseResult) -> Option<String> {
        if result.extra.is_empty() {
            return None;
        }

        let mut fields: Vec<_> = result.extra.iter().collect();
        fields.sort_by_key(|(k, _)| *k);

        let parts: Vec<String> = fields
            .into_iter()
            .filter_map(|(k, v)| super::extra::format_extra_entry(k, v, ": "))
            .collect();

        (!parts.is_empty()).then(|| parts.join(" | "))
    }

    /// Render a single result (without repost)
    fn render_single(&self, result: &ParseResult, prefix: &str) -> String {
        let mut lines = Vec::new();

        // Header
        lines.push(format!("{}{}", prefix, self.format_header(result)));

        // Extra info
        if let Some(extra) = self.format_extra(result) {
            lines.push(format!("{}{}", prefix, extra));
        }

        // URL
        if self.options.append_url
            && let Some(url) = &result.url
        {
            lines.push(format!("{}链接: {}", prefix, url));
        }

        lines.join("\n")
    }
}

impl Default for TextRenderer {
    fn default() -> Self {
        // RenderOptions::default() derives from Config::default(), so renderer
        // defaults track the config serde defaults.
        Self::new(RenderOptions::default())
    }
}

#[async_trait]
impl Renderer for TextRenderer {
    async fn render(
        &self,
        result: &ParseResult,
        downloader: &SharedDownloader,
    ) -> RenderResult<Vec<RenderOutput>> {
        tracing::debug!(
            platform = %result.platform,
            contents = result.contents.len(),
            "TextRenderer: rendering"
        );

        // Build main text message
        let mut text_parts = vec![self.render_single(result, "")];

        // Handle repost
        if let Some(repost) = &result.repost {
            text_parts.push(String::new());
            text_parts.push("转发自:".to_string());
            text_parts.push(self.render_single(repost, "  "));
        }

        let header_text = text_parts.join("\n");
        let media_segments =
            segments::collect_ordered_segments(result, downloader, &self.options).await;
        let mut messages = forward::build_messages(header_text, media_segments, &self.options);

        if self.options.use_base64 {
            for output in &mut messages {
                for msg in output.messages_mut() {
                    for seg in &mut msg.segments {
                        // Best-effort; keep original representation on failure.
                        let _ = seg.to_base64().await;
                    }
                }
            }
        }

        Ok(messages)
    }
}
