use std::time::Duration;

use karta_core::error::{ParserError, Result};
use tokio::time::{Instant, timeout_at};

use super::DouyinParser;
use super::api::CarrierParseResponse;

/// Share-carrier resolution: maps an in-app share token (口令) to the
/// target item's app schema. Served identically by every app API host;
/// the first is the one the current client talks to.
const API_CARRIER_PARSE_HOSTS: [&str; 2] = [
    "https://api5-normal-c-hl.amemv.com",
    "https://aweme.snssdk.com",
];
const API_CARRIER_PARSE_PATH: &str = "/aweme/share/carrier/parse/";

/// Douyin app id; carrier resolution answers "safety verification not
/// pass" without it.
const APP_AID: &str = "1128";

const CARRIER_TYPES: [&str; 4] = ["3", "4", "5", "6"];
const RESOLVE_SECONDS: u64 = 18;

pub(super) enum ShareTarget {
    Item {
        id: String,
        article: bool,
        note: bool,
    },
    Live {
        room_id: Option<String>,
        web_rid: Option<String>,
    },
}

impl DouyinParser {
    pub(super) async fn resolve_share_target(
        &self,
        token: &str,
        matched_text: &str,
    ) -> Result<ShareTarget> {
        let deadline = Instant::now() + Duration::from_secs(RESOLVE_SECONDS);
        let decorated = decorated_candidate(token, matched_text);
        let candidates = std::iter::once(token).chain(decorated);
        let mut answer_error = None;
        let mut transport_error = None;

        'candidates: for candidate in candidates {
            for carrier_type in CARRIER_TYPES {
                let query = url::form_urlencoded::Serializer::new(String::new())
                    .append_pair("carrier_type", carrier_type)
                    .append_pair("code_info", candidate)
                    .append_pair("aid", APP_AID)
                    .finish();

                for host in API_CARRIER_PARSE_HOSTS {
                    let request = async {
                        let response = self
                            .downloader
                            .client()
                            .get(format!("{host}{API_CARRIER_PARSE_PATH}?{query}"))
                            .headers(Self::redirect_headers())
                            .timeout(Duration::from_millis(4500))
                            .send()
                            .await?;
                        karta_core::http::check_status(&response)?;
                        response
                            .json::<CarrierParseResponse>()
                            .await
                            .map_err(ParserError::from)
                    };
                    match timeout_at(deadline, request).await {
                        Ok(Ok(data)) => {
                            match target_from_carrier(token, &data) {
                                Ok(target) => return Ok(target),
                                Err(error) => answer_error = Some(error),
                            }
                            // Alternate hosts only recover transport failures;
                            // another carrier type can answer a rejected code.
                            break;
                        }
                        Ok(Err(error)) => {
                            tracing::debug!(host, carrier_type, %error, "Douyin carrier request failed");
                            transport_error = Some(error);
                        }
                        Err(_) => break 'candidates,
                    }
                }
            }
        }

        Err(answer_error
            .or(transport_error)
            .unwrap_or(ParserError::Timeout {
                seconds: RESOLVE_SECONDS,
            }))
    }
}

pub(super) fn target_from_carrier(token: &str, data: &CarrierParseResponse) -> Result<ShareTarget> {
    let payload = data.data.as_ref();
    let schema = payload
        .and_then(|payload| payload.schema.as_deref())
        .unwrap_or_default();
    let message = payload
        .and_then(|payload| payload.message.as_deref())
        .unwrap_or_default();
    let code = data.status_code.unwrap_or_default();

    if code == 0
        && let Ok(url) = url::Url::parse(schema)
        && let Some(target) = schema_target(
            &url,
            payload.and_then(|payload| payload.share_scene.as_deref()),
        )
    {
        return Ok(target);
    }

    match code {
        100001 if message == "check_no_pass" => Err(ParserError::ContentNotFound(format!(
            "抖音口令 {token} 无效或已过期"
        ))),
        0 if !schema.is_empty() => Err(ParserError::ContentNotFound(format!(
            "抖音口令指向不支持的内容: {schema}"
        ))),
        code => Err(ParserError::Api {
            code,
            message: if message.is_empty() {
                "share carrier returned no schema".to_owned()
            } else {
                message.to_owned()
            },
        }),
    }
}

