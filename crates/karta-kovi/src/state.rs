use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use karta_core::config::atomic_write;
use karta_core::constants::Platform;

/// Runtime-mutable moderation state, persisted to `state.json`.
///
/// Kept separate from the static `Config` so admin commands never rewrite
/// `config.json` (which holds user-managed secrets like cookies).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RuntimeStateData {
    /// Globally banned user IDs (managed via /ban, /unban commands)
    #[serde(default)]
    pub banned_users: HashSet<i64>,

    /// Runtime group set for the toggle command (managed via /karta on|off).
    ///
    /// Interpreted per `Config::group_blacklist_enabled`: in blacklist mode
    /// (default) it holds the groups where the parser is disabled; in
    /// whitelist mode it holds the only groups where the parser is enabled.
    /// The on-disk key stays `disabled_groups` so existing `state.json`
    /// files carry over unchanged.
    #[serde(default, rename = "disabled_groups")]
    pub group_set: HashSet<i64>,

    /// Per-group platform overrides: group_id -> set of disabled platforms
    /// (managed via /enable, /disable commands)
    #[serde(default, deserialize_with = "deserialize_group_disabled_platforms")]
    pub group_disabled_platforms: HashMap<i64, HashSet<Platform>>,
}

/// Deserialize per-group platform sets, dropping unknown slugs with a warning
/// instead of failing the whole file.
///
/// A `state.json` written by a newer version (more platforms) must not reset
/// ALL bans and toggles after a rollback, so this mirrors the tolerance of
/// `migrate_from_config`.
fn deserialize_group_disabled_platforms<'de, D>(
    deserializer: D,
) -> Result<HashMap<i64, HashSet<Platform>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = HashMap::<i64, HashSet<String>>::deserialize(deserializer)?;
    let mut map = HashMap::with_capacity(raw.len());
    for (gid, slugs) in raw {
        let set: HashSet<Platform> = slugs
            .iter()
            .filter_map(|slug| {
                let platform = Platform::from_slug(slug);
                if platform.is_none() {
                    tracing::warn!(
                        "Dropping unknown platform '{}' from persisted state \
                         (written by a newer version?)",
                        slug
                    );
                }
                platform
            })
            .collect();
        if !set.is_empty() {
            map.insert(gid, set);
        }
    }
    Ok(map)
}

struct StateInner {
    data: RwLock<RuntimeStateData>,
    /// Serializes writers to `state.json`: the snapshot is taken and the disk
    /// write completes under this guard, so concurrent admin commands can
    /// never interleave writes or clobber a newer snapshot with an older one.
    save_lock: tokio::sync::Mutex<()>,
    state_path: PathBuf,
}

/// Persistent runtime state backed by `state.json`.
///
/// Wraps `RuntimeStateData` behind a lock and auto-saves on every mutation.
/// The parking_lot data lock is never held across an `.await`.
#[derive(Clone)]
pub struct RuntimeState {
    inner: Arc<StateInner>,
}

impl RuntimeState {
    /// Create from already-loaded state data (used by tests and `load`).
    pub fn new(data: RuntimeStateData, data_path: &Path) -> Self {
        Self {
            inner: Arc::new(StateInner {
                data: RwLock::new(data),
                save_lock: tokio::sync::Mutex::new(()),
                state_path: data_path.join("state.json"),
            }),
        }
    }

    /// Load state from `state.json` in the plugin data directory.
    ///
    /// When `state.json` does not exist yet, legacy runtime fields are
    /// migrated out of `config.json` (one-time) and persisted.
    pub fn load(data_path: &Path) -> Self {
        let state_path = data_path.join("state.json");

        let data = if state_path.exists() {
            let parsed = std::fs::read_to_string(&state_path)
                .map_err(|e| e.to_string())
                .and_then(|content| serde_json::from_str(&content).map_err(|e| e.to_string()));
            match parsed {
                Ok(data) => data,
                Err(e) => {
                    let bad_path = data_path.join("state.json.bad");
                    let _ = std::fs::copy(&state_path, &bad_path);
                    tracing::error!(
                        "Failed to load {}: {}. A copy was saved to {}; \
                         starting with empty runtime state.",
                        state_path.display(),
                        e,
                        bad_path.display()
                    );
                    RuntimeStateData::default()
                }
            }
        } else {
            let data = migrate_from_config(data_path);
            if let Err(e) = write_state_file(&state_path, &data) {
                tracing::error!("Failed to write initial state file: {}", e);
            }
            data
        };

        Self::new(data, data_path)
    }

