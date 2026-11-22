use karta_core::parser::RegisteredPattern;
use karta_core::patterns;

pub(super) fn registered_patterns() -> Vec<RegisteredPattern> {
    patterns![
        // Anchored to the acfun.cn host so fragments like "Mac=5" or foreign
        // URLs containing "/ac123" cannot match. The second alternative covers
        // mobile share links (m.acfun.cn/v/?ac=123&shareType=pic). Multi-part
        // URLs (ac{id}_{part}) must consume the "_{part}" suffix explicitly —
        // `_` is a word character, so `\b` alone would reject them — while
        // group 1 keeps only the acid (the parser always fetches part 1).
        "acfun" => r"acfun\.cn/(?:v/ac|\S*?[?&]ac=)(\d+)(?:_\d+)?\b",
    ]
}
