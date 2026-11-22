# Test URLs

Test URL catalog for pattern matching validation, checked by
`tests/test_patterns.rs` through the real `ParserRegistry` built by
`create_all_parsers` with the default config. Many rows are ported from the
Python `nonebot-plugin-parser` project.

Every `## <Platform>` section holds one table:

- `input`: the message text.
- `pattern`: the registered pattern the registry must pick. Patterns are tried
  in registration order across all platforms, so this column also pins
  precedence.
- `captures`: capture groups 1, 2, … in order, each in backticks. A bare `-`
  marks a group that does not participate. Every participating group must be
  listed; leave the cell empty for patterns without groups.
- `match`: when set, the exact full match. Parsers whose pattern has no
  capture group use the full match as their input.

The `## No match` section lists texts that no parser may claim. An unknown
heading or a malformed row fails the test.

## YouTube

The `ym` prefix requests audio only; it must win over the video patterns,
which also match the embedded URL.

| input | pattern | captures | match |
|---|---|---|---|
| `https://youtu.be/EKkzbbLYPuI?si=K_S9zIp5g7DhigVz` | youtu_be | `EKkzbbLYPuI` | |
| `https://youtu.be/aF_GTgeL11o?si=Bh3eeUUoJgWHyHdE` | youtu_be | `aF_GTgeL11o` | |
| `https://www.youtube.com/watch?v=EKkzbbLYPuI` | youtube_watch | `EKkzbbLYPuI` | |
| `https://www.youtube.com/watch?v=aF_GTgeL11o` | youtube_watch | `aF_GTgeL11o` | |
| `https://www.youtube.com/shorts/dQw4w9WgXcQ` | youtube_shorts | `dQw4w9WgXcQ` | |
| `ym https://www.youtube.com/watch?v=dQw4w9WgXcQ` | youtube_audio | `dQw4w9WgXcQ` | |
| `ym https://youtu.be/dQw4w9WgXcQ` | youtube_audio | `dQw4w9WgXcQ` | |
| `ym youtube.com/shorts/dQw4w9WgXcQ` | youtube_audio | `dQw4w9WgXcQ` | |

## Kuaishou

| input | pattern | captures | match |
|---|---|---|---|
| `https://www.kuaishou.com/short-video/3xhjgcmir24m4nm` | kuaishou | | `www.kuaishou.com/short-video/3xhjgcmir24m4nm` |
| `https://v.kuaishou.com/2yAnzeZ` | v_kuaishou | | `v.kuaishou.com/2yAnzeZ` |
| `https://v.kuaishou.com/nmcrgMMR` | v_kuaishou | | |
| `https://v.m.chenzhongtech.com/fw/photo/3xburnkmj3auazc` | chenzhongtech | | `v.m.chenzhongtech.com/fw/photo/3xburnkmj3auazc` |

## Hongguo

| input | pattern | captures | match |
|---|---|---|---|
| `https://hongguoduanju.com/player/7546798920437877785/7546798920437877785` | hongguo_player | `7546798920437877785` `7546798920437877785` | |
| `https://hongguoduanju.com/player/7546798920437877785/1234567890` | hongguo_player | `7546798920437877785` `1234567890` | |
| `https://hongguoduanju.com/detail?series_id=7546798920437877785` | hongguo_detail | `7546798920437877785` | |
| `https://hongguoduanju.com/comic/7111943588446997534` | hongguo_comic | `7111943588446997534` | |

## Bilibili

`bm` (audio only) must win over the BV patterns, and the paged forms over the
plain ones. Short links win over a BV id in their path.

