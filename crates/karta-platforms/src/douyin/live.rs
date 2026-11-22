use karta_core::data::ParseResult;
use karta_core::error::{ParserError, Result};
use karta_core::http;

use super::DouyinParser;
use super::api::{LiveReflowResponse, LiveResponse};
use super::result::build_live_result;
use super::token::gen_verify_fp;

/// Live stream enter room API.
const API_LIVE_ENTER: &str = "https://live.douyin.com/webcast/room/web/enter/";
const API_LIVE_ENTER_PARAMS: &str = "device_platform=webapp&aid=6383&channel=channel_pc_web&pc_client_type=1&version_code=190500&version_name=19.5.0&cookie_enabled=true&screen_width=1920&screen_height=1080&browser_language=zh-CN&browser_platform=Win32&browser_name=Firefox&browser_version=124.0&browser_online=true&engine_name=Gecko&engine_version=122.0.0.0&os_name=Windows&os_version=10&cpu_core_num=12&device_memory=8&platform=PC";

/// Live stream type 2 — reflow info (webcast.amemv.com).
const API_LIVE_REFLOW: &str = "https://webcast.amemv.com/webcast/room/reflow/info/";
const API_LIVE_REFLOW_PARAMS: &str =
    "type_id=0&live_id=1&sec_user_id=&version_code=99.99.99&app_id=1128";

impl DouyinParser {
    pub(super) async fn parse_live(&self, room_id: &str) -> Result<ParseResult> {
        let query = format!(
            "{}&web_rid={}&room_id_str={}",
            API_LIVE_ENTER_PARAMS, room_id, room_id
        );
        let url = Self::sign_url(API_LIVE_ENTER, &query);

        let client = self.downloader.client();
        let response = client.get(&url).headers(self.api_headers()).send().await?;
        http::check_status(&response)?;
        let data: LiveResponse = response
            .json()
            .await
            .map_err(|e| ParserError::Parse(format!("Failed to parse live response: {}", e)))?;

        let room = data
            .data
            .and_then(|d| d.data.and_then(|v| v.into_iter().next()))
            .ok_or_else(|| ParserError::ContentNotFound("Live room data not found".into()))?;

        Ok(build_live_result(&room))
    }

    /// Live stream type 2 — webcast.amemv.com reflow API.
    ///
    /// Uses `DY_LIVE_INFO_2` with `verifyFp` and `msToken` parameters.
    /// The response structure differs: `data.room` instead of `data.data[0]`.
    pub(super) async fn parse_live_reflow(
        &self,
        room_id: &str,
        ttwid: &str,
    ) -> Result<ParseResult> {
        let verify_fp = gen_verify_fp();
        let url = format!(
            "{}?{}&room_id={}&verifyFp={}&msToken={}",
            API_LIVE_REFLOW, API_LIVE_REFLOW_PARAMS, room_id, verify_fp, ttwid,
        );

        let client = self.downloader.client();
        let response = client.get(&url).headers(self.api_headers()).send().await?;
        http::check_status(&response)?;
        let data: LiveReflowResponse = response.json().await.map_err(|e| {
            ParserError::Parse(format!("Failed to parse live reflow response: {}", e))
        })?;

        let room = data.data.and_then(|d| d.room).ok_or_else(|| {
            ParserError::ContentNotFound("Live reflow room data not found".into())
        })?;

        Ok(build_live_result(&room))
    }
}

/// Pick the `ttwid` value out of the `Set-Cookie` headers collected across
/// all redirect hops (needed for live type 2).
pub(super) fn ttwid_from_set_cookies(set_cookies: &[String]) -> String {
    set_cookies
        .iter()
        .find_map(|cookie| {
            cookie
                .split(';')
                .next()
                .and_then(|kv| kv.trim().strip_prefix("ttwid="))
                .map(str::to_string)
        })
        .unwrap_or_default()
}
