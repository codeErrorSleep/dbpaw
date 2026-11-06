use crate::error::AppError;
use crate::models::ConnectionForm;
use base64::{engine::general_purpose, Engine as _};
use reqwest::header::AUTHORIZATION;
use reqwest::StatusCode;
use serde_json::Value;
use std::time::Duration;

use super::{
    ElasticsearchAuth, ElasticsearchClient, ElasticsearchConnectionInfo,
    ElasticsearchIndexOperationResult, ElasticsearchMutationResult, DEFAULT_CONNECT_TIMEOUT_MS,
    DEFAULT_ELASTICSEARCH_PORT,
};

pub(crate) fn trim_to_option(value: Option<&String>) -> Option<String> {
    value.map(|v| v.trim()).and_then(|v| {
        if v.is_empty() {
            None
        } else {
            Some(v.to_string())
        }
    })
}

fn parse_cloud_id(cloud_id: &str) -> Result<String, AppError> {
    let trimmed = cloud_id.trim();
    let encoded = trimmed
        .rsplit_once(':')
        .map(|(_, value)| value)
        .unwrap_or(trimmed);
    let decoded = general_purpose::STANDARD
        .decode(encoded)
        .or_else(|_| general_purpose::URL_SAFE_NO_PAD.decode(encoded))
        .map_err(|e| AppError::validation(format!("invalid Elasticsearch Cloud ID: {e}")))?;
    let decoded = String::from_utf8(decoded)
        .map_err(|e| AppError::validation(format!("invalid Elasticsearch Cloud ID UTF-8: {e}")))?;
    let mut parts = decoded.split('$');
    let base_domain = parts
        .next()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| AppError::validation("invalid Elasticsearch Cloud ID"))?;
    let es_id = parts
        .next()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| AppError::validation("invalid Elasticsearch Cloud ID"))?;
    Ok(format!(
        "https://{}.{}",
        es_id.trim().trim_end_matches('/'),
        base_domain.trim().trim_end_matches('/')
    ))
}

pub fn build_base_url(form: &ConnectionForm) -> Result<String, AppError> {
    if let Some(cloud_id) = trim_to_option(form.cloud_id.as_ref()) {
        return parse_cloud_id(&cloud_id);
    }
    let host = trim_to_option(form.host.as_ref())
        .ok_or_else(|| AppError::validation("host cannot be empty"))?;
    let port = form.port.unwrap_or(DEFAULT_ELASTICSEARCH_PORT);
    if !(1..=65535).contains(&port) {
        return Err(AppError::validation("port must be between 1 and 65535"));
    }
    let scheme = if form.ssl.unwrap_or(false) {
        "https"
    } else {
        "http"
    };
    let trimmed = host
        .trim_start_matches("http://")
        .trim_start_matches("https://")
        .trim_end_matches('/');
    Ok(format!("{scheme}://{trimmed}:{port}"))
}

pub(crate) fn build_api_key(form: &ConnectionForm) -> Result<Option<String>, AppError> {
    if let Some(encoded) = trim_to_option(form.api_key_encoded.as_ref()) {
        return Ok(Some(encoded));
    }
    let id = trim_to_option(form.api_key_id.as_ref());
    let secret = trim_to_option(form.api_key_secret.as_ref());
    match (id, secret) {
        (Some(id), Some(secret)) => Ok(Some(
            general_purpose::STANDARD.encode(format!("{id}:{secret}")),
        )),
        (Some(_), None) | (None, Some(_)) => Err(AppError::validation(
            "both apiKeyId and apiKeySecret are required for API key authentication",
        )),
        (None, None) => Ok(None),
    }
}

