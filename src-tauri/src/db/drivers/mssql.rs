use super::DatabaseDriver;
use crate::models::{
    ColumnInfo, ColumnSchema, ConnectionForm, ForeignKeyInfo, IndexInfo, QueryColumn, QueryResult,
    SchemaOverview, TableDataResponse, TableInfo, TableMetadata, TableSchema, TableStructure,
};
use async_trait::async_trait;
use chrono::{DateTime, FixedOffset, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use futures_util::TryStreamExt;
use rust_decimal::Decimal;
use std::collections::{HashMap, HashSet};
use tiberius::{AuthMethod, Client, Config, EncryptionLevel, QueryItem, Row};
use tokio::net::TcpStream;
use tokio_util::compat::{Compat, TokioAsyncWriteCompatExt};

use crate::ssh::SshTunnel;

pub struct MssqlDriver {
    config: MssqlConfig,
    pub ssh_tunnel: Option<SshTunnel>,
}

struct MssqlConfig {
    host: String,
    port: u16,
    database: String,
    username: String,
    password: String,
    ssl: bool,
}

fn build_config(form: &ConnectionForm) -> Result<MssqlConfig, String> {
    let host = form
        .host
        .clone()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.trim().is_empty())
        .ok_or("[VALIDATION_ERROR] host cannot be empty")?;
    let port = form.port.unwrap_or(1433);
    if !(0..=65535).contains(&port) {
        return Err("[VALIDATION_ERROR] port out of range".to_string());
    }
    let database = form
        .database
        .clone()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| "master".to_string());
    let username = form
        .username
        .clone()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.trim().is_empty())
        .ok_or("[VALIDATION_ERROR] username cannot be empty")?;
    let password = form.password.clone().unwrap_or_default();

    Ok(MssqlConfig {
        host,
        port: port as u16,
        database,
        username,
        password,
        ssl: form.ssl.unwrap_or(false),
    })
}

fn escape_literal(value: &str) -> String {
    value.replace('\'', "''")
}

fn quote_ident(ident: &str) -> Result<String, String> {
    let trimmed = ident.trim();
    if trimmed.is_empty() {
        return Err("[VALIDATION_ERROR] identifier cannot be empty".to_string());
    }
    if trimmed.chars().any(|c| c == '\0') {
        return Err("[VALIDATION_ERROR] identifier contains null byte".to_string());
    }
    Ok(format!("[{}]", trimmed.replace(']', "]]")))
}

fn table_ref(schema: &str, table: &str) -> Result<String, String> {
    Ok(format!("{}.{}", quote_ident(schema)?, quote_ident(table)?))
}

fn first_sql_keyword(sql: &str) -> Option<String> {
    let bytes = sql.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    loop {
        while i < len && (bytes[i].is_ascii_whitespace() || bytes[i] == b';') {
            i += 1;
        }

        if i + 1 < len && bytes[i] == b'-' && bytes[i + 1] == b'-' {
            i += 2;
            while i < len && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }

        if i + 1 < len && bytes[i] == b'/' && bytes[i + 1] == b'*' {
            i += 2;
            while i + 1 < len && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                i += 1;
            }
            if i + 1 >= len {
                return None;
            }
            i += 2;
            continue;
        }

        break;
    }

    if i >= len {
        return None;
    }

    let start = i;
    while i < len && bytes[i].is_ascii_alphabetic() {
        i += 1;
    }
    if start == i {
        return None;
    }

    Some(sql[start..i].to_ascii_lowercase())
}

impl MssqlDriver {
    fn build_tiberius_config(&self, encryption: EncryptionLevel, trust_cert: bool) -> Config {
        let mut config = Config::new();
        config.host(&self.config.host);
        config.port(self.config.port);
        config.database(&self.config.database);
        config.authentication(AuthMethod::sql_server(
            self.config.username.clone(),
            self.config.password.clone(),
        ));
        config.encryption(encryption);
        if trust_cert
            && !matches!(
                encryption,
                EncryptionLevel::Off | EncryptionLevel::NotSupported
            )
        {
            config.trust_cert();
        }
        config
    }

    async fn connect_with_config(config: Config) -> Result<Client<Compat<TcpStream>>, String> {
        let tcp = TcpStream::connect(config.get_addr())
            .await
            .map_err(|e| format!("[CONN_FAILED] {}", e))?;
        tcp.set_nodelay(true)
            .map_err(|e| format!("[CONN_FAILED] {}", e))?;

        Client::connect(config, tcp.compat_write())
            .await
            .map_err(|e| format!("[CONN_FAILED] {}", e))
    }

