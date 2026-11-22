mod registration;
mod signing;
pub(super) use registration::RegistrationState;

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use karta_core::config::DouyinMobileConfig;
use karta_core::data::ParseResult;
use karta_core::error::{ParserError, Result};
use karta_core::http;
use wreq::header::{HeaderMap, HeaderValue};

use super::DouyinParser;
use super::api::ApiResponse;
use super::filters::DetailErrors;
use super::result::build_aweme_result;

const HOSTS: [&str; 2] = [
    "https://api5-normal-c-hl.amemv.com",
    "https://api3-normal-c-hl.amemv.com",
];
const DETAIL_PATH: &str = "/aweme/v1/aweme/detail/";

/// Serialize once, then sign those exact bytes. No web cookie is forwarded.
fn request(config: &DouyinMobileConfig, item_id: &str, time: u32) -> Result<(String, HeaderMap)> {
    for value in [
        &config.device_id,
        &config.install_id,
        &config.version_code,
        &config.update_version_code,
    ] {
        if !registration::positive_id(value) {
            return Err(ParserError::Parse(
                "douyin_mobile requires numeric device/install IDs and version codes".into(),
            ));
        }
    }
    if config.version_name.is_empty()
        || config.version_name.len() > 32
        || !config
            .version_name
            .bytes()
            .all(|c| c.is_ascii_digit() || c == b'.')
    {
        return Err(ParserError::Parse(
            "invalid douyin_mobile version_name".into(),
        ));
    }
    if item_id.is_empty() || !item_id.bytes().all(|b| b.is_ascii_digit()) {
        return Err(ParserError::Parse("invalid Douyin item ID".into()));
    }
    let cdid = format!(
        "{:x}",
        md5::compute(format!("{}:{}", config.device_id, config.install_id))
    );
    let cdid = format!(
        "{}-{}-{}-{}-{}",
        &cdid[..8],
        &cdid[8..12],
        &cdid[12..16],
        &cdid[16..20],
        &cdid[20..]
    );
    let openudid = format!("{:x}", md5::compute(&config.device_id));
    let time_string = time.to_string();
    let mut query = url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs([
            ("aweme_id", item_id),
            ("request_source", "0"),
            ("is_story", "1"),
            ("origin_type", "0"),
            ("aid", "1128"),
            ("version_code", config.version_code.as_str()),
            ("version_name", config.version_name.as_str()),
            ("update_version_code", config.update_version_code.as_str()),
            ("device_type", "SM-A5560"),
            ("device_brand", "samsung"),
            ("device_platform", "android"),
            ("os_version", "15"),
            ("os_api", "35"),
            ("device_id", config.device_id.as_str()),
            ("iid", config.install_id.as_str()),
            (
                "openudid",
                config.openudid.as_deref().unwrap_or(&openudid[..16]),
            ),
            ("cdid", config.cdid.as_deref().unwrap_or(cdid.as_str())),
            ("ts", time_string.as_str()),
        ])
        .finish();
    if let Some(clientudid) = config.clientudid.as_deref() {
        query = url::form_urlencoded::Serializer::new(query)
            .append_pair("clientudid", clientudid)
            .finish();
    }
    let ua = format!(
        "com.ss.android.ugc.aweme/{} (Linux; U; Android 15; zh_CN; SM-A5560; Build/AP3A.240905.015.A2)",
        config.version_code
    );
    let mut headers = HeaderMap::new();
    headers.insert(
        "user-agent",
        HeaderValue::from_str(&ua).map_err(|e| ParserError::Parse(e.to_string()))?,
    );
    headers.insert("accept", HeaderValue::from_static("application/json"));
    headers.insert("x-tt-request-tag", HeaderValue::from_static("t=0"));
    for (name, value) in signing::sign_get(&query, &config.device_id, &config.version_name, time) {
        headers.insert(
            name,
            HeaderValue::from_str(&value).map_err(|e| ParserError::Parse(e.to_string()))?,
        );
    }
    Ok((query, headers))
}

impl DouyinParser {
    pub(super) async fn parse_mobile_detail(
        &self,
        item_id: &str,
        errors: &mut DetailErrors,
    ) -> Result<ParseResult> {
        let config = self.mobile_identity().await?;
        let time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| ParserError::Parse(e.to_string()))?
            .as_secs();
        let time = u32::try_from(time).map_err(|e| ParserError::Parse(e.to_string()))?;
        let (query, headers) = request(&config, item_id, time)?;
        let mut last = None;
        for host in HOSTS {
            // Two hosts, one attempt each. The outer ParserEngine deadline still applies.
            let attempt = async {
                let response = self
                    .downloader
                    .client()
                    .get(format!("{host}{DETAIL_PATH}?{query}"))
                    .headers(headers.clone())
                    .timeout(Duration::from_secs(8))
                    .send()
                    .await?;
                http::check_status(&response)?;
                let data: ApiResponse = response.json().await?;
                let item = super::detail_item(data, item_id, errors)?;
                let mut result = build_aweme_result(&item);
                self.probe_cdn_ratios(&mut result).await;
                Ok(result)
            }
            .await;
            match attempt {
                Ok(result) => {
                    tracing::debug!(item_id, host, "Douyin: mobile detail succeeded");
                    return Ok(result);
                }
                Err(e) => {
                    last = Some(e);
                }
            }
        }
        Err(last.unwrap_or_else(|| {
            ParserError::ContentNotFound("Mobile detail returned no item".into())
        }))
    }
}
