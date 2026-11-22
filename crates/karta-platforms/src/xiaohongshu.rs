use async_trait::async_trait;
use wreq::header::{ACCEPT, COOKIE, HeaderMap, HeaderValue, ORIGIN, REFERER};
use wreq::redirect::Policy;
use wreq_util::Emulation;

use karta_core::Platform;
use karta_core::constants::DEFAULT_USER_AGENT;
use karta_core::data::ParseResult;
use karta_core::download::SharedDownloader;
use karta_core::error::{ParserError, Result};
use karta_core::http;
use karta_core::parser::{MatchResult, Parser, RegisteredPattern};

mod patterns;
mod result;
mod state;

/// Note id inside an interstitial's `redirectPath` target: `/item/<id>`,
/// `/explore/<id>`, or a `noteId=` query param.
static REDIRECT_NOTE_ID_RE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
    regex::Regex::new(r"/(?:item|explore)/([0-9a-fA-F]+)|noteId=(\w+)")
        .expect("valid captcha note id regex")
});

/// Xiaohongshu parser
pub struct XiaohongshuParser {
    downloader: SharedDownloader,
    cookie: Option<String>,
    patterns: Vec<RegisteredPattern>,
}

/// Parsing mode for Xiaohongshu
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParseMode {
    /// Explore page (desktop)
    Explore,
    /// Discovery page (mobile/API)
    Discovery,
}

impl XiaohongshuParser {
    pub fn new(downloader: SharedDownloader, cookie: Option<String>) -> Self {
        Self {
            downloader,
            cookie,
            patterns: patterns::registered_patterns(),
        }
    }

    /// Get headers for explore page (desktop)
    fn get_explore_headers(&self) -> HeaderMap {
        let mut headers = http::headers_with_referer_and_cookie(
            DEFAULT_USER_AGENT,
            "https://www.xiaohongshu.com/",
            self.cookie.as_deref(),
        );
        headers.insert(
            ACCEPT,
            HeaderValue::from_static(
                "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8"
            ),
        );
        headers
    }

    /// Get headers for discovery page (mobile/API)
    fn get_discovery_headers(&self) -> HeaderMap {
        // User-Agent and browser headers come from the mobile emulation.
        let mut headers = HeaderMap::new();
        headers.insert(
            REFERER,
            HeaderValue::from_static("https://www.xiaohongshu.com/"),
        );
        if let Some(cookie) = &self.cookie
            && let Ok(value) = HeaderValue::from_str(cookie)
        {
            headers.insert(COOKIE, value);
        }
        headers.insert(
            ORIGIN,
            HeaderValue::from_static("https://www.xiaohongshu.com"),
        );
        headers.insert(
            "x-requested-with",
            HeaderValue::from_static("XMLHttpRequest"),
        );
        headers.insert("sec-fetch-site", HeaderValue::from_static("same-origin"));
        headers.insert("sec-fetch-mode", HeaderValue::from_static("cors"));
        headers.insert("sec-fetch-dest", HeaderValue::from_static("empty"));
        headers
    }

    fn mobile_request(&self, url: &str) -> wreq::RequestBuilder {
        self.downloader
            .client()
            .get(url)
            // Request emulation merges headers, so explicitly exclude the
            // shared client's desktop Chrome defaults (including sec-ch-ua).
            .default_headers(false)
            .emulation(Emulation::SafariIos17_2)
            .headers(self.get_discovery_headers())
    }

    async fn parse_short_url(&self, full_match: &str) -> Result<ParseResult> {
        let url = format!("https://{}", full_match);
        let response = self
            .mobile_request(&url)
            .redirect(Policy::limited(10))
            .send()
            .await?;
        http::check_status(&response)?;
        let final_url = response.uri().to_string();
        drop(response);

        // Login and captcha interstitials stash the real note URL and its
        // access token in `redirectPath`; recover them before rejecting access.
        if let Some((note_id, query_params)) = extract_from_redirect_url(&final_url) {
            return self
                .parse_note_with_fallback(&note_id, Some(&query_params))
                .await;
        }

        check_access_url(&final_url)?;

        // Extract query params from the final URL (includes xsec_token etc.)
        let query_params = url_query(&final_url).map(str::to_string);

        // Try to extract note ID from final URL path
        for pattern in &self.patterns {
            if pattern.name == "xhslink" {
                continue;
            }
            if let Some(caps) = pattern.regex.captures(&final_url) {
                let note_id = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                return self
                    .parse_note_with_fallback(note_id, query_params.as_deref())
                    .await;
            }
        }

        Err(ParserError::PatternNotMatched(format!(
            "Could not extract note ID from: {}",
            final_url
        )))
    }

