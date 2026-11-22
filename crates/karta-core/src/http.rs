use wreq::header::{HeaderMap, HeaderName, HeaderValue, LOCATION, REFERER, SET_COOKIE, USER_AGENT};
use wreq::redirect::Policy;
use wreq::{Client, Response, StatusCode};

use crate::constants::{DEFAULT_USER_AGENT, MOBILE_USER_AGENT, Platform};
use crate::error::{ParserError, Result};

/// Build a minimal header set for typical HTML/JSON fetches.
///
/// This centralizes the common `User-Agent` + `Referer` pattern used by most parsers.
pub fn headers_with_referer(user_agent: &'static str, referer: &'static str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, HeaderValue::from_static(user_agent));
    headers.insert(REFERER, HeaderValue::from_static(referer));
    headers
}

/// Same as [`headers_with_referer`], but adds a `cookie` header when provided.
pub fn headers_with_referer_and_cookie(
    user_agent: &'static str,
    referer: &'static str,
    cookie: Option<&str>,
) -> HeaderMap {
    let mut headers = headers_with_referer(user_agent, referer);
    if let Some(cookie) = cookie
        && let Ok(value) = HeaderValue::from_str(cookie)
    {
        headers.insert("cookie", value);
    }
    headers
}

/// Like [`headers_with_referer`], but with a runtime `Referer`.
pub fn headers_with_dynamic_referer(user_agent: &'static str, referer: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, HeaderValue::from_static(user_agent));
    if let Ok(value) = HeaderValue::from_str(referer) {
        headers.insert(REFERER, value);
    }
    headers
}

/// Build a [`HeaderMap`] from arbitrary key/value pairs, skipping any entry
/// whose name or value is not a valid header. Used for per-media download
/// headers that override the platform default.
pub fn headers_from_pairs(pairs: &[(String, String)]) -> HeaderMap {
    let mut headers = HeaderMap::new();
    for (name, value) in pairs {
        if let (Ok(name), Ok(value)) = (
            HeaderName::from_bytes(name.as_bytes()),
            HeaderValue::from_str(value),
        ) {
            headers.insert(name, value);
        }
    }
    headers
}

/// Return appropriate download headers (User-Agent + Referer) for a given platform.
///
/// Each platform's CDN may check Referer and/or User-Agent. This centralises
/// the mapping so both renderers use consistent headers for media downloads.
pub fn download_headers_for_platform(platform: Platform) -> HeaderMap {
    match platform {
        Platform::Bilibili => headers_with_referer(DEFAULT_USER_AGENT, "https://www.bilibili.com"),
        Platform::Douyin => headers_with_referer(MOBILE_USER_AGENT, "https://www.douyin.com/"),
        Platform::Twitter => headers_with_referer(DEFAULT_USER_AGENT, "https://x.com"),
        Platform::YouTube => headers_with_referer(DEFAULT_USER_AGENT, "https://www.youtube.com/"),
        Platform::TikTok => headers_with_referer(MOBILE_USER_AGENT, "https://www.tiktok.com/"),
        Platform::Weibo => headers_with_referer(DEFAULT_USER_AGENT, "https://weibo.com/"),
        Platform::Xiaohongshu => {
            headers_with_referer(DEFAULT_USER_AGENT, "https://www.xiaohongshu.com/")
        }
        Platform::Kuaishou => headers_with_referer(MOBILE_USER_AGENT, "https://v.kuaishou.com/"),
        Platform::AcFun => headers_with_referer(DEFAULT_USER_AGENT, "https://www.acfun.cn/"),
        Platform::Nga => headers_with_referer(DEFAULT_USER_AGENT, "https://nga.178.com/"),
        Platform::Xiaoheihe => headers_with_referer(DEFAULT_USER_AGENT, "https://xiaoheihe.cn/"),
        Platform::Instagram => {
            headers_with_referer(DEFAULT_USER_AGENT, "https://www.instagram.com/")
        }
        Platform::V2ex => headers_with_referer(DEFAULT_USER_AGENT, "https://www.v2ex.com/"),
        Platform::Hongguo => headers_with_referer(DEFAULT_USER_AGENT, "https://hongguoduanju.com/"),
        Platform::Wechat => headers_with_referer(DEFAULT_USER_AGENT, "https://mp.weixin.qq.com/"),
        Platform::WechatChannel => {
            headers_with_referer(DEFAULT_USER_AGENT, "https://channels.weixin.qq.com/")
        }
    }
}

// ---------------------------------------------------------------------------
// Redirect helpers
// ---------------------------------------------------------------------------

/// Maximum redirect hops followed by the helpers below.
const MAX_REDIRECTS: usize = 10;

/// Follow all redirects for `url` and return the final URL as a string.
pub async fn get_final_url(client: &Client, url: &str, headers: HeaderMap) -> Result<String> {
    let response = get_with_redirect(client, url, headers).await?;
    let final_url = response.uri().to_string();
    if final_url != url {
        tracing::debug!(from = url, to = %final_url, "Redirect resolved");
    }
    Ok(final_url)
}

/// Follow all redirects for `url` and return the full response.
///
/// The redirect policy is pinned per-request, so redirects are followed even
/// on a client whose global policy disables them (wreq's default). Callers
/// can inspect both `response.uri()` (the final URL) and
/// `response.text().await` (the body).
pub async fn get_with_redirect(client: &Client, url: &str, headers: HeaderMap) -> Result<Response> {
    let response = client
        .get(url)
        .headers(headers)
        .redirect(Policy::limited(MAX_REDIRECTS))
        .send()
        .await?;
    Ok(response)
}