    pub async fn connect(form: &ConnectionForm) -> Result<Self, String> {
        let mut cfg_form = form.clone();
        let mut ssh_tunnel = None;

        if let Some(true) = form.ssh_enabled {
            let tunnel = crate::ssh::start_ssh_tunnel(form)?;
            cfg_form.host = Some("127.0.0.1".to_string());
            cfg_form.port = Some(tunnel.local_port as i64);
            ssh_tunnel = Some(tunnel);
        }

        let config = build_config(&cfg_form)?;
        let driver = Self { config, ssh_tunnel };
        driver.test_connection().await?;
        Ok(driver)
    }

    async fn connect_client(&self) -> Result<Client<Compat<TcpStream>>, String> {
        let attempts = if self.config.ssl {
            vec![
                (
                    EncryptionLevel::Required,
                    false,
                    "encrypt=required,trust_cert=false",
                ),
                (EncryptionLevel::On, false, "encrypt=on,trust_cert=false"),
            ]
        } else {
            vec![
                (EncryptionLevel::Off, false, "encrypt=off"),
                (
                    EncryptionLevel::NotSupported,
                    false,
                    "encrypt=not_supported",
                ),
                (EncryptionLevel::On, true, "encrypt=on,trust_cert=true"),
                (
                    EncryptionLevel::Required,
                    true,
                    "encrypt=required,trust_cert=true",
                ),
            ]
        };

        let mut errors = Vec::new();
        for (encryption, trust_cert, label) in attempts {
            let config = self.build_tiberius_config(encryption, trust_cert);
            match Self::connect_with_config(config).await {
                Ok(client) => return Ok(client),
                Err(err) => errors.push(format!("{label}: {err}")),
            }
        }

        Err(format!(
            "[CONN_FAILED] SQL Server handshake failed after retries: {}",
            errors.join(" | ")
        ))
    }

    async fn fetch_rows(&self, sql: &str) -> Result<Vec<Row>, String> {
        Ok(self.fetch_rows_with_columns(sql).await?.0)
    }

    async fn fetch_rows_with_columns(
        &self,
        sql: &str,
    ) -> Result<(Vec<Row>, Vec<QueryColumn>), String> {
        let mut client = self.connect_client().await?;
        let mut stream = client
            .simple_query(sql)
            .await
            .map_err(|e| format!("[QUERY_ERROR] {}", e))?;
        let mut rows = Vec::new();
        let mut columns = Vec::new();

        while let Some(item) = stream
            .try_next()
            .await
            .map_err(|e| format!("[QUERY_ERROR] {}", e))?
        {
            match item {
                QueryItem::Metadata(meta) if columns.is_empty() => {
                    columns = meta
                        .columns()
                        .iter()
                        .map(|col| QueryColumn {
                            name: col.name().to_string(),
                            r#type: format!("{:?}", col.column_type()),
                        })
                        .collect();
                }
                QueryItem::Row(row) => rows.push(row),
                _ => {}
            }
        }

        Ok((rows, columns))
    }