    /// Parse note with fallback from explore to discovery mode
    async fn parse_note_with_fallback(
        &self,
        note_id: &str,
        query_params: Option<&str>,
    ) -> Result<ParseResult> {
        // Try explore mode first
        match self.parse_note_explore(note_id, query_params).await {
            Ok(result) => Ok(result),
            Err(e) => {
                tracing::debug!("Explore mode failed: {}, trying discovery mode", e);
                // Fallback to discovery mode
                self.parse_note_discovery(note_id, query_params).await
            }
        }
    }

    /// Parse note using explore page (desktop)
    async fn parse_note_explore(
        &self,
        note_id: &str,
        query_params: Option<&str>,
    ) -> Result<ParseResult> {
        let url = if let Some(params) = query_params {
            format!("https://www.xiaohongshu.com/explore/{}?{}", note_id, params)
        } else {
            format!("https://www.xiaohongshu.com/explore/{}", note_id)
        };
        let client = self.downloader.client();

        let response = client
            .get(&url)
            .headers(self.get_explore_headers())
            .send()
            .await?;

        check_access_url(&response.uri().to_string())?;
        http::check_status(&response)?;

        let html = response.text().await?;
        self.parse_initial_state(&html, note_id, ParseMode::Explore)
    }

    /// Parse note using discovery page (mobile/API)
    async fn parse_note_discovery(
        &self,
        note_id: &str,
        query_params: Option<&str>,
    ) -> Result<ParseResult> {
        let url = if let Some(params) = query_params {
            format!(
                "https://www.xiaohongshu.com/discovery/item/{}?{}",
                note_id, params
            )
        } else {
            format!("https://www.xiaohongshu.com/discovery/item/{}", note_id)
        };
        let response = self.mobile_request(&url).send().await?;

        check_access_url(&response.uri().to_string())?;
        http::check_status(&response)?;

        let html = response.text().await?;
        self.parse_initial_state(&html, note_id, ParseMode::Discovery)
    }

    /// Parse __INITIAL_STATE__ from HTML
    fn parse_initial_state(
        &self,
        html: &str,
        note_id: &str,
        mode: ParseMode,
    ) -> Result<ParseResult> {
        let data = state::extract_initial_state(html)?;

        match mode {
            ParseMode::Explore => result::build_from_explore_data(&data, note_id),
            ParseMode::Discovery => result::build_from_discovery_data(&data),
        }
    }

    async fn dispatch(&self, match_result: MatchResult<'_>) -> Result<ParseResult> {
        match match_result.pattern {
            "xhslink" => self.parse_short_url(match_result.full_match).await,
            "explore" | "discovery" => {
                let note_id = match_result.group(1).unwrap_or("");
                let query_params = match_result.group(2);
                self.parse_note_with_fallback(note_id, query_params).await
            }
            _ => Err(ParserError::PatternNotMatched(format!(
                "Unknown pattern: {}",
                match_result.pattern
            ))),
        }
    }
}

/// Recognize access interstitials before treating their HTML as a note page.
fn check_access_url(url: &str) -> Result<()> {
    let url = url::Url::parse(url)
        .map_err(|_| ParserError::Parse("Invalid Xiaohongshu response URL".to_string()))?;
    match url.path().trim_end_matches('/') {
        "/website-login/captcha" => Err(ParserError::Parse(
            "小红书访问需要验证码验证，请稍后重试".to_string(),
        )),
        "/login" | "/website-login" => Err(ParserError::Parse(
            "小红书访问需要登录，请检查登录状态或稍后重试".to_string(),
        )),
        _ => Ok(()),
    }
}

