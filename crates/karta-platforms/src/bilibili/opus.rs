//! Bilibili opus (图文动态) body extraction and [`ParseResult`] construction.
//!
//! The opus endpoint is the only one that preserves the original interleaving
//! of text and images: the dynamic endpoint flattens a post into a summary
//! string plus a flat picture array, which reorders a photo essay into "all
//! text, then all images".

use karta_core::Platform;
use karta_core::data::{Author, Content, ImageContent, ParseResult};
use karta_core::error::{ParserError, Result};

use super::BilibiliParser;
use super::api::{ApiResponse, OpusDetail, OpusModule, OpusParagraph, OpusTextNode};

/// Ordered text/image body of an opus (图文动态)
const API_OPUS_DETAIL: &str = "https://api.bilibili.com/x/polymer/web-dynamic/v1/opus/detail";

/// Color Bilibili uses for an image caption, treated as alt text rather than
/// body copy.
const CAPTION_COLOR: &str = "#999999";

impl BilibiliParser {
    /// Parse an opus (图文动态), preserving its text/image order.
    pub(super) async fn fetch_opus(&self, opus_id: &str) -> Result<ParseResult> {
        let client = self.downloader.client();

        // `timezone_offset` is UTC+8 in minutes, as the web client sends.
        let params = [("id", opus_id), ("timezone_offset", "-480")];

        let response: ApiResponse<OpusDetail> = client
            .get(API_OPUS_DETAIL)
            .headers(self.get_headers().await)
            .query(&params)
            .send()
            .await?
            .json()
            .await?;

        if response.code != 0 {
            return Err(ParserError::Api {
                code: response.code,
                message: response.message.unwrap_or_default(),
            });
        }

        let detail = response
            .data
            .ok_or_else(|| ParserError::ContentNotFound("Opus data not found".to_string()))?;

        Ok(build_opus_result(&detail, opus_id))
    }
}

/// One element of an opus body, in document order.
#[derive(Debug, Clone, PartialEq, Eq)]
enum OpusNode {
    Text(String),
    Image {
        url: String,
        alt: Option<String>,
        width: Option<u32>,
        height: Option<u32>,
    },
}

/// Build a [`ParseResult`] from an opus detail response.
fn build_opus_result(detail: &OpusDetail, opus_id: &str) -> ParseResult {
    let mut result = ParseResult::new(Platform::Bilibili)
        .with_url(format!("https://www.bilibili.com/opus/{opus_id}"));

    if let Some(title) = detail
        .item
        .basic
        .as_ref()
        .and_then(|basic| basic.title.as_deref())
        .filter(|title| !title.is_empty())
    {
        result = result.with_title(title);
    }

    if let Some(author) = author_module(&detail.item.modules) {
        result = result.with_author(
            Author::new(&author.name)
                .with_uid(author.mid.to_string())
                .with_avatar(&author.face)
                .with_url(format!("https://space.bilibili.com/{}", author.mid)),
        );
        if let Some(pub_ts) = author.pub_ts {
            result = result.with_timestamp(pub_ts);
        }
    }

    for node in extract_nodes(&detail.item.modules) {
        match node {
            OpusNode::Text(text) => result.add_content(Content::Text(text)),
            OpusNode::Image {
                url,
                alt,
                width,
                height,
            } => {
                let mut image = ImageContent::new().with_url(url);
                if let Some(alt) = alt {
                    image = image.with_alt(alt);
                }
                if let (Some(width), Some(height)) = (width, height) {
                    image = image.with_dimensions(width, height);
                }
                result.add_content(Content::Image(image));
            }
        }
    }

    result
}

fn author_module(modules: &[OpusModule]) -> Option<&super::api::OpusAuthor> {
    modules
        .iter()
        .find(|module| module.module_type == "MODULE_TYPE_AUTHOR")
        .and_then(|module| module.module_author.as_ref())
}

/// Walk the content module's paragraphs, preserving document order.
///
/// A text paragraph right after an image is that image's caption when it has
/// words in [`CAPTION_COLOR`]: those become alt text, anything else stays body
/// copy. Each picture takes at most one caption paragraph; any other
/// paragraph (body text, more pictures) is left for the main walk.
fn extract_nodes(modules: &[OpusModule]) -> Vec<OpusNode> {
    let mut nodes = Vec::new();

    for module in modules {
        if module.module_type != "MODULE_TYPE_CONTENT" {
            continue;
        }
        let Some(content) = &module.module_content else {
            continue;
        };

        let mut paragraphs = content.paragraphs.iter().peekable();
        while let Some(paragraph) = paragraphs.next() {
            if let Some(text) = paragraph_text(paragraph) {
                nodes.push(OpusNode::Text(text));
            }

            let Some(pic) = &paragraph.pic else {
                continue;
            };

            for picture in &pic.pics {
                let (alt, trailing_text) = paragraphs
                    .next_if(|next| is_caption(next))
                    .map(split_caption)
                    .unwrap_or((None, String::new()));

                nodes.push(OpusNode::Image {
                    url: picture.url.clone(),
                    alt,
                    width: picture.width,
                    height: picture.height,
                });

                let trailing_text = trailing_text.trim();
                if !trailing_text.is_empty() {
                    nodes.push(OpusNode::Text(trailing_text.to_string()));
                }
            }
        }
    }

    nodes
}