    fn row_to_json(row: &Row) -> serde_json::Value {
        let mut obj = serde_json::Map::new();

        for (i, col) in row.columns().iter().enumerate() {
            let key = col.name().to_string();

            let value = if let Ok(Some(v)) = row.try_get::<&str, _>(i) {
                serde_json::Value::String(v.to_string())
            } else if let Ok(Some(v)) = row.try_get::<i16, _>(i) {
                serde_json::Value::String(v.to_string())
            } else if let Ok(Some(v)) = row.try_get::<i32, _>(i) {
                serde_json::Value::String(v.to_string())
            } else if let Ok(Some(v)) = row.try_get::<i64, _>(i) {
                serde_json::Value::String(v.to_string())
            } else if let Ok(Some(v)) = row.try_get::<u8, _>(i) {
                serde_json::Value::String(v.to_string())
            } else if let Ok(Some(v)) = row.try_get::<Decimal, _>(i) {
                serde_json::Value::String(v.to_string())
            } else if let Ok(Some(v)) = row.try_get::<tiberius::numeric::Numeric, _>(i) {
                serde_json::Value::String(v.to_string())
            } else if let Ok(Some(v)) = row.try_get::<f32, _>(i) {
                serde_json::Number::from_f64(v as f64)
                    .map(serde_json::Value::Number)
                    .unwrap_or(serde_json::Value::Null)
            } else if let Ok(Some(v)) = row.try_get::<f64, _>(i) {
                serde_json::Number::from_f64(v)
                    .map(serde_json::Value::Number)
                    .unwrap_or(serde_json::Value::Null)
            } else if let Ok(Some(v)) = row.try_get::<bool, _>(i) {
                serde_json::Value::Bool(v)
            } else if let Ok(Some(v)) = row.try_get::<uuid::Uuid, _>(i) {
                serde_json::Value::String(v.to_string())
            } else if let Ok(Some(v)) = row.try_get::<NaiveDateTime, _>(i) {
                serde_json::Value::String(v.to_string())
            } else if let Ok(Some(v)) = row.try_get::<NaiveDate, _>(i) {
                serde_json::Value::String(v.to_string())
            } else if let Ok(Some(v)) = row.try_get::<NaiveTime, _>(i) {
                serde_json::Value::String(v.to_string())
            } else if let Ok(Some(v)) = row.try_get::<DateTime<Utc>, _>(i) {
                serde_json::Value::String(v.to_rfc3339())
            } else if let Ok(Some(v)) = row.try_get::<DateTime<FixedOffset>, _>(i) {
                serde_json::Value::String(v.to_rfc3339())
            } else if let Ok(Some(v)) = row.try_get::<&[u8], _>(i) {
                serde_json::Value::String(String::from_utf8_lossy(v).to_string())
            } else {
                serde_json::Value::Null
            };

            obj.insert(key, value);
        }

        serde_json::Value::Object(obj)
    }

    fn parse_i64(row: &Row, idx: usize) -> i64 {
        if let Ok(Some(v)) = row.try_get::<i64, _>(idx) {
            return v;
        }
        if let Ok(Some(v)) = row.try_get::<i32, _>(idx) {
            return v as i64;
        }
        if let Ok(Some(v)) = row.try_get::<&str, _>(idx) {
            return v.parse::<i64>().unwrap_or(0);
        }
        0
    }

    fn parse_string(row: &Row, idx: usize) -> String {
        if let Ok(Some(v)) = row.try_get::<&str, _>(idx) {
            return v.to_string();
        }
        if let Ok(Some(v)) = row.try_get::<&[u8], _>(idx) {
            return String::from_utf8_lossy(v).to_string();
        }
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::quote_ident;

    #[test]
    fn quote_ident_allows_common_mssql_names() {
        assert_eq!(
            quote_ident("order-detail 2026").unwrap(),
            "[order-detail 2026]"
        );
        assert_eq!(quote_ident("用户表").unwrap(), "[用户表]");
    }

    #[test]
    fn quote_ident_escapes_bracket_and_trims() {
        assert_eq!(quote_ident("  a]b ").unwrap(), "[a]]b]");
    }

    #[test]
    fn quote_ident_rejects_empty_and_null_byte() {
        assert!(quote_ident("   ").is_err());
        assert!(quote_ident("abc\0def").is_err());
    }
}

#[async_trait]
impl DatabaseDriver for MssqlDriver {
    async fn close(&self) {}

    async fn test_connection(&self) -> Result<(), String> {
        let rows = self.fetch_rows("SELECT 1").await?;
        if rows.is_empty() {
            return Err("[CONN_FAILED] Empty response".to_string());
        }
        Ok(())
    }

    async fn list_databases(&self) -> Result<Vec<String>, String> {
        let rows = self
            .fetch_rows(
                "SELECT name FROM sys.databases WHERE state = 0 AND name NOT IN ('tempdb') ORDER BY name",
            )
            .await?;

        Ok(rows
            .iter()
            .map(|row| Self::parse_string(row, 0))
            .filter(|s| !s.is_empty())
            .collect())
    }