/// Query string of a URL (without `?` and any `#fragment`), if non-empty.
fn url_query(url: &str) -> Option<&str> {
    let (_, rest) = url.split_once('?')?;
    let query = rest.split('#').next().unwrap_or(rest);
    (!query.is_empty()).then_some(query)
}

/// Value of `name` in a query string (`a=1&b=2`), verbatim (not decoded).
fn query_value<'a>(query: &'a str, name: &str) -> Option<&'a str> {
    query.split('&').find_map(|pair| {
        let (key, value) = pair.split_once('=')?;
        (key == name).then_some(value)
    })
}

/// Percent-decode `%hh` escapes; malformed escapes pass through verbatim.
fn percent_decode(s: &str) -> String {
    fn hex(byte: u8) -> Option<u8> {
        match byte {
            b'0'..=b'9' => Some(byte - b'0'),
            b'a'..=b'f' => Some(byte - b'a' + 10),
            b'A'..=b'F' => Some(byte - b'A' + 10),
            _ => None,
        }
    }

    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match (
            bytes[i],
            bytes.get(i + 1).copied().and_then(hex),
            bytes.get(i + 2).copied().and_then(hex),
        ) {
            (b'%', Some(hi), Some(lo)) => {
                out.push((hi << 4) | lo);
                i += 3;
            }
            (byte, _, _) => {
                out.push(byte);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Recover the note id and `xsec_*` params from a login or captcha URL's
/// `redirectPath` before treating the interstitial as an access failure.
///
/// The embedded note URL may be percent-encoded; when it is not, everything
/// after its first `&` leaks into the outer URL's own query, so `xsec_*`
/// params are looked up in both. `xsec_source` defaults to `pc_feed`.
fn extract_from_redirect_url(url: &str) -> Option<(String, String)> {
    let parsed = url::Url::parse(url).ok()?;
    if !matches!(
        parsed.path().trim_end_matches('/'),
        "/website-login/captcha" | "/login" | "/website-login"
    ) {
        return None;
    }

    let query = url_query(url)?;
    let redirect_path = percent_decode(query_value(query, "redirectPath")?);

    let caps = REDIRECT_NOTE_ID_RE.captures(&redirect_path)?;
    let note_id = caps.get(1).or_else(|| caps.get(2))?.as_str().to_string();

    let redirect_query = url_query(&redirect_path);
    let param = |name: &str| {
        redirect_query
            .and_then(|q| query_value(q, name))
            .or_else(|| query_value(query, name))
    };

    let xsec_source = param("xsec_source").unwrap_or("pc_feed");
    let query_params = match param("xsec_token") {
        Some(token) => format!("xsec_token={}&xsec_source={}", token, xsec_source),
        None => format!("xsec_source={}", xsec_source),
    };

    Some((note_id, query_params))
}

#[async_trait]
impl Parser for XiaohongshuParser {
    fn platform(&self) -> Platform {
        Platform::Xiaohongshu
    }

    fn patterns(&self) -> &[RegisteredPattern] {
        &self.patterns
    }

    async fn parse(&self, match_result: MatchResult<'_>) -> Result<ParseResult> {
        tracing::debug!(
            pattern = match_result.pattern,
            matched = match_result.full_match,
            "Xiaohongshu: parsing"
        );
        self.dispatch(match_result).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_percent_decode() {
        assert_eq!(percent_decode("%E4%B8%AD%E6%96%87"), "中文");
        // Malformed escapes pass through verbatim.
        assert_eq!(percent_decode("100%zz%2"), "100%zz%2");
    }

    #[test]
    fn test_extract_from_redirect_url_unencoded_redirect_path() {
        // Unencoded redirectPath: params after its first `&` show up as
        // params of the captcha URL itself.
        let url = concat!(
            "https://www.xiaohongshu.com/website-login/captcha?redirectPath=",
            "https://www.xiaohongshu.com/discovery/item/63c93ac3000000002203b28a",
            "?app_platform=android&app_version=8.23.1&xsec_token=abc&verifyUuid=uuid",
        );

        let (note_id, query) = extract_from_redirect_url(url).expect("captcha URL recovered");

        assert_eq!(note_id, "63c93ac3000000002203b28a");
        assert_eq!(query, "xsec_token=abc&xsec_source=pc_feed");
    }
}
