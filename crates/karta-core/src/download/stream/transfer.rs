use std::path::{Path, PathBuf};

use futures::StreamExt;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use wreq::header::{CONTENT_RANGE, HeaderMap, USER_AGENT};
use wreq::{Client, Response, StatusCode};

use crate::cache::FileCache;
use crate::constants::DEFAULT_USER_AGENT;
use crate::error::DownloadError;

use super::ProgressCallback;
use super::single_flight::SingleFlight;

/// Append `.part` to a final cache path; partial data lives here until it is
/// renamed into place. `FileCache` extensions are sanitized to `[A-Za-z0-9]`,
/// so a `.part` path can never collide with a real cache entry.
pub(super) fn part_path(final_path: &Path) -> PathBuf {
    let mut path = final_path.as_os_str().to_os_string();
    path.push(".part");
    PathBuf::from(path)
}

/// Promote `part` to `output` when `result` is `Ok`; on any failure remove the
/// partial file so `FileCache::get` (which treats existence as a hit) can
/// never serve it.
pub(super) async fn finalize_part(
    result: Result<(), DownloadError>,
    part: &Path,
    output: &Path,
) -> Result<PathBuf, DownloadError> {
    match result {
        Ok(()) => match tokio::fs::rename(part, output).await {
            Ok(()) => Ok(output.to_path_buf()),
            Err(e) => {
                let _ = tokio::fs::remove_file(part).await;
                Err(e.into())
            }
        },
        Err(e) => {
            let _ = tokio::fs::remove_file(part).await;
            Err(e)
        }
    }
}

pub(super) struct DownloadTransfer<'a> {
    client: &'a Client,
    file_cache: &'a FileCache,
    max_size: u64,
    flights: &'a SingleFlight,
}

impl<'a> DownloadTransfer<'a> {
    pub(super) fn new(
        client: &'a Client,
        file_cache: &'a FileCache,
        max_size: u64,
        flights: &'a SingleFlight,
    ) -> Self {
        Self {
            client,
            file_cache,
            max_size,
            flights,
        }
    }

    pub(super) async fn download_to_cache(
        &self,
        url: &str,
        extension: &str,
        headers: Option<HeaderMap>,
        progress: Option<ProgressCallback>,
    ) -> Result<PathBuf, DownloadError> {
        let cache_key = FileCache::key_from_url(url);
        let output_path = self.file_cache.get_path(&cache_key, extension);

        // Single-flight: one task downloads a given cache entry while
        // concurrent requests wait here, then re-check the cache below.
        let _flight = self.flights.acquire(&output_path.to_string_lossy()).await;

        if let Some(cached_path) = self.file_cache.get(&cache_key, extension) {
            tracing::debug!("Cache hit for {}", url);
            return Ok(cached_path);
        }

        if let Some(parent) = output_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let part = part_path(&output_path);
        let result = async {
            let response = self.send_download_request(url, headers).await?;
            self.stream_response_to_file(response, &part, progress)
                .await
        }
        .await;

        finalize_part(result, &part, &output_path).await
    }

    async fn send_download_request(
        &self,
        url: &str,
        headers: Option<HeaderMap>,
    ) -> Result<Response, DownloadError> {
        let mut request = self.client.get(url);
        if let Some(headers) = headers {
            request = request.headers(headers);
        } else {
            request = request.header(USER_AGENT, DEFAULT_USER_AGENT);
        }
        Ok(request.send().await?.error_for_status()?)
    }

    async fn stream_response_to_file(
        &self,
        response: Response,
        output_path: &Path,
        progress: Option<ProgressCallback>,
    ) -> Result<(), DownloadError> {
        let expected_size = self.expected_response_size(&response)?;
        if let Some(size) = expected_size {
            self.validate_content_length(size)?;
        }

        let downloaded = self
            .write_response_stream(response, output_path, expected_size, progress)
            .await?;

        // The partial file is removed by `finalize_part` on any error.
        if downloaded == 0 {
            return Err(DownloadError::ZeroSize);
        }
        if let Some(expected) = expected_size
            && downloaded != expected
        {
            return Err(DownloadError::Incomplete(format!(
                "downloaded {downloaded} of {expected} bytes"
            )));
        }

        tracing::debug!("Downloaded {} bytes to {:?}", downloaded, output_path);
        Ok(())
    }

