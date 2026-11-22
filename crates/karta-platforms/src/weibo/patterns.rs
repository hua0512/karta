use karta_core::parser::RegisteredPattern;
use karta_core::patterns;

pub(super) fn registered_patterns() -> Vec<RegisteredPattern> {
    patterns![
        "weibo_tv"          => r"weibo\.com/tv/show/\d+:\d+\?mid=(\d+)",
        "video_weibo"       => r"video\.weibo\.com/show\?fid=(\d+:\d+)",
        "article_ttarticle" => r"weibo\.com/ttarticle/p/show\?id=(\d+)",
        "article_id"        => r"weibo\.com/ttarticle/x/m/show(?:/id/|#/id=)(\d+)",
        "article_card"      => r"weibo\.com/article/m/show/id/(\d+)",
        "m_weibo"           => r"weibo\.cn/(?:status|detail|\d+)/([0-9a-zA-Z]+)",
        "weibo_status"      => r"weibo\.com/\d+/([0-9a-zA-Z]+)",
        "mapp_weibo"        => r"mapp\.api\.weibo\.cn/fx/[0-9A-Za-z]+\.html",
    ]
}
