use async_trait::async_trait;
use encoding_rs::{Encoding, GB18030, UTF_8};
use wreq::StatusCode;
use wreq::header::{ACCEPT, CONTENT_TYPE, HeaderMap, HeaderValue};

use karta_core::Platform;
use karta_core::constants::DEFAULT_USER_AGENT;
use karta_core::data::ParseResult;
use karta_core::download::SharedDownloader;
use karta_core::error::{ParserError, Result};
use karta_core::http;
use karta_core::parser::{MatchResult, Parser, RegisteredPattern};

mod patterns;
mod result;

use patterns::GUESTJS_COOKIE_RE;

/// NGA parser
pub struct NgaParser {
    downloader: SharedDownloader,
    patterns: Vec<RegisteredPattern>,
}

impl NgaParser {
    pub fn new(downloader: SharedDownloader) -> Self {
        Self {
            downloader,
            patterns: patterns::registered_patterns(),
        }
    }

    fn get_headers(&self) -> HeaderMap {
        let mut headers = http::headers_with_referer(DEFAULT_USER_AGENT, "https://nga.178.com/");
        headers.insert(
            ACCEPT,
            HeaderValue::from_static(
                "text/html,application/xhtml+xml,application/xml;q=0.9,image/webp,*/*;q=0.8",
            ),
        );
        headers
    }

    /// Fetch the thread page, retrying once with the anti-bot `guestJs` cookie.
    async fn fetch_thread_html(&self, url: &str) -> Result<String> {
        let client = self.downloader.client();

        // First request might return 403 with guestJs cookie
        let response = client.get(url).headers(self.get_headers()).send().await?;

        let status = response.status();
        let html = read_html(response).await?;

        let Some(guest_js) = guest_cookie(status, &html)? else {
            return Ok(html);
        };

        // NGA anti-bot: the page initially returns a `guestJs` cookie value
        // embedded in a script tag. We must wait before retrying so the
        // server-side session has time to associate the new cookie with a
        // valid guest identity. 300 ms is the minimum observed reliable delay.
        tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;

        let rand_param = rand::random::<u32>() % 1000;
        let retry_url = format!("{}&rand={}", url, rand_param);

        let mut headers = self.get_headers();
        if let Ok(cookie_value) = HeaderValue::from_str(&format!("guestJs={}", guest_js)) {
            headers.insert("cookie", cookie_value);
        }

        let retry_response = client.get(&retry_url).headers(headers).send().await?;

        if !retry_response.status().is_success() {
            return Err(ParserError::Api {
                code: retry_response.status().as_u16() as i64,
                message: "Failed to fetch NGA thread".to_string(),
            });
        }

        read_html(retry_response).await
    }

    async fn parse_thread(&self, tid: &str) -> Result<ParseResult> {
        let url = format!("https://nga.178.com/read.php?tid={}", tid);
        let page = self.fetch_thread_html(&url).await?;
        Ok(result::build_thread_result(&page, url))
    }
}

/// The `guestJs` cookie to retry with when the first response is NGA's
/// anti-bot interstitial, or `None` when `html` is the thread page itself.
///
/// A failed status without a usable cookie is an error: the interstitial
/// mentions `guestJs` even when its cookie script is not in the expected form.
fn guest_cookie(status: StatusCode, html: &str) -> Result<Option<&str>> {
    if let Some(caps) = GUESTJS_COOKIE_RE.captures(html) {
        return Ok(caps.get(1).map(|m| m.as_str()));
    }
    if !status.is_success() {
        return Err(ParserError::Api {
            code: status.as_u16() as i64,
            message: "Failed to fetch NGA thread".to_string(),
        });
    }
    Ok(None)
}

/// Read a response body as text in the charset the page is served in.
async fn read_html(response: wreq::Response) -> Result<String> {
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let body = response.bytes().await?;
    Ok(decode_html(&body, content_type.as_deref()))
}

/// Decode an NGA page body.
///
/// NGA serves GBK and says so in `Content-Type`. Without a usable charset the
/// body is taken as UTF-8 when it is valid UTF-8, otherwise as GB18030 (a
/// superset of GBK).
fn decode_html(body: &[u8], content_type: Option<&str>) -> String {
    let declared = content_type
        .and_then(charset_param)
        .and_then(|label| Encoding::for_label(label.as_bytes()));
    let encoding = declared.unwrap_or_else(|| {
        if std::str::from_utf8(body).is_ok() {
            UTF_8
        } else {
            GB18030
        }
    });
    let (text, _, _) = encoding.decode(body);
    text.into_owned()
}

/// The `charset` parameter of a `Content-Type` value.
fn charset_param(content_type: &str) -> Option<&str> {
    content_type.split(';').skip(1).find_map(|param| {
        let (name, value) = param.split_once('=')?;
        name.trim()
            .eq_ignore_ascii_case("charset")
            .then(|| value.trim().trim_matches('"'))
    })
}

#[async_trait]
impl Parser for NgaParser {
    fn platform(&self) -> Platform {
        Platform::Nga
    }

    fn patterns(&self) -> &[RegisteredPattern] {
        &self.patterns
    }

    async fn parse(&self, match_result: MatchResult<'_>) -> Result<ParseResult> {
        tracing::debug!(
            pattern = match_result.pattern,
            matched = match_result.full_match,
            "NGA: parsing"
        );
        let tid = match_result.group(1).unwrap_or("");
        self.parse_thread(tid).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use encoding_rs::GBK;

    #[test]
    fn page_body_is_decoded_in_its_served_charset() {
        let html = "<title>测试帖子 NGA玩家社区</title>";
        let (gbk, _, _) = GBK.encode(html);

        let cases = [
            // No charset: invalid UTF-8 falls back to GBK.
            (gbk.as_ref(), Some("text/html")),
            // No charset but valid UTF-8 stays UTF-8.
            (html.as_bytes(), None),
        ];

        for (body, content_type) in cases {
            assert_eq!(
                decode_html(body, content_type),
                html,
                "content type: {content_type:?}"
            );
        }
    }
}