    async fn list_tables(&self, schema: Option<String>) -> Result<Vec<TableInfo>, String> {
        let schema_filter = schema
            .filter(|s| !s.trim().is_empty())
            .map(|s| format!("AND s.name = '{}'", escape_literal(s.trim())));

        let sql = format!(
            "SELECT s.name AS schema_name, o.name AS table_name, CASE WHEN o.type = 'V' THEN 'view' ELSE 'table' END AS table_type \
             FROM sys.objects o \
             JOIN sys.schemas s ON s.schema_id = o.schema_id \
             WHERE o.type IN ('U','V') {} \
             ORDER BY s.name, o.name",
            schema_filter.unwrap_or_default(),
        );
        let rows = self.fetch_rows(&sql).await?;

        Ok(rows
            .into_iter()
            .map(|row| TableInfo {
                schema: Self::parse_string(&row, 0),
                name: Self::parse_string(&row, 1),
                r#type: Self::parse_string(&row, 2),
            })
            .collect())
    }

    async fn get_table_structure(
        &self,
        schema: String,
        table: String,
    ) -> Result<TableStructure, String> {
        let pk_sql = format!(
            "SELECT kcu.COLUMN_NAME \
             FROM INFORMATION_SCHEMA.TABLE_CONSTRAINTS tc \
             JOIN INFORMATION_SCHEMA.KEY_COLUMN_USAGE kcu \
               ON tc.CONSTRAINT_NAME = kcu.CONSTRAINT_NAME \
              AND tc.TABLE_SCHEMA = kcu.TABLE_SCHEMA \
              AND tc.TABLE_NAME = kcu.TABLE_NAME \
             WHERE tc.CONSTRAINT_TYPE = 'PRIMARY KEY' \
               AND tc.TABLE_SCHEMA = '{}' \
               AND tc.TABLE_NAME = '{}'",
            escape_literal(&schema),
            escape_literal(&table)
        );
        let pk_rows = self.fetch_rows(&pk_sql).await?;
        let pk_set: HashSet<String> = pk_rows
            .iter()
            .map(|row| Self::parse_string(row, 0))
            .collect();

        let sql = format!(
            "SELECT COLUMN_NAME, DATA_TYPE, IS_NULLABLE, COLUMN_DEFAULT \
             FROM INFORMATION_SCHEMA.COLUMNS \
             WHERE TABLE_SCHEMA = '{}' AND TABLE_NAME = '{}' \
             ORDER BY ORDINAL_POSITION",
            escape_literal(&schema),
            escape_literal(&table)
        );

        let rows = self.fetch_rows(&sql).await?;
        let mut columns = Vec::new();
        for row in rows {
            let name = Self::parse_string(&row, 0);
            let default_raw = Self::parse_string(&row, 3);
            columns.push(ColumnInfo {
                name: name.clone(),
                r#type: Self::parse_string(&row, 1),
                nullable: Self::parse_string(&row, 2).eq_ignore_ascii_case("YES"),
                default_value: if default_raw.is_empty() {
                    None
                } else {
                    Some(default_raw)
                },
                primary_key: pk_set.contains(&name),
                comment: None,
            });
        }

        Ok(TableStructure { columns })
    }