| input | pattern | captures | match |
|---|---|---|---|
| `https://www.bilibili.com/opus/998440765151510535` | opus | `998440765151510535` | |
| `https://www.bilibili.com/opus/1040093151889457152` | opus | `1040093151889457152` | |
| `https://www.bilibili.com/opus/1122430505331982343` | opus | `1122430505331982343` | |
| `https://www.bilibili.com/read/cv523868` | article | `523868` | |
| `https://bilibili.com/read/cv999` | article | `999` | |
| `https://t.bilibili.com/1120105154190770281` | dynamic | `1120105154190770281` | |
| `https://t.bilibili.com/1121388026537181241` | dynamic | `1121388026537181241` | |
| `https://www.bilibili.com/dynamic/123456789` | dynamic_page | `123456789` | |
| `https://bilibili.com/dynamic/998440765151510535` | dynamic_page | `998440765151510535` | |
| `https://space.bilibili.com/12345/dynamic/123456789` | space_dynamic | `123456789` | |
| `https://space.bilibili.com/396886341/favlist?fid=311147541&ftype=create` | favorites | `311147541` | |
| `favlist?fid=12345678` | favorites | `12345678` | |
| `https://live.bilibili.com/23585383` | live | `23585383` | |
| `https://live.bilibili.com/1` | live | `1` | |
| `https://b23.tv/YTg9oSw` | b23 | `YTg9oSw` | |
| `https://b23.tv/GwiHK6N` | b23 | `GwiHK6N` | |
| `b23.tv/abc123` | b23 | `abc123` | |
| `https://b23.tv/BV1xx411c7mD` | b23 | `BV1xx411c7mD` | |
| `https://bili2233.cn/rnrwIyU` | bili2233 | `rnrwIyU` | |
| `bili2233.cn/xyz789` | bili2233 | `xyz789` | |
| `https://www.bilibili.com/video/BV1VLk9YDEzB` | bv | `BV1VLk9YDEzB` | |
| `https://www.bilibili.com/video/BV1Qb411W76D` | bv | `BV1Qb411W76D` | |
| `https://bilibili.com/BV1uCzoYEEir` | bv | `BV1uCzoYEEir` | |
| `BV1uCzoYEEir` | bv | `BV1uCzoYEEir` | |
| `Check out BV1xx411c7mD video` | bv | `BV1xx411c7mD` | |
| `https://www.bilibili.com/video/BV1584y167sD?p=40` | bv_page | `BV1584y167sD` `40` | |
| `https://www.bilibili.com/video/BV1Qb411W76D?p=1` | bv_page | `BV1Qb411W76D` `1` | |
| `https://www.bilibili.com/video/BV1Qb411W76D/?p=1` | bv_page | `BV1Qb411W76D` `1` | |
| `https://www.bilibili.com/video/BV1pSouYsrRi/?buvid=XU7938D792855D2619DF1156C583C95ACD4A8&from_spmid=search.search-result.0.0&is_story_h5=false&mid=v6x8VuMawAnqsFPDnfgaIw%3D%3D&p=2` | bv_page | `BV1pSouYsrRi` `2` | |
| `BV1xx411c7mD?p=2` | bv_page | `BV1xx411c7mD` `2` | |
| `BV1584y167sD 40` | bv_space | `BV1584y167sD` `40` | |
| `BV1xx411c7mD 2` | bv_space | `BV1xx411c7mD` `2` | |
| `bm BV1584y167sD 40` | bm | `BV1584y167sD` `40` | |
| `bm BV1xx411c7mD` | bm | `BV1xx411c7mD` | |
| `https://bilibili.com/av1234567` | av | `1234567` | |
| `https://www.bilibili.com/video/av170001` | av | `170001` | |
| `av113706574811958` | av | `113706574811958` | |
| `看av170001` | av | `170001` | |
| `https://www.bilibili.com/video/av123456?p=1` | av_page | `123456` `1` | |
| `https://www.bilibili.com/video/av123456/?p=1` | av_page | `123456` `1` | |
| `https://www.bilibili.com/video/av123456/?a=1&b=2&p=3` | av_page | `123456` `3` | |
| `av170001?p=2` | av_page | `170001` `2` | |
| `av605821754 40` | av_space | `605821754` `40` | |
| `av170001 3` | av_space | `170001` `3` | |
| `https://www.bilibili.com/bangumi/play/ep199612` | bangumi | `199612` | |
| `https://bilibili.com/bangumi/play/ep12345` | bangumi | `12345` | |

## TikTok

