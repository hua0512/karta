//! OneBot merged-forward presentation of [`RenderOutput::Bundle`]s.
//!
//! karta-core's renderers emit platform-agnostic bundles; this module is the
//! QQ-specific half that turns each bundled message into a forward node and
//! deals with the OneBot API-shape zoo.
//!
//! [`RenderOutput::Bundle`]: karta_core::message::RenderOutput

use kovi::bot::runtimebot::CanSendApi;
use kovi::{Message, Segment};
use kovi_onebot::MsgEvent;
use serde_json::json;
use std::sync::atomic::{AtomicU8, Ordering};

use karta_core::message::MessageOutput;

use crate::output::{send_msg_awaited, to_kovi_message};

pub(crate) const MAX_FORWARD_NODES: usize = 20;

/// Sender name attached to synthesized forward nodes.
const FORWARD_SENDER_NAME: &str = "麦麦";

/// Sender uin attached to synthesized forward nodes.
const FORWARD_SENDER_UIN: &str = "0";

/// (action, payload key) API shapes probed when sending forward messages.
/// OneBot implementations disagree on both the action name and whether the
/// nodes go under `messages` or `message`.
type ForwardVariants = [(&'static str, &'static str); 3];

const PRIVATE_FORWARD_VARIANTS: ForwardVariants = [
    ("send_private_forward_msg", "messages"),
    ("send_private_forward_msg", "message"),
    ("send_forward_msg_to_user", "message"),
];

const GROUP_FORWARD_VARIANTS: ForwardVariants = [
    ("send_group_forward_msg", "messages"),
    ("send_group_forward_msg", "message"),
    ("send_forward_msg_to_group", "message"),
];

/// Sentinel for "no working variant cached yet".
const FORWARD_VARIANT_UNKNOWN: u8 = u8::MAX;

/// Cached index of the first variant that succeeded, probed once per scope
/// (mirrors the support cache in `reaction.rs`). A fully failed probe leaves
/// the cache unknown so a transient outage cannot latch a wrong variant.
static PRIVATE_FORWARD_VARIANT: AtomicU8 = AtomicU8::new(FORWARD_VARIANT_UNKNOWN);
static GROUP_FORWARD_VARIANT: AtomicU8 = AtomicU8::new(FORWARD_VARIANT_UNKNOWN);

#[derive(Clone, Copy)]
enum ForwardTarget {
    Group(i64),
    Private(i64),
}

/// Outcome of sending a bundle chunk by chunk.
pub(crate) struct ForwardSendReport {
    /// Number of chunks that were delivered.
    pub sent_chunks: usize,
    /// Messages from failed chunks, preserved in order for a fallback send.
    pub failed_messages: Vec<MessageOutput>,
}

/// Render one chunk of bundled messages as OneBot forward nodes. Every
/// message becomes a node from the synthesized sender; node content reuses
/// the same segment conversion as regular replies, wrapped as an
/// `OneBotMessage` so segments serialize with `type` (not kovi's `kind`).
pub(crate) fn bundle_chunk_to_onebot_nodes(chunk: &[MessageOutput]) -> serde_json::Value {
    serde_json::Value::Array(
        chunk
            .iter()
            .map(|message| {
                let content =
                    kovi_onebot::onebot_message::OneBotMessage::from(to_kovi_message(message));
                json!({
                    "type": "node",
                    "data": {
                        "uin": FORWARD_SENDER_UIN,
                        "name": FORWARD_SENDER_NAME,
                        "content": content,
                    }
                })
            })
            .collect(),
    )
}

/// Plain-text degradation of a bundle: the concatenated text segments.
pub(crate) fn bundle_plain_text_fallback(messages: &[MessageOutput]) -> Option<String> {
    let texts = messages
        .iter()
        .map(|message| message.plain_text())
        .map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>();

    (!texts.is_empty()).then(|| texts.join("\n"))
}

pub(crate) async fn send_forward_bundle(
    event: &MsgEvent,
    messages: &[MessageOutput],
) -> ForwardSendReport {
    let target = match event.group_id {
        Some(group_id) => ForwardTarget::Group(group_id),
        None => ForwardTarget::Private(event.user_id),
    };
    send_forward_bundle_to(event, target, messages).await
}

async fn send_forward_bundle_to(
    event: &impl CanSendApi,
    target: ForwardTarget,
    messages: &[MessageOutput],
) -> ForwardSendReport {
    let mut report = ForwardSendReport {
        sent_chunks: 0,
        failed_messages: Vec::new(),
    };

    for chunk in messages.chunks(MAX_FORWARD_NODES) {
        if send_forward_chunk(event, target, chunk).await {
            report.sent_chunks += 1;
        } else {
            tracing::warn!(messages = chunk.len(), "Failed to send a forward chunk");
            report.failed_messages.extend_from_slice(chunk);
        }
    }

    report
}

