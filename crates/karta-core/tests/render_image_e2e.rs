//! Image card renderer end to end: the engine renders a card, emoji come from
//! a fake emoji CDN (`emoji_cdn_url`) and media from a fake CDN.

use std::path::{Path, PathBuf};

use image::RgbaImage;
use karta_core::data::{Author, Content, GraphicsContent, ImageContent, VideoContent};
use karta_core::message::{RenderOutput, SegmentType};
use karta_core::{Config, ParseResult, Platform};
use serde_json::json;
use test_harness::{FakeUpstream, Reply, TempApp};

const RED: [u8; 4] = [255, 0, 0, 255];

#[tokio::test]
async fn every_header_supports_text_only_with_or_without_author() {
    use karta_core::config::ImageHeaderStyle;
    use karta_core::render::{RenderOptions, image::ImageRenderer};

    let directory = tempfile::tempdir().unwrap();
    let post = ParseResult::new(Platform::Douyin)
        .with_title("没有封面的文字内容")
        .with_text("第一段文字。\n\n第二段继续阅读，卡片应当保留完整的顶部和纸面。");
    let mut previews = Vec::new();
    for style in [
        ImageHeaderStyle::Auto,
        ImageHeaderStyle::MeshGlow,
        ImageHeaderStyle::Blur,
        ImageHeaderStyle::SoftPrism,
        ImageHeaderStyle::BlurBackground,
    ] {
        let renderer = ImageRenderer::new(
            directory.path().to_path_buf(),
            RenderOptions {
                image_header_style: style,
                emoji_enabled: false,
                ..Default::default()
            },
        );
        for result in [
            post.clone(),
            post.clone().with_author(Author::new("没有头像的作者")),
        ] {
            let image = renderer.render_card_image(&result).await.unwrap();
            assert_eq!(image.width(), 800);
            assert!(image.height() > 176);
            assert!(image.pixels().all(|pixel| pixel[3] == 255));
            if style == ImageHeaderStyle::Auto {
                let again = renderer.render_card_image(&result).await.unwrap();
                assert_eq!(image, again, "auto must stay stable without media");
            }
        }
        previews.push(renderer.render_card_image(&post).await.unwrap());
    }
    assert_ne!(previews[1], previews[2]);
    assert_ne!(previews[2], previews[3]);
    assert_ne!(previews[1], previews[3]);
}

#[tokio::test]
async fn append_url_is_a_separate_clickable_message() {
    let fake = FakeUpstream::start().await;
    let app =
        image_app(json!({"download_media": false, "append_url": true, "emoji_enabled": false}));
    let engine = app.engine(fake.downloader(app.config(), app.data_path()), vec![]);
    let result = ParseResult::new(Platform::Douyin)
        .with_text("A post without a source footer")
        .with_url("https://example.com/post");
    let outputs = engine.render(&result).await.unwrap();
    assert_eq!(outputs.len(), 2);
    let url = &outputs[1].messages()[0].segments[0];
    assert_eq!(url.segment_type, SegmentType::Text);
    assert_eq!(url.text.as_deref(), Some("https://example.com/post"));
}

fn png(color: [u8; 4]) -> Vec<u8> {
    let mut bytes = Vec::new();
    RgbaImage::from_pixel(72, 72, image::Rgba(color))
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .unwrap();
    bytes
}

fn image_app(extra: serde_json::Value) -> TempApp {
    let mut overrides = json!({
        "render_type": "image",
        "forward_contents": false,
        "forward_text_threshold": 0,
        "forward_media_threshold": 0,
    });
    overrides
        .as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    TempApp::new(overrides)
}

/// The file behind each output, in order; every output must be a single
/// image file.
fn output_files(outputs: &[RenderOutput]) -> Vec<PathBuf> {
    outputs
        .iter()
        .map(|output| {
            let RenderOutput::Message(message) = output else {
                panic!("unexpected bundle: {output:?}");
            };
            let [segment] = message.segments.as_slice() else {
                panic!("expected one segment: {message:?}");
            };
            assert_eq!(segment.segment_type, SegmentType::Image);
            segment.file_path.clone().expect("a local file")
        })
        .collect()
}

