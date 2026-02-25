use super::types::{AiChatMessage, AiChatResponse};
use async_trait::async_trait;
use serde_json::Value;

#[async_trait]
pub trait AIProvider: Send + Sync {
    fn name(&self) -> &str;
    fn validate_config(&self) -> Result<(), String>;
    async fn chat_once(&self, messages: Vec<AiChatMessage>) -> Result<AiChatResponse, String>;
}

pub fn parse_extra_headers(extra_json: Option<&str>) -> Vec<(String, String)> {
    let mut headers = Vec::new();
    let Some(raw) = extra_json else {
        return headers;
    };
    let Ok(v) = serde_json::from_str::<Value>(raw) else {
        return headers;
    };
    let Some(obj) = v.get("headers").and_then(|h| h.as_object()) else {
        return headers;
    };
    for (k, val) in obj {
        if let Some(text) = val.as_str() {
            headers.push((k.to_string(), text.to_string()));
        }
    }
    headers
}
