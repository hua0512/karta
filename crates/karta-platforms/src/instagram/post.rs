//! Instagram post/reel REST, GraphQL, and embed fallback flow.

use karta_core::data::ParseResult;
use karta_core::error::{ParserError, Result};
use karta_core::http;

use super::InstagramParser;
use super::api::{ApiResponse, GraphqlResponse};
use super::embed::{extract_context_json, extract_gql_data};
use super::result;
use super::shortcode::shortcode_to_pk;

impl InstagramParser {
    pub(super) async fn parse_post(&self, shortcode: &str) -> Result<ParseResult> {
        let url = format!("https://www.instagram.com/p/{}/", shortcode);

        // Tier 1: REST API (most reliable with cookie)
        if self.cookie.is_some() {
            match self.parse_via_api(shortcode, &url).await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    tracing::debug!("Instagram REST API failed ({}), trying GraphQL", e);
                }
            }
        }

        // Tier 2: GraphQL (works with or without cookie)
        match self.parse_via_graphql(shortcode, &url).await {
            Ok(result) => return Ok(result),
            Err(e) => {
                tracing::debug!("Instagram GraphQL failed ({}), trying embed", e);
            }
        }

        // Tier 3: Embed page (unauthenticated fallback)
        self.parse_via_embed(shortcode, &url).await
    }

    async fn parse_via_embed(&self, shortcode: &str, canonical_url: &str) -> Result<ParseResult> {
        let embed_url = format!("{}/p/{}/embed/captioned/", super::EMBED_BASE, shortcode);

        let client = self.downloader.client();
        let response = client
            .get(&embed_url)
            .headers(self.headers())
            .send()
            .await?;
        http::check_status(&response)?;

        let html = response.text().await?;
        self.parse_embed_html(&html, canonical_url)
    }

    pub(super) fn parse_embed_html(&self, html: &str, canonical_url: &str) -> Result<ParseResult> {
        if let Some(wrapper) = extract_context_json(html) {
            return result::build_result_from_context(wrapper.context, canonical_url);
        }

        if let Some(gql) = extract_gql_data(html)
            && let Some(media) = gql.shortcode_media
        {
            return Ok(result::build_result_from_media(&media, canonical_url));
        }

        Err(ParserError::ContentNotFound(
            "Could not extract Instagram post data from embed page".to_string(),
        ))
    }

    async fn parse_via_api(&self, shortcode: &str, canonical_url: &str) -> Result<ParseResult> {
        let pk = shortcode_to_pk(shortcode).ok_or_else(|| {
            ParserError::Parse(format!("Invalid Instagram shortcode: {}", shortcode))
        })?;

        let url = format!("{}/media/{}/info/", super::API_BASE, pk);
        let client = self.downloader.client();
        let response = client.get(&url).headers(self.rest_headers()).send().await?;
        self.update_www_claim(&response);
        http::check_status(&response)?;

        let data: ApiResponse = response.json().await.map_err(|e| {
            ParserError::Parse(format!("Failed to parse Instagram API response: {}", e))
        })?;

        let item = data.items.first().ok_or_else(|| {
            ParserError::ContentNotFound("Instagram API returned no media items".to_string())
        })?;

        Ok(result::build_result_from_api_item(item, canonical_url))
    }

    async fn parse_via_graphql(&self, shortcode: &str, canonical_url: &str) -> Result<ParseResult> {
        let client = self.downloader.client();

        // gallery-dl uses query_hash via GET (more stable than doc_id which rotates)
        let variables = serde_json::json!({
            "shortcode": shortcode,
            "child_comment_count": 3,
            "fetch_comment_count": 40,
            "parent_comment_count": 24,
            "has_threaded_comments": true,
        })
        .to_string();

        let response = client
            .get(super::GRAPHQL_API)
            .headers(self.graphql_headers())
            .query(&[
                ("query_hash", super::POST_QUERY_HASH),
                ("variables", &variables),
            ])
            .send()
            .await?;

        self.update_www_claim(&response);
        http::check_status(&response)?;

        let data: GraphqlResponse = response.json().await.map_err(|e| {
            ParserError::Parse(format!("Failed to parse Instagram GraphQL response: {}", e))
        })?;

        let media = data.data.and_then(|d| d.shortcode_media).ok_or_else(|| {
            ParserError::ContentNotFound("Post not found via GraphQL".to_string())
        })?;

        Ok(result::build_result_from_media(&media, canonical_url))
    }
}
