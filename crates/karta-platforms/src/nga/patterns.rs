use std::sync::LazyLock;

use karta_core::parser::RegisteredPattern;
use karta_core::patterns;

/// Anti-bot interstitials embed the guest cookie as `document.cookie = 'guestJs=...'`.
pub(super) static GUESTJS_COOKIE_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r#"document\.cookie\s*=\s*['\"]guestJs=([^;'\"]+)"#)
        .expect("valid nga guestJs cookie regex")
});

pub(super) fn registered_patterns() -> Vec<RegisteredPattern> {
    // `tid=` must stay anchored to NGA hosts: the bare query parameter appears
    // on unrelated sites (Steam, ad trackers) and would trigger a network
    // fetch of nga.178.com for every match.
    patterns![
        "nga" => r"(?:bbs\.nga\.cn|nga\.178\.com|ngabbs\.com)/read\.php\?(?:[^#\s]*&)?tid=(\d+)",
    ]
}
