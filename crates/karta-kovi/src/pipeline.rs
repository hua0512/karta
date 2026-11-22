use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use kovi_onebot::MsgEvent;
use kovi_onebot::event::PostType;

use karta_core::cache::cache_key;
use karta_core::message::{MessageOutput, MessageSegment, RenderOutput};
use karta_core::{ParseResult, ParserEngine, ParserError};
use karta_platforms::{CredentialManager, RefreshOutcome};

use crate::forward::{reply_bundle_plain_text_fallback, send_forward_bundle};
use crate::gate::{FlightOutcome, ParseGate};
use crate::input::extract_text_from_message;
use crate::output::reply_output_awaited;
use crate::reaction::{REACTION_ACK, REACTION_FAILURE, REACTION_SUCCESS, try_react};
use crate::state::RuntimeState;

/// Process an incoming message event through moderation checks, single-flight
/// parsing/rendering, reply delivery, and status reactions.
pub(crate) async fn handle_message(
    event: &MsgEvent,
    engine: &ParserEngine,
    state: &RuntimeState,
    gate: &ParseGate,
) {
    let config = engine.config();
    let need_upload = config.need_upload;
    let should_react = config.reaction_enabled;
    let group_blacklist_enabled = config.group_blacklist_enabled;

    // Ignore the bot's own messages. NapCat with message_sent
    // reporting delivers outgoing messages back as events; parsing
    // them (with append_url) would loop forever.
    if event.user_id == event.self_id || event.post_type == PostType::MessageSent {
        return;
    }

    // Cheap moderation checks before any matching or network work.
    if state.is_user_banned(event.user_id) {
        return;
    }
    // Blacklist mode: on unless toggled off; whitelist mode: off
    // unless toggled on. Private chats are always enabled.
    if let Some(gid) = event.group_id
        && !state.is_group_enabled(gid, group_blacklist_enabled)
    {
        return;
    }

    // Extract text from message: try JSON card URLs first, then plain text.
    let Some(text) = extract_text_from_message(&event.message, event.borrow_text()) else {
        return;
    };

    // Resolve the platform up front so a disabled platform never
    // triggers the ack reaction or a wasted network parse.
    let Some(platform) = engine.match_platform(&text) else {
        return;
    };
    if let Some(gid) = event.group_id
        && state.is_platform_disabled_in_group(gid, platform)
    {
        tracing::debug!(
            platform = platform.slug(),
            group = gid,
            "Platform disabled in group, skipping"
        );
        return;
    }

    if should_react {
        try_react(event, event.message_id, event.group_id, REACTION_ACK).await;
    }

    tracing::debug!(text = %text, "Processing message");

    match gate
        .run(cache_key(&text), run_parse_render(engine, &text))
        .await
    {
        FlightOutcome::Rendered(messages) => {
            tracing::debug!(count = messages.len(), "Parsed and rendered");
            let mut delivered_any = false;
            let mut incomplete = messages.iter().any(|output| {
                output.messages().iter().any(|message| {
                    message
                        .segments
                        .iter()
                        .any(|segment| segment.is_failure_notice)
                })
            });

            for output in messages.iter() {
                let message = match output {
                    RenderOutput::Bundle(items) => {
                        // QQ presents a bundle as a merged forward.
                        let report = send_forward_bundle(event, items).await;
                        if report.sent_chunks > 0 {
                            delivered_any = true;
                        }
                        if !report.failed_messages.is_empty() {
                            tracing::warn!(
                                failed = report.failed_messages.len(),
                                "Forward chunks failed; retrying failed messages as plain text"
                            );
                            if reply_bundle_plain_text_fallback(event, &report.failed_messages)
                                .await
                            {
                                delivered_any = true;
                            }
                        }
                        continue;
                    }
                    RenderOutput::Message(message) => message,
                };

                tracing::debug!(segments = message.segments.len(), "Replying");
                // Awaited so outputs land in order: a fire-and-forget
                // reply lets the next awaited send (e.g. the forward
                // bundle) overtake the rendered card image.
                if reply_output_awaited(event, message, need_upload).await {
                    delivered_any = true;
                } else {
                    incomplete = true;
                }
            }

            if !delivered_any {
                tracing::warn!("No reply was delivered for a rendered result");
            }
            if should_react {
                let reaction = if delivered_any && !incomplete {
                    REACTION_SUCCESS
                } else {
                    REACTION_FAILURE
                };
                try_react(event, event.message_id, event.group_id, reaction).await;
            }
        }
        FlightOutcome::NoResult => {
            // Pattern pre-matched but the full parse found nothing.
        }
        FlightOutcome::Failed(error) => {
            // The leading flight already logged at warn; this ties the
            // shared failure back to each waiting message.
            tracing::debug!(%error, "Parse flight failed for this message");
            if should_react {
                try_react(event, event.message_id, event.group_id, REACTION_FAILURE).await;
            }
        }
    }
}

