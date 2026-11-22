use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;

use uuid::Uuid;
use wreq::header::{ACCEPT, CONTENT_TYPE, HeaderMap, HeaderValue, ORIGIN};

use karta_core::Platform;
use karta_core::constants::DEFAULT_USER_AGENT;
use karta_core::data::ParseResult;
use karta_core::download::SharedDownloader;
use karta_core::error::{ParserError, Result};
use karta_core::http;
use karta_core::parser::{MatchResult, Parser, RegisteredPattern};

mod api;
mod id;
mod patterns;
mod result;

use api::{ArticleResponse, FidResponse, WeiboResponse};

/// Weibo parser
pub struct WeiboParser {
    downloader: SharedDownloader,
    patterns: Vec<RegisteredPattern>,
}

impl WeiboParser {
    pub fn new(downloader: SharedDownloader) -> Self {
        Self {
            downloader,
            patterns: patterns::registered_patterns(),
        }
    }

    fn get_headers(&self, weibo_id: &str) -> HeaderMap {
        let referer = format!("https://m.weibo.cn/detail/{weibo_id}");
        let mut headers = http::headers_with_dynamic_referer(DEFAULT_USER_AGENT, &referer);
        headers.insert(
            ACCEPT,
            HeaderValue::from_static("application/json, text/plain, */*"),
        );
        headers.insert(ORIGIN, HeaderValue::from_static("https://m.weibo.cn"));
        headers.insert(
            "x-requested-with",
            HeaderValue::from_static("XMLHttpRequest"),
        );
        headers.insert("mweibo-pwa", HeaderValue::from_static("1"));
        headers
    }

    async fn parse_weibo_id(&self, weibo_id: &str) -> Result<ParseResult> {
        let client = self.downloader.client();

        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();

        let url = format!("https://m.weibo.cn/statuses/show?id={}&_={}", weibo_id, ts);

        let response = client
            .get(&url)
            .headers(self.get_headers(weibo_id))
            .send()
            .await?;

        if response.status() == 403 || response.status() == 418 {
            return Err(ParserError::Api {
                code: response.status().as_u16() as i64,
                message: "被风控拦截".to_string(),
            });
        }

        let json: WeiboResponse = response.json().await?;

        let data = json
            .data
            .ok_or_else(|| ParserError::ContentNotFound("Weibo data not found".to_string()))?;

        Ok(result::build_status_result(&data))
    }

    async fn parse_short_url(&self, full_match: &str) -> Result<ParseResult> {
        let url = format!("https://{}", full_match);
        let client = self.downloader.client();

        let final_url = http::resolve_redirect(
            client,
            &url,
            http::headers_with_referer(DEFAULT_USER_AGENT, "https://m.weibo.cn/"),
        )
        .await?
        .final_url;

        // Try to extract weibo ID from final URL
        for pattern in &self.patterns {
            if pattern.name == "mapp_weibo" {
                continue;
            }
            if let Some(caps) = pattern.regex.captures(&final_url) {
                let weibo_id = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                return self.parse_weibo_id(weibo_id).await;
            }
        }

        Err(ParserError::PatternNotMatched(format!(
            "Could not extract weibo ID from: {}",
            final_url
        )))
    }

    async fn dispatch(&self, match_result: MatchResult<'_>) -> Result<ParseResult> {
        match match_result.pattern {
            "mapp_weibo" => self.parse_short_url(match_result.full_match).await,
            "weibo_tv" => {
                let mid = match_result.group(1).unwrap_or("");
                let weibo_id = id::mid_to_id(mid);
                self.parse_weibo_id(&weibo_id).await
            }
            "video_weibo" => {
                let fid = match_result.group(1).unwrap_or("");
                self.parse_fid(fid).await
            }
            "article_ttarticle" | "article_id" | "article_card" => {
                let article_id = match_result.group(1).unwrap_or("");
                self.parse_article(article_id).await
            }
            _ => {
                let weibo_id = match_result.group(1).unwrap_or("");
                self.parse_weibo_id(weibo_id).await
            }
        }
    }

    /// Parse FID video (video.weibo.com)
    async fn parse_fid(&self, fid: &str) -> Result<ParseResult> {
        let client = self.downloader.client();

        let url = format!(
            "https://h5.video.weibo.com/api/component?page=/show/{}",
            fid
        );

        let referer = format!("https://h5.video.weibo.com/show/{fid}");
        let mut headers = http::headers_with_dynamic_referer(DEFAULT_USER_AGENT, &referer);
        headers.insert(
            CONTENT_TYPE,
            HeaderValue::from_static("application/x-www-form-urlencoded"),
        );

        let post_body = format!(r#"data={{"Component_Play_Playinfo":{{"oid":"{}"}}}}"#, fid);

        let response: FidResponse = client
            .post(&url)
            .headers(headers)
            .body(post_body)
            .send()
            .await?
            .json()
            .await?;

        let data = response
            .data
            .ok_or_else(|| ParserError::ContentNotFound("FID data not found".to_string()))?;

        result::build_fid_result(&data)
    }

    /// Parse article
    async fn parse_article(&self, article_id: &str) -> Result<ParseResult> {
        let client = self.downloader.client();

        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();

        let url = "https://card.weibo.com/article/m/aj/detail";
        let rid = Uuid::new_v4().to_string();
        let ts = ts.to_string();
        let params = [
            ("_rid", rid.as_str()),
            ("id", article_id),
            ("_t", ts.as_str()),
        ];

        let headers = http::headers_with_referer(DEFAULT_USER_AGENT, "https://card.weibo.com/");

        let response: ArticleResponse = client
            .get(url)
            .headers(headers)
            .query(&params)
            .send()
            .await?
            .json()
            .await?;

        if response.msg.as_deref() != Some("success") {
            return Err(ParserError::Api {
                code: -1,
                message: response
                    .msg
                    .unwrap_or_else(|| "Article request failed".to_string()),
            });
        }

        let data = response
            .data
            .ok_or_else(|| ParserError::ContentNotFound("Article data not found".to_string()))?;

        Ok(result::build_article_result(&data))
    }
}

#[async_trait]
impl Parser for WeiboParser {
    fn platform(&self) -> Platform {
        Platform::Weibo
    }

    fn patterns(&self) -> &[RegisteredPattern] {
        &self.patterns
    }

    async fn parse(&self, match_result: MatchResult<'_>) -> Result<ParseResult> {
        tracing::debug!(
            pattern = match_result.pattern,
            matched = match_result.full_match,
            "Weibo: parsing"
        );
        self.dispatch(match_result).await
    }
}
