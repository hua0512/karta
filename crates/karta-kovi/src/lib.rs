//! Kovi framework adapter for karta-core
//!
//! This crate provides integration with the Kovi bot framework.

mod access_control;
pub mod commands;
mod forward;
mod gate;
mod input;
mod output;
mod pipeline;
mod reaction;
pub mod state;
#[cfg(test)]
mod test_support;

use std::path::PathBuf;
use std::sync::Arc;

use kovi::PluginBuilder;
use kovi_onebot::EventRegistrar;

use access_control::setup_access_control;
use gate::ParseGate;
use karta_core::ParserEngine;
use karta_core::download::SharedDownloader;
use karta_core::parser::Parser;
use karta_platforms::CredentialManager;
pub use output::{to_kovi_message, to_kovi_messages};
use pipeline::{
    bili_cookie_refresh_schedule, handle_message, refresh_bili_cookie, run_cache_cleanup,
};
use state::RuntimeState;

/// Register the parser plugin with Kovi
///
/// This is the main entry point for using the parser with Kovi.
///
/// # Example
///
/// ```ignore
/// use karta_kovi::register_plugin;
///
/// #[kovi::plugin]
/// async fn main() {
///     register_plugin().await;
/// }
/// ```
pub async fn register_plugin() {
    register_plugin_with(PluginOptions::default()).await;
}

/// Overrides for [`register_plugin_with`]; the defaults reproduce
/// [`register_plugin`].
#[doc(hidden)]
#[derive(Clone, Default)]
pub struct PluginOptions {
    /// Data directory (config, state, caches). Defaults to Kovi's plugin
    /// data dir, which lives under the process working directory.
    pub data_path: Option<PathBuf>,
    /// Downloader (and HTTP client) shared by the engine and the platform
    /// parsers. Defaults to the production downloader.
    pub downloader: Option<SharedDownloader>,
    /// Parsers to register instead of the platform parsers.
    pub parsers: Option<Vec<Arc<dyn Parser>>>,
}

/// [`register_plugin`] with test overrides.
#[doc(hidden)]
pub async fn register_plugin_with(options: PluginOptions) {
    // Get runtime bot for data path
    let bot = PluginBuilder::get_runtime_bot();
    let kovi_data_path = bot.get_data_path();
    // Derive plugin name from Kovi's data path (Kovi uses the crate name as directory)
    let plugin_name = kovi_data_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("karta")
        .to_string();
    let data_path = options.data_path.unwrap_or(kovi_data_path);

    // Load configuration
    let config = karta_core::config::Config::load(&data_path);
    let max_concurrent_parses = config.max_concurrent_parses;
    tracing::info!("Karta plugin loaded with config: {:?}", config.render_type);

    // Set up access control if configured
    if config.access_control.enabled {
        setup_access_control(&bot, &plugin_name, &config.access_control);
    }

    // Initialize credential manager for Bilibili
    let cm_client = match wreq::Client::builder().cookie_store(true).build() {
        Ok(client) => client,
        Err(e) => {
            // Degrade rather than abort: only Bilibili credential handling
            // relies on the cookie store, the rest of the plugin still works.
            tracing::error!(
                "Failed to build credential HTTP client with cookie store: {}. \
                 Falling back to a cookie-less client; Bilibili QR login and \
                 credential refresh will not work.",
                e
            );
            wreq::Client::default()
        }
    };
    let credential_manager = Arc::new({
        let cm = CredentialManager::new(cm_client, &data_path);
        if let Err(e) = cm.init(config.bili_cookie.as_deref()).await {
            tracing::warn!("Bilibili credential init failed: {}", e);
        }
        cm
    });
    if let Some(schedule) = bili_cookie_refresh_schedule(&config) {
        // In the background, so a slow passport API does not hold up startup.
        let cm = credential_manager.clone();
        kovi::spawn(async move { refresh_bili_cookie(&cm).await });

        let cm = credential_manager.clone();
        if let Err(e) = PluginBuilder::cron(schedule, move || {
            let cm = cm.clone();
            async move { refresh_bili_cookie(&cm).await }
        }) {
            tracing::warn!("Failed to schedule Bilibili cookie refresh cron: {}", e);
        }
    }

    // Load runtime state (bans, group toggles, platform overrides) from state.json
    let state = Arc::new(RuntimeState::load(&data_path));

    // Initialize parser engine with platform parsers
    let downloader = match options.downloader {
        Some(downloader) => downloader,
        None => match karta_core::download::StreamDownloader::new(&config, &data_path) {
            Ok(dl) => Arc::new(dl),
            Err(e) => {
                tracing::error!("Failed to initialize downloader: {}", e);
                return;
            }
        },
    };
    let parsers = options.parsers.unwrap_or_else(|| {
        karta_platforms::create_all_parsers_with_bili_credential(
            &config,
            downloader.clone(),
            &data_path,
            credential_manager.shared_credential(),
        )
    });
    let engine = Arc::new(ParserEngine::with_parsers(
        config,
        data_path.clone(),
        downloader,
        parsers,
    ));

    // Check FFmpeg availability
    let ffmpeg_available = engine.is_ffmpeg_available().await;
    if !ffmpeg_available {
        tracing::warn!("FFmpeg not found. Video merging will be disabled.");
    }

    // Register admin command handler
    {
        let engine = engine.clone();
        let cm = credential_manager.clone();
        let state = state.clone();
        PluginBuilder::on_admin_msg(move |event| {
            let engine = engine.clone();
            let cm = cm.clone();
            let state = state.clone();
            async move {
                if let Some(text) = event.borrow_text()
                    && text.trim().starts_with('/')
                {
                    commands::handle_command(
                        text,
                        event.group_id,
                        &engine,
                        &cm,
                        &state,
                        event.as_ref(),
                    )
                    .await;
                }
            }
        });
    }

    // Register message handler
    let engine_for_msg = engine.clone();
    let state_for_msg = state.clone();
    let gate_for_msg = Arc::new(ParseGate::new(max_concurrent_parses));
    PluginBuilder::on_msg(move |event| {
        let engine = engine_for_msg.clone();
        let state = state_for_msg.clone();
        let gate = gate_for_msg.clone();

        async move {
            handle_message(event.as_ref(), &engine, &state, &gate).await;
            Some(())
        }
    });

    // Schedule daily cache cleanup at 01:00.
    let ytdlp_dir = karta_core::config::Config::ytdlp_dir(&data_path);
    if let Err(e) = PluginBuilder::cron("0 0 1 * * *", move || {
        run_cache_cleanup(engine.clone(), ytdlp_dir.clone())
    }) {
        tracing::warn!("Failed to schedule cache cleanup cron: {}", e);
    }
}

/// Kovi plugin entry point macro helper
///
/// Use this in your plugin's main function:
///
/// ```ignore
/// #[kovi::plugin]
/// async fn main() {
///     karta_kovi::register_plugin().await;
/// }
/// ```
#[macro_export]
macro_rules! kovi_plugin {
    () => {
        #[kovi::plugin]
        async fn main() {
            $crate::register_plugin().await;
        }
    };
}