/// Send the plain-text degradation of a failed bundle, awaited so it keeps
/// its place among the other outputs.
///
/// Returns `true` only when the server acknowledged the send.
pub(crate) async fn reply_bundle_plain_text_fallback(
    event: &MsgEvent,
    messages: &[MessageOutput],
) -> bool {
    send_bundle_plain_text_fallback(event, event.group_id, event.user_id, messages).await
}

async fn send_bundle_plain_text_fallback(
    api: &impl CanSendApi,
    group_id: Option<i64>,
    user_id: i64,
    messages: &[MessageOutput],
) -> bool {
    let Some(fallback) = bundle_plain_text_fallback(messages) else {
        tracing::warn!("Plain-text fallback had no text content; bundle dropped");
        return false;
    };

    tracing::warn!(
        "Forward send failed; falling back to plain text ({} chars)",
        fallback.len()
    );
    let mut msg = Message::new();
    msg.push(Segment::new("text", json!({"text": fallback})));
    send_msg_awaited(api, group_id, user_id, msg).await
}

async fn send_forward_chunk(
    event: &impl CanSendApi,
    target: ForwardTarget,
    chunk: &[MessageOutput],
) -> bool {
    let nodes_value = bundle_chunk_to_onebot_nodes(chunk);

    match target {
        ForwardTarget::Group(group_id) => {
            send_forward_chunk_via(
                event,
                &GROUP_FORWARD_VARIANT,
                &GROUP_FORWARD_VARIANTS,
                "group_id",
                group_id,
                nodes_value,
            )
            .await
        }
        ForwardTarget::Private(user_id) => {
            send_forward_chunk_via(
                event,
                &PRIVATE_FORWARD_VARIANT,
                &PRIVATE_FORWARD_VARIANTS,
                "user_id",
                user_id,
                nodes_value,
            )
            .await
        }
    }
}

/// OneBot retcodes for a request the implementation rejected before acting on
/// it: 1404 is an unknown action, 1400 a payload it cannot parse (e.g. the
/// nodes under a key it does not read). Only these prove that the chunk was
/// not delivered, so only these may move on to another API shape.
const UNSUPPORTED_SHAPE_RETCODES: [i32; 2] = [1404, 1400];

