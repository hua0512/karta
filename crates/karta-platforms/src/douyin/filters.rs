use karta_core::error::ParserError;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub(super) struct ContentFilter {
    #[serde(alias = "item_id")]
    pub aweme_id: Option<String>,
    pub filter_reason: Option<String>,
}

/// Owned by one parse, including its fallbacks. Never retained on the parser.
#[derive(Default)]
pub(super) struct DetailErrors {
    filter: Option<String>,
    last: Option<ParserError>,
}

impl DetailErrors {
    pub fn observe(&mut self, item_id: &str, filters: Option<&[ContentFilter]>) {
        let filters = filters.unwrap_or_default();
        let selected = filters
            .iter()
            .find(|f| f.aweme_id.as_deref() == Some(item_id))
            .or_else(|| (filters.len() == 1 && filters[0].aweme_id.is_none()).then(|| &filters[0]));
        if let Some(reason) = selected
            .and_then(|f| f.filter_reason.as_deref())
            .filter(|s| !s.trim().is_empty())
        {
            self.filter = Some(
                reason
                    .chars()
                    .filter(|c| !c.is_control())
                    .take(128)
                    .collect(),
            );
        }
    }

    pub fn record(&mut self, error: ParserError) {
        self.last = Some(error);
    }

    pub fn finish(self) -> ParserError {
        if let Some(reason) = self.filter {
            let message = match reason.as_str() {
                "story_25_filter" => "该日常作品已被服务端过滤，当前不可用",
                "author_secret" => "该作品为私密或受可见范围限制",
                "item_deleted" => "该作品已被删除",
                _ => "该作品被服务端过滤",
            };
            return ParserError::ContentNotFound(format!("{message} ({reason})"));
        }
        self.last.unwrap_or_else(|| {
            ParserError::ContentNotFound("Douyin detail returned no content".into())
        })
    }
}
