use super::provider::{parse_extra_headers, AIProvider};
use super::types::{AiChatMessage, AiChatResponse, AiUsage};
use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct OpenAICompatProvider {
    pub name: String,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub temperature: f32,
    pub max_tokens: i64,
    pub extra_json: Option<String>,
}

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<AiChatMessage>,
    temperature: f32,
    max_tokens: i64,
    stream: bool,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    model: Option<String>,
    choices: Vec<ChatChoice>,
    usage: Option<UsageResponse>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatChoiceMessage,
}

#[derive(Debug, Deserialize)]
struct ChatChoiceMessage {
    content: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct UsageResponse {
    prompt_tokens: Option<i64>,
    completion_tokens: Option<i64>,
    total_tokens: Option<i64>,
}

#[async_trait]
impl AIProvider for OpenAICompatProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn validate_config(&self) -> Result<(), String> {
        if self.base_url.trim().is_empty() {
            return Err("Provider baseUrl is required".to_string());
        }
        if self.api_key.trim().is_empty() {
            return Err("Provider apiKey is required".to_string());
        }
        if self.model.trim().is_empty() {
            return Err("Provider model is required".to_string());
        }
        Ok(())
    }

    async fn chat_once(&self, messages: Vec<AiChatMessage>) -> Result<AiChatResponse, String> {
        self.validate_config()?;

        let endpoint = format!(
            "{}/chat/completions",
            self.base_url.trim_end_matches('/'),
        );

        let mut headers = HeaderMap::new();
        let auth = format!("Bearer {}", self.api_key);
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&auth).map_err(|e| e.to_string())?,
        );
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        for (k, v) in parse_extra_headers(self.extra_json.as_deref()) {
            let Ok(name) = HeaderName::from_bytes(k.as_bytes()) else {
                continue;
            };
            let Ok(value) = HeaderValue::from_str(&v) else {
                continue;
            };
            headers.insert(name, value);
        }

        // Normalize roles to maximize compatibility across OpenAI-like providers.
        let normalized_messages = messages
            .into_iter()
            .map(|m| {
                let role = match m.role.as_str() {
                    "system" | "user" | "assistant" | "tool" => m.role,
                    "developer" => "system".to_string(),
                    _ => "user".to_string(),
                };
                AiChatMessage {
                    role,
                    content: m.content,
                }
            })
            .collect();

        let req = ChatRequest {
            model: self.model.clone(),
            messages: normalized_messages,
            temperature: self.temperature,
            max_tokens: self.max_tokens,
            stream: false,
        };

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .map_err(|e| e.to_string())?;

        let resp = client
            .post(endpoint)
            .headers(headers)
            .json(&req)
            .send()
            .await
            .map_err(|e| format!("AI request failed: {e}"))?;

        let status = resp.status();
        let raw_text = resp
            .text()
            .await
            .map_err(|e| format!("AI response read failed: {e}"))?;

        if !status.is_success() {
            return Err(format!("AI provider returned {status}: {raw_text}"));
        }

        let decoded: ChatResponse = serde_json::from_str(&raw_text)
            .map_err(|e| format!("AI response parse failed: {e}; raw={raw_text}"))?;

        let choice = decoded
            .choices
            .first()
            .ok_or_else(|| "AI response has no choices".to_string())?;

        let content = match &choice.message.content {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Array(arr) => arr
                .iter()
                .filter_map(|item| item.get("text").and_then(|x| x.as_str()))
                .collect::<Vec<_>>()
                .join("\n"),
            other => other.to_string(),
        };

        let usage = decoded.usage.map(|u| AiUsage {
            prompt_tokens: u.prompt_tokens,
            completion_tokens: u.completion_tokens,
            total_tokens: u.total_tokens,
        });

        Ok(AiChatResponse {
            content,
            model: decoded.model.unwrap_or_else(|| self.model.clone()),
            usage,
        })
    }
}