    fn expected_response_size(&self, response: &Response) -> Result<Option<u64>, DownloadError> {
        if response.status() != StatusCode::PARTIAL_CONTENT {
            return Ok(response.content_length());
        }

        let raw_range = response
            .headers()
            .get(CONTENT_RANGE)
            .ok_or_else(|| {
                DownloadError::Incomplete("206 response missing Content-Range header".to_string())
            })?
            .to_str()
            .map_err(|_| {
                DownloadError::Incomplete(
                    "206 response has non-UTF-8 Content-Range header".to_string(),
                )
            })?;

        let total = parse_complete_content_range(raw_range).ok_or_else(|| {
            DownloadError::Incomplete(format!("incomplete Content-Range: {raw_range}"))
        })?;

        if let Some(content_length) = response.content_length()
            && content_length != total
        {
            return Err(DownloadError::Incomplete(format!(
                "206 Content-Length {content_length} mismatches Content-Range total {total}"
            )));
        }

        Ok(Some(total))
    }

    async fn write_response_stream(
        &self,
        response: Response,
        output_path: &Path,
        total_size: Option<u64>,
        progress: Option<ProgressCallback>,
    ) -> Result<u64, DownloadError> {
        let mut file = File::create(output_path).await?;
        let mut stream = response.bytes_stream();
        let mut downloaded = 0;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(DownloadError::Http)?;
            downloaded += chunk.len() as u64;

            if downloaded > self.max_size {
                return Err(DownloadError::FileTooLarge {
                    size: downloaded,
                    limit: self.max_size,
                });
            }

            file.write_all(&chunk).await?;

            if let Some(callback) = progress.as_ref() {
                callback(downloaded, total_size);
            }
        }

        file.flush().await?;
        Ok(downloaded)
    }

    fn validate_content_length(&self, content_length: u64) -> Result<(), DownloadError> {
        if content_length == 0 {
            return Err(DownloadError::ZeroSize);
        }
        if content_length > self.max_size {
            return Err(DownloadError::FileTooLarge {
                size: content_length,
                limit: self.max_size,
            });
        }
        Ok(())
    }
}

/// Parse an HTTP `Content-Range` value and return the total byte size only when
/// it describes the entire resource from byte 0 (`bytes 0-(total-1)/total`).
fn parse_complete_content_range(value: &str) -> Option<u64> {
    let trimmed = value.trim_ascii();
    let prefix = trimmed.get(..5)?;
    if !prefix.eq_ignore_ascii_case("bytes") {
        return None;
    }
    let after_unit = &trimmed[5..];
    if !after_unit.starts_with([' ', '\t']) {
        return None;
    }

    let spec = after_unit.trim_ascii_start();
    let (range, total_str) = spec.split_once('/')?;
    let (start_str, end_str) = range.split_once('-')?;

    let start = parse_ascii_u64(start_str)?;
    let end = parse_ascii_u64(end_str)?;
    let total = parse_ascii_u64(total_str)?;

    (total > 0 && start == 0 && end.checked_add(1) == Some(total)).then_some(total)
}

fn parse_ascii_u64(value: &str) -> Option<u64> {
    if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    value.parse::<u64>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn complete_content_range_parsing() {
        let cases = [
            ("bytes 0-4095/4096", Some(4096)),
            ("BYTES\t0-0/1", Some(1)),
            ("  bytes   0-9/10  ", Some(10)),
            ("bytes 0-1023/4096", None),
            ("bytes 1-4095/4096", None),
            ("bytes 0-4095/*", None),
            ("bytes 0-0/0", None),
            ("bytes 0-18446744073709551615/18446744073709551615", None),
            ("items 0-9/10", None),
            ("bytes0-9/10", None),
            ("bytes +0-9/10", None),
        ];

        for (raw, expected) in cases {
            assert_eq!(parse_complete_content_range(raw), expected, "{raw:?}");
        }
    }
}
