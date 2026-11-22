use std::path::{Path, PathBuf};

use anyhow::Context;
use karta_core::data::{Author, Content, ImageContent, ParseResult, VideoContent};
use serde_json::Value;

/// Read an image/video-cover fixture with local media paths, without network I/O.
pub fn local_post(path: &Path) -> anyhow::Result<ParseResult> {
    let value: Value = serde_json::from_slice(&std::fs::read(path)?)?;
    let platform = serde_json::from_value(value["platform"].clone())?;
    let mut result = ParseResult::new(platform);
    if let Some(author) = value.get("author").filter(|author| !author.is_null()) {
        let name = author["name"].as_str().context("author name is missing")?;
        let mut author_result = Author::new(name);
        if let Some(avatar) = author["avatar"].as_str() {
            anyhow::ensure!(Path::new(avatar).is_file(), "avatar must be a local file");
            author_result.avatar = Some(avatar.to_owned());
        }
        result.author = Some(author_result);
    }
    result.title = value["title"].as_str().map(str::to_owned);
    result.text = value["text"].as_str().map(str::to_owned);
    result.timestamp = value["timestamp"].as_i64();
    if let Some(extra) = value.get("extra") {
        result.extra = serde_json::from_value(extra.clone())?;
    }
    for content in value["contents"]
        .as_array()
        .context("contents is missing")?
    {
        let content = match content["type"].as_str() {
            Some("image") => {
                let path =
                    PathBuf::from(content["path"].as_str().context("image path is missing")?);
                anyhow::ensure!(path.is_file(), "fixture image must be a local file");
                Content::Image(ImageContent::new().with_path(path))
            }
            Some("video") => {
                let cover = content["cover"]
                    .as_str()
                    .context("video cover is missing")?;
                anyhow::ensure!(
                    Path::new(cover).is_file(),
                    "video cover must be a local file"
                );
                let mut video = VideoContent::new().with_cover(cover);
                video.duration = content["duration"].as_u64();
                Content::Video(video)
            }
            _ => anyhow::bail!("fixture supports images and video covers only"),
        };
        result.contents.push(content);
    }
    anyhow::ensure!(!result.contents.is_empty(), "fixture must contain media");
    Ok(result)
}