#[track_caller]
fn assert_card(path: &Path, data_path: &Path) -> RgbaImage {
    assert!(path.starts_with(Config::cache_dir(data_path)), "{path:?}");
    assert!(
        path.to_string_lossy().ends_with("_card.png"),
        "{path:?} is not a card"
    );
    let card = image::open(path).expect("card is a PNG").to_rgba8();
    assert_eq!(card.width(), 800);
    card
}

#[cfg(feature = "emoji")]
#[tokio::test]
async fn card_fetches_each_emoji_once_and_draws_it() {
    let fake = FakeUpstream::start().await;
    // The emoji client is not intercepted: it reaches the fake by address.
    let emoji_route = |codepoints: &str| {
        fake.route("127.0.0.1", "GET", &format!("/emoji/{codepoints}.png"))
            .reply(Reply::ok().content_type("image/png").body(png(RED)))
    };
    let served = [
        ("author", emoji_route("1f600")),
        ("title", emoji_route("1f30d")),
        ("text", emoji_route("1f389")),
        ("extra", emoji_route("1f44d")),
        ("graphics description", emoji_route("1f525")),
        ("graphics alt text", emoji_route("2728")),
    ];
    let missing = fake
        .route("127.0.0.1", "GET", "/emoji/1f680.png")
        .reply(Reply::status(404));
    let app = image_app(json!({
        "download_media": false,
        "emoji_enabled": true,
        "emoji_cdn_url": format!("{}/emoji", fake.base_url()),
    }));

    let mut graphics = GraphicsContent::new()
        .with_description("图 🔥")
        .with_alt_text("说明 ✨");
    graphics.add_image(ImageContent::new().with_url("https://i0.hdslb.com/1.jpg"));
    let result = ParseResult::new(Platform::Bilibili)
        .with_author(Author::new("UP主 😀"))
        // 😀 again: one fetch per unique emoji.
        .with_title("标题 🌍 😀")
        .with_text("正文 🎉")
        .with_extra("点赞", "👍")
        .with_url("https://www.bilibili.com/video/BV1xx411c7mD")
        .with_content(Content::Graphics(graphics))
        .with_repost(ParseResult::new(Platform::Douyin).with_title("转发 🚀"));
    let engine = app.engine(fake.downloader(app.config(), app.data_path()), vec![]);

    let first = output_files(&engine.render(&result).await.unwrap());
    assert_eq!(first.len(), 1, "the card only: {first:?}");
    let card = assert_card(&first[0], app.data_path());
    assert!(
        card.pixels().any(|pixel| pixel.0 == RED),
        "the fetched emoji is drawn into the card"
    );

    // Rendering again reuses fetched emoji and does not retry the 404.
    let second = output_files(&engine.render(&result).await.unwrap());
    assert_eq!(second, first, "the card path is stable per post");

    for (field, route) in &served {
        assert_eq!(route.hits(), 1, "emoji in the {field}");
    }
    assert_eq!(missing.hits(), 1, "emoji missing from the CDN");
    fake.assert_no_unmatched();
}

#[tokio::test]
async fn card_is_followed_by_downloaded_images_in_content_order() {
    let fake = FakeUpstream::start().await;
    let cdn = "i0.hdslb.com";
    for (path, body) in [("/a.jpg", "image-a"), ("/b.jpg", "image-b")] {
        fake.route(cdn, "GET", path).reply(Reply::ok().body(body));
    }
    let covers = [
        fake.route(cdn, "GET", "/cover-only.jpg")
            .reply(Reply::ok().body(png(RED))),
        fake.route(cdn, "GET", "/cover-dead.jpg")
            .reply(Reply::ok().body(png(RED))),
    ];
    let app = image_app(json!({ "download_media": true, "emoji_enabled": false }));

    let result = ParseResult::new(Platform::Bilibili)
        .with_title("标题")
        .with_content(Content::Image(
            ImageContent::new().with_url(format!("https://{cdn}/a.jpg")),
        ))
        // No stream: the card shows the cover, nothing else is sent.
        .with_content(Content::Video(
            VideoContent::new().with_cover(format!("https://{cdn}/cover-only.jpg")),
        ))
        // Unreachable streams: same, no duplicate cover after the card.
        .with_content(Content::Video(
            VideoContent::new()
                .with_streams("http://127.0.0.1:1/v.m4s", "http://127.0.0.1:1/a.m4s")
                .with_cover(format!("https://{cdn}/cover-dead.jpg")),
        ))
        .with_content(Content::Image(
            ImageContent::new().with_url(format!("https://{cdn}/b.jpg")),
        ));
    let engine = app.engine(fake.downloader(app.config(), app.data_path()), vec![]);

    let files = output_files(&engine.render(&result).await.unwrap());

    assert_eq!(files.len(), 3, "{files:?}");
    assert_card(&files[0], app.data_path());
    let bodies: Vec<Vec<u8>> = files[1..]
        .iter()
        .map(|path| std::fs::read(path).unwrap())
        .collect();
    assert_eq!(bodies, [b"image-a".to_vec(), b"image-b".to_vec()]);
    // The covers were downloaded for the card itself.
    for cover in &covers {
        assert_eq!(cover.hits(), 1);
    }
    fake.assert_no_unmatched();
}