pub(crate) fn build_auth(form: &ConnectionForm) -> Result<ElasticsearchAuth, AppError> {
    let auth_mode = trim_to_option(form.auth_mode.as_ref()).unwrap_or_else(|| {
        if form
            .api_key_encoded
            .as_ref()
            .and_then(|v| trim_to_option(Some(v)))
            .is_some()
            || form
                .api_key_id
                .as_ref()
                .and_then(|v| trim_to_option(Some(v)))
                .is_some()
            || form
                .api_key_secret
                .as_ref()
                .and_then(|v| trim_to_option(Some(v)))
                .is_some()
        {
            "api_key".to_string()
        } else if form
            .username
            .as_ref()
            .and_then(|v| trim_to_option(Some(v)))
            .is_some()
        {
            "basic".to_string()
        } else {
            "none".to_string()
        }
    });

    match auth_mode.as_str() {
        "none" => Ok(ElasticsearchAuth::None),
        "basic" => {
            let username = trim_to_option(form.username.as_ref()).ok_or_else(|| {
                AppError::validation("username is required for basic authentication")
            })?;
            Ok(ElasticsearchAuth::Basic {
                username,
                password: form.password.clone(),
            })
        }
        "api_key" => {
            let api_key = build_api_key(form)?.ok_or_else(|| {
                AppError::validation("API key is required for API key authentication")
            })?;
            Ok(ElasticsearchAuth::ApiKey(api_key))
        }
        _ => Err(AppError::validation("unsupported Elasticsearch auth mode")),
    }
}

pub(crate) fn build_reqwest_client(
    form: &ConnectionForm,
    timeout_ms: i64,
) -> Result<reqwest::Client, AppError> {
    crate::network::validate_proxy(form)?;
    let mut builder = reqwest::Client::builder().timeout(Duration::from_millis(timeout_ms as u64));
    if let Some(proxy) = form.proxy.as_ref().filter(|proxy| proxy.enabled) {
        builder = builder.proxy(crate::network::reqwest_proxy(proxy)?);
    }
    if form.ssl.unwrap_or(false) {
        let ssl_mode =
            trim_to_option(form.ssl_mode.as_ref()).unwrap_or_else(|| "require".to_string());
        if ssl_mode == "verify_ca" {
            let ca_cert = trim_to_option(form.ssl_ca_cert.as_ref()).ok_or_else(|| {
                AppError::validation("sslCaCert cannot be empty in verify_ca mode")
            })?;
            let cert = reqwest::Certificate::from_pem(ca_cert.as_bytes())
                .map_err(|e| AppError::validation(format!("invalid CA certificate: {e}")))?;
            builder = builder.add_root_certificate(cert);
        } else {
            builder = builder.danger_accept_invalid_certs(true);
        }
    }
    builder
        .build()
        .map_err(|e| AppError::internal(format!("failed to build client: {e}")))
}

pub(crate) fn normalize_error(status: StatusCode, body: &str) -> AppError {
    if let Ok(value) = serde_json::from_str::<Value>(body) {
        if let Some(reason) = value
            .pointer("/error/reason")
            .and_then(Value::as_str)
            .or_else(|| {
                value
                    .pointer("/error/root_cause/0/reason")
                    .and_then(Value::as_str)
            })
        {
            return AppError::internal(format!("HTTP {}: {}", status.as_u16(), reason));
        }
        if let Some(error_type) = value.pointer("/error/type").and_then(Value::as_str) {
            return AppError::internal(format!("HTTP {}: {}", status.as_u16(), error_type));
        }
    }
    let compact = body.trim();
    if compact.is_empty() {
        AppError::internal(format!("HTTP {}", status.as_u16()))
    } else {
        AppError::internal(format!("HTTP {}: {}", status.as_u16(), compact))
    }
}

impl ElasticsearchClient {
    pub fn connect(form: &ConnectionForm) -> Result<Self, AppError> {
        let timeout_ms = form
            .connect_timeout_ms
            .filter(|&v| v > 0)
            .unwrap_or(DEFAULT_CONNECT_TIMEOUT_MS);

        let mut effective_form = form.clone();
        let ssh_tunnel = if let Some(true) = form.ssh_enabled {
            let tunnel = crate::ssh::start_ssh_tunnel(form)?;
            effective_form.host = Some("127.0.0.1".to_string());
            effective_form.port = Some(tunnel.local_port as i64);
            Some(tunnel)
        } else {
            None
        };

        let client = build_reqwest_client(&effective_form, timeout_ms)?;
        Ok(Self {
            client,
            base_url: build_base_url(&effective_form)?,
            auth: build_auth(&effective_form)?,
            ssh_tunnel,
        })
    }

