use std::collections::{HashMap, VecDeque};
use std::hash::Hash;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use parking_lot::Mutex;

/// Hash arbitrary input into a stable, filesystem-safe cache key.
///
/// SHA-256 truncated to 32 hex chars: collision-resistant against
/// attacker-chosen inputs (URLs come from untrusted chat messages) while
/// keeping filenames compact.
pub fn cache_key(input: &str) -> String {
    use sha2::{Digest, Sha256};
    use std::fmt::Write;

    let digest = Sha256::digest(input.as_bytes());
    digest[..16]
        .iter()
        .fold(String::with_capacity(32), |mut key, byte| {
            let _ = write!(key, "{:02x}", byte);
            key
        })
}

struct Entry<V> {
    value: V,
    created: Instant,
}

/// A size-limited map that evicts the least-recently-used entry when capacity
/// is reached (true LRU: `get` refreshes an entry's position).
///
/// An optional TTL turns stale entries into misses; they are evicted lazily
/// when accessed (other expired entries linger until capacity eviction).
pub struct LimitedSizeMap<K, V> {
    map: HashMap<K, Entry<V>>,
    order: VecDeque<K>,
    capacity: usize,
    ttl: Option<Duration>,
}

impl<K: Clone + Eq + Hash, V: Clone> LimitedSizeMap<K, V> {
    /// Create a new LimitedSizeMap with the given capacity and no TTL
    pub fn new(capacity: usize) -> Self {
        Self::with_ttl_opt(capacity, None)
    }

    /// Create a new LimitedSizeMap whose entries expire after `ttl`
    pub fn with_ttl(capacity: usize, ttl: Duration) -> Self {
        Self::with_ttl_opt(capacity, Some(ttl))
    }

    fn with_ttl_opt(capacity: usize, ttl: Option<Duration>) -> Self {
        Self {
            map: HashMap::with_capacity(capacity),
            order: VecDeque::with_capacity(capacity),
            capacity,
            ttl,
        }
    }

    fn is_expired(&self, entry: &Entry<V>, now: Instant) -> bool {
        self.ttl
            .is_some_and(|ttl| now.saturating_duration_since(entry.created) > ttl)
    }

    /// Insert a key-value pair, evicting the least-recently-used entry if at
    /// capacity. Re-inserting an existing key refreshes its TTL.
    pub fn insert(&mut self, key: K, value: V) {
        self.insert_at(key, value, Instant::now());
    }

    /// [`Self::insert`] at a given instant; tests drive the TTL clock with it.
    fn insert_at(&mut self, key: K, value: V, now: Instant) {
        let entry = Entry {
            value,
            created: now,
        };
        if self.map.contains_key(&key) {
            // Update existing entry and move to end
            self.map.insert(key.clone(), entry);
            self.order.retain(|k| k != &key);
            self.order.push_back(key);
        } else {
            // Evict oldest if at capacity
            if self.order.len() >= self.capacity
                && let Some(oldest) = self.order.pop_front()
            {
                self.map.remove(&oldest);
            }

            self.map.insert(key.clone(), entry);
            self.order.push_back(key);
        }
    }

    /// Get a value by key.
    ///
    /// A hit refreshes the entry's LRU position; an expired entry reads as a
    /// miss and is evicted.
    pub fn get(&mut self, key: &K) -> Option<&V> {
        self.get_at(key, Instant::now())
    }

    fn get_at(&mut self, key: &K, now: Instant) -> Option<&V> {
        if let Some(entry) = self.map.get(key)
            && self.is_expired(entry, now)
        {
            self.remove(key);
            return None;
        }
        if self.map.contains_key(key) {
            self.order.retain(|k| k != key);
            self.order.push_back(key.clone());
        }
        self.map.get(key).map(|entry| &entry.value)
    }

    /// Check if the map contains a (non-expired) key
    pub fn contains_key(&self, key: &K) -> bool {
        self.contains_key_at(key, Instant::now())
    }