/// Outcome of [`resolve_redirect`].
#[derive(Debug)]
pub struct ResolvedRedirect {
    /// The URL the redirect chain ended on.
    pub final_url: String,
    /// Every `Set-Cookie` header value seen across all hops, in order.
    /// Platforms harvest tokens from these (e.g. Douyin's `ttwid`).
    pub set_cookies: Vec<String>,
}

/// Resolve a short URL by following redirects hop-by-hop.
///
/// Unlike [`get_with_redirect`], this collects `Set-Cookie` headers from every
/// intermediate response — automatic redirect following only exposes the final
/// response's headers, losing cookies set by the redirecting hop. The final
/// response's body is discarded; use [`get_with_redirect`] when it is needed.
pub async fn resolve_redirect(
    client: &Client,
    url: &str,
    headers: HeaderMap,
) -> Result<ResolvedRedirect> {
    let mut current = url.to_string();
    let mut set_cookies: Vec<String> = Vec::new();

    for _ in 0..=MAX_REDIRECTS {
        let response = client
            .get(current.as_str())
            .headers(headers.clone())
            .redirect(Policy::none())
            .send()
            .await?;

        set_cookies.extend(
            response
                .headers()
                .get_all(SET_COOKIE)
                .iter()
                .filter_map(|value| value.to_str().ok())
                .map(str::to_string),
        );

        // A redirect status without a Location header is treated as final.
        let location = response
            .headers()
            .get(LOCATION)
            .and_then(|value| value.to_str().ok())
            .filter(|_| response.status().is_redirection());
        let Some(location) = location else {
            let final_url = response.uri().to_string();
            if final_url != url {
                tracing::debug!(from = url, to = %final_url, "Redirect resolved");
            }
            return Ok(ResolvedRedirect {
                final_url,
                set_cookies,
            });
        };

        // `Location` may be relative; resolve it against the current URL.
        current = url::Url::parse(&current)
            .and_then(|base| base.join(location))
            .map_err(|_| ParserError::Parse(format!("invalid redirect Location: {location}")))?
            .to_string();
    }

    Err(ParserError::Parse(format!(
        "too many redirects resolving {url}"
    )))
}

// ---------------------------------------------------------------------------
// Response status checking
// ---------------------------------------------------------------------------

/// Check that a response has a successful status code, returning a
/// `ParserError::Api` on failure.
pub fn check_status(response: &Response) -> Result<()> {
    let status = response.status();
    if !status.is_success() {
        return Err(ParserError::Api {
            code: status.as_u16() as i64,
            message: format!("HTTP {}", status),
        });
    }
    Ok(())
}

/// Check the status code and return the body text.
///
/// Combines a status check with `.text().await` for the common pattern of
/// "fetch page, ensure 200, read body".
pub async fn checked_text(response: Response) -> Result<String> {
    check_status(&response)?;
    Ok(response.text().await?)
}

// ---------------------------------------------------------------------------
// Retry helper
// ---------------------------------------------------------------------------

/// Maximum number of retry attempts for transient errors.
const MAX_RETRIES: u32 = 2;

/// Base delay between retries (doubled on each attempt).
const BASE_DELAY_MS: u64 = 500;

/// Retryable status codes.
fn is_retryable(status: StatusCode) -> bool {
    matches!(
        status,
        StatusCode::TOO_MANY_REQUESTS
            | StatusCode::INTERNAL_SERVER_ERROR
            | StatusCode::BAD_GATEWAY
            | StatusCode::SERVICE_UNAVAILABLE
            | StatusCode::GATEWAY_TIMEOUT
    )
}

/// Execute a GET request with automatic retry on transient failures (429, 5xx).
///
/// Uses exponential back-off: 500 ms, 1 000 ms, ...
pub async fn get_with_retry(client: &Client, url: &str, headers: HeaderMap) -> Result<Response> {
    let mut last_err: Option<ParserError> = None;

    for attempt in 0..=MAX_RETRIES {
        if attempt > 0 {
            let delay = BASE_DELAY_MS * (1u64 << (attempt - 1));
            tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
        }

        let response = match client.get(url).headers(headers.clone()).send().await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(
                    url,
                    attempt = attempt + 1,
                    max = MAX_RETRIES + 1,
                    error = %e,
                    "HTTP request failed"
                );
                last_err = Some(ParserError::Http(e));
                continue;
            }
        };

        if is_retryable(response.status()) && attempt < MAX_RETRIES {
            tracing::warn!(
                url,
                status = %response.status(),
                attempt = attempt + 1,
                max = MAX_RETRIES + 1,
                "Retryable HTTP status"
            );
            last_err = Some(ParserError::Api {
                code: response.status().as_u16() as i64,
                message: format!("HTTP {} (will retry)", response.status()),
            });
            continue;
        }

        if attempt > 0 {
            tracing::debug!(
                url,
                retries = attempt,
                "HTTP request succeeded after retries"
            );
        }

        return Ok(response);
    }

    Err(last_err.unwrap_or_else(|| ParserError::Parse("retry exhausted".into())))
}
