//! WeChat official-account articles end to end: short links redirected to the
//! canonical article, and deleted articles.

use karta_core::Platform;
use karta_core::data::Content;
use karta_core::error::ParserError;
use serde_json::json;
use test_harness::{FakeUpstream, Fixtures, Reply, TempApp};

const FIXTURES: Fixtures = Fixtures(env!("CARGO_MANIFEST_DIR"));

#[tokio::test]
async fn short_link_follows_the_redirect_to_the_article() {
    let fake = FakeUpstream::start().await;
    let short = fake
        .route("mp.weixin.qq.com", "GET", "/s/dQnHCXfeqJ0WHkeriL4kOw")
        .reply(Reply::redirect(
            "https://mp.weixin.qq.com/s?__biz=MzA0MDAwMTIzNA==&mid=2650000001&idx=1&sn=0123456789abcdef",
        ));
    let article = fake
        .route("mp.weixin.qq.com", "GET", "/s?sn=0123456789abcdef")
        .reply(FIXTURES.html("wechat/article.html"));
    let (_app, engine) = TempApp::with_platforms(&fake, json!({ "download_media": false }));

    let result = engine
        .parse("看看这篇 https://mp.weixin.qq.com/s/dQnHCXfeqJ0WHkeriL4kOw ，很有意思")
        .await
        .unwrap()
        .expect("wechat link matches");

    assert_eq!(result.platform, Platform::Wechat);
    assert_eq!(
        result.url.as_deref(),
        Some("https://mp.weixin.qq.com/s/dQnHCXfeqJ0WHkeriL4kOw")
    );
    assert_eq!(
        result.title.as_deref(),
        Some("测试文章标题：微信公众号解析")
    );
    assert_eq!(
        result.author.as_ref().map(|a| a.name.as_str()),
        Some("测试公众号")
    );
    assert_eq!(result.images().len(), 3);
    assert_eq!(result.timestamp, Some(1_712_345_678));
    assert_eq!(
        result.extra.get("作者").map(String::as_str),
        Some("署名作者")
    );

    let text = result.text.as_deref().expect("body text");
    for paragraph in [
        "第一段正文，介绍文章主旨。",
        "第二段包含\"实体&符号\"。",
        "图片之后的第三段文字。",
        "结尾段落，收工。",
    ] {
        assert!(text.contains(paragraph), "{text}");
    }
    assert!(!text.contains('<'), "{text}");
    assert!(
        !text.contains("这是文章摘要"),
        "body must win over description: {text}"
    );
    let images: Vec<_> = result
        .images()
        .into_iter()
        .map(|image| image.url.as_deref().expect("image URL"))
        .collect();
    assert_eq!(
        images,
        [
            "https://mmbiz.qpic.cn/mmbiz_jpg/COVERHASH/640?wx_fmt=jpeg",
            "https://mmbiz.qpic.cn/mmbiz_jpg/IMGONE/640?wx_fmt=jpeg&from=appmsg",
            "https://mmbiz.qpic.cn/mmbiz_png/IMGTWO/640?wx_fmt=png",
        ]
    );
    let kinds: Vec<_> = result
        .contents
        .iter()
        .map(|content| match content {
            Content::Text(_) => "text",
            Content::Image(_) => "image",
            other => panic!("unexpected content: {other:?}"),
        })
        .collect();
    assert_eq!(kinds, ["image", "text", "image", "text", "image", "text"]);

    let request = short.last_request();
    assert_eq!(request.header("referer"), Some("https://mp.weixin.qq.com/"));
    assert_eq!(
        request.header("cookie"),
        None,
        "articles are fetched anonymously"
    );
    assert_eq!(article.hits(), 1);
    fake.assert_no_unmatched();
}

#[tokio::test]
async fn deleted_article_reports_the_page_message() {
    let fake = FakeUpstream::start().await;
    fake.route("mp.weixin.qq.com", "GET", "/s/deletedArticle1")
        .reply(FIXTURES.html("wechat/deleted.html"));
    let (_app, engine) = TempApp::with_platforms(&fake, json!({ "download_media": false }));

    let err = engine
        .parse("https://mp.weixin.qq.com/s/deletedArticle1")
        .await
        .unwrap_err();

    assert!(
        matches!(&err, ParserError::ContentNotFound(message) if message == "该内容已被发布者删除"),
        "{err:?}"
    );
    fake.assert_no_unmatched();
}