#[tokio::test]
async fn dead_video_leaves_only_the_card() {
    let fake = FakeUpstream::start().await;
    let cover = fake
        .route("i0.hdslb.com", "GET", "/cover.jpg")
        .reply(Reply::ok().body(png(RED)));
    let app = image_app(json!({ "download_media": true, "emoji_enabled": false }));
    let result = ParseResult::new(Platform::Bilibili)
        .with_title("标题")
        .with_content(Content::Video(
            VideoContent::new()
                .with_url_candidates(vec![
                    "http://127.0.0.1:1/v.mp4".to_string(),
                    "http://127.0.0.1:1/backup.mp4".to_string(),
                ])
                .with_cover("https://i0.hdslb.com/cover.jpg"),
        ));
    let engine = app.engine(fake.downloader(app.config(), app.data_path()), vec![]);

    let files = output_files(&engine.render(&result).await.unwrap());

    // The card shows the cover; it is not sent again.
    assert_eq!(files.len(), 1, "{files:?}");
    assert_card(&files[0], app.data_path());
    assert_eq!(cover.hits(), 1);
}

#[tokio::test]
async fn card_renders_tags_metrics_and_native_repost() {
    use karta_core::config::ImageHeaderStyle;
    use karta_core::render::{RenderOptions, image::ImageRenderer};

    let dir = tempfile::tempdir().unwrap();
    let cover_path = dir.path().join("cover.png");
    let cover_img = RgbaImage::from_fn(960, 540, |x, y| {
        let u = x as f32 / 959.0;
        let v = y as f32 / 539.0;
        let sun = (-((u - 0.72).powi(2) / 0.04 + (v - 0.30).powi(2) / 0.08)).exp();
        let r = (28.0 + v * 175.0 + sun * 110.0).min(255.0) as u8;
        let g = (34.0 + v * 92.0 + sun * 95.0).min(255.0) as u8;
        let b = (72.0 + (1.0 - v) * 65.0 + sun * 45.0).min(255.0) as u8;
        image::Rgba([r, g, b, 255])
    });
    cover_img.save(&cover_path).unwrap();

    let avatar_path = dir.path().join("avatar.png");
    let avatar_img = RgbaImage::from_fn(128, 128, |x, y| {
        let u = x as f32 / 127.0;
        let v = y as f32 / 127.0;
        image::Rgba([
            (235.0 - u * 40.0) as u8,
            (130.0 + v * 50.0) as u8,
            (85.0 + u * 60.0) as u8,
            255,
        ])
    });
    avatar_img.save(&avatar_path).unwrap();

    let repost = ParseResult::new(Platform::Bilibili)
        .with_author(Author::new("原博主 · 风光研究所"))
        .with_title("川西秋冬机位合集与延迟摄影参数分享")
        .with_text("所有素材均为 10-bit Log 直出，完整路线图和海拔注意事项放在简介里了。")
        .with_extra("点赞", "6,240")
        .with_extra("收藏", "4,190")
        .with_extra("标签", "#摄影攻略 #川西");

    let post = ParseResult::new(Platform::Bilibili)
        .with_author(
            Author::new("林深见鹿 · 摄影志").with_avatar(avatar_path.to_string_lossy().to_string()),
        )
        .with_timestamp(1_763_725_320)
        .with_title("日落前二十分钟的川西垭口，风把云吹成了橘色")
        .with_text(
            "一路盘山上来本来以为要白跑一趟，结果在海拔 4300 米突然破云，整个山谷都被染透了。",
        )
        .with_content(Content::Video(
            VideoContent::new()
                .with_cover(cover_path.to_string_lossy().to_string())
                .with_duration(222),
        ))
        .with_extra("播放", "12.8万")
        .with_extra("点赞", "9,420")
        .with_extra("收藏", "3,180")
        .with_extra("评论", "412")
        .with_extra("弹幕", "856")
        .with_extra("标签", "#川西自驾 #日落摄影 #4K航拍 #风光大片")
        .with_extra("IP属地", "四川")
        .with_repost(repost);

    let renderer = ImageRenderer::new(
        dir.path().to_path_buf(),
        RenderOptions {
            image_header_style: ImageHeaderStyle::BlurBackground,
            emoji_enabled: false,
            ..Default::default()
        },
    );

    let image = renderer.render_card_image(&post).await.unwrap();
    assert_eq!(image.width(), 800);
    assert!(image.height() > 600);
    assert!(image.pixels().all(|pixel| pixel[3] == 255));
}