/// Send one chunk, trying the cached working variant first. When no variant
/// is cached yet, probe them in order and remember the first success.
///
/// A variant is skipped only when the implementation rejects its shape (see
/// [`UNSUPPORTED_SHAPE_RETCODES`]). Any other failure, including a timeout,
/// may come after the forward was already delivered, so the chunk is not
/// resent under another shape and the cached variant stays. When the cached
/// variant's shape is rejected (e.g. a OneBot implementation update removed
/// it), the cache is reset and the remaining variants are probed for this
/// send.
async fn send_forward_chunk_via(
    event: &impl CanSendApi,
    cached_variant: &AtomicU8,
    variants: &ForwardVariants,
    id_key: &str,
    id: i64,
    nodes_value: serde_json::Value,
) -> bool {
    let cached = usize::from(cached_variant.load(Ordering::Relaxed));
    let order = (cached < variants.len())
        .then_some(cached)
        .into_iter()
        .chain((0..variants.len()).filter(|&idx| idx != cached));

    for idx in order {
        let (action, payload_key) = variants[idx];
        match event
            .send_api_return(
                action,
                json!({(id_key): id, (payload_key): nodes_value.clone()}),
            )
            .await
        {
            Ok(_) => {
                cached_variant.store(idx as u8, Ordering::Relaxed);
                return true;
            }
            Err(e) if UNSUPPORTED_SHAPE_RETCODES.contains(&e.retcode) => {
                if idx == cached {
                    cached_variant.store(FORWARD_VARIANT_UNKNOWN, Ordering::Relaxed);
                    tracing::debug!(
                        action,
                        retcode = e.retcode,
                        "Cached forward variant rejected; probing the others"
                    );
                }
            }
            Err(e) => {
                tracing::warn!(
                    action,
                    retcode = e.retcode,
                    message = ?e.message,
                    "Forward send failed; not retrying in case it was delivered"
                );
                return false;
            }
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::mock_api;
    use karta_core::message::MessageOutput;
    use kovi::bot::SendApi;
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;

    fn messages(count: usize) -> Vec<MessageOutput> {
        (0..count)
            .map(|i| MessageOutput::from_text(format!("msg {i}")))
            .collect()
    }

    #[tokio::test]
    async fn probe_latches_only_a_working_variant() {
        type Respond = fn(&SendApi) -> Result<(), i32>;
        // (case, answers, latched variant, requests after the 1st and 2nd send)
        let cases: [(&str, Respond, u8, [usize; 2]); 2] = [
            (
                // Only the second shape (action + "message" key) works; the
                // first is rejected as a payload the server cannot read. The
                // second send goes straight to the latched shape.
                "second shape works",
                |api| {
                    if api.action == "send_private_forward_msg"
                        && api.params.get("message").is_some()
                    {
                        Ok(())
                    } else {
                        Err(1400)
                    }
                },
                1,
                [2, 3],
            ),
            (
                // Nothing latched, so the next send probes every shape again.
                "no shape works",
                |_| Err(1404),
                FORWARD_VARIANT_UNKNOWN,
                [3, 6],
            ),
        ];

        for (case, respond, latched, requests) in cases {
            let (event, log) = mock_api(respond);
            let cache = AtomicU8::new(FORWARD_VARIANT_UNKNOWN);
            let works = latched != FORWARD_VARIANT_UNKNOWN;

            for expected_requests in requests {
                let sent = send_forward_chunk_via(
                    &event,
                    &cache,
                    &PRIVATE_FORWARD_VARIANTS,
                    "user_id",
                    123,
                    bundle_chunk_to_onebot_nodes(&messages(1)),
                )
                .await;
                assert_eq!(sent, works, "{case}");
                assert_eq!(cache.load(Ordering::Relaxed), latched, "{case}");
                assert_eq!(log.lock().unwrap().len(), expected_requests, "{case}");
            }
        }
    }

    #[tokio::test]
    async fn cached_variant_failure_unlatches_and_reprobes() {
        // Succeeds on variant 0 until "broken" flips, then only variant 2
        // (a different action name) works — simulating a OneBot update that
        // drops the previously working API shape.
        let broken = Arc::new(AtomicBool::new(false));
        let broken_in_mock = broken.clone();
        let (event, log) = mock_api(move |api| {
            let works = if broken_in_mock.load(Ordering::SeqCst) {
                api.action == "send_forward_msg_to_group"
            } else {
                api.action == "send_group_forward_msg" && api.params.get("messages").is_some()
            };
            if works { Ok(()) } else { Err(1404) }
        });
        let cache = AtomicU8::new(FORWARD_VARIANT_UNKNOWN);

        // First send latches variant 0.
        let sent = send_forward_chunk_via(
            &event,
            &cache,
            &GROUP_FORWARD_VARIANTS,
            "group_id",
            42,
            bundle_chunk_to_onebot_nodes(&messages(1)),
        )
        .await;
        assert!(sent);
        assert_eq!(cache.load(Ordering::Relaxed), 0);
        assert_eq!(log.lock().unwrap().len(), 1);

        // The latched variant is now rejected: the send must un-latch, probe
        // the other variants within this send, and latch the working one.
        broken.store(true, Ordering::SeqCst);
        let sent = send_forward_chunk_via(
            &event,
            &cache,
            &GROUP_FORWARD_VARIANTS,
            "group_id",
            42,
            bundle_chunk_to_onebot_nodes(&messages(1)),
        )
        .await;
        assert!(sent, "send must succeed via re-probe, not fail outright");
        assert_eq!(cache.load(Ordering::Relaxed), 2);
        // First send (1) + rejected cached attempt (1) + the 2 other variants.
        assert_eq!(log.lock().unwrap().len(), 4);
    }

    #[tokio::test]
    async fn failure_that_may_have_delivered_is_not_resent() {
        // Variant 0 works, but this send fails in a way that does not rule
        // out delivery (a timeout surfaces as kovi's -500).
        let fail = Arc::new(AtomicBool::new(false));
        let fail_in_mock = fail.clone();
        let (event, log) = mock_api(move |api| {
            if fail_in_mock.load(Ordering::SeqCst) {
                Err(-500)
            } else if api.params.get("messages").is_some() {
                Ok(())
            } else {
                Err(1400)
            }
        });
        let send = |cache| {
            send_forward_chunk_via(
                &event,
                cache,
                &GROUP_FORWARD_VARIANTS,
                "group_id",
                42,
                bundle_chunk_to_onebot_nodes(&messages(1)),
            )
        };

        // Known-working variant: one send, the latch stays.
        let cache = AtomicU8::new(0);
        fail.store(true, Ordering::SeqCst);
        assert!(!send(&cache).await);
        assert_eq!(log.lock().unwrap().len(), 1);
        assert_eq!(cache.load(Ordering::Relaxed), 0);

        // The latch still works once the server recovers.
        fail.store(false, Ordering::SeqCst);
        assert!(send(&cache).await);
        assert_eq!(log.lock().unwrap().len(), 2);
    }
}
