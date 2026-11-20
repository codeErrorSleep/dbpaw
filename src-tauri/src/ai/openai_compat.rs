use super::provider::{parse_extra_headers, AIProvider};
use super::types::{AiChatMessage, AiChatResponse, AiUsage};
use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};

const MAX_REQUEST_ATTEMPTS: usize = 3;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RequestOptions {
    use_max_completion_tokens: bool,
    send_temperature: bool,
}

impl Default for RequestOptions {
    fn default() -> Self {
        Self {
            use_max_completion_tokens: false,
            send_temperature: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParamAdjustment {
    UseMaxCompletionTokens,
    OmitTemperature,
}

impl RequestOptions {
    fn apply(&mut self, adjustment: ParamAdjustment) -> bool {
        match adjustment {
            ParamAdjustment::UseMaxCompletionTokens => {
                if self.use_max_completion_tokens {
                    return false;
                }
                self.use_max_completion_tokens = true;
                true
            }
            ParamAdjustment::OmitTemperature => {
                if !self.send_temperature {
                    return false;
                }
                self.send_temperature = false;
                true
            }
        }
    }
}

#[derive(Debug, Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: &'a [AiChatMessage],
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_completion_tokens: Option<i64>,
    stream: bool,
}

#[derive(Debug, Deserialize)]
struct ProviderErrorEnvelope {
    error: Option<ProviderErrorDetail>,
}

#[derive(Debug, Deserialize)]
struct ProviderErrorDetail {
    param: Option<String>,
    code: Option<String>,
}

fn param_to_adjust(status: reqwest::StatusCode, body: &str) -> Option<ParamAdjustment> {
    if status != reqwest::StatusCode::BAD_REQUEST {
        return None;
    }
    let parsed: ProviderErrorEnvelope = serde_json::from_str(body).ok()?;
    let detail = parsed.error?;
    match (detail.param.as_deref(), detail.code.as_deref()) {
        (Some("max_tokens"), Some("unsupported_parameter")) => {
            Some(ParamAdjustment::UseMaxCompletionTokens)
        }
        (Some("temperature"), Some("unsupported_value" | "unsupported_parameter")) => {
            Some(ParamAdjustment::OmitTemperature)
        }
        _ => None,
    }
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

fn normalize_role(role: &str) -> String {
    match role {
        "system" | "user" | "assistant" | "tool" => role.to_string(),
        "developer" => "system".to_string(),
        _ => "user".to_string(),
    }
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
        let resp = self.send_chat_request(messages, false, 60).await?;
        let raw_text = resp
            .text()
            .await
            .map_err(|e| format!("AI response read failed: {e}"))?;

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
    fn build_chat_request<'a>(
        &'a self,
        messages: &'a [AiChatMessage],
        stream: bool,
        options: RequestOptions,
    ) -> ChatRequest<'a> {
        ChatRequest {
            model: &self.model,
            messages,
            temperature: options.send_temperature.then_some(self.temperature),
            max_tokens: (!options.use_max_completion_tokens).then_some(self.max_tokens),
            max_completion_tokens: options.use_max_completion_tokens.then_some(self.max_tokens),
            stream,
        }
    }

    async fn send_chat_request(
        &self,
        messages: Vec<AiChatMessage>,
        stream: bool,
        timeout_secs: u64,
    ) -> Result<reqwest::Response, String> {
        self.validate_config()?;

        let endpoint = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));

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
        let normalized_messages: Vec<AiChatMessage> = messages
            .into_iter()
            .map(|m| {
                let role = normalize_role(&m.role);
                AiChatMessage {
                    role,
                    content: m.content,
                }
            })
            .collect();

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(timeout_secs))
            .build()
            .map_err(|e| e.to_string())?;

        let mut options = RequestOptions::default();
        let mut attempt = 0;
        loop {
            attempt += 1;
            let req = self.build_chat_request(&normalized_messages, stream, options);
            let resp = client
                .post(&endpoint)
                .headers(headers.clone())
                .json(&req)
                .send()
                .await
                .map_err(|e| format!("AI request failed: {e}"))?;

            let status = resp.status();
            if status.is_success() {
                return Ok(resp);
            }

            let raw_text = resp
                .text()
                .await
                .map_err(|e| format!("AI response read failed: {e}"))?;

            if attempt < MAX_REQUEST_ATTEMPTS
                && param_to_adjust(status, &raw_text).is_some_and(|adj| options.apply(adj))
            {
                continue;
            }

            return Err(format!("AI provider returned {status}: {raw_text}"));
        }
    }

    pub async fn chat_stream<F>(
        &self,
        messages: Vec<AiChatMessage>,
        mut on_chunk: F,
    ) -> Result<AiChatResponse, String>
    where
        F: FnMut(&str),
    {
        let resp = self.send_chat_request(messages, true, 120).await?;

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

#[cfg(test)]
mod tests {
    use super::*;

    fn make_provider(base_url: &str, api_key: &str, model: &str) -> OpenAICompatProvider {
        OpenAICompatProvider {
            name: "test".to_string(),
            base_url: base_url.to_string(),
            api_key: api_key.to_string(),
            model: model.to_string(),
            temperature: 0.7,
            max_tokens: 4096,
            extra_json: None,
        }
    }

    #[test]
    fn validate_config_all_valid() {
        let p = make_provider("http://localhost", "key", "gpt-4");
        assert!(p.validate_config().is_ok());
    }

    #[test]
    fn validate_config_empty_base_url() {
        let p = make_provider("", "key", "gpt-4");
        let err = p.validate_config().unwrap_err();
        assert!(err.contains("baseUrl"));
    }

    #[test]
    fn validate_config_empty_api_key() {
        let p = make_provider("http://localhost", "", "gpt-4");
        let err = p.validate_config().unwrap_err();
        assert!(err.contains("apiKey"));
    }

    #[test]
    fn validate_config_empty_model() {
        let p = make_provider("http://localhost", "key", "");
        let err = p.validate_config().unwrap_err();
        assert!(err.contains("model"));
    }

    #[test]
    fn normalize_role_system() {
        assert_eq!(normalize_role("system"), "system");
    }

    #[test]
    fn normalize_role_user() {
        assert_eq!(normalize_role("user"), "user");
    }

    #[test]
    fn normalize_role_assistant() {
        assert_eq!(normalize_role("assistant"), "assistant");
    }

    #[test]
    fn normalize_role_developer_to_system() {
        assert_eq!(normalize_role("developer"), "system");
    }

    #[test]
    fn normalize_role_unknown_to_user() {
        assert_eq!(normalize_role("unknown_role"), "user");
    }

    fn sample_messages() -> Vec<AiChatMessage> {
        vec![AiChatMessage {
            role: "user".to_string(),
            content: "SELECT 1".to_string(),
        }]
    }

    #[test]
    fn build_chat_request_default_sends_max_tokens_and_temperature() {
        let p = make_provider("http://localhost", "key", "gpt-4");
        let messages = sample_messages();
        let body =
            serde_json::to_value(p.build_chat_request(&messages, false, RequestOptions::default()))
                .unwrap();
        let obj = body.as_object().unwrap();

        assert_eq!(obj.get("model"), Some(&serde_json::json!("gpt-4")));
        assert_eq!(obj.get("max_tokens"), Some(&serde_json::json!(4096)));
        assert!((obj.get("temperature").unwrap().as_f64().unwrap() - 0.7).abs() < 1e-5);
        assert_eq!(obj.get("stream"), Some(&serde_json::json!(false)));
        assert!(!obj.contains_key("max_completion_tokens"));
    }

    #[test]
    fn build_chat_request_max_completion_tokens_variant() {
        let p = make_provider("http://localhost", "key", "gpt-5.4-mini");
        let messages = sample_messages();
        let options = RequestOptions {
            use_max_completion_tokens: true,
            send_temperature: true,
        };
        let body = serde_json::to_value(p.build_chat_request(&messages, true, options)).unwrap();
        let obj = body.as_object().unwrap();

        assert!(!obj.contains_key("max_tokens"));
        assert_eq!(
            obj.get("max_completion_tokens"),
            Some(&serde_json::json!(4096))
        );
        assert!((obj.get("temperature").unwrap().as_f64().unwrap() - 0.7).abs() < 1e-5);
        assert_eq!(obj.get("stream"), Some(&serde_json::json!(true)));
    }

    #[test]
    fn build_chat_request_omits_temperature() {
        let p = make_provider("http://localhost", "key", "gpt-5.4-mini");
        let messages = sample_messages();
        let options = RequestOptions {
            use_max_completion_tokens: false,
            send_temperature: false,
        };
        let body = serde_json::to_value(p.build_chat_request(&messages, false, options)).unwrap();
        let obj = body.as_object().unwrap();

        assert_eq!(obj.get("max_tokens"), Some(&serde_json::json!(4096)));
        assert!(!obj.contains_key("max_completion_tokens"));
        assert!(!obj.contains_key("temperature"));
    }

    #[test]
    fn build_chat_request_both_adjustments() {
        let p = make_provider("http://localhost", "key", "gpt-5.4-mini");
        let messages = sample_messages();
        let options = RequestOptions {
            use_max_completion_tokens: true,
            send_temperature: false,
        };
        let body = serde_json::to_value(p.build_chat_request(&messages, false, options)).unwrap();
        let obj = body.as_object().unwrap();

        assert!(!obj.contains_key("max_tokens"));
        assert_eq!(
            obj.get("max_completion_tokens"),
            Some(&serde_json::json!(4096))
        );
        assert!(!obj.contains_key("temperature"));
    }

    const OPENAI_MAX_TOKENS_UNSUPPORTED_ERR: &str = r#"{"error":{"message":"Unsupported parameter: 'max_tokens' is not supported with this model. Use 'max_completion_tokens' instead.","type":"invalid_request_error","param":"max_tokens","code":"unsupported_parameter"}}"#;
    const OPENAI_TEMPERATURE_UNSUPPORTED_ERR: &str = r#"{"error":{"message":"Unsupported value: 'temperature' does not support 0.1 with this model. Only the default (1) value is supported.","type":"invalid_request_error","param":"temperature","code":"unsupported_value"}}"#;
    const OPENAI_CONTEXT_LENGTH_EXCEEDED_ERR: &str =
        r#"{"error":{"param":"max_tokens","code":"context_length_exceeded"}}"#;
    const MOCK_SERVER_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

    #[test]
    fn param_to_adjust_detects_exact_openai_max_tokens_error() {
        assert_eq!(
            param_to_adjust(
                reqwest::StatusCode::BAD_REQUEST,
                OPENAI_MAX_TOKENS_UNSUPPORTED_ERR,
            ),
            Some(ParamAdjustment::UseMaxCompletionTokens)
        );
    }

    #[test]
    fn param_to_adjust_detects_supported_temperature_error_codes() {
        let unsupported_param = r#"{"error":{"message":"Unsupported parameter: 'temperature' is not supported with this model.","type":"invalid_request_error","param":"temperature","code":"unsupported_parameter"}}"#;

        assert_eq!(
            param_to_adjust(
                reqwest::StatusCode::BAD_REQUEST,
                OPENAI_TEMPERATURE_UNSUPPORTED_ERR,
            ),
            Some(ParamAdjustment::OmitTemperature)
        );
        assert_eq!(
            param_to_adjust(reqwest::StatusCode::BAD_REQUEST, unsupported_param),
            Some(ParamAdjustment::OmitTemperature)
        );
    }

    #[test]
    fn param_to_adjust_requires_matching_code_and_ignores_unrelated_errors() {
        let max_tokens_wrong_code =
            r#"{"error":{"param":"max_tokens","code":"unsupported_value"}}"#;
        let max_tokens_no_code = r#"{"error":{"param":"max_tokens"}}"#;
        let temp_invalid_type = r#"{"error":{"param":"temperature","code":"invalid_type"}}"#;
        let temp_no_code = r#"{"error":{"param":"temperature"}}"#;
        let other_param =
            r#"{"error":{"message":"bad","param":"top_p","code":"unsupported_parameter"}}"#;

        assert_eq!(
            param_to_adjust(
                reqwest::StatusCode::BAD_REQUEST,
                OPENAI_CONTEXT_LENGTH_EXCEEDED_ERR,
            ),
            None
        );
        assert_eq!(
            param_to_adjust(reqwest::StatusCode::BAD_REQUEST, max_tokens_wrong_code),
            None
        );
        assert_eq!(
            param_to_adjust(reqwest::StatusCode::BAD_REQUEST, max_tokens_no_code),
            None
        );
        assert_eq!(
            param_to_adjust(reqwest::StatusCode::BAD_REQUEST, temp_invalid_type),
            None
        );
        assert_eq!(
            param_to_adjust(reqwest::StatusCode::BAD_REQUEST, temp_no_code),
            None
        );
        assert_eq!(
            param_to_adjust(reqwest::StatusCode::BAD_REQUEST, other_param),
            None
        );
        assert_eq!(
            param_to_adjust(
                reqwest::StatusCode::UNAUTHORIZED,
                OPENAI_MAX_TOKENS_UNSUPPORTED_ERR,
            ),
            None
        );
        assert_eq!(
            param_to_adjust(
                reqwest::StatusCode::INTERNAL_SERVER_ERROR,
                OPENAI_MAX_TOKENS_UNSUPPORTED_ERR,
            ),
            None
        );
        assert_eq!(
            param_to_adjust(reqwest::StatusCode::BAD_REQUEST, "not json at all"),
            None
        );
        assert_eq!(
            param_to_adjust(reqwest::StatusCode::BAD_REQUEST, r#"{"error":"string"}"#),
            None
        );
    }

    #[test]
    fn request_options_apply_is_once_per_adjustment() {
        let mut options = RequestOptions::default();
        assert!(options.apply(ParamAdjustment::UseMaxCompletionTokens));
        assert!(!options.apply(ParamAdjustment::UseMaxCompletionTokens));
        assert!(options.apply(ParamAdjustment::OmitTemperature));
        assert!(!options.apply(ParamAdjustment::OmitTemperature));
    }

    async fn read_http_json_body(stream: &tokio::net::TcpStream) -> serde_json::Value {
        let mut buf = Vec::new();
        let mut tmp = [0u8; 4096];
        let mut header_end = None;
        let mut content_length = 0usize;

        loop {
            if let Some(end) = header_end {
                if buf.len() >= end + content_length {
                    break;
                }
            }
            stream.readable().await.unwrap();
            match stream.try_read(&mut tmp) {
                Ok(0) => break,
                Ok(n) => {
                    buf.extend_from_slice(&tmp[..n]);
                    if header_end.is_none() {
                        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                            let end = pos + 4;
                            header_end = Some(end);
                            let headers_text = String::from_utf8_lossy(&buf[..pos]);
                            for line in headers_text.lines() {
                                if let Some((k, v)) = line.split_once(':') {
                                    if k.eq_ignore_ascii_case("content-length") {
                                        content_length = v.trim().parse().unwrap_or(0);
                                    }
                                }
                            }
                        }
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => continue,
                Err(e) => panic!("read failed: {e}"),
            }
        }

        let start = header_end.expect("missing HTTP header terminator");
        serde_json::from_slice(&buf[start..start + content_length]).unwrap()
    }

    async fn write_all_http(stream: &tokio::net::TcpStream, data: &[u8]) {
        let mut written = 0;
        while written < data.len() {
            stream.writable().await.unwrap();
            match stream.try_write(&data[written..]) {
                Ok(n) => written += n,
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => continue,
                Err(e) => panic!("write failed: {e}"),
            }
        }
    }

    struct MockServer {
        base_url: String,
        shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
        handle: Option<tokio::task::JoinHandle<Vec<serde_json::Value>>>,
    }

    impl MockServer {
        async fn spawn(responses: Vec<(u16, &'static str, &'static str, String)>) -> Self {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = listener.local_addr().unwrap();
            let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel();
            let handle = tokio::spawn(async move {
                let mut captured = Vec::new();
                for (status, reason, content_type, body) in responses {
                    let accept_result = tokio::select! {
                        biased;
                        _ = &mut shutdown_rx => break,
                        res = listener.accept() => res,
                    };
                    let (stream, _) = accept_result.unwrap();
                    let req_body = read_http_json_body(&stream).await;
                    captured.push(req_body);
                    let reply = format!(
                        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    write_all_http(&stream, reply.as_bytes()).await;
                }
                captured
            });
            Self {
                base_url: format!("http://{addr}/v1"),
                shutdown_tx: Some(shutdown_tx),
                handle: Some(handle),
            }
        }

        async fn finish(mut self) -> Vec<serde_json::Value> {
            if let Some(tx) = self.shutdown_tx.take() {
                let _ = tx.send(());
            }
            self.handle.take().unwrap().await.unwrap()
        }
    }

    impl Drop for MockServer {
        fn drop(&mut self) {
            if let Some(tx) = self.shutdown_tx.take() {
                let _ = tx.send(());
            }
            if let Some(handle) = self.handle.take() {
                handle.abort();
            }
        }
    }

    async fn run_with_timeout<F, T>(name: &str, fut: F) -> T
    where
        F: std::future::Future<Output = T>,
    {
        tokio::time::timeout(MOCK_SERVER_TIMEOUT, fut)
            .await
            .unwrap_or_else(|_| panic!("{name} timed out after {MOCK_SERVER_TIMEOUT:?}"))
    }

    #[tokio::test]
    async fn chat_once_retries_max_tokens_and_temperature_then_succeeds() {
        run_with_timeout(
            "chat_once_retries_max_tokens_and_temperature_then_succeeds",
            async {
                let ok_body = r#"{"model":"gpt-6.2-sol","choices":[{"message":{"content":"SELECT 1;"}}],"usage":{"prompt_tokens":10,"completion_tokens":5,"total_tokens":15}}"#.to_string();

                let server = MockServer::spawn(vec![
                    (
                        400,
                        "Bad Request",
                        "application/json",
                        OPENAI_MAX_TOKENS_UNSUPPORTED_ERR.to_string(),
                    ),
                    (
                        400,
                        "Bad Request",
                        "application/json",
                        OPENAI_TEMPERATURE_UNSUPPORTED_ERR.to_string(),
                    ),
                    (200, "OK", "application/json", ok_body),
                ])
                .await;

                let mut provider = make_provider(&server.base_url, "test-key", "gpt-6.2-sol");
                provider.temperature = 0.1;
                let resp = provider.chat_once(sample_messages()).await.unwrap();

                assert_eq!(resp.content, "SELECT 1;");
                assert_eq!(resp.model, "gpt-6.2-sol");

                let captured = server.finish().await;
                assert_eq!(captured.len(), 3);

                let req0 = captured[0].as_object().unwrap();
                assert_eq!(req0.get("max_tokens"), Some(&serde_json::json!(4096)));
                assert!(req0.contains_key("temperature"));
                assert!(!req0.contains_key("max_completion_tokens"));

                let req1 = captured[1].as_object().unwrap();
                assert!(!req1.contains_key("max_tokens"));
                assert_eq!(
                    req1.get("max_completion_tokens"),
                    Some(&serde_json::json!(4096))
                );
                assert!(req1.contains_key("temperature"));

                let req2 = captured[2].as_object().unwrap();
                assert!(!req2.contains_key("max_tokens"));
                assert_eq!(
                    req2.get("max_completion_tokens"),
                    Some(&serde_json::json!(4096))
                );
                assert!(!req2.contains_key("temperature"));
            },
        )
        .await;
    }

    #[tokio::test]
    async fn chat_stream_retries_on_400_before_streaming() {
        run_with_timeout("chat_stream_retries_on_400_before_streaming", async {
            let sse_body = concat!(
                "data: {\"model\":\"gpt-5.4-mini\",\"choices\":[{\"delta\":{\"content\":\"SELECT \"}}]}\n\n",
                "data: {\"model\":\"gpt-5.4-mini\",\"choices\":[{\"delta\":{\"content\":\"1;\"}}]}\n\n",
                "data: [DONE]\n\n"
            )
            .to_string();

            let server = MockServer::spawn(vec![
                (
                    400,
                    "Bad Request",
                    "application/json",
                    OPENAI_MAX_TOKENS_UNSUPPORTED_ERR.to_string(),
                ),
                (200, "OK", "text/event-stream", sse_body),
            ])
            .await;

            let provider = make_provider(&server.base_url, "test-key", "gpt-5.4-mini");
            let mut chunks = Vec::new();
            let resp = provider
                .chat_stream(sample_messages(), |c| chunks.push(c.to_string()))
                .await
                .unwrap();

            assert_eq!(resp.content, "SELECT 1;");
            assert_eq!(chunks, vec!["SELECT ", "1;"]);

            let captured = server.finish().await;
            assert_eq!(captured.len(), 2);
            assert_eq!(captured[0].get("stream"), Some(&serde_json::json!(true)));
            assert!(captured[0].as_object().unwrap().contains_key("max_tokens"));
            assert!(!captured[1].as_object().unwrap().contains_key("max_tokens"));
            assert_eq!(
                captured[1].get("max_completion_tokens"),
                Some(&serde_json::json!(4096))
            );
        })
        .await;
    }

    #[tokio::test]
    async fn chat_once_stops_if_same_param_rejected_twice() {
        run_with_timeout("chat_once_stops_if_same_param_rejected_twice", async {
            let server = MockServer::spawn(vec![
                (
                    400,
                    "Bad Request",
                    "application/json",
                    OPENAI_MAX_TOKENS_UNSUPPORTED_ERR.to_string(),
                ),
                (
                    400,
                    "Bad Request",
                    "application/json",
                    OPENAI_MAX_TOKENS_UNSUPPORTED_ERR.to_string(),
                ),
            ])
            .await;

            let provider = make_provider(&server.base_url, "test-key", "gpt-5.4-mini");
            let err = provider.chat_once(sample_messages()).await.unwrap_err();
            assert_eq!(
                err,
                format!(
                    "AI provider returned 400 Bad Request: {OPENAI_MAX_TOKENS_UNSUPPORTED_ERR}"
                )
            );

            let captured = server.finish().await;
            assert_eq!(captured.len(), 2);
            assert!(captured[0].as_object().unwrap().contains_key("max_tokens"));
            assert!(!captured[1].as_object().unwrap().contains_key("max_tokens"));
            assert_eq!(
                captured[1].get("max_completion_tokens"),
                Some(&serde_json::json!(4096))
            );
        })
        .await;
    }

    #[tokio::test]
    async fn chat_once_does_not_retry_unrelated_400() {
        run_with_timeout("chat_once_does_not_retry_unrelated_400", async {
            let server = MockServer::spawn(vec![(
                400,
                "Bad Request",
                "application/json",
                OPENAI_CONTEXT_LENGTH_EXCEEDED_ERR.to_string(),
            )])
            .await;

            let provider = make_provider(&server.base_url, "test-key", "gpt-5.4-mini");
            let err = provider.chat_once(sample_messages()).await.unwrap_err();
            assert_eq!(
                err,
                format!(
                    "AI provider returned 400 Bad Request: {OPENAI_CONTEXT_LENGTH_EXCEEDED_ERR}"
                )
            );

            let captured = server.finish().await;
            assert_eq!(captured.len(), 1);
            assert!(captured[0].as_object().unwrap().contains_key("max_tokens"));
            assert!(!captured[0]
                .as_object()
                .unwrap()
                .contains_key("max_completion_tokens"));
        })
        .await;
    }
}