| input | pattern | captures | match |
|---|---|---|---|
| `https://www.tiktok.com/@maligoshik/video/7472584144510373125` | tiktok | `7472584144510373125` | |
| `https://www.tiktok.com/@arentyoubeth/video/7562414808973315383` | tiktok | `7562414808973315383` | |
| `https://www.tiktok.com/@fdznews/video/7575810064078884116?is_from_webapp=1&sender_device=pc` | tiktok | `7575810064078884116` | |
| `https://vm.tiktok.com/abc123` | tiktok_short | | `vm.tiktok.com/abc123` |
| `https://vt.tiktok.com/xyz789` | tiktok_short | | `vt.tiktok.com/xyz789` |

## AcFun

Multi-part URLs (`ac{id}_{part}`) capture only the acid.

| input | pattern | captures | match |
|---|---|---|---|
| `https://www.acfun.cn/v/ac46593564` | acfun | `46593564` | |
| `https://www.acfun.cn/v/ac44260463` | acfun | `44260463` | |
| `https://www.acfun.cn/v/ac11348130` | acfun | `11348130` | |
| `https://www.acfun.cn/v/ac12345?quickViewId=videoInfo_new&ajaxpipe=1` | acfun | `12345` | |
| `https://www.acfun.cn/v/ac4626695_2` | acfun | `4626695` | |
| `https://m.acfun.cn/v/?ac=46593564` | acfun | `46593564` | |
| `https://m.acfun.cn/v/?ac=46593564&shareType=pic` | acfun | `46593564` | |
| `https://m.acfun.cn/v/?ac=46593564_2&shareType=pic` | acfun | `46593564` | |
| `看看这个 https://www.acfun.cn/v/ac87654321 不错` | acfun | `87654321` | |

## X / Twitter

| input | pattern | captures | match |
|---|---|---|---|
| `https://x.com/fortnitegame/status/1870484479980052921?s=46` | twitter | `1870484479980052921` | |
| `https://x.com/fortnitegame/status/1864640869900668940?s=46` | twitter | `1864640869900668940` | |
| `https://x.com/Fortnite/status/1904171341735178552` | twitter | `1904171341735178552` | |
| `https://x.com/matcha__ore_p/status/2025067664830497203?s=46` | twitter | `2025067664830497203` | |
| `https://r.com/fortnitegame/status/1870484479980052921?s=46` | twitter | `1870484479980052921` | |
| `https://twitter.com/user/status/9876543210` | twitter | `9876543210` | |

## Douyin

URL patterns win over the share-text patterns. Article share tokens are
wrapped in doubled decorations (`douyin_share_token`); other decorations fall
back to the share wording (`douyin_share_phrase`). The `^^7wS3qugWQrcf99※※`
token has expired upstream; the row only covers the pattern.

