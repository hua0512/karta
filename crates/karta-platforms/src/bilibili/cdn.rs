//! Bilibili CDN selection helpers.

use std::sync::LazyLock;

use karta_core::config::BiliCdnMode;
use url::Url;

/// CDN hostname patterns known to be slow.
const SLOW_CDN_PATTERNS: &[&str] = &[".mcdn.bilivideo.cn", "mountaintoys.cn", ".szbdyd.com"];

/// Akamai and `08c`/`08h` edges that are routinely unreachable from mainland
/// networks; Bilibili's own player swaps them for the Huawei mirror.
static MIRROR_HUAWEI_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(
        r"^(?:upos-sz-mirror08[ch]\.bilivideo\.com|upos-hz-mirrorakam\.akamaized\.net)$",
    )
    .expect("valid huawei mirror regex")
});

/// Alibaba-hosted edges that map onto the `alio1` mirror.
static MIRROR_ALI_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(
        r"^(?:upos-sz-estgoss\.bilivideo\.com|upos-sz-mirrorali(?:ov|b)?\.bilivideo\.com)$",
    )
    .expect("valid ali mirror regex")
});

/// Regional P2P nodes such as `cn-jsnt-ct-01-07.bilivideo.com`.
static CN_NODE_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"^cn(?:-[a-z]+){2}(?:-\d{2}){2}\.bilivideo\.com$")
        .expect("valid cn node regex")
});

/// Check if a URL's hostname matches a slow CDN pattern.
#[inline]
fn is_slow_cdn(url: &str) -> bool {
    extract_hostname(url)
        .map(|h| SLOW_CDN_PATTERNS.iter().any(|p| h.contains(p)))
        .unwrap_or(false)
}

/// Check if a URL's hostname matches a fast CDN pattern.
#[inline]
fn is_fast_cdn(url: &str) -> bool {
    let Some(host) = extract_hostname(url) else {
        return false;
    };

    if host.starts_with("cn-") && host.ends_with(".bilivideo.com") {
        return true;
    }

    (host.contains("upos-sz-mirror") || host.contains("upos-hz-mirror"))
        && host.ends_with(".bilivideo.com")
}

/// Extract hostname from a URL string.
#[inline]
fn extract_hostname(url: &str) -> Option<&str> {
    let after_scheme = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    Some(after_scheme.split('/').next().unwrap_or(after_scheme))
}

/// Preference rank: fast edges first, unclassified next, known-slow last.
#[inline]
fn cdn_rank(url: &str) -> u8 {
    if is_fast_cdn(url) {
        0
    } else if is_slow_cdn(url) {
        2
    } else {
        1
    }
}

/// Rewrite a host that is known to route badly onto its CDN mirror.
///
/// Returns `None` when the host has no known mapping, so callers can tell a
/// rewrite apart from a pass-through.
fn mirror_url(raw: &str) -> Option<String> {
    let mut url = Url::parse(raw).ok()?;
    let host = url.host_str()?.to_ascii_lowercase();

    let replacement = if MIRROR_HUAWEI_RE.is_match(&host) {
        "upos-sz-mirrorhwo1.bilivideo.com".to_string()
    } else if MIRROR_ALI_RE.is_match(&host) {
        "upos-sz-mirroralio1.bilivideo.com".to_string()
    } else if host.ends_with(".mcdn.bilivideo.cn") || CN_NODE_RE.is_match(&host) {
        "upos-sz-mirrorcoso1.bilivideo.com".to_string()
    } else if host.ends_with(".szbdyd.com") {
        // These edges carry their own origin in the query string.
        url.query_pairs()
            .find(|(key, _)| key == "xy_usource")
            .map(|(_, value)| value.into_owned())
            .filter(|origin| !origin.is_empty())?
    } else {
        return None;
    };

    if replacement == host {
        return None;
    }

    url.set_host(Some(&replacement)).ok()?;
    // The mirrors serve HTTPS on the default port; clear any inherited one.
    let _ = url.set_port(None);
    Some(url.to_string())
}

