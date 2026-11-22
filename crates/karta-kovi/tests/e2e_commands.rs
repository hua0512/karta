//! End-to-end admin commands: the command reply, and its effect on the
//! links that follow (moderation state lives in `state.json`).

mod common;

use std::sync::Arc;

use common::*;
use serde_json::json;
use test_harness::onebot::{group_msg, private_msg};

#[tokio::test]
async fn ban_ignores_the_user_across_restarts() {
    let parser = Arc::new(bili_parser().returning(bili_result("t")));
    let adapter = start(json!({}), vec![parser.clone()]).await;

    let usage = adapter.admin_command(admin_msg(Some(1), "/ban abc")).await;
    assert!(usage.starts_with("Usage: /ban"), "{usage}");
    assert_eq!(
        adapter.admin_command(admin_msg(Some(1), "/ban 9000")).await,
        "Banned user 9000"
    );

    adapter
        .assert_ignored(group_msg(1, 9000, BILI_LINK), group_msg(1, 9001, BILI_LINK))
        .await;

    // The ban is persisted: a restarted plugin on the same data dir still
    // ignores the user.
    let Adapter { bot, app } = adapter;
    drop(bot);
    let bot = start_on(&app, vec![parser.clone()]).await;
    let adapter = Adapter { bot, app };
    adapter
        .assert_ignored(
            group_msg(1, 9000, OTHER_BILI_LINK),
            group_msg(1, 9001, OTHER_BILI_LINK),
        )
        .await;

    assert_eq!(parser.inputs(), ["BV1xx411c7mD", "BV1yy411c7mE"]);
}

#[tokio::test]
async fn platform_toggle_applies_per_group() {
    let parser = Arc::new(bili_parser().returning(bili_result("t")));
    let adapter = start(json!({ "disabled_platforms": ["weibo"] }), vec![parser]).await;

    for (command, reply) in [
        (
            admin_msg(None, "/disable bili"),
            "This command can only be used in a group.",
        ),
        (
            admin_msg(Some(1), "/disable b站"),
            "Disabled bilibili in this group",
        ),
        // No parser exists for a platform disabled in config, so a group
        // toggle could never take effect.
        (
            admin_msg(Some(1), "/enable 微博"),
            "weibo is disabled globally in config; it cannot be enabled per group",
        ),
    ] {
        assert_eq!(adapter.admin_command(command).await, reply);
    }
    let unknown = adapter
        .admin_command(admin_msg(Some(1), "/disable nope"))
        .await;
    assert!(unknown.starts_with("Unknown platform 'nope'"), "{unknown}");

    // Disabled in group 1 only: no ack, no parse there; group 2 still works.
    adapter
        .assert_ignored(group_msg(1, 200, BILI_LINK), group_msg(2, 200, BILI_LINK))
        .await;

    assert_eq!(
        adapter
            .admin_command(admin_msg(Some(1), "/enable bili"))
            .await,
        "Enabled bilibili in this group"
    );
    adapter
        .push_and_wait_reaction(group_msg(1, 200, BILI_LINK), REACTION_SUCCESS)
        .await;
}

#[tokio::test]
async fn karta_toggle_in_blacklist_mode() {
    let parser = Arc::new(bili_parser().returning(bili_result("t")));
    let adapter = start(json!({}), vec![parser]).await;

    // On by default.
    adapter
        .push_and_wait_reaction(group_msg(1, 200, BILI_LINK), REACTION_SUCCESS)
        .await;

    assert_eq!(
        adapter
            .admin_command(admin_msg(Some(1), "/karta off"))
            .await,
        "Karta disabled in this group"
    );
    adapter
        .assert_ignored(group_msg(1, 200, BILI_LINK), group_msg(2, 200, BILI_LINK))
        .await;

    assert_eq!(
        adapter.admin_command(admin_msg(Some(1), "/karta on")).await,
        "Karta enabled in this group"
    );
    adapter
        .push_and_wait_reaction(group_msg(1, 200, BILI_LINK), REACTION_SUCCESS)
        .await;
}

#[tokio::test]
async fn karta_toggle_in_whitelist_mode() {
    let parser = Arc::new(bili_parser().returning(bili_result("t")));
    let adapter = start(json!({ "group_blacklist_enabled": false }), vec![parser]).await;

    // Off by default in groups; private chats are always on.
    adapter
        .assert_ignored(group_msg(1, 200, BILI_LINK), private_msg(200, BILI_LINK))
        .await;

    assert_eq!(
        adapter.admin_command(admin_msg(Some(1), "/karta on")).await,
        "Karta enabled in this group"
    );
    adapter
        .push_and_wait_reaction(group_msg(1, 200, BILI_LINK), REACTION_SUCCESS)
        .await;

    adapter
        .admin_command(admin_msg(Some(1), "/karta off"))
        .await;
    adapter
        .assert_ignored(group_msg(1, 200, BILI_LINK), private_msg(200, BILI_LINK))
        .await;
}

#[tokio::test]
async fn foreign_commands_get_no_reply() {
    let adapter = start(json!({}), vec![Arc::new(bili_parser())]).await;

    // Another bot's command (and removed legacy `/parser`): silent.
    // `/status` afterwards is answered, and is the only reply.
    adapter.bot.push(admin_msg(Some(1), "/help"));
    adapter.bot.push(admin_msg(Some(1), "/parser off"));
    let status = adapter.admin_command(admin_msg(Some(1), "/status")).await;
    assert_eq!(adapter.bot.calls_of("send_msg").len(), 1);

    for line in [
        "Render: Text",
        "  bilibili: enabled",
        "  Banned users: 0",
        "  Disabled groups: 0",
    ] {
        assert!(status.lines().any(|l| l == line), "{line:?} in:\n{status}");
    }
}