/// Parse `text` and render the result, mapping everything into a cloneable
/// [`FlightOutcome`] so identical concurrent requests can share it.
async fn run_parse_render(engine: &ParserEngine, text: &str) -> FlightOutcome {
    let parsed = match engine.parse(text).await {
        Ok(Some(result)) => result,
        Ok(None) => return FlightOutcome::NoResult,
        Err(e) => {
            tracing::warn!("Failed to parse: {}", e);
            if let Some(notice) = e.limit_notice() {
                return limit_notice_outcome(notice);
            }
            return FlightOutcome::Failed(Arc::from(e.to_string()));
        }
    };

    match render_with_timeout(engine, &parsed).await {
        Ok(messages) => FlightOutcome::Rendered(Arc::new(messages)),
        Err(e) => {
            tracing::warn!("Failed to render: {}", e);
            if let Some(notice) = e.limit_notice() {
                return limit_notice_outcome(notice);
            }
            FlightOutcome::Failed(Arc::from(e.to_string()))
        }
    }
}

fn limit_notice_outcome(notice: String) -> FlightOutcome {
    FlightOutcome::Rendered(Arc::new(vec![
        MessageOutput::from_segment(MessageSegment::failure_notice(notice)).into(),
    ]))
}

/// `render()` downloads media and runs ffmpeg merges, so it gets its own
/// deadline equal to `parse_timeout` (a separate budget; 0 disables it).
async fn render_with_timeout(
    engine: &ParserEngine,
    result: &ParseResult,
) -> Result<Vec<RenderOutput>, ParserError> {
    let seconds = engine.config().parse_timeout;
    if seconds == 0 {
        return engine.render(result).await;
    }
    match tokio::time::timeout(Duration::from_secs(seconds), engine.render(result)).await {
        Ok(rendered) => rendered,
        Err(_) => {
            tracing::warn!(
                platform = result.platform.slug(),
                timeout = seconds,
                "Render timed out"
            );
            Err(ParserError::Timeout { seconds })
        }
    }
}

/// Sweep stale files from the file cache and yt-dlp output directory, then
/// clear the in-memory parse cache.
pub(crate) async fn run_cache_cleanup(engine: Arc<ParserEngine>, ytdlp_dir: PathBuf) {
    // The directory walks are synchronous; keep them off the async runtime.
    let cleanup_engine = engine.clone();
    let result = tokio::task::spawn_blocking(move || {
        let cache = cleanup_engine.downloader().file_cache().cleanup(7);
        // yt-dlp writes outside the file cache; sweep its output dir too.
        let ytdlp = cleanup_stale_files(&ytdlp_dir, 7);
        (cache, ytdlp)
    })
    .await;

    match result {
        Ok((cache, ytdlp)) => {
            let mut removed = 0;
            match cache {
                Ok(n) => removed += n,
                Err(e) => tracing::warn!("Cache cleanup failed: {}", e),
            }
            match ytdlp {
                Ok(n) => removed += n,
                Err(e) => tracing::warn!("yt-dlp dir cleanup failed: {}", e),
            }
            if removed > 0 {
                tracing::info!("Cache cleanup: removed {} old files", removed);
            }
        }
        Err(e) => tracing::warn!("Cache cleanup task failed: {}", e),
    }
    engine.clear_cache();
}