/// Concatenated body text of a paragraph, or `None` when it carries none.
fn paragraph_text(paragraph: &OpusParagraph) -> Option<String> {
    let nodes = &paragraph.text.as_ref()?.nodes;
    let text: String = text_pieces(nodes)
        .map(|(words, _)| words)
        .collect::<Vec<_>>()
        .concat();

    let text = text.trim();
    (!text.is_empty()).then(|| text.to_string())
}

/// Whether a paragraph is an image caption: no pictures of its own and at
/// least one word in [`CAPTION_COLOR`].
fn is_caption(paragraph: &OpusParagraph) -> bool {
    paragraph.pic.is_none()
        && paragraph.text.as_ref().is_some_and(|text| {
            text_pieces(&text.nodes).any(|(_, color)| color == Some(CAPTION_COLOR))
        })
}

/// Split a caption paragraph into (alt text, remaining body text).
fn split_caption(paragraph: &OpusParagraph) -> (Option<String>, String) {
    let Some(text) = &paragraph.text else {
        return (None, String::new());
    };

    let mut alt = String::new();
    let mut body = String::new();
    for (words, color) in text_pieces(&text.nodes) {
        if color == Some(CAPTION_COLOR) {
            alt.push_str(words);
        } else {
            body.push_str(words);
        }
    }

    let alt = alt.trim();
    ((!alt.is_empty()).then(|| alt.to_string()), body)
}

/// Word/color pairs of the renderable nodes in a text paragraph.
fn text_pieces(nodes: &[OpusTextNode]) -> impl Iterator<Item = (&str, Option<&str>)> {
    nodes.iter().filter_map(|node| {
        let node_type = node.node_type.as_deref()?;
        if !matches!(node_type, "TEXT_NODE_TYPE_WORD" | "TEXT_NODE_TYPE_RICH") {
            return None;
        }
        let word = node.word.as_ref()?;
        (!word.words.is_empty()).then_some((word.words.as_str(), word.color.as_deref()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn detail(json: serde_json::Value) -> OpusDetail {
        serde_json::from_value(json).expect("opus detail should deserialize")
    }

    fn word_node(words: &str, color: Option<&str>) -> serde_json::Value {
        serde_json::json!({
            "type": "TEXT_NODE_TYPE_WORD",
            "word": {"words": words, "color": color},
        })
    }

    #[test]
    fn only_caption_paragraphs_are_consumed_by_pictures() {
        let image = |url: &str, alt: Option<&str>| OpusNode::Image {
            url: url.to_string(),
            alt: alt.map(str::to_string),
            width: None,
            height: None,
        };
        let detail = detail(serde_json::json!({
            "item": {
                "modules": [{
                    "module_type": "MODULE_TYPE_CONTENT",
                    "module_content": {"paragraphs": [
                        {"pic": {"pics": [{"url": "https://example.com/1.jpg"}]}},
                        {"pic": {"pics": [{"url": "https://example.com/2.jpg"}]}},
                        {"text": {"nodes": [word_node("第二张的说明", Some(CAPTION_COLOR)), word_node("说明后正文", None)]}},
                        {"pic": {"pics": [
                            {"url": "https://example.com/3.jpg"},
                            {"url": "https://example.com/4.jpg"}
                        ]}},
                        {"text": {"nodes": [word_node("正文", None)]}},
                        {"pic": {"pics": [{"url": "https://example.com/5.jpg"}]}}
                    ]}
                }]
            }
        }));

        let nodes = extract_nodes(&detail.item.modules);

        assert_eq!(
            nodes,
            vec![
                image("https://example.com/1.jpg", None),
                image("https://example.com/2.jpg", Some("第二张的说明")),
                OpusNode::Text("说明后正文".to_string()),
                image("https://example.com/3.jpg", None),
                image("https://example.com/4.jpg", None),
                OpusNode::Text("正文".to_string()),
                image("https://example.com/5.jpg", None),
            ]
        );
    }
}