| input | pattern | captures | match |
|---|---|---|---|
| `https://v.douyin.com/_2ljF4AmKL8/` | v.douyin | | `v.douyin.com/_2ljF4AmKL8` |
| `https://v.douyin.com/iP6Uu1Kh` | v.douyin | | |
| `https://v.douyin.com/GeIocABVe5U/` | v.douyin | | |
| `https://v.douyin.com/iUrHrruH` | v.douyin | | |
| `https://v.douyin.com/CeiJfqyWs` | v.douyin | | |
| `8.23 复制打开抖音，看看【特特羊的作品】  https://v.douyin.com/CxPdeP4iqG4/ FHv:/ :9pm q@E.hB 12/22.` | v.douyin | | `v.douyin.com/CxPdeP4iqG4` |
| `5.67 复制打开抖音，看看【某人的作品】 https://v.douyin.com/iABCDEFG/` | v.douyin | | `v.douyin.com/iABCDEFG` |
| `https://jx.douyin.com/abc123` | jx.douyin | | `jx.douyin.com/abc123` |
| `https://webcast.amemv.com/douyin/webcast/reflow/12345678` | webcast_amemv | | `webcast.amemv.com/douyin/webcast/reflow/12345678` |
| `https://live.douyin.com/23585383` | douyin_live | `23585383` | |
| `https://www.douyin.com/live/23585383` | douyin_live_path | `23585383` | |
| `https://www.douyin.com/video/7521023890996514083` | douyin | `7521023890996514083` | |
| `https://www.douyin.com/note/7469411074119322899` | douyin | `7469411074119322899` | |
| `https://www.douyin.com/note/7450744229229235491` | douyin | `7450744229229235491` | |
| `https://www.douyin.com/share/slides/123456789` | douyin_slides | `123456789` | |
| `https://www.douyin.com/jingxuan?modal_id=123456789` | douyin_modal | `123456789` | |
| `https://www.douyin.com/discover?modal_id=123456789` | douyin_modal | `123456789` | |
| `https://www.iesdouyin.com/share/video/123456789` | iesdouyin | `123456789` | |
| `https://www.iesdouyin.com/share/slides/123456789` | iesdouyin | `123456789` | |
| `https://m.douyin.com/share/video/123456789` | m.douyin | `123456789` | |
| `https://m.douyin.com/share/slides/123456789` | m.douyin | `123456789` | |
| `https://jingxuan.douyin.com/m/video/7574300896016862490` | jingxuan.douyin | `7574300896016862490` | |
| `https://jingxuan.douyin.com/m/slides/7574300896016862490` | jingxuan.douyin | `7574300896016862490` | |
| `长按复制打开抖音，看看Ta的日常 dailyToken67890` | douyin_share_phrase | `dailyToken67890` | |
| `复制打开抖音，看看我的日常 dailyToken67890` | douyin_share_phrase | `dailyToken67890` | |
| `打开抖音，查看日常 dailyToken67890` | douyin_share_phrase | `dailyToken67890` | |
| `打开抖音，看看我的日常 ˇˇdailyToken67890➝➝` | douyin_share_token | `dailyToken67890` | |
| `4-M:/. 09/19 :0pm e@B.TL 【当了三年朋友，她突然不干了】长按复制打开抖音，即可阅读文章 ˇˇ5w0ncBiCzhF99➝➝` | douyin_share_token | `5w0ncBiCzhF99` | |
| `4:/ 10/25 r@E.Hi :5pm 【#媒体原创 杭州电梯猥亵案反转：别让造谣透支了共情】长按复制打开抖音，即可阅读文章 ŠŠw82al1lN3Akd99︽︽` | douyin_share_token | `w82al1lN3Akd99` | |
| `【#新华社 聚焦深海探测】长按复制打开抖音 ǚǚabc123XYZ456ǚǚ` | douyin_share_token | `abc123XYZ456` | |
| `3O 10/12 :0pm j@C.Ul 【关于云顶之弈s18赛季（测试）自然之力的:/ 更新说明】长按复制打开抖音，即可阅读文章 ^^7wS3qugWQrcf99※※` | douyin_share_token | `7wS3qugWQrcf99` | |
| `ˇˇ5w0ncBiCzhF99➝➝` | douyin_share_token | `5w0ncBiCzhF99` | |
| `^^7wS3qugWQrcf99※※` | douyin_share_token | `7wS3qugWQrcf99` | |
| `【标题】长按复制打开抖音，即可阅读文章 ══eqToken123456══` | douyin_share_phrase | `eqToken123456` | |
| `复制打开抖音，即可阅读文章 %%%abcdefghij123%%%` | douyin_share_phrase | `abcdefghij123` | |
| `长按复制打开抖音，即可阅读文章 ˇ5w0ncBiCzhF99➝` | douyin_share_phrase | `5w0ncBiCzhF99` | |

## Xiaohongshu

The share query stops at the first character a URL query cannot contain, so
chat text glued to a link is not sent upstream.

