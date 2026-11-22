use async_trait::async_trait;
use wreq::header::HeaderMap;

use karta_core::Platform;
use karta_core::constants::DEFAULT_USER_AGENT;
use karta_core::data::ParseResult;
use karta_core::download::SharedDownloader;
use karta_core::error::{ParserError, Result};
use karta_core::http;
use karta_core::parser::{MatchResult, Parser, RegisteredPattern};

mod auth;
mod game;
mod media;
mod patterns;
mod result;

use auth::build_auth_params;
use game::GameType;

const API_BBS_LINK: &str = "https://api.xiaoheihe.cn/bbs/app/link/tree";

/// Xiaoheihe (小黑盒) parser.
pub struct XiaoheiheParser {
    downloader: SharedDownloader,
    cookie: Option<String>,
    patterns: Vec<RegisteredPattern>,
}

impl XiaoheiheParser {
    pub fn new(downloader: SharedDownloader, cookie: Option<String>) -> Self {
        Self {
            downloader,
            cookie,
            patterns: patterns::registered_patterns(),
        }
    }

    fn get_headers(&self) -> HeaderMap {
        http::headers_with_referer_and_cookie(
            DEFAULT_USER_AGENT,
            "https://xiaoheihe.cn/",
            self.cookie.as_deref(),
        )
    }

    async fn parse_bbs_post(&self, link_id: &str) -> Result<ParseResult> {
        let client = self.downloader.client();
        let api_path = "bbs/app/link/tree";

        let mut params = build_auth_params(api_path);
        params.push(("link_id".into(), link_id.into()));
        params.push(("limit".into(), "20".into()));
        params.push(("web_version".into(), "2.5".into()));
        params.push(("x_client_type".into(), "web".into()));
        params.push(("x_app".into(), "heybox_website".into()));
        params.push(("x_os_type".into(), "Android".into()));

        let response = client
            .get(API_BBS_LINK)
            .headers(self.get_headers())
            .query(&params)
            .send()
            .await?;

        http::check_status(&response)?;

        let data: serde_json::Value = response.json().await?;
        tracing::debug!("Xiaoheihe BBS API response: {}", data);
        ensure_api_ok(&data)?;

        let link = data
            .pointer("/result/link")
            .ok_or_else(|| ParserError::ContentNotFound("BBS link data not found".into()))?;

        Ok(result::build_bbs_result(link, link_id))
    }

    async fn parse_game(&self, app_id: &str, game_type: GameType) -> Result<ParseResult> {
        let client = self.downloader.client();
        let api_path = game_type.api_path();

        let mut params = build_auth_params(api_path);
        params.push((game_type.id_param_name().into(), app_id.into()));

        let response = client
            .get(game_type.api_url())
            .headers(self.get_headers())
            .query(&params)
            .send()
            .await?;

        http::check_status(&response)?;

        let data: serde_json::Value = response.json().await?;
        ensure_api_ok(&data)?;

        let game = data
            .get("result")
            .ok_or_else(|| ParserError::ContentNotFound("Game data not found".into()))?;

        Ok(result::build_game_result(game, game_type))
    }

    async fn dispatch(&self, match_result: MatchResult<'_>) -> Result<ParseResult> {
        let id = match_result.group(1).unwrap_or("");
        match match_result.pattern {
            "bbs_link" | "bbs_share" => self.parse_bbs_post(id).await,
            "game_pc" | "game_share_pc" => self.parse_game(id, GameType::Pc).await,
            "game_console" | "game_share_console" => self.parse_game(id, GameType::Console).await,
            "game_mobile" | "game_share_mobile" => self.parse_game(id, GameType::Mobile).await,
            _ => Err(ParserError::PatternNotMatched(format!(
                "Unknown pattern: {}",
                match_result.pattern
            ))),
        }
    }
}

fn ensure_api_ok(data: &serde_json::Value) -> Result<()> {
    if data.get("status").and_then(|value| value.as_str()) == Some("ok") {
        return Ok(());
    }

    let message = data
        .get("msg")
        .and_then(|value| value.as_str())
        .unwrap_or("Unknown error");
    Err(ParserError::Api {
        code: -1,
        message: message.to_string(),
    })
}

#[async_trait]
impl Parser for XiaoheiheParser {
    fn platform(&self) -> Platform {
        Platform::Xiaoheihe
    }

    fn patterns(&self) -> &[RegisteredPattern] {
        &self.patterns
    }

    async fn parse(&self, match_result: MatchResult<'_>) -> Result<ParseResult> {
        tracing::debug!(
            pattern = match_result.pattern,
            matched = match_result.full_match,
            "Xiaoheihe: parsing"
        );
        self.dispatch(match_result).await
    }
}