    pub(crate) fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        let url = format!(
            "{}/{}",
            self.base_url.trim_end_matches('/'),
            path.trim_start_matches('/')
        );
        let req = self.client.request(method, url);
        match &self.auth {
            ElasticsearchAuth::None => req,
            ElasticsearchAuth::Basic { username, password } => {
                req.basic_auth(username, password.clone())
            }
            ElasticsearchAuth::ApiKey(api_key) => {
                req.header(AUTHORIZATION, format!("ApiKey {api_key}"))
            }
        }
    }

    pub(crate) async fn read_json(&self, req: reqwest::RequestBuilder) -> Result<Value, AppError> {
        let response = req
            .send()
            .await
            .map_err(|e| AppError::internal(format!("{e}")))?;
        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| AppError::internal(format!("{e}")))?;
        if !status.is_success() {
            return Err(normalize_error(status, &body));
        }
        serde_json::from_str::<Value>(&body).map_err(|e| AppError::internal(format!("{e}")))
    }

    pub(crate) async fn read_mutation(
        &self,
        req: reqwest::RequestBuilder,
    ) -> Result<ElasticsearchMutationResult, AppError> {
        let response = req
            .send()
            .await
            .map_err(|e| AppError::internal(format!("{e}")))?;
        let status = response.status();
        let status_code = status.as_u16();
        let body = response
            .text()
            .await
            .map_err(|e| AppError::internal(format!("{e}")))?;
        if !status.is_success() {
            return Err(normalize_error(status, &body));
        }
        let value = serde_json::from_str::<Value>(&body).unwrap_or(Value::Null);
        Ok(ElasticsearchMutationResult {
            index: value
                .get("_index")
                .and_then(Value::as_str)
                .map(str::to_string),
            id: value.get("_id").and_then(Value::as_str).map(str::to_string),
            result: value
                .get("result")
                .and_then(Value::as_str)
                .map(str::to_string),
            status: status_code,
        })
    }

    pub(crate) async fn read_index_operation(
        &self,
        req: reqwest::RequestBuilder,
        index: Option<String>,
    ) -> Result<ElasticsearchIndexOperationResult, AppError> {
        let response = req
            .send()
            .await
            .map_err(|e| AppError::internal(format!("{e}")))?;
        let status = response.status();
        let status_code = status.as_u16();
        let body = response
            .text()
            .await
            .map_err(|e| AppError::internal(format!("{e}")))?;
        if !status.is_success() {
            return Err(normalize_error(status, &body));
        }
        let value = serde_json::from_str::<Value>(&body).unwrap_or(Value::Null);
        Ok(ElasticsearchIndexOperationResult {
            index,
            acknowledged: value.get("acknowledged").and_then(Value::as_bool),
            shards_acknowledged: value.get("shards_acknowledged").and_then(Value::as_bool),
            status: status_code,
        })
    }

    pub async fn test_connection(&self) -> Result<ElasticsearchConnectionInfo, AppError> {
        let value = self
            .read_json(self.request(reqwest::Method::GET, "/"))
            .await?;
        Ok(ElasticsearchConnectionInfo {
            cluster_name: value
                .get("cluster_name")
                .and_then(Value::as_str)
                .map(str::to_string),
            cluster_uuid: value
                .get("cluster_uuid")
                .and_then(Value::as_str)
                .map(str::to_string),
            version: value
                .pointer("/version/number")
                .and_then(Value::as_str)
                .map(str::to_string),
            tagline: value
                .get("tagline")
                .and_then(Value::as_str)
                .map(str::to_string),
        })
    }
}