| input | pattern | captures | match |
|---|---|---|---|
| `http://xhslink.com/o/49ufwc0ipbn` | xhslink | | `xhslink.com/o/49ufwc0ipbn` |
| `http://xhslink.cn/o/8bNxn8Tbi0r` | xhslink | | `xhslink.cn/o/8bNxn8Tbi0r` |
| `https://xhslink.com/abc123` | xhslink | | `xhslink.com/abc123` |
| `https://www.xiaohongshu.com/discovery/item/68e8e3fa00000000030342ec` | discovery | `68e8e3fa00000000030342ec` | |
| `https://www.xiaohongshu.com/explore/68e8e3fa00000000030342ec` | explore | `68e8e3fa00000000030342ec` | |
| `https://www.xiaohongshu.com/explore/68f963260000000004005843?xsec_token=ABn8T0lrJdPfRlWpNvLoYWTd0x54P44tsJtYgqhujHTjo=&xsec_source=pc_feed` | explore | `68f963260000000004005843` `xsec_token=ABn8T0lrJdPfRlWpNvLoYWTd0x54P44tsJtYgqhujHTjo=&xsec_source=pc_feed` | |
| `https://www.xiaohongshu.com/discovery/item/691e68a8000000001e02bcda?source=webshare&xhsshare=pc_web&xsec_token=CBunzr4Cq8N7jbcXqpWDxGn11k7XwVIJ59KOvkRS_Qabw=&xsec_source=pc_share` | discovery | `691e68a8000000001e02bcda` `source=webshare&xhsshare=pc_web&xsec_token=CBunzr4Cq8N7jbcXqpWDxGn11k7XwVIJ59KOvkRS_Qabw=&xsec_source=pc_share` | |
| `https://www.xiaohongshu.com/explore/6789abcdef012345?xsec_token=AB-c_1%3D&xsec_source=pc_share，好看` | explore | `6789abcdef012345` `xsec_token=AB-c_1%3D&xsec_source=pc_share` | |
| `https://www.xiaohongshu.com/discovery/item/6789abcdef012345?xsec_token=XYZ=好看吗` | discovery | `6789abcdef012345` `xsec_token=XYZ=` | |
| `https://www.xiaohongshu.com/explore/6789abcdef012345?xsec_token=XYZ 快看` | explore | `6789abcdef012345` `xsec_token=XYZ` | |

## Weibo

| input | pattern | captures | match |
|---|---|---|---|
| `https://video.weibo.com/show?fid=1034:5145615399845897` | video_weibo | `1034:5145615399845897` | |
| `https://weibo.com/tv/show/1034:5197094538117264?mid=5197095191711778` | weibo_tv | `5197095191711778` | |
| `https://weibo.com/7207262816/O70aCbjnd` | weibo_status | `O70aCbjnd` | |
| `https://weibo.com/7207262816/P5kWdcfDe` | weibo_status | `P5kWdcfDe` | |
| `https://weibo.com/7207262816/P2AFBk387` | weibo_status | `P2AFBk387` | |
| `https://weibo.com/7207262816/Q6YCbtAn8` | weibo_status | `Q6YCbtAn8` | |
| `https://weibo.com/7207262816/Q617WgOm4` | weibo_status | `Q617WgOm4` | |
| `https://weibo.com/7207262816/PsFzpzUX2` | weibo_status | `PsFzpzUX2` | |
| `https://weibo.com/7207262816/P2rJE157H` | weibo_status | `P2rJE157H` | |
| `https://weibo.com/1694917363/Q0KtXh6z2` | weibo_status | `Q0KtXh6z2` | |
| `https://weibo.com/1854467892/QsPbt51HH` | weibo_status | `QsPbt51HH` | |
| `https://m.weibo.cn/status/Q0KtXh6z2` | m_weibo | `Q0KtXh6z2` | |
| `http://m.weibo.cn/status/5112672433738061` | m_weibo | `5112672433738061` | |
| `https://m.weibo.cn/detail/4976424138313924` | m_weibo | `4976424138313924` | |
| `https://mapp.api.weibo.cn/fx/8102df2b26100b2e608e6498a0d3cfe2.html` | mapp_weibo | | `mapp.api.weibo.cn/fx/8102df2b26100b2e608e6498a0d3cfe2.html` |
| `https://mapp.api.weibo.cn/fx/77eaa5c2f741894631a87fc4806a1f05.html` | mapp_weibo | | |
| `https://card.weibo.com/article/m/show/id/2309404962180771742222` | article_card | `2309404962180771742222` | |
| `https://weibo.com/ttarticle/p/show?id=2309405232896864026685` | article_ttarticle | `2309405232896864026685` | |
| `https://weibo.com/ttarticle/x/m/show/id/2309404976424138313924` | article_id | `2309404976424138313924` | |
| `https://weibo.com/ttarticle/x/m/show#/id=2309404962180771742222` | article_id | `2309404962180771742222` | |

## NGA

`tid` does not have to be the first query parameter.