    async fn get_table_metadata(
        &self,
        schema: String,
        table: String,
    ) -> Result<TableMetadata, String> {
        let columns = self
            .get_table_structure(schema.clone(), table.clone())
            .await?
            .columns;

        let index_sql = format!(
            "SELECT i.name AS index_name, i.is_unique, i.type_desc, ic.key_ordinal, c.name AS column_name \
             FROM sys.indexes i \
             JOIN sys.index_columns ic ON i.object_id = ic.object_id AND i.index_id = ic.index_id \
             JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id \
             JOIN sys.tables t ON t.object_id = i.object_id \
             JOIN sys.schemas s ON s.schema_id = t.schema_id \
             WHERE s.name = '{}' AND t.name = '{}' AND i.name IS NOT NULL \
             ORDER BY i.name, ic.key_ordinal",
            escape_literal(&schema),
            escape_literal(&table)
        );
        let idx_rows = self.fetch_rows(&index_sql).await?;
        let mut idx_map: HashMap<String, (bool, Option<String>, Vec<(i64, String)>)> =
            HashMap::new();
        for row in idx_rows {
            let name = Self::parse_string(&row, 0);
            let unique = Self::parse_i64(&row, 1) == 1;
            let idx_type = Self::parse_string(&row, 2);
            let ord = Self::parse_i64(&row, 3);
            let col_name = Self::parse_string(&row, 4);
            let entry = idx_map
                .entry(name)
                .or_insert((unique, Some(idx_type.clone()), Vec::new()));
            entry.0 = unique;
            if entry.1.is_none() && !idx_type.is_empty() {
                entry.1 = Some(idx_type);
            }
            entry.2.push((ord, col_name));
        }
        let mut indexes = idx_map
            .into_iter()
            .map(|(name, (unique, index_type, mut cols))| {
                cols.sort_by_key(|(ord, _)| *ord);
                IndexInfo {
                    name,
                    unique,
                    index_type,
                    columns: cols.into_iter().map(|(_, col)| col).collect(),
                }
            })
            .collect::<Vec<_>>();
        indexes.sort_by(|a, b| a.name.cmp(&b.name));

        let fk_sql = format!(
            "SELECT fk.name AS fk_name, pc.name AS parent_col, rs.name AS referenced_schema, rt.name AS referenced_table, rc.name AS referenced_col, fk.update_referential_action_desc, fk.delete_referential_action_desc \
             FROM sys.foreign_keys fk \
             JOIN sys.foreign_key_columns fkc ON fk.object_id = fkc.constraint_object_id \
             JOIN sys.tables pt ON pt.object_id = fk.parent_object_id \
             JOIN sys.schemas ps ON ps.schema_id = pt.schema_id \
             JOIN sys.columns pc ON pc.object_id = pt.object_id AND pc.column_id = fkc.parent_column_id \
             JOIN sys.tables rt ON rt.object_id = fk.referenced_object_id \
             JOIN sys.schemas rs ON rs.schema_id = rt.schema_id \
             JOIN sys.columns rc ON rc.object_id = rt.object_id AND rc.column_id = fkc.referenced_column_id \
             WHERE ps.name = '{}' AND pt.name = '{}' \
             ORDER BY fk.name, fkc.constraint_column_id",
            escape_literal(&schema),
            escape_literal(&table)
        );
        let fk_rows = self.fetch_rows(&fk_sql).await?;
        let mut foreign_keys = Vec::new();
        for row in fk_rows {
            foreign_keys.push(ForeignKeyInfo {
                name: Self::parse_string(&row, 0),
                column: Self::parse_string(&row, 1),
                referenced_schema: Some(Self::parse_string(&row, 2)),
                referenced_table: Self::parse_string(&row, 3),
                referenced_column: Self::parse_string(&row, 4),
                on_update: Some(Self::parse_string(&row, 5)),
                on_delete: Some(Self::parse_string(&row, 6)),
            });
        }

        Ok(TableMetadata {
            columns,
            indexes,
            foreign_keys,
            clickhouse_extra: None,
        })
    }

    async fn get_table_ddl(&self, schema: String, table: String) -> Result<String, String> {
        let structure = self
            .get_table_structure(schema.clone(), table.clone())
            .await?;

        let mut lines = Vec::new();
        let mut pk_cols = Vec::new();
        for c in &structure.columns {
            let mut line = format!("    {} {}", quote_ident(&c.name)?, c.r#type);
            if !c.nullable {
                line.push_str(" NOT NULL");
            }
            if let Some(default_value) = &c.default_value {
                line.push_str(" DEFAULT ");
                line.push_str(default_value);
            }
            lines.push(line);
            if c.primary_key {
                pk_cols.push(quote_ident(&c.name)?);
            }
        }

        if !pk_cols.is_empty() {
            lines.push(format!("    PRIMARY KEY ({})", pk_cols.join(", ")));
        }

        let ddl = format!(
            "CREATE TABLE {}.{} (\n{}\n);",
            quote_ident(&schema)?,
            quote_ident(&table)?,
            lines.join(",\n")
        );

        Ok(ddl)
    }

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
    ) -> Result<TableDataResponse, String> {
        let start = std::time::Instant::now();
        let safe_page = if page < 1 { 1 } else { page };
        let safe_limit = if limit < 1 { 100 } else { limit };
        let offset = (safe_page - 1) * safe_limit;
        let qualified = table_ref(&schema, &table)?;

        let filter = filter.map(|f| super::normalize_quotes(&f));
        let order_by = order_by.map(|f| super::normalize_quotes(&f));

        let where_clause = match &filter {
            Some(f) if !f.trim().is_empty() => format!(" WHERE {}", f.trim()),
            _ => String::new(),
        };

        let count_sql = format!(
            "SELECT COUNT_BIG(1) AS total FROM {}{}",
            qualified, where_clause
        );
        let count_rows = self.fetch_rows(&count_sql).await?;
        let total = count_rows
            .first()
            .map(|row| Self::parse_i64(row, 0))
            .unwrap_or(0);

        let order_clause = if let Some(ref raw) = order_by {
            if raw.trim().is_empty() {
                " ORDER BY (SELECT NULL)".to_string()
            } else {
                format!(" ORDER BY {}", raw.trim())
            }
        } else if let Some(ref col) = sort_column {
            let dir = if matches!(sort_direction.as_deref(), Some("desc")) {
                "DESC"
            } else {
                "ASC"
            };
            format!(" ORDER BY {} {}", quote_ident(col)?, dir)
        } else {
            " ORDER BY (SELECT NULL)".to_string()
        };

        let sql = format!(
            "SELECT * FROM {}{}{} OFFSET {} ROWS FETCH NEXT {} ROWS ONLY",
            qualified, where_clause, order_clause, offset, safe_limit
        );
        let rows = self.fetch_rows(&sql).await?;
        let data = rows.iter().map(Self::row_to_json).collect::<Vec<_>>();

        Ok(TableDataResponse {
            data,
            total,
            page: safe_page,
            limit: safe_limit,
            execution_time_ms: start.elapsed().as_millis() as i64,
        })
    }

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
    ) -> Result<TableDataResponse, String> {
        self.get_table_data(
            schema,
            table,
            page,
            limit,
            sort_column,
            sort_direction,
            filter,
            order_by,
        )
        .await
    }