    /// Persist the current state to disk.
    ///
    /// The snapshot is serialized under `save_lock` and written on a blocking
    /// thread, keeping writes ordered without blocking the async runtime.
    async fn persist(&self) {
        let _guard = self.inner.save_lock.lock().await;

        let bytes = match serde_json::to_vec_pretty(&*self.inner.data.read()) {
            Ok(bytes) => bytes,
            Err(e) => {
                tracing::error!("Failed to serialize runtime state: {}", e);
                return;
            }
        };

        let path = self.inner.state_path.clone();
        match tokio::task::spawn_blocking(move || atomic_write(&path, &bytes)).await {
            Ok(Ok(())) => {}
            Ok(Err(e)) => tracing::error!("Failed to save runtime state: {}", e),
            Err(e) => tracing::error!("Runtime state save task failed: {}", e),
        }
    }

    // --- User bans ---

    pub fn is_user_banned(&self, uid: i64) -> bool {
        self.inner.data.read().banned_users.contains(&uid)
    }

    /// Ban a user. Returns `true` if the user was newly banned.
    pub async fn ban_user(&self, uid: i64) -> bool {
        let changed = self.inner.data.write().banned_users.insert(uid);
        if changed {
            self.persist().await;
        }
        changed
    }

    /// Unban a user. Returns `true` if the user was actually banned.
    pub async fn unban_user(&self, uid: i64) -> bool {
        let changed = self.inner.data.write().banned_users.remove(&uid);
        if changed {
            self.persist().await;
        }
        changed
    }

    pub fn banned_user_count(&self) -> usize {
        self.inner.data.read().banned_users.len()
    }

    // --- Group parser toggle ---

    /// Whether the parser should run in a group under the configured mode.
    ///
    /// Blacklist mode: enabled unless the group is in the set. Whitelist
    /// mode: enabled only if the group is in the set.
    pub fn is_group_enabled(&self, gid: i64, blacklist_mode: bool) -> bool {
        let in_set = self.inner.data.read().group_set.contains(&gid);
        if blacklist_mode { !in_set } else { in_set }
    }

    /// Enable or disable the parser for a group entirely (idempotent).
    pub async fn set_group_enabled(&self, gid: i64, enabled: bool, blacklist_mode: bool) {
        {
            let mut data = self.inner.data.write();
            // The set holds disabled groups in blacklist mode and enabled
            // groups in whitelist mode.
            let insert = if blacklist_mode { !enabled } else { enabled };
            if insert {
                data.group_set.insert(gid);
            } else {
                data.group_set.remove(&gid);
            }
        }
        self.persist().await;
    }

    pub fn group_set_count(&self) -> usize {
        self.inner.data.read().group_set.len()
    }

    // --- Per-group platform overrides ---

    pub fn is_platform_disabled_in_group(&self, gid: i64, platform: Platform) -> bool {
        self.inner
            .data
            .read()
            .group_disabled_platforms
            .get(&gid)
            .is_some_and(|s| s.contains(&platform))
    }

    /// Enable or disable a specific platform in a group.
    pub async fn set_platform_in_group(&self, gid: i64, platform: Platform, enabled: bool) {
        {
            let mut data = self.inner.data.write();
            let set = data.group_disabled_platforms.entry(gid).or_default();
            if enabled {
                set.remove(&platform);
                if set.is_empty() {
                    data.group_disabled_platforms.remove(&gid);
                }
            } else {
                set.insert(platform);
            }
        }
        self.persist().await;
    }

    /// Get the set of disabled platforms in a group.
    pub fn disabled_platforms_in_group(&self, gid: i64) -> Vec<Platform> {
        self.inner
            .data
            .read()
            .group_disabled_platforms
            .get(&gid)
            .map(|s| s.iter().copied().collect())
            .unwrap_or_default()
    }
}

/// Write `state.json` atomically.
fn write_state_file(state_path: &Path, data: &RuntimeStateData) -> std::io::Result<()> {
    if let Some(parent) = state_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(data)?;
    atomic_write(state_path, &bytes)
}

