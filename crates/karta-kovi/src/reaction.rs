use kovi::bot::runtimebot::CanSendApi;
use serde_json::json;
use std::sync::atomic::{AtomicU8, Ordering};

// Reaction emoji IDs sent via `set_msg_emoji_like` / `set_group_reaction`.
// Values below 9728 are QQ face IDs; larger values are Unicode code points.

/// Acknowledgment: a supported link was spotted and parsing has started.
pub(crate) const REACTION_ACK: &str = "424";
/// Success: replies delivered without a skipped or failed attachment (QQ face 144).
pub(crate) const REACTION_SUCCESS: &str = "144";
/// Failure: parse, render, media limit, or delivery failed (U+274C ❌ cross mark).
pub(crate) const REACTION_FAILURE: &str = "10060";

const SUPPORT_UNKNOWN: u8 = 0;
const SUPPORT_YES: u8 = 1;
const SUPPORT_NO: u8 = 2;

static SUPPORT_SET_MSG_EMOJI_LIKE: AtomicU8 = AtomicU8::new(SUPPORT_UNKNOWN);
static SUPPORT_SET_GROUP_REACTION: AtomicU8 = AtomicU8::new(SUPPORT_UNKNOWN);

pub(crate) async fn try_react(
    event: &impl CanSendApi,
    message_id: i32,
    group_id: Option<i64>,
    emoji_id: &str,
) {
    try_react_with(
        &SUPPORT_SET_MSG_EMOJI_LIKE,
        &SUPPORT_SET_GROUP_REACTION,
        event,
        message_id,
        group_id,
        emoji_id,
    )
    .await
}

/// Probe logic with injectable support flags so tests can use local state.
///
/// Only retcode 1404 ("action not found") latches a verdict; any other error
/// is treated as transient and leaves the flag `SUPPORT_UNKNOWN` so the next
/// message re-probes instead of permanently committing to a wrong answer.
async fn try_react_with(
    emoji_like_support: &AtomicU8,
    group_reaction_support: &AtomicU8,
    event: &impl CanSendApi,
    message_id: i32,
    group_id: Option<i64>,
    emoji_id: &str,
) {
    let support = emoji_like_support.load(Ordering::Relaxed);
    if support == SUPPORT_UNKNOWN {
        let r = event
            .send_api_return(
                "set_msg_emoji_like",
                json!({"message_id": message_id, "emoji_id": emoji_id, "set": true}),
            )
            .await;
        match r {
            Ok(_) => {
                emoji_like_support.store(SUPPORT_YES, Ordering::Relaxed);
                return;
            }
            Err(e) if e.retcode == 1404 => {
                emoji_like_support.store(SUPPORT_NO, Ordering::Relaxed);
            }
            Err(e) => {
                tracing::debug!(
                    retcode = e.retcode,
                    "set_msg_emoji_like probe failed transiently; will re-probe"
                );
                return;
            }
        }
    } else if support == SUPPORT_YES {
        event.send_api(
            "set_msg_emoji_like",
            json!({"message_id": message_id, "emoji_id": emoji_id, "set": true}),
        );
        return;
    }

    let Some(gid) = group_id else {
        return;
    };

    let support = group_reaction_support.load(Ordering::Relaxed);
    if support == SUPPORT_UNKNOWN {
        let r = event
            .send_api_return(
                "set_group_reaction",
                json!({"group_id": gid, "message_id": message_id, "code": emoji_id, "is_add": true}),
            )
            .await;
        match r {
            Ok(_) => {
                group_reaction_support.store(SUPPORT_YES, Ordering::Relaxed);
            }
            Err(e) if e.retcode == 1404 => {
                group_reaction_support.store(SUPPORT_NO, Ordering::Relaxed);
            }
            Err(e) => {
                tracing::debug!(
                    retcode = e.retcode,
                    "set_group_reaction probe failed transiently; will re-probe"
                );
            }
        }
    } else if support == SUPPORT_YES {
        event.send_api(
            "set_group_reaction",
            json!({"group_id": gid, "message_id": message_id, "code": emoji_id, "is_add": true}),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{actions, mock_api};

    #[tokio::test]
    async fn transient_error_leaves_support_unknown_and_reprobes() {
        let (event, log) = mock_api(|_| Err(500));
        let emoji = AtomicU8::new(SUPPORT_UNKNOWN);
        let group = AtomicU8::new(SUPPORT_UNKNOWN);

        try_react_with(&emoji, &group, &event, 1, Some(42), REACTION_ACK).await;
        try_react_with(&emoji, &group, &event, 2, Some(42), REACTION_ACK).await;

        // Not latched: both messages probed the primary API again.
        assert_eq!(emoji.load(Ordering::Relaxed), SUPPORT_UNKNOWN);
        assert_eq!(group.load(Ordering::Relaxed), SUPPORT_UNKNOWN);
        assert_eq!(actions(&log), ["set_msg_emoji_like", "set_msg_emoji_like"]);
    }

    #[tokio::test]
    async fn unsupported_emoji_like_probes_group_reaction() {
        // (case, set_group_reaction answer, expected group support)
        let cases = [
            ("group reaction unsupported", Err(1404), SUPPORT_NO),
            ("group reaction transient error", Err(502), SUPPORT_UNKNOWN),
        ];

        for (case, group_answer, expected_group) in cases {
            let (event, log) = mock_api(move |api| {
                if api.action == "set_msg_emoji_like" {
                    Err(1404)
                } else {
                    group_answer
                }
            });
            let emoji = AtomicU8::new(SUPPORT_UNKNOWN);
            let group = AtomicU8::new(SUPPORT_UNKNOWN);

            try_react_with(&emoji, &group, &event, 1, Some(42), REACTION_ACK).await;

            assert_eq!(emoji.load(Ordering::Relaxed), SUPPORT_NO, "{case}");
            assert_eq!(group.load(Ordering::Relaxed), expected_group, "{case}");
            assert_eq!(
                actions(&log),
                ["set_msg_emoji_like", "set_group_reaction"],
                "{case}"
            );
        }
    }
}
