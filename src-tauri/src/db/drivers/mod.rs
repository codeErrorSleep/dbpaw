use self::clickhouse::ClickHouseDriver;
use self::duckdb::DuckdbDriver;
use self::mssql::MssqlDriver;
use self::mysql::MysqlDriver;
use self::oracle::OracleDriver;
use self::postgres::PostgresDriver;
use self::sqlite::SqliteDriver;
use crate::models::{
    ConnectionForm, QueryResult, SchemaOverview, TableDataResponse, TableInfo, TableMetadata,
    TableStructure,
};
use async_trait::async_trait;

pub mod clickhouse;
pub mod duckdb;
pub mod mssql;
pub mod mysql;
pub mod oracle;
pub mod postgres;
pub mod sqlite;

pub fn is_mysql_family_driver(driver: &str) -> bool {
    matches!(driver, "mysql" | "mariadb" | "tidb" | "starrocks")
}

/// Build a `[CONN_FAILED]` error message with a context-aware hint derived from the
/// underlying error text, so users are not misled by a generic credential warning
/// when the actual problem is TLS incompatibility, a network issue, etc.
pub(crate) fn conn_failed_error(e: &dyn std::fmt::Display) -> String {
    let raw = e.to_string();
    let lower = raw.to_ascii_lowercase();

    let hint = if lower.contains("dpi-1047")
        || lower.contains("cannot locate a 64-bit oracle client")
    {
        "hint: Oracle Instant Client is not installed — download it from \
         https://www.oracle.com/database/technologies/instant-client/downloads.html \
         and add the directory containing libclntsh to your library path \
         (macOS: DYLD_LIBRARY_PATH; Linux: LD_LIBRARY_PATH)"
    } else if lower.contains("handshake")
        || lower.contains("fatal alert")
        || lower.contains("tls")
        || lower.contains("ssl")
        || lower.contains("certificate")
    {
        "hint: TLS/SSL handshake failed — the server may use a TLS version or cipher suite \
         incompatible with the client (TLS 1.2+ required); try disabling SSL in the connection settings"
    } else if lower.contains("access denied")
        || lower.contains("authentication")
        || lower.contains("password")
        || lower.contains("login failed")
        || lower.contains("invalid password")
        || lower.contains("1045")
    {
        "hint: authentication failed — verify the username/password are correct; \
         if they contain special characters they must be URL-encoded"
    } else if lower.contains("connection refused")
        || lower.contains("timed out")
        || lower.contains("timeout")
        || lower.contains("broken pipe")
        || lower.contains("network unreachable")
    {
        "hint: could not reach the server — check host, port, firewall rules, and SSH tunnel settings"
    } else if lower.contains("name resolution")
        || lower.contains("no such host")
        || lower.contains("failed to lookup")
        || lower.contains("dns")
    {
        "hint: hostname could not be resolved — check that the host address is correct"
    } else {
        "hint: check host, port, credentials, and SSL settings"
    };

    format!("[CONN_FAILED] {raw} ({hint})")
}

pub(crate) fn strip_trailing_statement_terminator(sql: &str) -> &str {
    let mut out = sql.trim_end();
    while let Some(stripped) = out.strip_suffix(';') {
        out = stripped.trim_end();
    }
    out
}

#[async_trait]
pub trait DatabaseDriver: Send + Sync {
    async fn test_connection(&self) -> Result<(), String>;
    async fn list_databases(&self) -> Result<Vec<String>, String>;
    async fn list_tables(&self, schema: Option<String>) -> Result<Vec<TableInfo>, String>;
    async fn get_table_structure(
        &self,
        schema: String,
        table: String,
    ) -> Result<TableStructure, String>;
    async fn get_table_metadata(
        &self,
        schema: String,
        table: String,
    ) -> Result<TableMetadata, String>;
    async fn get_table_ddl(&self, schema: String, table: String) -> Result<String, String>;
    async fn get_table_data(
        &self,
        schema: String,
        table: String,
        page: i64,
        limit: i64,
        sort_column: Option<String>,
        sort_direction: Option<String>,
        filter: Option<String>,
        order_by: Option<String>,
    ) -> Result<TableDataResponse, String>;
    async fn get_table_data_chunk(
        &self,
        schema: String,
        table: String,
        page: i64,
        limit: i64,
        sort_column: Option<String>,
        sort_direction: Option<String>,
        filter: Option<String>,
        order_by: Option<String>,
    ) -> Result<TableDataResponse, String>;
    async fn execute_query(&self, sql: String) -> Result<QueryResult, String>;
    async fn execute_query_with_id(
        &self,
        sql: String,
        query_id: Option<&str>,
    ) -> Result<QueryResult, String> {
        let _ = query_id;
        self.execute_query(sql).await
    }
    async fn get_schema_overview(&self, schema: Option<String>) -> Result<SchemaOverview, String>;
    async fn close(&self);
}

