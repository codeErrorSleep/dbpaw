use super::super::{conn_failed_error, DriverResult};
use super::helpers::quote_literal;
use super::ClickHouseDriver;
use crate::error::AppError;
use crate::models::ConnectionForm;
use reqwest;
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug)]
pub struct ClickHouseConfig {
    pub base_url: String,
    pub database: String,
    pub username: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct ClickHouseMeta {
    pub name: String,
    #[serde(rename = "type")]
    pub type_name: String,
}

#[derive(Debug, Deserialize)]
pub struct ClickHouseJsonResponse {
    #[serde(default)]
    pub meta: Vec<ClickHouseMeta>,
    #[serde(default)]
    pub data: Vec<Value>,
    pub rows: Option<u64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ClickHouseSummary {
    pub read_rows: Option<i64>,
    pub written_rows: Option<i64>,
    pub result_rows: Option<i64>,
    pub total_rows_to_read: Option<i64>,
}

#[derive(Debug)]
pub struct ClickHouseRawResponse {
    pub body: String,
    pub summary: Option<ClickHouseSummary>,
}

pub fn build_config(form: &ConnectionForm) -> DriverResult<ClickHouseConfig> {
    let host = form
        .host
        .clone()
        .filter(|v| !v.trim().is_empty())
        .ok_or_else(|| AppError::validation("host cannot be empty"))?;

    let ssl = form.ssl.unwrap_or(false);
    let scheme = if ssl { "https" } else { "http" };
    let port = form.port.unwrap_or(8123);
    let database = form
        .database
        .clone()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| "default".to_string());
    let username = form
        .username
        .clone()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| "default".to_string());
    let password = form.password.clone().unwrap_or_default();

    Ok(ClickHouseConfig {
        base_url: format!("{}://{}:{}", scheme, host, port),
        database,
        username,
        password,
    })
}

pub fn parse_summary_header(headers: &reqwest::header::HeaderMap) -> Option<ClickHouseSummary> {
    let header = headers.get("X-ClickHouse-Summary")?;
    let text = header.to_str().ok()?;
    serde_json::from_str::<ClickHouseSummary>(text).ok()
}

impl ClickHouseDriver {
    pub async fn connect(form: &ConnectionForm) -> DriverResult<Self> {
        crate::network::validate_proxy(form)?;
        let mut dsn_form = form.clone();
        let mut ssh_tunnel = None;

        if let Some(true) = form.ssh_enabled {
            let tunnel = crate::ssh::start_ssh_tunnel(form)?;
            dsn_form.host = Some("127.0.0.1".to_string());
            dsn_form.port = Some(tunnel.local_port as i64);
            ssh_tunnel = Some(tunnel);
        }

        let config = build_config(&dsn_form)?;
        let mut builder = reqwest::Client::builder().timeout(std::time::Duration::from_secs(10));
        if let Some(proxy) = form.proxy.as_ref().filter(|proxy| proxy.enabled) {
            builder = builder.proxy(crate::network::reqwest_proxy(proxy)?);
        }
        let client = builder
            .build()
            .map_err(|e| conn_failed_error(&format!("Failed to create HTTP client: {e}")))?;

        Ok(Self {
            client,
            base_url: config.base_url,
            database: config.database,
            username: config.username,
            password: config.password,
            ssh_tunnel,
        })
    }

    pub async fn execute_raw(
        &self,
        sql: &str,
        query_id: Option<&str>,
    ) -> DriverResult<ClickHouseRawResponse> {
        let mut request = self
            .client
            .post(&self.base_url)
            .query(&[("database", self.database.as_str())]);
        if let Some(qid) = query_id.filter(|v| !v.trim().is_empty()) {
            request = request.query(&[("query_id", qid)]);
        }
        let response = request
            .basic_auth(&self.username, Some(&self.password))
            .body(sql.to_string())
            .send()
            .await
            .map_err(|e| AppError::query_failed(format!("HTTP request failed: {e}")))?;

        let status = response.status();
        let summary = parse_summary_header(response.headers());
        let body = response
            .text()
            .await
            .map_err(|e| AppError::query_failed(format!("Failed to read response body: {e}")))?;

        if !status.is_success() {
            let message = body.trim();
            return Err(AppError::query_failed(format!(
                "HTTP {}: {}",
                status, message
            )));
        }

        Ok(ClickHouseRawResponse { body, summary })
    }

    pub async fn execute_json(
        &self,
        sql: &str,
        query_id: Option<&str>,
    ) -> DriverResult<ClickHouseJsonResponse> {
        let raw = self.execute_raw(sql, query_id).await?;
        let body = raw.body;
        serde_json::from_str::<ClickHouseJsonResponse>(&body).map_err(|e| {
            let snippet = if body.len() > 240 {
                format!("{}...", &body[..240])
            } else {
                body
            };
            AppError::query_failed(format!(
                "Failed to parse ClickHouse JSON response: {} | body: {}",
                e, snippet
            ))
        })
    }

    pub async fn kill_query(&self, query_id: &str) -> DriverResult<()> {
        let qid = query_id.trim();
        if qid.is_empty() {
            return Err(AppError::validation("query_id cannot be empty"));
        }
        let sql = format!("KILL QUERY WHERE query_id = {} ASYNC", quote_literal(qid));
        self.execute_raw(&sql, None).await.map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_config_uses_defaults() {
        let form = ConnectionForm {
            driver: "clickhouse".to_string(),
            host: Some("localhost".to_string()),
            ..Default::default()
        };

        let cfg = build_config(&form).unwrap();
        assert_eq!(cfg.base_url, "http://localhost:8123");
        assert_eq!(cfg.database, "default");
        assert_eq!(cfg.username, "default");
    }

    #[test]
    fn build_config_respects_ssl_and_custom_values() {
        let form = ConnectionForm {
            driver: "clickhouse".to_string(),
            host: Some("db.internal".to_string()),
            port: Some(9440),
            database: Some("analytics".to_string()),
            username: Some("app".to_string()),
            password: Some("secret".to_string()),
            ssl: Some(true),
            ..Default::default()
        };

        let cfg = build_config(&form).unwrap();
        assert_eq!(cfg.base_url, "https://db.internal:9440");
        assert_eq!(cfg.database, "analytics");
        assert_eq!(cfg.username, "app");
        assert_eq!(cfg.password, "secret");
    }
}