/// One-time migration: extract legacy runtime fields from a raw `config.json`.
///
/// Older versions stored bans/toggles inside `Config`; unknown platform names
/// are dropped with a warning. `config.json` itself is left untouched (the
/// leftover fields are ignored by `Config` deserialization).
fn migrate_from_config(data_path: &Path) -> RuntimeStateData {
    let config_path = data_path.join("config.json");
    let mut data = RuntimeStateData::default();

    let Ok(content) = std::fs::read_to_string(&config_path) else {
        return data;
    };
    let Ok(raw) = serde_json::from_str::<serde_json::Value>(&content) else {
        return data;
    };

    let mut migrated = false;

    if let Some(users) = raw.get("banned_users").and_then(|v| v.as_array()) {
        data.banned_users = users.iter().filter_map(|v| v.as_i64()).collect();
        migrated = true;
    }

    if let Some(groups) = raw.get("disabled_groups").and_then(|v| v.as_array()) {
        data.group_set = groups.iter().filter_map(|v| v.as_i64()).collect();
        migrated = true;
    }

    if let Some(map) = raw
        .get("group_disabled_platforms")
        .and_then(|v| v.as_object())
    {
        for (gid, names) in map {
            let Ok(gid) = gid.parse::<i64>() else {
                continue;
            };
            let Some(names) = names.as_array() else {
                continue;
            };
            let set: HashSet<Platform> = names
                .iter()
                .filter_map(|n| {
                    let slug = n.as_str()?;
                    let platform = Platform::from_slug(slug);
                    if platform.is_none() {
                        tracing::warn!(
                            "Dropping unknown platform '{}' during state migration",
                            slug
                        );
                    }
                    platform
                })
                .collect();
            if !set.is_empty() {
                data.group_disabled_platforms.insert(gid, set);
            }
        }
        migrated = true;
    }

    if migrated {
        tracing::info!(
            "Migrated runtime state (bans, group toggles) from config.json to state.json"
        );
    }

    data
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn load_migrates_legacy_fields_from_config_json() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            dir.path().join("config.json"),
            r#"{
                "bili_cookie": "secret",
                "banned_users": [111, 222],
                "disabled_groups": [333],
                "group_disabled_platforms": {"444": ["bilibili", "not-a-platform"]}
            }"#,
        )
        .expect("write legacy config");

        let state = RuntimeState::load(dir.path());

        assert!(state.is_user_banned(111));
        assert!(state.is_user_banned(222));
        assert!(!state.is_group_enabled(333, true));
        assert!(state.is_platform_disabled_in_group(444, Platform::Bilibili));
        assert_eq!(state.disabled_platforms_in_group(444).len(), 1);

        // state.json was persisted; config.json is untouched.
        assert!(dir.path().join("state.json").exists());
        assert!(
            std::fs::read_to_string(dir.path().join("config.json"))
                .expect("read config")
                .contains("secret")
        );

        // A second load takes the state.json path, not the migration path.
        let reloaded = RuntimeState::load(dir.path());
        assert!(reloaded.is_user_banned(111));
    }

    #[tokio::test]
    async fn load_drops_unknown_platform_slugs_instead_of_resetting_state() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            dir.path().join("state.json"),
            r#"{
                "banned_users": [111],
                "disabled_groups": [222],
                "group_disabled_platforms": {
                    "333": ["bilibili", "platform-from-the-future"],
                    "444": ["platform-from-the-future"]
                }
            }"#,
        )
        .expect("write state");

        let state = RuntimeState::load(dir.path());

        // Everything known survives; only the unknown slug is dropped.
        assert!(state.is_user_banned(111));
        assert!(!state.is_group_enabled(222, true));
        assert!(state.is_platform_disabled_in_group(333, Platform::Bilibili));
        assert_eq!(state.disabled_platforms_in_group(333).len(), 1);
        // A group left with no known platforms drops its entry entirely.
        assert!(state.disabled_platforms_in_group(444).is_empty());

        // The file parsed fine, so it was not quarantined.
        assert!(!dir.path().join("state.json.bad").exists());
    }

    #[tokio::test]
    async fn concurrent_mutations_all_reach_disk() {
        let dir = tempfile::tempdir().expect("temp dir");
        let state = RuntimeState::new(RuntimeStateData::default(), dir.path());

        let mut handles = Vec::new();
        for uid in 0..16 {
            let state = state.clone();
            handles.push(tokio::spawn(async move {
                state.ban_user(uid).await;
            }));
        }
        for handle in handles {
            handle.await.expect("task");
        }

        let persisted: RuntimeStateData = serde_json::from_str(
            &std::fs::read_to_string(dir.path().join("state.json")).expect("read state"),
        )
        .expect("parse state");
        assert_eq!(persisted.banned_users.len(), 16);
    }
}
