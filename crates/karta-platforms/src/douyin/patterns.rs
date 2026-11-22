use std::sync::LazyLock;

use karta_core::parser::RegisteredPattern;
use karta_core::patterns;

pub(super) static VIDEO_ID_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"(?:video|note|slides|article)/(\d+)").expect("valid douyin video id regex")
});

pub(super) static LIVE_ID_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"live\.douyin\.com/(\d+)").expect("valid douyin live id regex")
});

pub(super) static WEBCAST_ID_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"webcast\.amemv\.com/douyin/webcast/reflow/(\d+)")
        .expect("valid douyin webcast id regex")
});

pub(super) fn registered_patterns() -> Vec<RegisteredPattern> {
    let mut patterns = patterns![
        "v.douyin"        => r"v\.douyin\.com/[a-zA-Z0-9_\-]+",
        "jx.douyin"       => r"jx\.douyin\.com/[a-zA-Z0-9_\-]+",
        "webcast_amemv"   => r"webcast\.amemv\.com/[A-Za-z\d._?%&+\-=/#]*",
        "douyin_live"     => r"live\.douyin\.com/(\d+)",
        "douyin_live_path"=> r"www\.douyin\.com/live/(\d+)",
        "douyin"          => r"(?:douyin\.com/(?:video|note|article)|www\.douyin\.com/share/(?:video|note|article))/(\d+)",
        "douyin_slides"   => r"www\.douyin\.com/share/slides/(\d+)",
        "douyin_modal"    => r"douyin\.com/(?:jingxuan|discover)\?[^#]*modal_id=(\d+)",
        "iesdouyin"       => r"iesdouyin\.com/share/(?:slides|video|note|article)/(\d+)",
        "m.douyin"        => r"m\.douyin\.com/share/(?:slides|video|note|article)/(\d+)",
        "jingxuan.douyin" => r"jingxuan\.douyin\.com/m/(?:slides|video|note)/(\d+)",
        // Long-article share 口令 carry no URL, only a token wrapped in
        // doubled decorations that rotate between app versions: ˇˇ…➝➝
        // (caron/arrow), ŠŠ…︽︽, ǚǚ…ǚǚ, ^^…※※ (ASCII caret / reference
        // mark). Requiring the pair keeps this off ordinary text such as
        // names (`Šarlota123★`); single-delimiter variants still resolve
        // through the wording-anchored pattern below.
        "douyin_share_token" => r"[\x{02C6}-\x{02CF}\x{0160}\x{0161}\x{01D9}\x{01DA}\^]{2}([A-Za-z0-9_+/=-]{6,256})[\x{2190}-\x{27FF}\x{FE1D}-\x{FE1F}\x{FE3D}-\x{FE44}\x{01D9}\x{01DA}\x{203B}]{2}",
        // Fallback for decoration styles not listed above (══…══, %%…%%),
        // anchored on the share text's fixed wording. URL-bearing share texts
        // never reach it: the URL patterns above take precedence.
        "douyin_share_phrase" => r"(?:即可阅读文章|打开(?:抖音|[Dd]ou音)(?:[，,、\s]{0,4}(?:来?看看(?:我|Ta|TA|ta)的(?:日常|作品)|查看日常))?)[^A-Za-z0-9_+/=\p{Han}-]{1,12}([A-Za-z0-9_+/=-]{10,256})(?:##|%%|\$\$|ωω|ΔΔ|€|[￥¥]|[^A-Za-z0-9_+/=-]|$)",
        "douyin_activity" => r"(?:^|[^A-Za-z0-9_])(activity_[A-Za-z0-9_=-]{6,256})(?:[^A-Za-z0-9_=-]|$)",
    ];

    // These wrappers require nearby Douyin wording, so ordinary passwords or
    // currency amounts do not trigger carrier requests. Keep legacy patterns
    // first to preserve their capture and precedence contracts.
    for delimiter in [r"##", r"%%", r"\$\$", r"[￥¥]", r"ωω", r"ΔΔ", r"€"] {
        for pattern in [
            format!(
                r"(?:抖音|[Dd]ou音)[\s\S]{{0,80}}?{delimiter}([A-Za-z0-9_+/=-]{{6,256}}){delimiter}"
            ),
            format!(
                r"{delimiter}([A-Za-z0-9_+/=-]{{6,256}}){delimiter}[\s\S]{{0,80}}?(?:抖音|[Dd]ou音)"
            ),
        ] {
            patterns.push(RegisteredPattern {
                name: "douyin_share_command",
                regex: regex::Regex::new(&pattern).expect("valid douyin command regex"),
            });
        }
    }
    patterns
}