/// Normalize macOS smart quotes (U+2018/U+2019/U+201C/U+201D) to ASCII equivalents.
/// WKWebView on macOS inherits the system "Smart Quotes" setting and may
/// automatically replace straight quotes typed by the user.
pub fn normalize_quotes(s: &str) -> String {
    s.replace('\u{2018}', "'")
        .replace('\u{2019}', "'")
        .replace('\u{201C}', "\"")
        .replace('\u{201D}', "\"")
}

pub async fn connect(form: &ConnectionForm) -> Result<Box<dyn DatabaseDriver>, String> {
    match form.driver.as_str() {
        "postgres" => {
            let driver = PostgresDriver::connect(form).await?;
            Ok(Box::new(driver) as Box<dyn DatabaseDriver>)
        }
        driver if is_mysql_family_driver(driver) => {
            let driver = MysqlDriver::connect(form).await?;
            Ok(Box::new(driver) as Box<dyn DatabaseDriver>)
        }
        "sqlite" => {
            let driver = SqliteDriver::connect(form).await?;
            Ok(Box::new(driver) as Box<dyn DatabaseDriver>)
        }
        "duckdb" => {
            let driver = DuckdbDriver::connect(form).await?;
            Ok(Box::new(driver) as Box<dyn DatabaseDriver>)
        }
        "clickhouse" => {
            let driver = ClickHouseDriver::connect(form).await?;
            Ok(Box::new(driver) as Box<dyn DatabaseDriver>)
        }
        "mssql" => {
            let driver = MssqlDriver::connect(form).await?;
            Ok(Box::new(driver) as Box<dyn DatabaseDriver>)
        }
        "oracle" => {
            let driver = OracleDriver::connect(form).await?;
            Ok(Box::new(driver) as Box<dyn DatabaseDriver>)
        }
        _ => Err(format!(
            "[UNSUPPORTED] Driver {} not supported",
            form.driver
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::{conn_failed_error, is_mysql_family_driver, strip_trailing_statement_terminator};

    #[test]
    fn conn_failed_error_oracle_client_hint() {
        let msg = conn_failed_error(
            &"DPI-1047: Cannot locate a 64-bit Oracle Client library: \"dlopen(libclntsh.dylib, 0x0001): tried: '/usr/local/lib/libclntsh.dylib' (no such file)\"",
        );
        assert!(msg.starts_with("[CONN_FAILED]"));
        assert!(msg.contains("Oracle Instant Client is not installed"));
        assert!(msg.contains("DYLD_LIBRARY_PATH"));
        assert!(!msg.contains("TLS/SSL handshake failed"));
    }

    #[test]
    fn conn_failed_error_tls_hint() {
        let msg = conn_failed_error(
            &"error communicating with database: received fatal alert: HandshakeFailure",
        );
        assert!(msg.starts_with("[CONN_FAILED]"));
        assert!(msg.contains("TLS/SSL handshake failed"));
        assert!(!msg.contains("username/password"));
    }

    #[test]
    fn conn_failed_error_auth_hint() {
        let msg = conn_failed_error(&"Access denied for user 'root'@'localhost'");
        assert!(msg.contains("authentication failed"));
        assert!(msg.contains("URL-encoded"));
    }

    #[test]
    fn conn_failed_error_connection_refused_hint() {
        let msg = conn_failed_error(&"Connection refused (os error 111)");
        assert!(msg.contains("could not reach the server"));
    }

    #[test]
    fn conn_failed_error_timeout_hint() {
        let msg = conn_failed_error(&"connection timed out");
        assert!(msg.contains("could not reach the server"));
    }

    #[test]
    fn conn_failed_error_dns_hint() {
        let msg = conn_failed_error(&"failed to lookup address information: no such host");
        assert!(msg.contains("hostname could not be resolved"));
    }

    #[test]
    fn conn_failed_error_generic_hint() {
        let msg = conn_failed_error(&"some unknown database error");
        assert!(msg.starts_with("[CONN_FAILED]"));
        assert!(msg.contains("hint:"));
        assert!(!msg.contains("username/password"));
    }

    #[test]
    fn strip_trailing_statement_terminator_removes_single_semicolon() {
        assert_eq!(strip_trailing_statement_terminator("SELECT 1;"), "SELECT 1");
    }

    #[test]
    fn strip_trailing_statement_terminator_removes_multiple_semicolons_and_spaces() {
        assert_eq!(
            strip_trailing_statement_terminator("SELECT 1;;;   "),
            "SELECT 1"
        );
    }

    #[test]
    fn strip_trailing_statement_terminator_keeps_sql_without_semicolon() {
        assert_eq!(strip_trailing_statement_terminator("SELECT 1"), "SELECT 1");
    }

    #[test]
    fn mysql_family_helper_includes_starrocks() {
        assert!(is_mysql_family_driver("mysql"));
        assert!(is_mysql_family_driver("starrocks"));
        assert!(!is_mysql_family_driver("postgres"));
    }
}