/// Rank the primary and backup CDN URLs while preserving every fallback.
///
/// The first entry is what the downloader tries first; the rest are retried in
/// order when a transfer fails, so no candidate is ever discarded.
pub(super) fn ordered_cdn_urls(
    base_url: &str,
    backup_urls: &[String],
    mode: BiliCdnMode,
) -> Vec<String> {
    let raw: Vec<&str> = std::iter::once(base_url)
        .chain(backup_urls.iter().map(String::as_str))
        .collect();

    let candidates: Vec<String> = match mode {
        // Trust the API's assignment; backups stay available as retries.
        BiliCdnMode::Original => raw.iter().map(|url| (*url).to_owned()).collect(),
        BiliCdnMode::Mirror => {
            let mut ordered: Vec<String> = raw.iter().filter_map(|url| mirror_url(url)).collect();
            ordered.extend(raw.iter().map(|url| (*url).to_owned()));
            ordered
        }
        BiliCdnMode::Auto => {
            let mut ranked = raw.clone();
            // Stable sort keeps the API's order within the same CDN class.
            ranked.sort_by_key(|url| cdn_rank(url));
            let mut ordered: Vec<String> = ranked.iter().map(|url| (*url).to_owned()).collect();

            // Nothing to promote: rewriting is the only way off a slow edge.
            if raw.iter().all(|url| is_slow_cdn(url))
                && let Some(mirror) = mirror_url(base_url)
            {
                ordered.insert(0, mirror);
            }
            ordered
        }
    };

    let candidates = dedup_preserving_order(candidates);

    if let Some(selected) = candidates.first()
        && selected != base_url
    {
        tracing::debug!(
            original = extract_hostname(base_url).unwrap_or("?"),
            selected = extract_hostname(selected).unwrap_or("?"),
            ?mode,
            "Selected preferred Bilibili CDN"
        );
    }

    candidates
}

/// Drop repeated URLs while keeping the first occurrence of each.
///
/// Ranking can interleave equal-rank duplicates, so adjacent-only dedup would
/// leave the downloader retrying a URL that just failed.
fn dedup_preserving_order(urls: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    urls.into_iter()
        .filter(|url| seen.insert(url.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_slow_candidates_are_rescued_by_a_mirror_rewrite() {
        let slow1 = "https://xy1.mcdn.bilivideo.cn/video.m4s";
        let slow2 = "https://xy2.mcdn.bilivideo.cn/video.m4s".to_string();
        let backups = vec![slow2.clone()];

        let ordered = ordered_cdn_urls(slow1, &backups, BiliCdnMode::Auto);

        assert_eq!(
            ordered[0],
            "https://upos-sz-mirrorcoso1.bilivideo.com/video.m4s"
        );
        // The originals stay available as retries.
        assert_eq!(&ordered[1..], &[slow1.to_string(), slow2]);
    }

    /// Auto mode: fast edges first, unclassified next, known-slow last, API
    /// order within a class, and every unique URL kept as a fallback.
    #[test]
    fn ordered_urls_keep_all_unique_fallbacks_by_rank() {
        const MCDN: &str = "https://xy1.mcdn.bilivideo.cn/video.m4s";
        const NORMAL: &str = "https://normal.example.com/video.m4s";
        const CN_NODE: &str = "https://cn-jsnt-ct-01-07.bilivideo.com/video.m4s";
        const MIRROR_COS: &str = "https://upos-sz-mirrorcos.bilivideo.com/video.m4s";

        let cases: [(&str, &str, &[&str], &[&str]); 2] = [
            (
                "mixed classes",
                MCDN,
                &[NORMAL, CN_NODE, NORMAL],
                &[CN_NODE, NORMAL, MCDN],
            ),
            (
                "same class keeps API order",
                MIRROR_COS,
                &[CN_NODE],
                &[MIRROR_COS, CN_NODE],
            ),
        ];

        for (name, base, backups, expected) in cases {
            let backups: Vec<String> = backups.iter().map(|url| url.to_string()).collect();
            assert_eq!(
                ordered_cdn_urls(base, &backups, BiliCdnMode::Auto),
                expected,
                "{name}"
            );
        }
    }

    #[test]
    fn mirror_rewrites_cover_every_known_host_family() {
        assert_eq!(
            mirror_url("https://upos-hz-mirrorakam.akamaized.net/v.m4s").as_deref(),
            Some("https://upos-sz-mirrorhwo1.bilivideo.com/v.m4s")
        );
        assert_eq!(
            mirror_url("https://upos-sz-mirroraliov.bilivideo.com/v.m4s").as_deref(),
            Some("https://upos-sz-mirroralio1.bilivideo.com/v.m4s")
        );
        assert_eq!(
            mirror_url("https://cn-jsnt-ct-01-07.bilivideo.com/v.m4s").as_deref(),
            Some("https://upos-sz-mirrorcoso1.bilivideo.com/v.m4s")
        );
        // `.szbdyd.com` edges name their origin in the query string.
        assert_eq!(
            mirror_url("https://node.szbdyd.com/v.m4s?xy_usource=upos-sz-mirrorcos.bilivideo.com")
                .as_deref(),
            Some(
                "https://upos-sz-mirrorcos.bilivideo.com/v.m4s?xy_usource=upos-sz-mirrorcos.bilivideo.com"
            )
        );
        // Unknown hosts are left alone.
        assert_eq!(mirror_url("https://normal.example.com/v.m4s"), None);
        assert_eq!(mirror_url("https://node.szbdyd.com/v.m4s"), None);
    }
}