#[tokio::test]
async fn dark_auto_split_hero_and_bento_collage_render_cleanly() {
    use karta_core::config::{ImageHeaderStyle, ImageThemeMode};
    use karta_core::render::{RenderOptions, image::ImageRenderer};

    let dir = tempfile::tempdir().unwrap();

    // 1. Dark moody 9:16 portrait video cover -> Auto picks Dark Glass + SplitHero
    let portrait_path = dir.path().join("portrait.png");
    RgbaImage::from_fn(540, 960, |x, y| {
        let u = x as f32 / 539.0;
        let v = y as f32 / 959.0;
        let glow = (-((u - 0.65).powi(2) / 0.08 + (v - 0.32).powi(2) / 0.05)).exp();
        let r = (18.0 + v * 58.0 + glow * 155.0).min(255.0) as u8;
        let g = (20.0 + v * 32.0 + glow * 88.0).min(255.0) as u8;
        let b = (42.0 + (1.0 - v) * 44.0 + glow * 42.0).min(255.0) as u8;
        image::Rgba([r, g, b, 255])
    })
    .save(&portrait_path)
    .unwrap();

    let douyin = ParseResult::new(Platform::Douyin)
        .with_author(Author::new("林深见鹿 · 摄影志"))
        .with_timestamp(1_763_725_320)
        .with_title("日落前二十分钟的川西垭口，风把云吹成了橘色")
        .with_text("竖屏 9:16 视频自动进入左右海报分栏，暗色调封面自动切到深色磨砂玻璃，彻底告别大片留白。")
        .with_content(Content::Video(
            VideoContent::new()
                .with_cover(portrait_path.to_string_lossy().to_string())
                .with_duration(58),
        ))
        .with_extra("播放", "28.4万")
        .with_extra("点赞", "2.1万")
        .with_extra("收藏", "6,340")
        .with_extra("评论", "890")
        .with_extra("分享", "1,520")
        .with_extra("标签", "#川西自驾 #延时摄影 #暗调风光");

    let auto_renderer = ImageRenderer::new(
        dir.path().to_path_buf(),
        RenderOptions {
            image_header_style: ImageHeaderStyle::BlurBackground,
            image_theme_mode: ImageThemeMode::Auto,
            emoji_enabled: false,
            ..Default::default()
        },
    );
    let dark_card = auto_renderer.render_card_image(&douyin).await.unwrap();
    assert_eq!(dark_card.width(), 800);
    // SplitHero keeps portrait cards compact instead of 1200px+ tall
    assert!(dark_card.height() < 850, "height={}", dark_card.height());
    // Outer page corner should be dark obsidian under Auto mode
    let corner = dark_card.get_pixel(4, 4);
    assert!(
        corner[0] < 90 && corner[1] < 90 && corner[2] < 110,
        "{corner:?}"
    );

    // 2. Bright 3-image Xiaohongshu gallery -> Auto stays Light + hole-free 1+2 Bento collage
    let mut gallery_paths = Vec::new();
    for (idx, base) in [
        ([232.0_f32, 188.0, 152.0], [186.0_f32, 132.0, 102.0]),
        ([148.0, 198.0, 224.0], [92.0, 148.0, 182.0]),
        ([168.0, 206.0, 162.0], [108.0, 154.0, 118.0]),
    ]
    .into_iter()
    .enumerate()
    {
        let p = dir.path().join(format!("photo_{idx}.png"));
        RgbaImage::from_fn(600, 750, |x, y| {
            let u = x as f32 / 599.0;
            let v = y as f32 / 749.0;
            let t = (u * 0.4 + v * 0.6).clamp(0.0, 1.0);
            image::Rgba([
                (base.0[0] * (1.0 - t) + base.1[0] * t) as u8,
                (base.0[1] * (1.0 - t) + base.1[1] * t) as u8,
                (base.0[2] * (1.0 - t) + base.1[2] * t) as u8,
                255,
            ])
        })
        .save(&p)
        .unwrap();
        gallery_paths.push(p);
    }

    let mut xhs = ParseResult::new(Platform::Xiaohongshu)
        .with_author(Author::new("岛屿漫游笔记"))
        .with_timestamp(1_763_712_480)
        .with_title("秋日环湖散步的三张胶片随拍")
        .with_text("午后三点的光线最柔和，从咖啡馆拐进梧桐巷一路走到湖边，不用修图就很有质感。")
        .with_extra("点赞", "4,821")
        .with_extra("收藏", "1,930")
        .with_extra("评论", "218")
        .with_extra("标签", "#胶片摄影 #秋日漫步 #杭州咖啡")
        .with_extra("IP属地", "浙江");
    for p in &gallery_paths {
        xhs = xhs.with_content(Content::Image(ImageContent::new().with_path(p.clone())));
    }

    let bento_card = auto_renderer.render_card_image(&xhs).await.unwrap();
    assert_eq!(bento_card.width(), 800);
    assert!(bento_card.pixels().all(|pixel| pixel[3] == 255));
    // Bright images stay light under Auto mode
    let light_corner = bento_card.get_pixel(4, 4);
    assert!(
        light_corner[0] > 160 && light_corner[1] > 150,
        "{light_corner:?}"
    );

    // 3. Explicit Dark mode renders dark obsidian even for text-only posts across header styles
    for style in [
        ImageHeaderStyle::BlurBackground,
        ImageHeaderStyle::MeshGlow,
        ImageHeaderStyle::Blur,
        ImageHeaderStyle::SoftPrism,
    ] {
        let dark_renderer = ImageRenderer::new(
            dir.path().to_path_buf(),
            RenderOptions {
                image_header_style: style,
                image_theme_mode: ImageThemeMode::Dark,
                emoji_enabled: false,
                ..Default::default()
            },
        );
        let text_only = ParseResult::new(Platform::Xiaoheihe)
            .with_author(Author::new("黑盒玩家"))
            .with_title("纯文字暗色卡片检查")
            .with_text("确保暗色模式与填充徽章 Logo 在所有样式下都正常。");
        let rendered = dark_renderer.render_card_image(&text_only).await.unwrap();
        let px = rendered.get_pixel(4, 4);
        assert!(px[0] < 90 && px[1] < 90 && px[2] < 110, "{style:?}: {px:?}");
    }

    // 4. Caption-in-title platforms with very long title fall back from SplitHero to full-width stack
    let long_title_post = ParseResult::new(Platform::TikTok)
        .with_author(Author::new("TikTok Creator"))
        .with_title(
            "这是一段非常长的标题用来模拟快手和海外版把整篇正文与标签塞进标题字段的情况。"
                .repeat(8),
        )
        .with_content(Content::Video(
            VideoContent::new()
                .with_cover(portrait_path.to_string_lossy().to_string())
                .with_duration(30),
        ));
    let long_card = auto_renderer
        .render_card_image(&long_title_post)
        .await
        .unwrap();
    assert!(
        long_card.height() > 900,
        "long caption-in-title should fall back to full-width cover stack, got {}",
        long_card.height()
    );

    // 5. >9 image gallery triggers +N overflow cleanly
    let mut overflow_post = ParseResult::new(Platform::Weibo).with_title("十二图测试");
    for i in 0..12 {
        overflow_post = overflow_post.with_content(Content::Image(
            ImageContent::new().with_path(gallery_paths[i % gallery_paths.len()].clone()),
        ));
    }
    let overflow_card = auto_renderer
        .render_card_image(&overflow_post)
        .await
        .unwrap();
    assert_eq!(overflow_card.width(), 800);
    assert!(overflow_card.pixels().all(|p| p[3] == 255));
}

