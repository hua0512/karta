//! The bot's plugin entry point starts with the real platform parsers and
//! answers an admin command, on the harness's fake OneBot driver.

use test_harness::WAIT;
use test_harness::onebot::{ADMIN_ID, FakeBot, group_msg};

#[tokio::test]
async fn plugin_starts_and_answers_status() {
    // `register_plugin` keeps its data under `<cwd>/data/<plugin>`, so run
    // in a temp dir to keep the checkout clean. This is the only test in
    // this binary, which makes changing the process-wide cwd safe.
    let dir = tempfile::tempdir().expect("temp dir");
    std::env::set_current_dir(dir.path()).expect("enter temp dir");

    let bot = FakeBot::start(karta::__kovi_run_async_plugin).await;
    bot.push(group_msg(1, ADMIN_ID, "/status"));
    let reply = bot.wait_for_call("send_msg", 1, WAIT).await;

    let text = reply.params["message"][0]["data"]["text"]
        .as_str()
        .expect("text reply");
    assert!(text.starts_with("=== Karta Status ==="), "{text}");
    assert!(text.lines().any(|l| l == "  bilibili: enabled"), "{text}");
    assert!(
        dir.path().join("data/karta/config.json").exists(),
        "plugin data lives under the working directory"
    );
}
