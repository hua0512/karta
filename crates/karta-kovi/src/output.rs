use kovi::bot::runtimebot::CanSendApi;
use kovi::{Message, Segment};
use kovi_onebot::MsgEvent;
use serde_json::json;

use karta_core::message::{MessageOutput, SegmentType};

/// Convert a MessageOutput to a Kovi Message.
pub fn to_kovi_message(output: &MessageOutput) -> Message {
    let mut message = Message::new();

    for segment in &output.segments {
        match &segment.segment_type {
            SegmentType::Text => {
                if let Some(text) = &segment.text {
                    message.push(Segment::new("text", json!({ "text": text })));
                }
            }
            SegmentType::Image => {
                if let Some(file_ref) = segment.file_ref() {
                    message.push(Segment::new("image", json!({ "file": file_ref })));
                }
            }
            SegmentType::Video => {
                if let Some(file_ref) = segment.file_ref() {
                    message.push(Segment::new("video", json!({ "file": file_ref })));
                }
            }
            SegmentType::Audio => {
                if let Some(file_ref) = segment.file_ref() {
                    message.push(Segment::new("record", json!({ "file": file_ref })));
                }
            }
            SegmentType::At => {
                if let Some(target) = &segment.target_id {
                    message.push(Segment::new("at", json!({ "qq": target })));
                }
            }
            SegmentType::Reply => {
                if let Some(msg_id) = &segment.target_id {
                    message.push(Segment::new("reply", json!({ "id": msg_id })));
                }
            }
            SegmentType::Face => {
                if let Some(id) = &segment.target_id {
                    message.push(Segment::new("face", json!({ "id": id })));
                }
            }
            SegmentType::Custom(type_name) => {
                let mut data = serde_json::Map::new();
                for (key, value) in &segment.extra {
                    data.insert(key.clone(), json!(value));
                }
                message.push(Segment::new(type_name, serde_json::Value::Object(data)));
            }
        }
    }

    message
}

/// Convert multiple MessageOutputs to Kovi Messages.
pub fn to_kovi_messages(outputs: &[MessageOutput]) -> Vec<Message> {
    outputs.iter().map(to_kovi_message).collect()
}

/// Deliver an output in order, with file delivery for standalone audio/video.
pub(crate) async fn reply_output_awaited(
    event: &MsgEvent,
    message: &MessageOutput,
    need_upload: bool,
) -> bool {
    send_output_awaited(event, event.group_id, event.user_id, message, need_upload).await
}

async fn send_output_awaited(
    api: &impl CanSendApi,
    group_id: Option<i64>,
    user_id: i64,
    message: &MessageOutput,
    need_upload: bool,
) -> bool {
    let media = uploadable_media(message);
    if need_upload && let Some(media) = &media {
        if upload_media(api, group_id, user_id, media).await {
            return true;
        }
    } else {
        match send_msg_result(api, group_id, user_id, to_kovi_message(message)).await {
            Ok(()) => return true,
            // These reject the request before delivery. Runtime errors (1200)
            // and transport failures may arrive after QQ already sent it.
            Err(error) if matches!(error.retcode, 1400 | 1404) => {
                if let Some(media) = &media
                    && upload_media(api, group_id, user_id, media).await
                {
                    return true;
                }
            }
            Err(_) => {}
        }
    }

    if let Some(media) = media {
        let notice = MessageOutput::from_text(format!(
            "{}发送失败或未获确认，请稍后检查聊天记录。",
            media.label
        ));
        // This notice does not turn the failed attachment into a success.
        send_msg_awaited(api, group_id, user_id, to_kovi_message(&notice)).await;
    }
    false
}

struct UploadMedia {
    file: String,
    name: String,
    label: &'static str,
}

fn uploadable_media(message: &MessageOutput) -> Option<UploadMedia> {
    let [segment] = message.segments.as_slice() else {
        return None;
    };
    let (label, default_name) = match segment.segment_type {
        SegmentType::Video => ("视频", "video.mp4"),
        SegmentType::Audio => ("音频", "audio"),
        _ => return None,
    };
    let file = segment.file_ref()?;
    let file = local_file_uri_path(&file).unwrap_or(file);
    let name = if let Some(path) = &segment.file_path {
        path.file_name()
            .map(|name| name.to_string_lossy().into_owned())
    } else {
        None
    };
    Some(UploadMedia {
        file,
        name: name.unwrap_or_else(|| default_name.to_string()),
        label,
    })
}

async fn upload_media(
    api: &impl CanSendApi,
    group_id: Option<i64>,
    user_id: i64,
    media: &UploadMedia,
) -> bool {
    let (action, target, id) = match group_id {
        Some(group_id) => ("upload_group_file", "group_id", group_id),
        None => ("upload_private_file", "user_id", user_id),
    };
    match api
        .send_api_return(
            action,
            json!({
                (target): id, "file": media.file, "name": media.name,
            }),
        )
        .await
    {
        Ok(_) => true,
        Err(error) => {
            tracing::warn!(action, ?error, "Media file upload failed");
            false
        }
    }
}

