//! Pure steps of the web cookie refresh (bilibili-API-collect
//! `docs/login/cookie_refresh.md`): reading `refresh_csrf`, the two request
//! forms, and the credential the refresh produces.

use std::sync::LazyLock;

use scraper::{Html, Selector};

use super::model::BiliCredential;

/// `source` sent to `cookie/refresh`, as the web page does.
const REFRESH_SOURCE: &str = "main_web";

/// The `correspond/1/{path}` page renders `refresh_csrf` server-side as the
/// text of `<div id="1-name">`. An id starting with a digit is not a valid
/// `#id` selector, hence the attribute form.
static REFRESH_CSRF_SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse(r#"div[id="1-name"]"#).expect("valid refresh_csrf selector"));

/// `refresh_csrf` from the correspond page, or `None` when the page does not
/// carry one (an expired or wrong path gets a 404 page instead).
pub(super) fn extract_refresh_csrf(html: &str) -> Option<String> {
    let document = Html::parse_document(html);
    let text: String = document
        .select(&REFRESH_CSRF_SELECTOR)
        .next()?
        .text()
        .collect();
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_string())
}

/// The refresh token (`ac_time_value`) a credential holds.
pub(super) fn refresh_token(cred: &BiliCredential) -> Option<&str> {
    cred.ac_time_value
        .as_deref()
        .filter(|token| !token.is_empty())
}

/// Form for `cookie/refresh`: the current cookie's csrf and the refresh token
/// that belongs to it.
pub(super) fn refresh_form<'a>(
    old: &'a BiliCredential,
    refresh_csrf: &'a str,
) -> [(&'static str, &'a str); 4] {
    [
        ("csrf", &old.bili_jct),
        ("refresh_csrf", refresh_csrf),
        ("source", REFRESH_SOURCE),
        ("refresh_token", refresh_token(old).unwrap_or_default()),
    ]
}

/// Form for `confirm/refresh`, sent with the new cookie: the csrf of the
/// *new* cookie, but the *old* refresh token, which names the session that
/// Bilibili then invalidates.
pub(super) fn confirm_form<'a>(
    old: &'a BiliCredential,
    new: &'a BiliCredential,
) -> [(&'static str, &'a str); 2] {
    [
        ("csrf", &new.bili_jct),
        ("refresh_token", refresh_token(old).unwrap_or_default()),
    ]
}

/// The credential after a successful `cookie/refresh`: `old` with the cookies
/// the response set and `new_token` as its refresh token. `None` unless both
/// `SESSDATA` and `bili_jct` were set.
pub(super) fn refreshed_credential(
    old: &BiliCredential,
    cookies: impl IntoIterator<Item = (String, String)>,
    new_token: String,
) -> Option<BiliCredential> {
    let mut sessdata = None;
    let mut bili_jct = None;
    let mut dedeuserid = old.dedeuserid.clone();
    for (name, value) in cookies {
        match name.as_str() {
            "SESSDATA" => sessdata = Some(value),
            "bili_jct" => bili_jct = Some(value),
            "DedeUserID" => dedeuserid = Some(value),
            _ => {}
        }
    }
    Some(BiliCredential {
        sessdata: sessdata?,
        bili_jct: bili_jct?,
        buvid3: old.buvid3.clone(),
        dedeuserid,
        ac_time_value: Some(new_token.clone()),
        refresh_token: Some(new_token),
        expires_at: None,
    })
}
