use std::time::Duration;

use karta_core::data::ParseResult;
use karta_core::error::{ParserError, Result};
use karta_core::extract;
use karta_core::http;
use wreq::header::HeaderValue;

use super::api::{RouterData, SlidesResponse};
use super::filters::DetailErrors;
use super::live::ttwid_from_set_cookies;
use super::result::{build_router_item_result, build_slide_result};
use super::{DouyinParser, detail_attempt};

/// Anonymous ttwid registration (raises share-page SSR success rate).
const API_TTWID_REGISTER: &str = "https://ttwid.bytedance.com/ttwid/union/register/";

/// Fallback slides API.
const API_SLIDES_INFO: &str = "https://www.iesdouyin.com/web/api/v2/aweme/slidesinfo/";

/// Anonymous ttwid registration payload.
const TTWID_REGISTER_PAYLOAD: &str = r#"{"aid":1768,"union":true,"needFid":false,"region":"cn","cbUrlProtocol":"https","service":"www.ixigua.com","migrate_info":{"ticket":"","source":"node"}}"#;

/// Anonymous ttwid shared by all share-page requests for the process
/// lifetime. Registration failures are not cached, so later parses retry.
static ANONYMOUS_TTWID: tokio::sync::OnceCell<String> = tokio::sync::OnceCell::const_new();

impl DouyinParser {
    // -----------------------------------------------------------------------
    // Anonymous ttwid registration
    // -----------------------------------------------------------------------

    /// Return the process-wide anonymous ttwid, registering it on first use.
    ///
    /// Returns `None` (and logs) when registration fails so the share-page
    /// request falls back to cookie-less fetching.
    async fn anonymous_ttwid(&self) -> Option<String> {
        match ANONYMOUS_TTWID
            .get_or_try_init(|| self.register_anonymous_ttwid())
            .await
        {
            Ok(ttwid) => Some(ttwid.clone()),
            Err(e) => {
                tracing::warn!("Douyin: anonymous ttwid registration failed: {}", e);
                None
            }
        }
    }

    /// Register an anonymous ttwid with ByteDance and return the `ttwid` cookie value.
    async fn register_anonymous_ttwid(&self) -> Result<String> {
        let mut headers = self.mobile_headers();
        headers.insert(
            wreq::header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );

        let client = self.downloader.client();
        let response = client
            .post(API_TTWID_REGISTER)
            .headers(headers)
            .body(TTWID_REGISTER_PAYLOAD)
            .send()
            .await?;
        http::check_status(&response)?;

        let set_cookies: Vec<String> = response
            .headers()
            .get_all(wreq::header::SET_COOKIE)
            .iter()
            .filter_map(|value| value.to_str().ok())
            .map(str::to_string)
            .collect();

        let ttwid = ttwid_from_set_cookies(&set_cookies);
        if ttwid.is_empty() {
            return Err(ParserError::Parse(
                "ttwid registration returned no ttwid cookie".into(),
            ));
        }
        Ok(ttwid)
    }

    // -----------------------------------------------------------------------
    // Fallback: HTML scraping
    // -----------------------------------------------------------------------

    pub(super) async fn parse_video_html(
        &self,
        video_id: &str,
        errors: &mut DetailErrors,
    ) -> Result<ParseResult> {
        // Without a configured cookie the share pages often serve an empty
        // shell; an anonymous ttwid raises the SSR success rate.
        let ttwid = if self.cookie.is_none() {
            self.anonymous_ttwid().await
        } else {
            None
        };

        let urls = [
            format!("https://m.douyin.com/share/video/{}", video_id),
            format!("https://m.douyin.com/share/note/{}", video_id),
            format!("https://www.iesdouyin.com/share/video/{}", video_id),
            format!("https://www.iesdouyin.com/share/note/{}", video_id),
        ];

        let mut last_err: Option<ParserError> = None;
        for url in &urls {
            match self
                .try_parse_video_html(url, video_id, ttwid.as_deref(), errors)
                .await
            {
                Ok(result) => return Ok(result),
                Err(e) => {
                    tracing::warn!("Douyin HTML scraping failed for {}: {}", url, e);
                    last_err = Some(e);
                }
            }
        }

        Err(last_err.unwrap_or_else(|| {
            ParserError::Parse("分享已删除或资源直链提取失败, 请稍后再试".into())
        }))
    }

    async fn try_parse_video_html(
        &self,
        url: &str,
        item_id: &str,
        ttwid: Option<&str>,
        errors: &mut DetailErrors,
    ) -> Result<ParseResult> {
        let client = self.downloader.client();

        let mut headers = self.mobile_headers();
        if let Some(ttwid) = ttwid
            && let Ok(value) = format!("ttwid={}", ttwid).parse()
        {
            headers.insert("cookie", value);
        }

        let mut request = client.get(url).headers(headers);
        if self.mobile.is_some() {
            request = request.timeout(Duration::from_secs(4));
        }
        let response = request.send().await?;

        let html = response.text().await?;

        // Extract _ROUTER_DATA from HTML
        let matched = extract::extract_json_after(&html, "window._ROUTER_DATA")
            .ok_or_else(|| ParserError::Parse("Cannot find _ROUTER_DATA".to_string()))?;

        let data: RouterData = serde_json::from_str(matched)
            .map_err(|e| ParserError::Parse(format!("Failed to parse Douyin data: {}", e)))?;

        let page = data
            .loader_data
            .as_ref()
            .and_then(|l| l.video_page.as_ref().or(l.note_page.as_ref()))
            .ok_or_else(|| {
                ParserError::ContentNotFound("Douyin loaderData page not found".into())
            })?;

        let info = page
            .video_info_res
            .as_ref()
            .ok_or_else(|| ParserError::ContentNotFound("Video data not found".into()))?;
        let items = info.item_list.as_deref().unwrap_or_default();
        let item = items
            .iter()
            .find(|item| item.aweme_id.as_deref() == Some(item_id))
            .or_else(|| (items.len() == 1 && items[0].aweme_id.is_none()).then(|| &items[0]));
        let Some(item) = item else {
            errors.observe(item_id, info.filter_list.as_deref());
            return Err(ParserError::ContentNotFound("Video data not found".into()));
        };

        let result = build_router_item_result(item);
        if result.contents.is_empty() {
            errors.observe(item_id, info.filter_list.as_deref());
            return Err(ParserError::ContentNotFound(
                "Douyin share page returned no content".into(),
            ));
        }
        Ok(result)
    }

    // -----------------------------------------------------------------------
    // Fallback: slides API
    // -----------------------------------------------------------------------

    pub(super) async fn parse_slides_with_fallback(&self, item_id: &str) -> Result<ParseResult> {
        match detail_attempt(self.parse_slides(item_id), self.mobile.as_ref().map(|_| 8)).await {
            Ok(result) if !result.contents.is_empty() => Ok(result),
            _ => self.parse_content(item_id).await,
        }
    }

    async fn parse_slides(&self, video_id: &str) -> Result<ParseResult> {
        let client = self.downloader.client();

        let response = client
            .get(API_SLIDES_INFO)
            .query(&[
                ("aweme_ids", format!("[{}]", video_id)),
                ("request_source", "200".to_string()),
            ])
            .headers(self.mobile_headers())
            .send()
            .await?;

        let slides_data: SlidesResponse = response.json().await?;

        let detail = slides_data
            .aweme_details
            .and_then(|v| v.into_iter().next())
            .ok_or_else(|| ParserError::ContentNotFound("Slides data not found".to_string()))?;

        Ok(build_slide_result(&detail))
    }
}
