use std::path::{Path, PathBuf};

use crate::cache::cache_key;
use crate::data::ParseResult;
use crate::error::RenderResult;

/// Build the cache-key input for a rendered card.
///
/// The URL uniquely identifies a post; URL-less results (e.g. `append_url`
/// off) fall back to a platform + title + timestamp composite so concurrent
/// renders of different posts can never overwrite each other's PNG.
fn card_key_input(result: &ParseResult) -> String {
    match &result.url {
        Some(url) => url.clone(),
        None => format!(
            "{}|{}|{}",
            result.platform.slug(),
            result.title.as_deref().unwrap_or(""),
            result.timestamp.unwrap_or_default(),
        ),
    }
}

/// Save rendered PNG to the cache directory.
pub(crate) async fn save_png_to_cache(
    data: &[u8],
    result: &ParseResult,
    data_path: &Path,
) -> RenderResult<PathBuf> {
    let cache_key = cache_key(&card_key_input(result));
    let cache_dir = crate::config::Config::cache_dir(data_path);
    let output_path = cache_dir.join(format!("{cache_key}_card.png"));

    if let Some(parent) = output_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    tokio::fs::write(&output_path, data).await?;

    Ok(output_path)
}
