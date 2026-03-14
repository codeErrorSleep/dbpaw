use super::DatabaseDriver;
use crate::models::{
    ColumnInfo, ColumnSchema, ConnectionForm, ForeignKeyInfo, IndexInfo, QueryColumn, QueryResult,
    SchemaOverview, TableDataResponse, TableInfo, TableMetadata, TableSchema, TableStructure,
};
use async_trait::async_trait;
use futures_util::TryStreamExt;
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

    async fn load_table_column_type_map(
        &self,
        schema: &str,
        table: &str,
    ) -> Result<HashMap<String, String>, String> {
        let sql = format!(
            "SELECT COLUMN_NAME, DATA_TYPE \
             FROM INFORMATION_SCHEMA.COLUMNS \
             WHERE TABLE_SCHEMA = '{}' AND TABLE_NAME = '{}'",
            escape_literal(schema),
            escape_literal(table)
        );
        let rows = self.fetch_rows(&sql).await?;
        let mut map = HashMap::new();
        for row in rows {
            let col = Self::parse_string(&row, 0);
            let data_type = Self::parse_string(&row, 1);
            if !col.is_empty() {
                map.insert(col, data_type);
            }
        }
        Ok(map)
    }

    fn build_for_json_query(sql: &str) -> String {
        let trimmed = sql.trim_end().trim_end_matches(';').trim_end();
        format!("{trimmed} FOR JSON PATH, INCLUDE_NULL_VALUES")
    }

    async fn fetch_json_rows(
        &self,
        sql: &str,
        high_precision_cols: &HashSet<String>,
    ) -> Result<Vec<serde_json::Value>, String> {
        let rows = self.fetch_rows(sql).await?;
        let mut json_text = String::new();
        for row in rows {
            json_text.push_str(&Self::parse_string(&row, 0));
        }
        if json_text.trim().is_empty() {
            return Ok(Vec::new());
        }
        let parsed: serde_json::Value = serde_json::from_str(&json_text)
            .map_err(|e| format!("[QUERY_ERROR] Failed to parse MSSQL JSON result: {e}"))?;
        let mut out = match parsed {
            serde_json::Value::Array(arr) => arr,
            serde_json::Value::Object(obj) => vec![serde_json::Value::Object(obj)],
            _ => {
                return Err("[QUERY_ERROR] MSSQL FOR JSON result is not array/object".to_string());
            }
        };
        for row in &mut out {
            normalize_mssql_row_json(row, high_precision_cols)?;
        }
        Ok(out)
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

fn is_high_precision_mssql_data_type(data_type: &str) -> bool {
    matches!(
        data_type.trim().to_ascii_lowercase().as_str(),
        "bigint" | "decimal" | "numeric" | "money" | "smallmoney"
    )
}

fn is_high_precision_mssql_query_type(type_name: &str) -> bool {
    let t = type_name.trim().to_ascii_lowercase();
    t.contains("int8")
        || t.contains("bigint")
        || t.contains("numeric")
        || t.contains("decimal")
        || t.contains("money")
}

fn normalize_mssql_row_json(
    row_json: &mut serde_json::Value,
    high_precision_cols: &HashSet<String>,
) -> Result<(), String> {
    let obj = row_json
        .as_object_mut()
        .ok_or("[QUERY_ERROR] Expected JSON object row from MSSQL FOR JSON".to_string())?;

    let mut lookup: HashMap<String, String> = HashMap::new();
    for key in obj.keys() {
        lookup.insert(key.to_ascii_lowercase(), key.clone());
    }

    for col in high_precision_cols {
        let Some(actual_key) = lookup.get(&col.to_ascii_lowercase()) else {
            continue;
        };
        let Some(v) = obj.get_mut(actual_key) else {
            continue;
        };
        if v.is_number() {
            *v = serde_json::Value::String(v.to_string());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        is_high_precision_mssql_data_type, is_high_precision_mssql_query_type, quote_ident,
        MssqlDriver,
    };
    use std::collections::HashSet;

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

    #[test]
    fn test_is_high_precision_mssql_data_type() {
        assert!(is_high_precision_mssql_data_type("bigint"));
        assert!(is_high_precision_mssql_data_type("DECIMAL"));
        assert!(is_high_precision_mssql_data_type("money"));
        assert!(!is_high_precision_mssql_data_type("int"));
    }

    #[test]
    fn test_is_high_precision_mssql_query_type() {
        assert!(is_high_precision_mssql_query_type("Int8"));
        assert!(is_high_precision_mssql_query_type("Numericn"));
        assert!(is_high_precision_mssql_query_type("Money"));
        assert!(!is_high_precision_mssql_query_type("Int4"));
    }

    #[test]
    fn test_normalize_mssql_row_json_stringify_high_precision() {
        let mut row = serde_json::json!({
            "id": 9223372036854775807_i64,
            "amount": 1234.56,
            "name": "x"
        });
        let hp = HashSet::from(["ID".to_string(), "amount".to_string()]);
        super::normalize_mssql_row_json(&mut row, &hp).unwrap();
        assert_eq!(row.get("id").and_then(|v| v.as_str()), Some("9223372036854775807"));
        assert_eq!(row.get("amount").and_then(|v| v.as_str()), Some("1234.56"));
        assert_eq!(row.get("name").and_then(|v| v.as_str()), Some("x"));
    }

    #[test]
    fn test_build_for_json_query_trims_trailing_semicolon() {
        let sql = "SELECT id, name FROM dbo.users;";
        assert_eq!(
            MssqlDriver::build_for_json_query(sql),
            "SELECT id, name FROM dbo.users FOR JSON PATH, INCLUDE_NULL_VALUES"
        );
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

        let col_type_map = self.load_table_column_type_map(&schema, &table).await?;
        let high_precision_cols: HashSet<String> = col_type_map
            .into_iter()
            .filter(|(_, data_type)| is_high_precision_mssql_data_type(data_type))
            .map(|(col, _)| col)
            .collect();

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
        let json_sql = Self::build_for_json_query(&sql);
        let data = self.fetch_json_rows(&json_sql, &high_precision_cols).await?;

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
            let (_, columns) = self.fetch_rows_with_columns(&sql).await?;
            let high_precision_cols: HashSet<String> = columns
                .iter()
                .filter(|col| is_high_precision_mssql_query_type(&col.r#type))
                .map(|col| col.name.clone())
                .collect();
            let json_sql = Self::build_for_json_query(&sql);
            let data = self.fetch_json_rows(&json_sql, &high_precision_cols).await?;

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