    fn contains_key_at(&self, key: &K, now: Instant) -> bool {
        self.map
            .get(key)
            .is_some_and(|entry| !self.is_expired(entry, now))
    }

    /// Remove a key-value pair
    pub fn remove(&mut self, key: &K) -> Option<V> {
        self.order.retain(|k| k != key);
        self.map.remove(key).map(|entry| entry.value)
    }

    /// Get the current size
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Clear all entries
    pub fn clear(&mut self) {
        self.map.clear();
        self.order.clear();
    }
}

/// Thread-safe cache wrapper
pub struct Cache<K, V> {
    inner: Mutex<LimitedSizeMap<K, V>>,
}

impl<K: Clone + Eq + Hash, V: Clone> Cache<K, V> {
    /// Create a new Cache with the given capacity
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: Mutex::new(LimitedSizeMap::new(capacity)),
        }
    }

    /// Create a new Cache whose entries expire after `ttl`
    pub fn with_ttl(capacity: usize, ttl: Duration) -> Self {
        Self {
            inner: Mutex::new(LimitedSizeMap::with_ttl(capacity, ttl)),
        }
    }

    /// Insert a key-value pair
    pub fn insert(&self, key: K, value: V) {
        self.inner.lock().insert(key, value);
    }

    /// Get a cloned value by key.
    ///
    /// Note: this cache stores owned values; returning references would require
    /// holding the mutex guard across call boundaries.
    pub fn get_cloned(&self, key: &K) -> Option<V> {
        self.inner.lock().get(key).cloned()
    }

    /// Check if the cache contains a key
    pub fn contains_key(&self, key: &K) -> bool {
        self.inner.lock().contains_key(key)
    }

    /// Remove a key-value pair
    pub fn remove(&self, key: &K) -> Option<V> {
        self.inner.lock().remove(key)
    }

    /// Get the current size
    pub fn len(&self) -> usize {
        self.inner.lock().len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.inner.lock().is_empty()
    }

    /// Clear all entries
    pub fn clear(&self) {
        self.inner.lock().clear();
    }
}

/// File cache for downloaded media
pub struct FileCache {
    cache_dir: PathBuf,
}

impl FileCache {
    /// Fallback extension when the caller-supplied one is unsafe.
    const FALLBACK_EXTENSION: &'static str = "bin";

    /// Create a new FileCache with the given directory
    pub fn new(cache_dir: PathBuf) -> std::io::Result<Self> {
        std::fs::create_dir_all(&cache_dir)?;
        Ok(Self { cache_dir })
    }

    /// Generate a cache key from a URL
    pub fn key_from_url(url: &str) -> String {
        cache_key(url)
    }

    /// Restrict extensions to `[A-Za-z0-9]{1,8}` so a caller-supplied value
    /// can never escape the cache directory (e.g. `../../etc/cron.d/x`).
    fn sanitize_extension(extension: &str) -> &str {
        let valid = !extension.is_empty()
            && extension.len() <= 8
            && extension.bytes().all(|b| b.is_ascii_alphanumeric());
        if valid {
            extension
        } else {
            Self::FALLBACK_EXTENSION
        }
    }

    /// Get the cached file path for a key
    pub fn get_path(&self, key: &str, extension: &str) -> PathBuf {
        let extension = Self::sanitize_extension(extension);
        self.cache_dir.join(format!("{}.{}", key, extension))
    }

    /// Check if a file is cached
    pub fn exists(&self, key: &str, extension: &str) -> bool {
        self.get_path(key, extension).exists()
    }

    /// Get the cached file if it exists
    pub fn get(&self, key: &str, extension: &str) -> Option<PathBuf> {
        let path = self.get_path(key, extension);
        if path.exists() { Some(path) } else { None }
    }

