use karta_core::parser::RegisteredPattern;
use karta_core::patterns;

pub(super) fn registered_patterns() -> Vec<RegisteredPattern> {
    patterns![
        "twitter" => r"(?:twitter\.com|x\.com|r\.com)/[0-9a-zA-Z_]{1,20}/status/(\d+)",
    ]
}