#[tokio::test]
async fn repost_media_is_hydrated_before_rendering() {
    let fake = FakeUpstream::start().await;
    let cdn = "i0.hdslb.com";
    let repost_avatar = fake
        .route(cdn, "GET", "/repost-avatar.png")
        .reply(Reply::ok().body(png(RED)));
    let repost_cover = fake
        .route(cdn, "GET", "/repost-cover.png")
        .reply(Reply::ok().body(png([15, 20, 35, 255])));
    let app = image_app(json!({
        "download_media": true,
        "emoji_enabled": false,
        "image_theme_mode": "auto"
    }));
    let repost = ParseResult::new(Platform::Bilibili)
        .with_author(Author::new("原作者").with_avatar(format!("https://{cdn}/repost-avatar.png")))
        .with_title("转发暗调视频")
        .with_content(Content::Video(
            VideoContent::new().with_cover(format!("https://{cdn}/repost-cover.png")),
        ));
    let result = ParseResult::new(Platform::Bilibili)
        .with_author(Author::new("转发者"))
        .with_text("太好看了")
        .with_repost(repost);
    let engine = app.engine(fake.downloader(app.config(), app.data_path()), vec![]);

    let files = output_files(&engine.render(&result).await.unwrap());
    assert_eq!(files.len(), 1);
    let card = assert_card(&files[0], app.data_path());
    // Dark repost cover hydrates and drives Auto into obsidian dark mode
    let corner = card.get_pixel(4, 4);
    assert!(
        corner[0] < 90 && corner[1] < 90 && corner[2] < 110,
        "{corner:?}"
    );
    assert_eq!(repost_avatar.hits(), 1);
    assert_eq!(repost_cover.hits(), 1);
    fake.assert_no_unmatched();
}