| input | pattern | captures | match |
|---|---|---|---|
| `https://nga.178.com/read.php?tid=45263995` | nga | `45263995` | |
| `https://ngabbs.com/read.php?tid=45263995` | nga | `45263995` | |
| `https://bbs.nga.cn/read.php?tid=45263995` | nga | `45263995` | |
| `https://bbs.nga.cn/read.php?tid=11111111&page=2` | nga | `11111111` | |
| `https://bbs.nga.cn/read.php?fid=7&tid=99999999` | nga | `99999999` | |
| `nga.178.com/read.php?tid=45263995` | nga | `45263995` | |

## V2EX

| input | pattern | captures | match |
|---|---|---|---|
| `https://www.v2ex.com/t/1` | v2ex_topic | `1` | |
| `https://v2ex.com/t/12345#reply1` | v2ex_topic | `12345` | |
| `www.v2ex.com/t/98765?p=2` | v2ex_topic | `98765` | |

## Instagram

Stories need a cookie and are not registered with the default config.

| input | pattern | captures | match |
|---|---|---|---|
| `https://www.instagram.com/p/C0xFHGOrBN7/` | instagram_post | `C0xFHGOrBN7` | |
| `https://instagram.com/p/ABC123_-x/` | instagram_post | `ABC123_-x` | |
| `https://www.instagram.com/reel/DEF456/` | instagram_post | `DEF456` | |
| `https://www.instagram.com/reels/GHI789/` | instagram_post | `GHI789` | |

## Xiaoheihe

| input | pattern | captures | match |
|---|---|---|---|
| `https://xiaoheihe.cn/bbs/link/abc123` | bbs_link | `abc123` | |
| `https://xiaoheihe.cn/bbs/app/api/web/share?link_id=def456` | bbs_share | `def456` | |
| `https://api.xiaoheihe.cn/v3/bbs/app/api/web/share?h_camp=link&h_src=YXBwX3NoYXJl&link_id=5c0971876027` | bbs_share | `5c0971876027` | |
| `https://xiaoheihe.cn/game/pc/730` | game_pc | `730` | |
| `https://xiaoheihe.cn/game/console/12345` | game_console | `12345` | |
| `https://xiaoheihe.cn/game/mobile/67890` | game_mobile | `67890` | |
| `https://xiaoheihe.cn/game/share_game_detail?appid=730&game_type=pc` | game_share_pc | `730` | |
| `https://xiaoheihe.cn/game/share_game_detail?appid=100&game_type=console` | game_share_console | `100` | |
| `https://xiaoheihe.cn/game/share_game_detail?appid=200&game_type=mobile` | game_share_mobile | `200` | |

## WeChat Article

The query form is fetched verbatim, so its match must stop before CJK
punctuation. Parameter order varies between share sources.

| input | pattern | captures | match |
|---|---|---|---|
| `https://mp.weixin.qq.com/s/Y7dyRC7CJ09miHWU6LBzBA` | mp_weixin_short | `Y7dyRC7CJ09miHWU6LBzBA` | |
| `https://mp.weixin.qq.com/s/dQnHCXfeqJ0WHkeriL4kOw` | mp_weixin_short | `dQnHCXfeqJ0WHkeriL4kOw` | |
| `mp.weixin.qq.com/s/abc_DEF-123` | mp_weixin_short | `abc_DEF-123` | |
| `看看这篇 https://mp.weixin.qq.com/s/QF3x5S8QIfr1x1O9UB3mFw ，很有意思` | mp_weixin_short | `QF3x5S8QIfr1x1O9UB3mFw` | |
| `https://mp.weixin.qq.com/s?__biz=MzA0MDAwMTIzNA==&mid=2650000001&idx=1&sn=0123456789abcdef&chksm=abc#rd` | mp_weixin_query | | `mp.weixin.qq.com/s?__biz=MzA0MDAwMTIzNA==&mid=2650000001&idx=1&sn=0123456789abcdef&chksm=abc#rd` |
| `http://mp.weixin.qq.com/s?sn=0123456789abcdef&__biz=MzA0MDAwMTIzNA==&idx=1&mid=2650000001&scene=21#wechat_redirect` | mp_weixin_query | | `mp.weixin.qq.com/s?sn=0123456789abcdef&__biz=MzA0MDAwMTIzNA==&idx=1&mid=2650000001&scene=21#wechat_redirect` |
| `mp.weixin.qq.com/s?__biz=MzI2NDU4OTExOQ==&mid=2247678901&idx=2&sn=fedcba98` | mp_weixin_query | | `mp.weixin.qq.com/s?__biz=MzI2NDU4OTExOQ==&mid=2247678901&idx=2&sn=fedcba98` |
| `见 mp.weixin.qq.com/s?__biz=MzA=&mid=1&idx=1&sn=ab，很棒` | mp_weixin_query | | `mp.weixin.qq.com/s?__biz=MzA=&mid=1&idx=1&sn=ab` |

