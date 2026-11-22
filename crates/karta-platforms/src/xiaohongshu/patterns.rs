use karta_core::parser::RegisteredPattern;
use karta_core::patterns;

pub(super) fn registered_patterns() -> Vec<RegisteredPattern> {
    // The query is limited to characters a URL query may contain (RFC 3986),
    // so chat text glued to a shared link (full-width punctuation, CJK) is not
    // sent upstream as part of `xsec_token` and friends.
    patterns![
        "xhslink"   => r"xhslink\.(?:com|cn)/[A-Za-z0-9._?%&+=/#@-]+",
        "explore"   => r"(?:xiaohongshu\.com|rednote\.com)/explore/([a-f0-9]+)(?:\?([A-Za-z0-9._~%!$&'()*+,;=:@/?-]*))?",
        "discovery" => r"(?:xiaohongshu\.com|rednote\.com)/discovery/item/([a-f0-9]+)(?:\?([A-Za-z0-9._~%!$&'()*+,;=:@/?-]*))?",
    ]
}