/// Best-effort removal of stale files in `dir` (non-recursive).
///
/// Companion to `FileCache::cleanup` for directories that are not file
/// caches (e.g. the yt-dlp output dir); skips subdirectories and returns the
/// number of files removed. A missing directory counts as zero.
fn cleanup_stale_files(dir: &Path, max_age_days: u64) -> std::io::Result<usize> {
    let max_age = Duration::from_secs(max_age_days * 24 * 60 * 60);
    let now = std::time::SystemTime::now();
    let mut removed = 0;

    if !dir.exists() {
        return Ok(0);
    }

    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let metadata = entry.metadata()?;
        if !metadata.is_file() {
            continue;
        }

        if let Ok(modified) = metadata.modified()
            && let Ok(age) = now.duration_since(modified)
            && age > max_age
        {
            std::fs::remove_file(entry.path())?;
            removed += 1;
        }
    }

    Ok(removed)
}

/// When the Bilibili cookie refresh check repeats: 00:00 and 12:00.
const BILI_COOKIE_REFRESH_CRON: &str = "0 0 0,12 * * *";

/// Cron schedule for the Bilibili cookie refresh check, or `None` when
/// `bili_cookie_auto_refresh` is off, in which case it does not run at
/// startup either.
pub(crate) fn bili_cookie_refresh_schedule(config: &karta_core::Config) -> Option<&'static str> {
    config
        .bili_cookie_auto_refresh
        .then_some(BILI_COOKIE_REFRESH_CRON)
}

/// One Bilibili cookie refresh check, logged.
pub(crate) async fn refresh_bili_cookie(credential_manager: &CredentialManager) {
    match credential_manager.refresh_if_requested().await {
        Ok(RefreshOutcome::Refreshed) => tracing::info!("Bilibili cookie refreshed"),
        Ok(RefreshOutcome::NotNeeded) => {
            tracing::debug!("Bilibili cookie does not need a refresh yet");
        }
        Ok(RefreshOutcome::NoCredential) => {}
        Ok(RefreshOutcome::MissingRefreshToken) => tracing::info!(
            "Bilibili cookie has no ac_time_value, so it cannot be refreshed automatically; \
             log in with /login bilibili or add ac_time_value to bili_cookie"
        ),
        Err(e) => tracing::warn!("Bilibili cookie refresh failed: {}", e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleanup_stale_files_removes_only_old_files() {
        let dir = tempfile::tempdir().expect("temp dir");
        let old_file = dir.path().join("old.mp4");
        let new_file = dir.path().join("new.mp4");
        let sub_dir = dir.path().join("nested");
        std::fs::write(&old_file, b"old").expect("write old");
        std::fs::write(&new_file, b"new").expect("write new");
        std::fs::create_dir(&sub_dir).expect("create nested dir");

        // Backdate the old file well past the cutoff.
        let old_mtime = std::time::SystemTime::now() - Duration::from_secs(8 * 24 * 60 * 60);
        std::fs::File::options()
            .write(true)
            .open(&old_file)
            .expect("open old")
            .set_modified(old_mtime)
            .expect("set mtime");

        let removed = cleanup_stale_files(dir.path(), 7).expect("cleanup");

        assert_eq!(removed, 1);
        assert!(!old_file.exists());
        assert!(new_file.exists());
        assert!(sub_dir.exists(), "subdirectories must be left alone");

        let missing = dir.path().join("does-not-exist");
        assert_eq!(cleanup_stale_files(&missing, 7).expect("missing dir"), 0);
    }
}