#[cfg(feature = "emoji")]
async fn emoji_origin(body: Vec<u8>) -> FakeUpstream {
    let fake = FakeUpstream::start().await;
    fake.route("127.0.0.1", "GET", "/emoji/1f600.png")
        .reply(Reply::ok().body(body));
    fake
}

#[cfg(feature = "emoji")]
#[tokio::test]
async fn changing_custom_cdn_does_not_reuse_previous_disk_cache() {
    use karta_core::render::image::emoji::{EmojiConfig, EmojiSource};
    const BLUE: [u8; 4] = [0, 0, 255, 255];

    let dir = tempfile::tempdir().unwrap();
    let config_for = |cdn: String| EmojiConfig {
        custom_cdn_url: Some(cdn),
        cache_dir: dir.path().to_path_buf(),
        enabled: true,
        ..Default::default()
    };

    let red_origin = emoji_origin(png(RED)).await;
    let red = EmojiSource::new(config_for(format!("{}/emoji", red_origin.base_url())));
    let img = red.get_emoji_image("😀").await.expect("red emoji");
    assert_eq!(img.get_pixel(0, 0).0, RED);

    // Same cache dir, different CDN: the new CDN's artwork must be used.
    let blue_origin = emoji_origin(png(BLUE)).await;
    let blue = EmojiSource::new(config_for(format!("{}/emoji", blue_origin.base_url())));
    let img = blue.get_emoji_image("😀").await.expect("blue emoji");
    assert_eq!(img.get_pixel(0, 0).0, BLUE);
}

#[cfg(feature = "emoji")]
#[tokio::test]
async fn non_image_response_is_not_persisted() {
    use karta_core::render::image::emoji::{EmojiConfig, EmojiSource};
    let dir = tempfile::tempdir().unwrap();
    let origin = emoji_origin(b"<html>captive portal</html>".to_vec()).await;
    let source = EmojiSource::new(EmojiConfig {
        custom_cdn_url: Some(format!("{}/emoji", origin.base_url())),
        cache_dir: dir.path().to_path_buf(),
        enabled: true,
        ..Default::default()
    });

    assert!(source.get_emoji_image("😀").await.is_none());
    assert!(files_under(dir.path()).is_empty());
    assert_eq!(origin.requests().len(), 1);
    origin.assert_no_unmatched();
}

#[cfg(feature = "emoji")]
fn files_under(dir: &std::path::Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return files;
    };
    for entry in entries {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files.extend(files_under(&path));
        } else {
            files.push(path);
        }
    }
    files
}
