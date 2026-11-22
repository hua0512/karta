use karta_core::parser::RegisteredPattern;
use karta_core::patterns;

/// Share links look like `https://weixin.qq.com/sph/AbCd123`.
///
/// Matched scheme-less and case-insensitively while preserving the original
/// casing of the share id.
pub(super) fn registered_patterns() -> Vec<RegisteredPattern> {
    patterns![
        "weixin_sph" => r"(?i)weixin\.qq\.com/sph/[A-Za-z0-9]+",
    ]
}
