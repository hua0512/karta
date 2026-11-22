use karta_core::parser::RegisteredPattern;
use karta_core::patterns;

pub(super) fn registered_patterns() -> Vec<RegisteredPattern> {
    patterns![
        // Short share link: mp.weixin.qq.com/s/<id>
        "mp_weixin_short" => r"mp\.weixin\.qq\.com/s/([A-Za-z\d_-]+)",
        // Long form: mp.weixin.qq.com/s?__biz=...&mid=...&idx=...&sn=... (any param order)
        "mp_weixin_query" => r"mp\.weixin\.qq\.com/s\?[A-Za-z\d._~?%&+\-=/#]+",
    ]
}
