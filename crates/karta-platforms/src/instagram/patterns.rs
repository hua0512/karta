use karta_core::parser::RegisteredPattern;
use karta_core::patterns;

pub(super) fn registered_patterns(has_cookie: bool) -> Vec<RegisteredPattern> {
    let mut patterns = patterns![
        "instagram_post" => r"instagram\.com/(?:p|reels?)/([A-Za-z0-9_-]+)",
    ];

    if has_cookie {
        patterns.extend(patterns![
            "instagram_story" => r"instagram\.com/stories/([A-Za-z0-9._]+)/(\d+)",
        ]);
    }

    patterns
}