## WeChat Channel

The match is the parser's input: it drops the scheme and keeps the share id's
casing. Host and path match case-insensitively, as upstream does.

| input | pattern | captures | match |
|---|---|---|---|
| `https://weixin.qq.com/sph/AbCd123` | weixin_sph | | `weixin.qq.com/sph/AbCd123` |
| `看看这个 https://weixin.qq.com/sph/AbCd123 视频` | weixin_sph | | `weixin.qq.com/sph/AbCd123` |
| `http://weixin.qq.com/sph/x1Y2z3` | weixin_sph | | `weixin.qq.com/sph/x1Y2z3` |
| `weixin.qq.com/sph/0aB9` | weixin_sph | | `weixin.qq.com/sph/0aB9` |
| `https://WEIXIN.QQ.COM/SPH/AbCd123` | weixin_sph | | `WEIXIN.QQ.COM/SPH/AbCd123` |

## No match

| input |
|---|
| `Hello, this is a normal message` |
| `Check out this cool website` |
| `The quick brown fox jumps over the lazy dog` |
| `random text without any bilibili links` |
| `hello` |
| `12345678` |
| `https://example.com` |
| `https://www.google.com/search?q=rust` |
| `https://www.google.com/search?q=instagram` |
| `https://github.com/anthropics/claude-code` |
| `BV1xx411c` |
| `bm` |
| `nav123` |
| `trav42 is my username` |
| `nav123?p=2` |
| `trav42 7` |
| `https://youtube.com/watch?v=abc123` |
| `看看我的日常 dailyToken67890` |
| `查看日常 dailyToken67890` |
| `dailyToken67890` |
| `会议密码：abcdefghij12` |
| `══════ HEADER1234 ══════` |
| `══════ABCDEFGH══════` |
| `nǚshēng 123456` |
| `Šarlota123★ 上线了` |
| `ˇ5w0ncBiCzhF99➝` |
| `^7wS3qugWQrcf99※` |
| `2^^10 = 1024` |
| `^^abcdef^^` |
| `x^^y※※` |
| `长按复制打开抖音，即可阅读文章` |
| `打开抖音 https://www.douyin.com/` |
| `打开抖音 搜索 abcdefghij12` |
| `打开抖音看看 ABCDEFGHIJKL 这个号` |
| `https://x.com/user` |
| `https://example.com/@user/video/123` |
| `https://www.tiktok.com/@user` |
| `hello tiktok` |
| `https://kuaishou.com/` |
| `https://weibo.com/` |
| `https://www.xiaohongshu.com/user/profile/123` |
| `https://steamcommunity.com/sharedfiles/filedetails/?tid=12345678` |
| `https://example.com/forum/thread?utm_source=x&tid=999` |
| `https://example.com/read.php?tid=12345678` |
| `tid=123` |
| `看这个帖子 tid=456789` |
| `Mac=5` |
| `ac=99999` |
| `/ac12345` |
| `https://example.com/nav/ac123` |
| `https://example.com/ac123` |
| `https://example.com/page?ac=5` |
| `https://www.acfun.cn/v/ab123` |
| `https://www.instagram.com/someuser/` |
| `https://www.instagram.com/explore/` |
| `https://www.instagram.com/stories/someuser/1234567890/` |
| `https://weixin.qq.com/` |
| `https://mp.weixin.qq.com/mp/homepage?__biz=MzA0MDAwMTIzNA==` |
| `https://channels.weixin.qq.com/web/pages/feed` |
| `https://weixin.qq.com/other/AbCd123` |
| `https://weixin.qq.com/sph/` |
| `https://channels.weixin.qq.com/finder-preview/pages/feed` |
| `no links here` |