impl OpenAICompatProvider {
    pub async fn chat_stream<F>(
        &self,
        messages: Vec<AiChatMessage>,
        mut on_chunk: F,
    ) -> Result<AiChatResponse, String>
    where
        F: FnMut(&str),
    {
        self.validate_config()?;

        let endpoint = format!(
            "{}/chat/completions",
            self.base_url.trim_end_matches('/'),
        );

        let mut headers = HeaderMap::new();
        let auth = format!("Bearer {}", self.api_key);
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&auth).map_err(|e| e.to_string())?,
        );
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        for (k, v) in parse_extra_headers(self.extra_json.as_deref()) {
            let Ok(name) = HeaderName::from_bytes(k.as_bytes()) else {
                continue;
            };
            let Ok(value) = HeaderValue::from_str(&v) else {
                continue;
            };
            headers.insert(name, value);
        }

        let normalized_messages = messages
            .into_iter()
            .map(|m| {
                let role = match m.role.as_str() {
                    "system" | "user" | "assistant" | "tool" => m.role,
                    "developer" => "system".to_string(),
                    _ => "user".to_string(),
                };
                AiChatMessage {
                    role,
                    content: m.content,
                }
            })
            .collect();

        let req = ChatRequest {
            model: self.model.clone(),
            messages: normalized_messages,
            temperature: self.temperature,
            max_tokens: self.max_tokens,
            stream: true,
        };

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .map_err(|e| e.to_string())?;

        let resp = client
            .post(endpoint)
            .headers(headers)
            .json(&req)
            .send()
            .await
            .map_err(|e| format!("AI request failed: {e}"))?;

        let status = resp.status();
        if !status.is_success() {
            let raw_text = resp
                .text()
                .await
                .map_err(|e| format!("AI response read failed: {e}"))?;
            return Err(format!("AI provider returned {status}: {raw_text}"));
        }

        let mut stream = resp.bytes_stream();
        let mut carry = String::new();
        let mut event_data = String::new();
        let mut full_response = String::new();
        let mut usage: Option<AiUsage> = None;
        let mut model = self.model.clone();

        while let Some(chunk_result) = stream.next().await {
            let bytes = chunk_result.map_err(|e| format!("AI stream read failed: {e}"))?;
            let text = String::from_utf8_lossy(&bytes);
            carry.push_str(&text);

            while let Some(idx) = carry.find('\n') {
                let mut line = carry[..idx].to_string();
                carry = carry[idx + 1..].to_string();
                if line.ends_with('\r') {
                    line.pop();
                }

                if line.is_empty() {
                    if event_data.is_empty() {
                        continue;
                    }
                    let data = event_data.trim().to_string();
                    event_data.clear();

                    if data == "[DONE]" {
                        continue;
                    }

                    let parsed: serde_json::Value = match serde_json::from_str(&data) {
                        Ok(v) => v,
                        Err(_) => continue,
                    };

                    if let Some(m) = parsed.get("model").and_then(|x| x.as_str()) {
                        model = m.to_string();
                    }

                    if let Some(u) = parsed.get("usage") {
                        usage = Some(AiUsage {
                            prompt_tokens: u.get("prompt_tokens").and_then(|x| x.as_i64()),
                            completion_tokens: u.get("completion_tokens").and_then(|x| x.as_i64()),
                            total_tokens: u.get("total_tokens").and_then(|x| x.as_i64()),
                        });
                    }

                    let delta_content = parsed
                        .get("choices")
                        .and_then(|c| c.get(0))
                        .and_then(|c| c.get("delta"))
                        .and_then(|d| d.get("content"));

                    let piece = match delta_content {
                        Some(serde_json::Value::String(s)) => s.clone(),
                        Some(serde_json::Value::Array(arr)) => arr
                            .iter()
                            .filter_map(|item| item.get("text").and_then(|x| x.as_str()))
                            .collect::<Vec<_>>()
                            .join(""),
                        _ => String::new(),
                    };

                    if !piece.is_empty() {
                        full_response.push_str(&piece);
                        on_chunk(&piece);
                    }
                    continue;
                }

                if let Some(rest) = line.strip_prefix("data:") {
                    if !event_data.is_empty() {
                        event_data.push('\n');
                    }
                    event_data.push_str(rest.trim_start());
                }
            }
        }

        if full_response.is_empty() {
            return Err("AI stream finished without content".to_string());
        }

        Ok(AiChatResponse {
            content: full_response,
            model,
            usage,
        })
    }
}