/// Payload shape mirrors kovi's `reply_builder` (`send_msg` action).
///
/// The message must cross the wire as an [`OneBotMessage`]: `kovi::Message`
/// serializes its segments with a `kind` key (`type` is only a deserialize
/// alias), which OneBot servers reject.
///
/// [`OneBotMessage`]: kovi_onebot::onebot_message::OneBotMessage
pub(crate) async fn send_msg_awaited(
    api: &impl CanSendApi,
    group_id: Option<i64>,
    user_id: i64,
    message: Message,
) -> bool {
    send_msg_result(api, group_id, user_id, message)
        .await
        .is_ok()
}

async fn send_msg_result(
    api: &impl CanSendApi,
    group_id: Option<i64>,
    user_id: i64,
    message: Message,
) -> Result<(), kovi::ApiReturn> {
    let message = kovi_onebot::onebot_message::OneBotMessage::from(message);
    let params = match group_id {
        Some(group_id) => json!({
            "message_type": "group",
            "group_id": group_id,
            "message": message,
            "auto_escape": false,
        }),
        None => json!({
            "message_type": "private",
            "user_id": user_id,
            "message": message,
            "auto_escape": false,
        }),
    };

    match api.send_api_return("send_msg", params).await {
        Ok(_) => Ok(()),
        Err(error) => {
            tracing::warn!(?error, "send_msg failed");
            Err(error)
        }
    }
}

/// Extract the local filesystem path from a `file://` URI.
///
/// `MessageSegment::file_ref()` emits RFC-8089 URIs via
/// `url::Url::from_file_path`, which percent-encodes spaces, CJK, `#` and
/// `%`, so the URI must be parsed and decoded rather than prefix-stripped.
/// The legacy unencoded `file:////path` form (four slashes, from naive
/// `file:///{path}` concatenation) predates that and is still stripped as-is.
fn local_file_uri_path(file: &str) -> Option<String> {
    let rest = file.strip_prefix("file://")?;

    let leading_slashes = rest.bytes().take_while(|byte| *byte == b'/').count();
    if leading_slashes >= 2 {
        return Some(rest[leading_slashes - 1..].to_string());
    }

    let url = url::Url::parse(file).ok()?;
    let path = url.to_file_path().ok()?;
    Some(path.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use karta_core::MessageSegment;

    #[tokio::test]
    async fn media_delivery_uses_files_only_when_requested_or_definitely_rejected() {
        use crate::test_support::mock_api;

        for (group_id, action) in [
            (Some(42), "upload_group_file"),
            (None, "upload_private_file"),
        ] {
            for (need_upload, send_error, delivered, expected_actions) in [
                (false, Some(1400), true, vec!["send_msg", action]),
                (false, Some(1404), true, vec!["send_msg", action]),
                // QQ may deliver a timed-out message later; never upload it again.
                (false, Some(1200), false, vec!["send_msg", "send_msg"]),
                (true, None, true, vec![action]),
            ] {
                let (api, log) = mock_api(move |request| match request.action.as_str() {
                    "send_msg" => send_error.map_or(Ok(()), Err),
                    "upload_group_file" | "upload_private_file" => Ok(()),
                    _ => panic!("unexpected action"),
                });
                let segment = if group_id.is_some() {
                    MessageSegment::video_file("/tmp/kovi 视频 #1.mp4".into())
                } else {
                    MessageSegment::audio_file("/tmp/kovi 语音 #1.m4a".into())
                };
                let original_path = segment.file_path.clone().unwrap();
                let message = MessageOutput::from_segment(segment);
                assert_eq!(
                    send_output_awaited(&api, group_id, 7, &message, need_upload).await,
                    delivered
                );
                let calls = log.lock().unwrap();
                assert_eq!(
                    calls.iter().map(|c| c.action.as_str()).collect::<Vec<_>>(),
                    expected_actions
                );
                for call in calls.iter().filter(|c| c.action == action) {
                    assert!(call.awaited);
                    assert_eq!(call.params["file"], original_path.to_str().unwrap());
                    assert_eq!(
                        call.params["name"],
                        original_path.file_name().unwrap().to_str().unwrap()
                    );
                    match group_id {
                        Some(id) => assert_eq!(call.params["group_id"], id),
                        None => assert_eq!(call.params["user_id"], 7),
                    }
                }
                if !delivered {
                    let notice = calls.last().unwrap();
                    assert_eq!(notice.params["message"][0]["type"], "text");
                    assert!(
                        notice.params["message"][0]["data"]["text"]
                            .as_str()
                            .unwrap()
                            .contains("发送失败或未获确认")
                    );
                }
            }
        }
    }
}