fn schema_target(url: &url::Url, scene: Option<&str>) -> Option<ShareTarget> {
    let host = url.host_str().unwrap_or_default();
    let path: Vec<_> = url
        .path_segments()?
        .filter(|part| !part.is_empty())
        .collect();
    if matches!(host, "user" | "profile")
        || path.iter().any(|part| matches!(*part, "user" | "profile"))
    {
        return None;
    }

    let query_id = |key: &str| {
        url.query_pairs().find_map(|(name, value)| {
            (name == key && positive_id(&value)).then(|| value.into_owned())
        })
    };
    let room_id = query_id("room_id");
    let web_rid = query_id("web_rid");
    if room_id.is_some() || web_rid.is_some() {
        return Some(ShareTarget::Live { room_id, web_rid });
    }
    if matches!(host, "live" | "webcast")
        || path.iter().any(|part| matches!(*part, "live" | "webcast"))
    {
        return None;
    }

    let item_path = |part: &str| matches!(part, "detail" | "note" | "slides" | "video" | "article");
    let id = query_id("item_id")
        .or_else(|| query_id("aweme_id"))
        .or_else(|| {
            (item_path(host) || path.iter().any(|part| item_path(part)))
                .then(|| query_id("id"))
                .flatten()
        })
        .or_else(|| {
            if item_path(host)
                && let Some(id) = path.first().filter(|id| positive_id(id))
            {
                Some((*id).to_owned())
            } else {
                None
            }
        })
        .or_else(|| {
            path.windows(2).find_map(|parts| {
                (item_path(parts[0]) && positive_id(parts[1])).then(|| parts[1].to_owned())
            })
        })?;
    if id.len() < 5 {
        return None;
    }

    let article = scene == Some("long_article")
        || host == "article"
        || path.contains(&"article")
        || url
            .query_pairs()
            .any(|(key, value)| key == "share_scene" && value == "long_article");
    let note_scene = |scene: &str| scene.contains("note") || scene.contains("image");
    let note = !article
        && (matches!(host, "note" | "slides")
            || path.iter().any(|part| matches!(*part, "note" | "slides"))
            || scene.is_some_and(note_scene)
            || url
                .query_pairs()
                .any(|(key, value)| key == "share_scene" && note_scene(&value)));
    Some(ShareTarget::Item { id, article, note })
}

fn positive_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 32
        && value.as_bytes()[0] != b'0'
        && value.bytes().all(|byte| byte.is_ascii_digit())
}

fn decorated_candidate<'a>(token: &str, text: &'a str) -> Option<&'a str> {
    for (index, _) in text.match_indices(token) {
        let before = &text[..index];
        let after = &text[index + token.len()..];
        for (open, close) in [
            ("##", "##"),
            ("%%", "%%"),
            ("$$", "$$"),
            ("￥", "￥"),
            ("￥", "¥"),
            ("¥", "￥"),
            ("¥", "¥"),
            ("ωω", "ωω"),
            ("ΔΔ", "ΔΔ"),
            ("€", "€"),
            ("══", "══"),
        ] {
            if before.ends_with(open) && after.starts_with(close) {
                return Some(&text[index - open.len()..index + token.len() + close.len()]);
            }
        }

        let open: Vec<_> = before.char_indices().rev().take(2).collect();
        let close: Vec<_> = after.char_indices().take(2).collect();
        if open.len() == 2
            && close.len() == 2
            && open.iter().all(|(_, ch)| {
                matches!(*ch, '\u{02c6}'..='\u{02cf}' | 'Š' | 'š' | 'Ǚ' | 'ǚ' | '^')
            })
            && close.iter().all(|(_, ch)| {
                matches!(*ch, '\u{2190}'..='\u{27ff}' | '\u{fe1d}'..='\u{fe1f}' | '\u{fe3d}'..='\u{fe44}' | 'Ǚ' | 'ǚ' | '※')
            })
        {
            return Some(&text[open[1].0..index + token.len() + close[1].0 + close[1].1.len_utf8()]);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pull the item id out of a share-carrier response, turning the endpoint's
    /// rejection shapes into user-facing errors.
    fn item_id_from_carrier(token: &str, data: &CarrierParseResponse) -> Result<String> {
        match target_from_carrier(token, data)? {
            ShareTarget::Item { id, .. } => Ok(id),
            ShareTarget::Live { .. } => Err(ParserError::ContentNotFound(
                "Douyin share points to a live room".into(),
            )),
        }
    }

    #[test]
    fn test_item_id_from_carrier() {
        let rejected: CarrierParseResponse =
            serde_json::from_str(r#"{"data":{"message":"check_no_pass"},"status_code":100001}"#)
                .unwrap();
        match item_id_from_carrier("bogus", &rejected).unwrap_err() {
            ParserError::ContentNotFound(message) => {
                assert!(message.contains("bogus"), "{message}");
            }
            other => panic!("expected ContentNotFound, got {other:?}"),
        }

        // Success status with a schema that carries no item id (e.g. a
        // profile deep link) is reported as unsupported content.
        let profile: CarrierParseResponse = serde_json::from_str(
            r#"{"data":{"schema":"aweme://user/profile?sec_uid=MS4wLjABAAAA"},"status_code":0}"#,
        )
        .unwrap();
        assert!(matches!(
            item_id_from_carrier("tok", &profile).unwrap_err(),
            ParserError::ContentNotFound(_)
        ));

        // Other rejections surface as API errors with the server message.
        let verification: CarrierParseResponse = serde_json::from_str(
            r#"{"data":{"message":"safety verification not pass","prompts":"golink解析失败"},"status_code":100001}"#,
        )
        .unwrap();
        match item_id_from_carrier("tok", &verification).unwrap_err() {
            ParserError::Api { code, message } => {
                assert_eq!(code, 100001);
                assert_eq!(message, "safety verification not pass");
            }
            other => panic!("expected Api error, got {other:?}"),
        }

        // Other schema shapes carrying an item id.
        for schema in [
            "aweme://aweme/detail?id=7521023890996514083",
            "snssdk1128://aweme/detail/7521023890996514083?refer=web",
        ] {
            let data: CarrierParseResponse = serde_json::from_value(serde_json::json!({
                "status_code": 0,
                "data": { "schema": schema }
            }))
            .unwrap();
            assert_eq!(
                item_id_from_carrier("tok", &data).unwrap(),
                "7521023890996514083",
                "{schema}"
            );
        }
    }
}