    /// Clean up old cache files (older than max_age_days).
    ///
    /// Subdirectories are skipped and a failed removal only logs a warning:
    /// one bad entry must never abort the whole sweep, or the cache would
    /// grow unbounded from that day on.
    pub fn cleanup(&self, max_age_days: u64) -> std::io::Result<usize> {
        let max_age = std::time::Duration::from_secs(max_age_days * 24 * 60 * 60);
        let now = std::time::SystemTime::now();
        let mut removed = 0;

        for entry in std::fs::read_dir(&self.cache_dir)? {
            let entry = entry?;
            let Ok(metadata) = entry.metadata() else {
                // Entry vanished between read_dir and stat; nothing to clean.
                continue;
            };

            // Only plain files are cache entries; remove_file on a directory
            // would fail (and stale directories are not ours to delete).
            if !metadata.is_file() {
                continue;
            }

            if let Ok(modified) = metadata.modified()
                && let Ok(age) = now.duration_since(modified)
                && age > max_age
            {
                match std::fs::remove_file(entry.path()) {
                    Ok(()) => removed += 1,
                    Err(e) => {
                        tracing::warn!("Failed to remove stale cache file {:?}: {e}", entry.path());
                    }
                }
            }
        }

        Ok(removed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type Map = LimitedSizeMap<&'static str, i32>;
    type Touch = fn(&mut Map);

    #[test]
    fn test_limited_size_map() {
        // Fill a, b, c; `touch` uses "a"; inserting "d" must then evict the
        // least recently used entry, "b".
        let cases: [(&str, Touch, i32); 2] = [
            (
                "get refreshes recency",
                |map| assert!(map.get(&"a").is_some()),
                1,
            ),
            ("re-insert refreshes recency", |map| map.insert("a", 10), 10),
        ];
        for (case, touch, a) in cases {
            let mut map = Map::new(3);
            map.insert("a", 1);
            map.insert("b", 2);
            map.insert("c", 3);
            touch(&mut map);
            map.insert("d", 4);

            assert_eq!(map.len(), 3, "{case}");
            assert_eq!(map.get(&"a"), Some(&a), "{case}");
            assert_eq!(map.get(&"b"), None, "{case}");
            assert_eq!(map.get(&"c"), Some(&3), "{case}");
            assert_eq!(map.get(&"d"), Some(&4), "{case}");
        }
    }

    #[test]
    fn cleanup_survives_stale_subdirectory() {
        let dir = tempfile::tempdir().unwrap();
        let cache = FileCache::new(dir.path().to_path_buf()).unwrap();
        let two_days_ago = std::time::SystemTime::now() - Duration::from_secs(2 * 24 * 60 * 60);
        let write_stale = |path: &std::path::Path| {
            std::fs::write(path, b"x").unwrap();
            std::fs::File::options()
                .write(true)
                .open(path)
                .unwrap()
                .set_modified(two_days_ago)
                .unwrap();
        };

        let stale_file = dir.path().join("old.bin");
        write_stale(&stale_file);
        let fresh_file = dir.path().join("new.bin");
        std::fs::write(&fresh_file, b"x").unwrap();

        // A subdirectory inside the cache root (e.g. dropped there by an
        // older release) must be skipped, not abort the sweep.
        let subdir = dir.path().join("emojis");
        std::fs::create_dir(&subdir).unwrap();
        write_stale(&subdir.join("1f600.png"));

        let removed = cache.cleanup(1).unwrap();
        assert_eq!(removed, 1);
        assert!(!stale_file.exists());
        assert!(fresh_file.exists());
        assert!(subdir.exists(), "subdirectory must survive cleanup");
        assert!(subdir.join("1f600.png").exists());
    }
    #[test]
    fn test_get_path_sanitizes_extension() {
        let dir = tempfile::tempdir().unwrap();
        let cache = FileCache::new(dir.path().to_path_buf()).unwrap();

        let ok = cache.get_path("key", "jpg");
        assert!(ok.ends_with("key.jpg"));

        // Traversal attempts, over-long and empty extensions fall back to .bin.
        for evil in ["../../etc/cron", "a/b", "", "verylongext", "jp g", "jp."] {
            let path = cache.get_path("key", evil);
            assert!(path.ends_with("key.bin"), "extension {:?}", evil);
            assert!(path.starts_with(dir.path()));
        }
    }
}
