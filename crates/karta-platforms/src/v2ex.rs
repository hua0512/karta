use async_trait::async_trait;
use serde::Deserialize;
use wreq::header::HeaderMap;

use karta_core::Platform;
use karta_core::constants::DEFAULT_USER_AGENT;
use karta_core::data::{Author, ParseResult};
use karta_core::download::SharedDownloader;
use karta_core::error::{ParserError, Result};
use karta_core::html;
use karta_core::http;
use karta_core::parser::{MatchResult, Parser, RegisteredPattern};
use karta_core::patterns;

const API_TOPIC_SHOW: &str = "https://www.v2ex.com/api/topics/show.json";
const API_REPLIES_SHOW: &str = "https://www.v2ex.com/api/replies/show.json";
const REPLY_PREVIEW_LIMIT: usize = 3;

#[derive(Debug, Deserialize)]
struct Topic {
    title: String,
    url: Option<String>,
    created: Option<i64>,
    content: Option<String>,
    content_rendered: Option<String>,
    replies: Option<u64>,
    node: Option<Node>,
    member: Option<Member>,
}

#[derive(Debug, Deserialize)]
struct Node {
    title: Option<String>,
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Member {
    username: String,
    url: Option<String>,
    avatar_large: Option<String>,
    avatar_normal: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Reply {
    content: Option<String>,
    content_rendered: Option<String>,
    member: Option<Member>,
}

pub struct V2exParser {
    downloader: SharedDownloader,
    patterns: Vec<RegisteredPattern>,
}

impl V2exParser {
    pub fn new(downloader: SharedDownloader) -> Self {
        let patterns = patterns![
            "v2ex_topic" => r"(?:https?://)?(?:www\.)?v2ex\.com/t/(\d+)(?:[?#][A-Za-z0-9\d._?%&+\-=/#]*)?",
        ];

        Self {
            downloader,
            patterns,
        }
    }

    fn get_headers(&self) -> HeaderMap {
        http::headers_with_referer(DEFAULT_USER_AGENT, "https://www.v2ex.com/")
    }

    async fn parse_topic(&self, topic_id: &str) -> Result<ParseResult> {
        let client = self.downloader.client();
        let topic_url = format!("{}?id={}", API_TOPIC_SHOW, topic_id);

        let topics: Vec<Topic> = client
            .get(&topic_url)
            .headers(self.get_headers())
            .send()
            .await?
            .json()
            .await?;

        let topic = topics.into_iter().next().ok_or_else(|| {
            ParserError::ContentNotFound(format!("V2EX topic {topic_id} not found"))
        })?;

        let display_url = topic
            .url
            .clone()
            .unwrap_or_else(|| format!("https://www.v2ex.com/t/{}", topic_id));

        let mut result = ParseResult::new(Platform::V2ex)
            .with_title(&topic.title)
            .with_url(display_url);

        if let Some(member) = topic.member {
            let mut author = Author::new(&member.username);
            if let Some(url) = member.url {
                author = author.with_url(url);
            }
            if let Some(avatar) = member.avatar_large.or(member.avatar_normal) {
                author = author.with_avatar(normalize_v2ex_url(&avatar));
            }
            result = result.with_author(author);
        }

        if let Some(created) = topic.created {
            result = result.with_timestamp(created);
        }

        if let Some(node) = topic.node {
            let node_name = node.title.or(node.name);
            if let Some(node_name) = node_name
                && !node_name.trim().is_empty()
            {
                result.add_extra("节点", node_name);
            }
        }

        if let Some(replies) = topic.replies {
            result.add_extra("回复", replies.to_string());
        }

        let body = topic
            .content_rendered
            .as_deref()
            .or(topic.content.as_deref())
            .map(clean_v2ex_text)
            .filter(|s| !s.is_empty());

        let replies = self.fetch_replies(topic_id).await.unwrap_or_default();
        let reply_preview = format_reply_preview(&replies);

        let text = match (body, reply_preview) {
            (Some(body), Some(reply_preview)) => format!("{body}\n\n{reply_preview}"),
            (Some(body), None) => body,
            (None, Some(reply_preview)) => reply_preview,
            (None, None) => String::new(),
        };

        if !text.is_empty() {
            result = result.with_text(text);
        }

        Ok(result)
    }

    async fn fetch_replies(&self, topic_id: &str) -> Result<Vec<Reply>> {
        let client = self.downloader.client();
        let replies_url = format!("{}?topic_id={}", API_REPLIES_SHOW, topic_id);

        let replies = client
            .get(&replies_url)
            .headers(self.get_headers())
            .send()
            .await?
            .json()
            .await?;

        Ok(replies)
    }
}

#[async_trait]
impl Parser for V2exParser {
    fn platform(&self) -> Platform {
        Platform::V2ex
    }

    fn patterns(&self) -> &[RegisteredPattern] {
        &self.patterns
    }

    async fn parse(&self, match_result: MatchResult<'_>) -> Result<ParseResult> {
        tracing::debug!(
            pattern = match_result.pattern,
            matched = match_result.full_match,
            "V2EX: parsing"
        );

        let topic_id = match_result.require_group(1)?;
        self.parse_topic(topic_id).await
    }
}

fn normalize_v2ex_url(url: &str) -> String {
    if url.starts_with("//") {
        format!("https:{}", url)
    } else {
        url.to_string()
    }
}

fn clean_v2ex_text(text: &str) -> String {
    html::html_to_text(text).chars().take(700).collect()
}

/// Preview of the latest non-empty replies, oldest of them first.
///
/// `replies/show.json` lists replies oldest first, so walk it from the end.
fn format_reply_preview(replies: &[Reply]) -> Option<String> {
    let mut preview = replies
        .iter()
        .rev()
        .filter_map(|reply| {
            let text = reply
                .content_rendered
                .as_deref()
                .or(reply.content.as_deref())
                .map(clean_v2ex_text)
                .filter(|s| !s.is_empty())?;
            let author = reply
                .member
                .as_ref()
                .map(|m| m.username.as_str())
                .unwrap_or("匿名");
            Some(format!("@{author}: {text}"))
        })
        .take(REPLY_PREVIEW_LIMIT)
        .collect::<Vec<_>>();
    preview.reverse();

    if preview.is_empty() {
        None
    } else {
        Some(format!("最近回复:\n{}", preview.join("\n\n")))
    }
}