    async fn execute_query(&self, sql: String) -> Result<QueryResult, String> {
        let start = std::time::Instant::now();
        let first_keyword = first_sql_keyword(&sql);
        let is_read_query = matches!(
            first_keyword.as_deref(),
            Some("select") | Some("with") | Some("show")
        );

        if is_read_query {
            let (rows, columns) = self.fetch_rows_with_columns(&sql).await?;
            let data = rows.iter().map(Self::row_to_json).collect::<Vec<_>>();

            return Ok(QueryResult {
                row_count: data.len() as i64,
                data,
                columns,
                time_taken_ms: start.elapsed().as_millis() as i64,
                success: true,
                error: None,
            });
        }

        let mut client = self.connect_client().await?;
        let result = client
            .execute(&sql, &[])
            .await
            .map_err(|e| format!("[QUERY_ERROR] {}", e))?;
        let row_count = result.rows_affected().iter().sum::<u64>() as i64;

        Ok(QueryResult {
            data: vec![],
            row_count,
            columns: vec![],
            time_taken_ms: start.elapsed().as_millis() as i64,
            success: true,
            error: None,
        })
    }

    async fn get_schema_overview(&self, schema: Option<String>) -> Result<SchemaOverview, String> {
        let sql = if let Some(schema_name) = schema.filter(|s| !s.trim().is_empty()) {
            format!(
                "SELECT TABLE_SCHEMA, TABLE_NAME, COLUMN_NAME, DATA_TYPE \
                 FROM INFORMATION_SCHEMA.COLUMNS \
                 WHERE TABLE_SCHEMA = '{}' \
                 ORDER BY TABLE_SCHEMA, TABLE_NAME, ORDINAL_POSITION",
                escape_literal(schema_name.trim())
            )
        } else {
            "SELECT TABLE_SCHEMA, TABLE_NAME, COLUMN_NAME, DATA_TYPE \
             FROM INFORMATION_SCHEMA.COLUMNS \
             WHERE TABLE_SCHEMA NOT IN ('INFORMATION_SCHEMA', 'sys') \
             ORDER BY TABLE_SCHEMA, TABLE_NAME, ORDINAL_POSITION"
                .to_string()
        };

        let rows = self.fetch_rows(&sql).await?;
        let mut table_map: HashMap<(String, String), Vec<ColumnSchema>> = HashMap::new();

        for row in rows {
            let schema_name = Self::parse_string(&row, 0);
            let table_name = Self::parse_string(&row, 1);
            let col_name = Self::parse_string(&row, 2);
            let col_type = Self::parse_string(&row, 3);

            table_map
                .entry((schema_name, table_name))
                .or_default()
                .push(ColumnSchema {
                    name: col_name,
                    r#type: col_type,
                });
        }

        let mut tables = table_map
            .into_iter()
            .map(|((schema, name), columns)| TableSchema {
                schema,
                name,
                columns,
            })
            .collect::<Vec<_>>();

        tables.sort_by(|a, b| a.schema.cmp(&b.schema).then(a.name.cmp(&b.name)));
        Ok(SchemaOverview { tables })
    }
}
