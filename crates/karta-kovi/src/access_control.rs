use kovi::bot::AccessControlMode;
use kovi::bot::runtimebot::kovi_api::SetAccessControlList;
use kovi::event::id::ID;

use karta_core::config::{AccessControlConfig, AccessMode};

/// Set up Kovi framework-level access control for the parser plugin.
///
/// This configures blacklist/whitelist at the framework level so blocked
/// users/groups never trigger any of the plugin's listeners.
pub(crate) fn setup_access_control(
    bot: &kovi::bot::runtimebot::RuntimeBot,
    plugin_name: &str,
    ac: &AccessControlConfig,
) {
    if let Err(e) = bot.set_plugin_access_control(plugin_name, true) {
        tracing::error!("Failed to enable access control: {}", e);
        return;
    }

    let mode = match ac.mode {
        AccessMode::Blacklist => AccessControlMode::BlackList,
        AccessMode::Whitelist => AccessControlMode::WhiteList,
    };
    if let Err(e) = bot.set_plugin_access_control_mode(plugin_name, mode) {
        tracing::error!("Failed to set access control mode: {}", e);
        return;
    }

    if !ac.users.is_empty()
        && let Err(e) = bot.set_plugin_access_control_list(
            plugin_name,
            false,
            SetAccessControlList::Changes(ac.users.iter().copied().map(ID::new).collect()),
        )
    {
        tracing::error!("Failed to set access control user list: {}", e);
    }

    if !ac.groups.is_empty()
        && let Err(e) = bot.set_plugin_access_control_list(
            plugin_name,
            true,
            SetAccessControlList::Changes(ac.groups.iter().copied().map(ID::new).collect()),
        )
    {
        tracing::error!("Failed to set access control group list: {}", e);
    }

    tracing::info!(
        "Access control enabled: mode={:?}, {} users, {} groups",
        ac.mode,
        ac.users.len(),
        ac.groups.len(),
    );
}
