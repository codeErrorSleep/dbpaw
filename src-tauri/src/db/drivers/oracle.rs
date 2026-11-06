use super::{
    DatabaseDriver, DriverCapabilities, DriverResult, ForeignKeyDriver, PackageDriver,
    SequenceDriver, TypeDriver, conn_failed_error,
};
use crate::error::AppError;
use crate::models::{
    ColumnInfo, ColumnSchema, ConnectionForm, ForeignKeyInfo, IndexInfo, PackageInfo, QueryColumn,
    QueryResult, SchemaForeignKey, SchemaOverview, SequenceInfo, SingleResultSet,
    TableDataResponse, TableInfo, TableMetadata, TableSchema, TableStructure, TypeInfo,
};
use async_trait::async_trait;
use std::collections::HashMap;

pub struct OracleDriver {
    config: OracleConfig,
    _network_tunnel: Option<crate::network::NetworkTunnel>,
}

#[derive(Clone)]
struct OracleConfig {
    host: String,
    port: u16,
    /// Oracle Easy Connect service name (e.g. "ORCL", "FREE", "XE")
    service_name: String,
    username: String,
    password: String,
}

fn build_connect_string(cfg: &OracleConfig) -> String {
    format!("//{}:{}/{}", cfg.host, cfg.port, cfg.service_name)
}

/// Oracle uses double-quote identifiers. Upper-case is the Oracle default.
fn quote_ident(ident: &str) -> String {
    format!("\"{}\"", ident.replace('"', "\"\""))
}

fn escape_literal(value: &str) -> String {
    value.replace('\'', "''")
}

/// Convert a single Oracle column value (by index) into a `serde_json::Value`.
///
/// Strategy: try integer → float → bytes → string → null. This cascade avoids
/// the need to inspect `OracleType` while still producing sensible JSON types
/// for the common Oracle types (NUMBER, VARCHAR2, DATE, TIMESTAMP, CLOB, BLOB).
///
/// Precision note: very large NUMBER values (> i64::MAX) fall through to f64,
/// which may lose precision. This is acceptable for a v1 display client.
fn oracle_value_to_json(row: &oracle::Row, idx: usize) -> serde_json::Value {
    // Try integer (covers NUMBER(p,0), INTEGER, SMALLINT, etc.)
    match row.get::<_, Option<i64>>(idx) {
        Ok(None) => return serde_json::Value::Null,
        Ok(Some(v)) => return serde_json::Value::Number(v.into()),
        Err(_) => {}
    }
    // Try float (covers NUMBER(p,s) with fractional part, FLOAT, BINARY_FLOAT, etc.)
    match row.get::<_, Option<f64>>(idx) {
        Ok(None) => return serde_json::Value::Null,
        Ok(Some(v)) => {
            if let Some(n) = serde_json::Number::from_f64(v) {
                return serde_json::Value::Number(n);
            }
            return serde_json::Value::String(v.to_string());
        }
        Err(_) => {}
    }
    // Try bytes (covers BLOB, RAW — returned as hex string)
    match row.get::<_, Option<Vec<u8>>>(idx) {
        Ok(None) => return serde_json::Value::Null,
        Ok(Some(v)) => {
            return serde_json::Value::String(v.iter().map(|b| format!("{b:02x}")).collect());
        }
        Err(_) => {}
    }
    // Try string (covers VARCHAR2, NVARCHAR2, CHAR, DATE, TIMESTAMP, CLOB, etc.)
    match row.get::<_, Option<String>>(idx) {
        Ok(None) => serde_json::Value::Null,
        Ok(Some(v)) => serde_json::Value::String(v),
        Err(_) => serde_json::Value::Null,
    }
}

impl OracleDriver {
    pub async fn connect(form: &ConnectionForm) -> DriverResult<Self> {
        let mut effective_form = form.clone();
        let mut network_tunnel = None;

        if crate::network::tunnel_enabled(form) {
            let tunnel = crate::network::start_network_tunnel(form).await?;
            effective_form.host = Some("127.0.0.1".to_string());
            effective_form.port = Some(tunnel.local_port as i64);
            network_tunnel = Some(tunnel);
        }

        let host = effective_form
            .host
            .clone()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .ok_or_else(|| AppError::validation("host cannot be empty"))?;
        let port = effective_form.port.unwrap_or(1521);
        if !(1..=65535).contains(&port) {
            return Err(AppError::validation("port out of range"));
        }
        let service_name = effective_form
            .database
            .clone()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "ORCL".to_string());
        let username = effective_form
            .username
            .clone()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .ok_or_else(|| AppError::validation("username cannot be empty"))?;
        let password = effective_form.password.clone().unwrap_or_default();

        let config = OracleConfig {
            host,
            port: port as u16,
            service_name,
            username,
            password,
        };
        let driver = Self {
            config,
            _network_tunnel: network_tunnel,
        };
        driver.test_connection().await?;
        Ok(driver)
    }

    /// Run a blocking Oracle OCI call on tokio's blocking thread pool.
    ///
    /// A fresh `oracle::Connection` is created for each call (reconnect-per-call
    /// pattern). This avoids the complexity of sharing a `!Sync` OCI handle
    /// across async tasks at the cost of a new TCP handshake per driver method.
    /// For a desktop database-client with low concurrency this is acceptable.
    ///
    /// DML statements are followed by an explicit `conn.commit()` so that the
    /// work is persisted even though the connection is dropped at the end of
    /// the closure.
    async fn run_blocking<F, T>(&self, f: F) -> DriverResult<T>
    where
        F: FnOnce(oracle::Connection) -> DriverResult<T> + Send + 'static,
        T: Send + 'static,
    {
        let cfg = self.config.clone();
        tokio::task::spawn_blocking(move || {
            let connect_string = build_connect_string(&cfg);
            let conn = oracle::Connection::connect(&cfg.username, &cfg.password, &connect_string)
                .map_err(|e| conn_failed_error(&e))?;
            f(conn)
        })
        .await
        .map_err(|e| AppError::query_failed(format!("Oracle blocking task failed: {e}")))?
    }
}

#[async_trait]
impl DatabaseDriver for OracleDriver {
    fn capabilities(&self) -> DriverCapabilities {
        DriverCapabilities::SEQUENCES
            | DriverCapabilities::TYPES
            | DriverCapabilities::PACKAGES
            | DriverCapabilities::FOREIGN_KEYS
    }

    async fn close(&self) {
        // No persistent connection to close.
    }

    async fn test_connection(&self) -> DriverResult<()> {
        self.run_blocking(|conn| {
            conn.query("SELECT 1 FROM DUAL", &[] as &[&dyn oracle::sql_type::ToSql])
                .map_err(|e| conn_failed_error(&e))?
                .next()
                .ok_or_else(|| {
                    AppError::conn_failed("Empty response from DUAL", "Check connection settings")
                })?
                .map_err(|e| conn_failed_error(&e))?;
            Ok(())
        })
        .await
    }

    /// In Oracle, "databases" map to schemas (users visible via ALL_USERS).
    async fn list_databases(&self) -> DriverResult<Vec<String>> {
        self.run_blocking(|conn| {
            let rows = conn
                .query(
                    "SELECT USERNAME FROM ALL_USERS ORDER BY USERNAME",
                    &[] as &[&dyn oracle::sql_type::ToSql],
                )
                .map_err(|e| AppError::query_failed(format!("{e}")))?;
            let mut result = Vec::new();
            for row_result in rows {
                let row = row_result.map_err(|e| AppError::query_failed(format!("{e}")))?;
                let name: Option<String> = row.get(0).ok().flatten();
                if let Some(n) = name {
                    if !n.is_empty() {
                        result.push(n);
                    }
                }
            }
            Ok(result)
        })
        .await
    }

    async fn list_schemas(&self) -> DriverResult<Vec<String>> {
        self.list_databases().await
    }

    async fn list_tables(&self, schema: Option<String>) -> DriverResult<Vec<TableInfo>> {
        let schema_upper = schema
            .map(|s| s.trim().to_uppercase())
            .filter(|s| !s.is_empty());
        self.run_blocking(move |conn| {
            let sql = if let Some(ref s) = schema_upper {
                format!(
                    "SELECT OWNER, TABLE_NAME, 'table' AS TABLE_TYPE \
                     FROM ALL_TABLES WHERE OWNER = '{}' \
                     UNION ALL \
                     SELECT OWNER, VIEW_NAME, 'view' \
                     FROM ALL_VIEWS WHERE OWNER = '{}' \
                     ORDER BY 1, 2",
                    escape_literal(s),
                    escape_literal(s),
                )
            } else {
                "SELECT OWNER, TABLE_NAME, 'table' AS TABLE_TYPE \
                 FROM ALL_TABLES \
                 UNION ALL \
                 SELECT OWNER, VIEW_NAME, 'view' \
                 FROM ALL_VIEWS \
                 ORDER BY 1, 2"
                    .to_string()
            };
            let rows = conn
                .query(&sql, &[] as &[&dyn oracle::sql_type::ToSql])
                .map_err(|e| AppError::query_failed(format!("{e}")))?;
            let mut result = Vec::new();
            for row_result in rows {
                let row = row_result.map_err(|e| AppError::query_failed(format!("{e}")))?;
                let schema_name: Option<String> = row.get(0).ok().flatten();
                let table_name: Option<String> = row.get(1).ok().flatten();
                let table_type: Option<String> = row.get(2).ok().flatten();
                if let (Some(s), Some(t), Some(ty)) = (schema_name, table_name, table_type) {
                    result.push(TableInfo {
                        schema: s,
                        name: t,
                        r#type: ty,
                    });
                }
            }
            Ok(result)
        })
        .await
    }

    async fn get_table_structure(
        &self,
        schema: String,
        table: String,
    ) -> DriverResult<TableStructure> {
        self.run_blocking(move |conn| {
            // Primary keys
            let pk_sql = format!(
                "SELECT ac.COLUMN_NAME \
                 FROM ALL_CONSTRAINTS con \
                 JOIN ALL_CONS_COLUMNS ac \
                   ON con.CONSTRAINT_NAME = ac.CONSTRAINT_NAME \
                  AND con.OWNER = ac.OWNER \
                 WHERE con.CONSTRAINT_TYPE = 'P' \
                   AND con.OWNER = '{}' \
                   AND con.TABLE_NAME = '{}' \
                 ORDER BY ac.POSITION",
                escape_literal(&schema.to_uppercase()),
                escape_literal(&table.to_uppercase()),
            );
            let pk_rows = conn
                .query(&pk_sql, &[] as &[&dyn oracle::sql_type::ToSql])
                .map_err(|e| AppError::query_failed(format!("{e}")))?;
            let mut pk_set = std::collections::HashSet::<String>::new();
            for row_result in pk_rows {
                let row = row_result.map_err(|e| AppError::query_failed(format!("{e}")))?;
                let col: Option<String> = row.get(0).ok().flatten();
                if let Some(c) = col {
                    pk_set.insert(c);
                }
            }

            // Columns
            let col_sql = format!(
                "SELECT \
                    COLUMN_NAME, \
                    DATA_TYPE || \
                    CASE \
                        WHEN DATA_TYPE IN ('VARCHAR2','NVARCHAR2','CHAR','NCHAR') \
                            THEN '(' || CHAR_LENGTH || ')' \
                        WHEN DATA_TYPE = 'NUMBER' AND DATA_PRECISION IS NOT NULL \
                            THEN '(' || DATA_PRECISION || \
                                 CASE WHEN DATA_SCALE > 0 THEN ',' || DATA_SCALE ELSE '' END \
                                 || ')' \
                        ELSE '' \
                    END AS FULL_TYPE, \
                    NULLABLE, \
                    DATA_DEFAULT \
                 FROM ALL_TAB_COLUMNS \
                 WHERE OWNER = '{}' AND TABLE_NAME = '{}' \
                 ORDER BY COLUMN_ID",
                escape_literal(&schema.to_uppercase()),
                escape_literal(&table.to_uppercase()),
            );
            let col_rows = conn
                .query(&col_sql, &[] as &[&dyn oracle::sql_type::ToSql])
                .map_err(|e| AppError::query_failed(format!("{e}")))?;
            let mut columns = Vec::new();
            for row_result in col_rows {
                let row = row_result.map_err(|e| AppError::query_failed(format!("{e}")))?;
                let name: Option<String> = row.get(0).ok().flatten();
                let col_type: Option<String> = row.get(1).ok().flatten();
                let nullable: Option<String> = row.get(2).ok().flatten();
                let default_val: Option<String> = row.get(3).ok().flatten();
                if let (Some(name), Some(col_type)) = (name, col_type) {
                    let is_nullable = nullable.as_deref() != Some("N");
                    let default_value = default_val
                        .map(|d| d.trim().to_string())
                        .filter(|d| !d.is_empty());
                    let primary_key = pk_set.contains(&name);
                    columns.push(ColumnInfo {
                        name,
                        r#type: col_type,
                        nullable: is_nullable,
                        default_value,
                        primary_key,
                        comment: None,
                        default_constraint_name: None,
                    });
                }
            }
            Ok(TableStructure { columns })
        })
        .await
    }

    async fn get_table_metadata(
        &self,
        schema: String,
        table: String,
    ) -> DriverResult<TableMetadata> {
        let columns = self
            .get_table_structure(schema.clone(), table.clone())
            .await?
            .columns;

        let (indexes, foreign_keys) = self
            .run_blocking(move |conn| {
                // Indexes
                let idx_sql = format!(
                    "SELECT i.INDEX_NAME, \
                            CASE WHEN i.UNIQUENESS = 'UNIQUE' THEN 1 ELSE 0 END AS IS_UNIQUE, \
                            i.INDEX_TYPE, \
                            ic.COLUMN_NAME, \
                            ic.COLUMN_POSITION \
                     FROM ALL_INDEXES i \
                     JOIN ALL_IND_COLUMNS ic \
                       ON ic.INDEX_NAME = i.INDEX_NAME \
                      AND ic.TABLE_OWNER = i.TABLE_OWNER \
                     WHERE i.TABLE_OWNER = '{}' \
                       AND i.TABLE_NAME = '{}' \
                     ORDER BY i.INDEX_NAME, ic.COLUMN_POSITION",
                    escape_literal(&schema.to_uppercase()),
                    escape_literal(&table.to_uppercase()),
                );
                let idx_rows = conn
                    .query(&idx_sql, &[] as &[&dyn oracle::sql_type::ToSql])
                    .map_err(|e| AppError::query_failed(format!("{e}")))?;
                let mut idx_map: HashMap<String, (bool, Option<String>, Vec<(i64, String)>)> =
                    HashMap::new();
                for row_result in idx_rows {
                    let row = row_result.map_err(|e| AppError::query_failed(format!("{e}")))?;
                    let idx_name: Option<String> = row.get(0).ok().flatten();
                    let is_unique: Option<i64> = row.get(1).ok().flatten();
                    let idx_type: Option<String> = row.get(2).ok().flatten();
                    let col_name: Option<String> = row.get(3).ok().flatten();
                    let position: Option<i64> = row.get(4).ok().flatten();
                    if let (Some(name), Some(col_name)) = (idx_name, col_name) {
                        let unique = is_unique.unwrap_or(0) == 1;
                        let pos = position.unwrap_or(0);
                        let entry =
                            idx_map
                                .entry(name)
                                .or_insert((unique, idx_type.clone(), Vec::new()));
                        entry.0 = unique;
                        if entry.1.is_none() {
                            entry.1 = idx_type;
                        }
                        entry.2.push((pos, col_name));
                    }
                }
                let mut indexes: Vec<IndexInfo> = idx_map
                    .into_iter()
                    .map(|(name, (unique, index_type, mut cols))| {
                        cols.sort_by_key(|(pos, _)| *pos);
                        IndexInfo {
                            name,
                            unique,
                            index_type,
                            columns: cols.into_iter().map(|(_, c)| c).collect(),
                        }
                    })
                    .collect();
                indexes.sort_by(|a, b| a.name.cmp(&b.name));

                // Foreign keys
                let fk_sql = format!(
                    "SELECT c.CONSTRAINT_NAME, \
                            cc.COLUMN_NAME, \
                            rc.OWNER AS REF_OWNER, \
                            rc.TABLE_NAME AS REF_TABLE, \
                            rcc.COLUMN_NAME AS REF_COLUMN, \
                            c.DELETE_RULE \
                     FROM ALL_CONSTRAINTS c \
                     JOIN ALL_CONS_COLUMNS cc \
                       ON cc.CONSTRAINT_NAME = c.CONSTRAINT_NAME \
                      AND cc.OWNER = c.OWNER \
                     JOIN ALL_CONSTRAINTS rc \
                       ON rc.CONSTRAINT_NAME = c.R_CONSTRAINT_NAME \
                      AND rc.OWNER = c.R_OWNER \
                     JOIN ALL_CONS_COLUMNS rcc \
                       ON rcc.CONSTRAINT_NAME = rc.CONSTRAINT_NAME \
                      AND rcc.OWNER = rc.OWNER \
                      AND rcc.POSITION = cc.POSITION \
                     WHERE c.CONSTRAINT_TYPE = 'R' \
                       AND c.OWNER = '{}' \
                       AND c.TABLE_NAME = '{}' \
                     ORDER BY c.CONSTRAINT_NAME, cc.POSITION",
                    escape_literal(&schema.to_uppercase()),
                    escape_literal(&table.to_uppercase()),
                );
                let fk_rows = conn
                    .query(&fk_sql, &[] as &[&dyn oracle::sql_type::ToSql])
                    .map_err(|e| AppError::query_failed(format!("{e}")))?;
                let mut foreign_keys = Vec::new();
                for row_result in fk_rows {
                    let row = row_result.map_err(|e| AppError::query_failed(format!("{e}")))?;
                    let fk_name: Option<String> = row.get(0).ok().flatten();
                    let col_name: Option<String> = row.get(1).ok().flatten();
                    let ref_schema: Option<String> = row.get(2).ok().flatten();
                    let ref_table: Option<String> = row.get(3).ok().flatten();
                    let ref_col: Option<String> = row.get(4).ok().flatten();
                    let delete_rule: Option<String> = row.get(5).ok().flatten();
                    if let (Some(fk_name), Some(col_name), Some(ref_table), Some(ref_col)) =
                        (fk_name, col_name, ref_table, ref_col)
                    {
                        foreign_keys.push(ForeignKeyInfo {
                            name: fk_name,
                            column: col_name,
                            referenced_schema: ref_schema,
                            referenced_table: ref_table,
                            referenced_column: ref_col,
                            on_update: None, // Oracle does not support ON UPDATE in FK constraints
                            on_delete: delete_rule,
                        });
                    }
                }
                Ok((indexes, foreign_keys))
            })
            .await?;

        Ok(TableMetadata {
            columns,
            indexes,
            foreign_keys,
            clickhouse_extra: None,
            cassandra_extra: None,
            special_type_summaries: vec![],
        })
    }

    /// Returns the table DDL using DBMS_METADATA.GET_DDL.
    /// Requires EXECUTE privilege on DBMS_METADATA (granted to public in most installs).
    async fn get_table_ddl(&self, schema: String, table: String) -> DriverResult<String> {
        self.run_blocking(move |conn| {
            let sql = format!(
                "SELECT DBMS_METADATA.GET_DDL('TABLE', '{}', '{}') AS DDL FROM DUAL",
                escape_literal(&table.to_uppercase()),
                escape_literal(&schema.to_uppercase()),
            );
            let rows = conn
                .query(&sql, &[] as &[&dyn oracle::sql_type::ToSql])
                .map_err(|e| AppError::query_failed(format!("{e}")))?;
            for row_result in rows {
                let row = row_result.map_err(|e| AppError::query_failed(format!("{e}")))?;
                let ddl: Option<String> = row.get(0).ok().flatten();
                if let Some(d) = ddl {
                    return Ok(d.trim().to_string());
                }
            }
            Err(AppError::query_failed(
                "DBMS_METADATA.GET_DDL returned no result",
            ))
        })
        .await
    }

    /// Paginated table data. Requires Oracle 12c+ for OFFSET/FETCH syntax.
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
        include_total: bool,
    ) -> DriverResult<TableDataResponse> {
        let start = std::time::Instant::now();
        let safe_page = if page < 1 { 1 } else { page };
        let safe_limit = if limit < 1 { 100 } else { limit };
        let offset = (safe_page - 1) * safe_limit;

        let filter = filter.map(|f| super::normalize_quotes(&f));
        let order_by = order_by.map(|f| super::normalize_quotes(&f));

        let table_ref = format!(
            "{}.{}",
            quote_ident(&schema.to_uppercase()),
            quote_ident(&table.to_uppercase())
        );

        let where_clause = match &filter {
            Some(f) if !f.trim().is_empty() => format!(" WHERE {}", f.trim()),
            _ => String::new(),
        };

        let order_clause = if let Some(ref raw) = order_by {
            if raw.trim().is_empty() {
                String::new()
            } else {
                format!(" ORDER BY {}", raw.trim())
            }
        } else if let Some(ref col) = sort_column {
            let dir = if matches!(sort_direction.as_deref(), Some("desc")) {
                "DESC"
            } else {
                "ASC"
            };
            format!(" ORDER BY {} {}", quote_ident(col), dir)
        } else {
            String::new()
        };

        self.run_blocking(move |conn| {
            let total = if include_total {
                let count_sql = format!("SELECT COUNT(*) FROM {}{}", table_ref, where_clause);
                let count_rows = conn
                    .query(&count_sql, &[] as &[&dyn oracle::sql_type::ToSql])
                    .map_err(|e| AppError::query_failed(format!("{e}")))?;
                let mut total: i64 = 0;
                for row_result in count_rows {
                    let row = row_result.map_err(|e| AppError::query_failed(format!("{e}")))?;
                    total = row.get::<_, Option<i64>>(0).ok().flatten().unwrap_or(0);
                }
                Some(total)
            } else {
                None
            };

            // Paginated data (Oracle 12c+ OFFSET/FETCH)
            let data_sql = format!(
                "SELECT * FROM {}{}{} OFFSET {} ROWS FETCH NEXT {} ROWS ONLY",
                table_ref, where_clause, order_clause, offset, safe_limit
            );
            let rows = conn
                .query(&data_sql, &[] as &[&dyn oracle::sql_type::ToSql])
                .map_err(|e| AppError::query_failed(format!("{e}")))?;

            // Collect column metadata before consuming rows
            let col_names: Vec<String> = rows
                .column_info()
                .iter()
                .map(|c| c.name().to_string())
                .collect();

            let mut data = Vec::new();
            for row_result in rows {
                let row = row_result.map_err(|e| AppError::query_failed(format!("{e}")))?;
                let mut map = serde_json::Map::new();
                for (i, name) in col_names.iter().enumerate() {
                    map.insert(name.clone(), oracle_value_to_json(&row, i));
                }
                data.push(serde_json::Value::Object(map));
            }

            Ok(TableDataResponse {
                data,
                total,
                page: safe_page,
                limit: safe_limit,
                execution_time_ms: start.elapsed().as_millis() as i64,
            })
        })
        .await
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
    ) -> DriverResult<TableDataResponse> {
        self.get_table_data(
            schema,
            table,
            page,
            limit,
            sort_column,
            sort_direction,
            filter,
            order_by,
            true,
        )
        .await
    }

    async fn execute_query(&self, sql: String) -> DriverResult<QueryResult> {
        let start = std::time::Instant::now();
        let statements = super::split_sql_statements(&sql);
        if statements.is_empty() {
            return Err(AppError::query_failed("Empty SQL statement"));
        }

        // Single statement: keep original behavior
        if statements.len() == 1 {
            let last_sql = statements.last().unwrap().clone();
            return self
                .run_blocking(move |conn| {
                    let sql_clean = super::strip_trailing_statement_terminator(&last_sql);
                    let first_kw = super::first_sql_keyword(sql_clean);
                    let is_read = matches!(
                        first_kw.as_deref(),
                        Some("SELECT") | Some("WITH") | Some("SHOW")
                    );

                    if is_read {
                        let rows = conn
                            .query(sql_clean, &[] as &[&dyn oracle::sql_type::ToSql])
                            .map_err(|e| AppError::query_failed(format!("{e}")))?;

                        let col_info: Vec<(String, String)> = rows
                            .column_info()
                            .iter()
                            .map(|c| (c.name().to_string(), format!("{}", c.oracle_type())))
                            .collect();
                        let columns: Vec<QueryColumn> = col_info
                            .iter()
                            .map(|(name, ty)| QueryColumn {
                                name: name.clone(),
                                r#type: ty.clone(),
                            })
                            .collect();

                        let mut data = Vec::new();
                        for row_result in rows {
                            let row =
                                row_result.map_err(|e| AppError::query_failed(format!("{e}")))?;
                            let mut map = serde_json::Map::new();
                            for (i, (name, _)) in col_info.iter().enumerate() {
                                map.insert(name.clone(), oracle_value_to_json(&row, i));
                            }
                            data.push(serde_json::Value::Object(map));
                        }

                        Ok(QueryResult {
                            row_count: data.len() as i64,
                            data,
                            columns,
                            time_taken_ms: start.elapsed().as_millis() as i64,
                            success: true,
                            error: None,
                            result_sets: None,
                        })
                    } else {
                        let mut stmt = conn
                            .statement(sql_clean)
                            .build()
                            .map_err(|e| AppError::query_failed(format!("{e}")))?;
                        stmt.execute(&[] as &[&dyn oracle::sql_type::ToSql])
                            .map_err(|e| AppError::query_failed(format!("{e}")))?;
                        let row_count = stmt.row_count().unwrap_or(0) as i64;
                        conn.commit()
                            .map_err(|e| AppError::query_failed(format!("commit failed: {e}")))?;
                        Ok(QueryResult {
                            row_count,
                            data: vec![],
                            columns: vec![],
                            time_taken_ms: start.elapsed().as_millis() as i64,
                            success: true,
                            error: None,
                            result_sets: None,
                        })
                    }
                })
                .await;
        }

        // Multiple statements: execute each and collect results
        self.run_blocking(move |conn| {
            let mut result_sets = Vec::new();
            let mut last_error: Option<String> = None;

            for (idx, statement) in statements.iter().enumerate() {
                let sql_clean = super::strip_trailing_statement_terminator(statement);
                let first_kw = super::first_sql_keyword(sql_clean);
                let is_read = matches!(
                    first_kw.as_deref(),
                    Some("SELECT") | Some("WITH") | Some("SHOW")
                );

                let result: Result<(Vec<QueryColumn>, Vec<serde_json::Value>, i64), AppError> =
                    if is_read {
                        let rows = conn
                            .query(sql_clean, &[] as &[&dyn oracle::sql_type::ToSql])
                            .map_err(|e| AppError::query_failed(format!("{e}")))?;

                        let col_info: Vec<(String, String)> = rows
                            .column_info()
                            .iter()
                            .map(|c| (c.name().to_string(), format!("{}", c.oracle_type())))
                            .collect();
                        let columns: Vec<QueryColumn> = col_info
                            .iter()
                            .map(|(name, ty)| QueryColumn {
                                name: name.clone(),
                                r#type: ty.clone(),
                            })
                            .collect();

                        let mut data = Vec::new();
                        for row_result in rows {
                            let row =
                                row_result.map_err(|e| AppError::query_failed(format!("{e}")))?;
                            let mut map = serde_json::Map::new();
                            for (i, (name, _)) in col_info.iter().enumerate() {
                                map.insert(name.clone(), oracle_value_to_json(&row, i));
                            }
                            data.push(serde_json::Value::Object(map));
                        }
                        let row_count = data.len() as i64;
                        Ok((columns, data, row_count))
                    } else {
                        let mut stmt = conn
                            .statement(sql_clean)
                            .build()
                            .map_err(|e| AppError::query_failed(format!("{e}")))?;
                        stmt.execute(&[] as &[&dyn oracle::sql_type::ToSql])
                            .map_err(|e| AppError::query_failed(format!("{e}")))?;
                        let row_count = stmt.row_count().unwrap_or(0) as i64;
                        conn.commit()
                            .map_err(|e| AppError::query_failed(format!("commit failed: {e}")))?;
                        Ok((Vec::new(), Vec::new(), row_count))
                    };

                match result {
                    Ok((columns, data, row_count)) => {
                        result_sets.push(SingleResultSet {
                            data,
                            row_count,
                            columns,
                            index: idx as u32,
                            statement: statement.clone(),
                        });
                    }
                    Err(e) => {
                        last_error = Some(e.to_string());
                        break;
                    }
                }
            }

            if let Some(err) = last_error {
                // Partial success: return collected results + error
                return Ok(QueryResult {
                    data: vec![],
                    row_count: 0,
                    columns: vec![],
                    time_taken_ms: start.elapsed().as_millis() as i64,
                    success: false,
                    error: Some(err),
                    result_sets: Some(result_sets),
                });
            }

            // All succeeded
            Ok(QueryResult {
                data: vec![],
                row_count: 0,
                columns: vec![],
                time_taken_ms: start.elapsed().as_millis() as i64,
                success: true,
                error: None,
                result_sets: Some(result_sets),
            })
        })
        .await
    }

    async fn get_schema_overview(&self, schema: Option<String>) -> DriverResult<SchemaOverview> {
        let schema_upper = schema
            .map(|s| s.trim().to_uppercase())
            .filter(|s| !s.is_empty());
        self.run_blocking(move |conn| {
            let sql = if let Some(ref s) = schema_upper {
                format!(
                    "SELECT OWNER, TABLE_NAME, COLUMN_NAME, DATA_TYPE \
                     FROM ALL_TAB_COLUMNS \
                     WHERE OWNER = '{}' \
                     ORDER BY OWNER, TABLE_NAME, COLUMN_ID",
                    escape_literal(s),
                )
            } else {
                "SELECT OWNER, TABLE_NAME, COLUMN_NAME, DATA_TYPE \
                 FROM ALL_TAB_COLUMNS \
                 ORDER BY OWNER, TABLE_NAME, COLUMN_ID"
                    .to_string()
            };

            let rows = conn
                .query(&sql, &[] as &[&dyn oracle::sql_type::ToSql])
                .map_err(|e| AppError::query_failed(format!("{e}")))?;
            let mut table_map: HashMap<(String, String), Vec<ColumnSchema>> = HashMap::new();
            for row_result in rows {
                let row = row_result.map_err(|e| AppError::query_failed(format!("{e}")))?;
                let schema_name: Option<String> = row.get(0).ok().flatten();
                let table_name: Option<String> = row.get(1).ok().flatten();
                let col_name: Option<String> = row.get(2).ok().flatten();
                let col_type: Option<String> = row.get(3).ok().flatten();
                if let (Some(sn), Some(tn), Some(cn), Some(ct)) =
                    (schema_name, table_name, col_name, col_type)
                {
                    table_map.entry((sn, tn)).or_default().push(ColumnSchema {
                        name: cn,
                        r#type: ct,
                    });
                }
            }
            let mut tables: Vec<TableSchema> = table_map
                .into_iter()
                .map(|((s, n), cols)| TableSchema {
                    schema: s,
                    name: n,
                    columns: cols,
                })
                .collect();
            tables.sort_by(|a, b| a.schema.cmp(&b.schema).then(a.name.cmp(&b.name)));
            Ok(SchemaOverview { tables })
        })
        .await
    }
}

#[async_trait]
impl SequenceDriver for OracleDriver {
    async fn list_sequences(&self, schema: Option<String>) -> DriverResult<Vec<SequenceInfo>> {
        let schema_upper = schema
            .map(|s| s.trim().to_uppercase())
            .filter(|s| !s.is_empty());
        self.run_blocking(move |conn| {
            let sql = if let Some(ref s) = schema_upper {
                format!(
                    "SELECT SEQUENCE_OWNER, SEQUENCE_NAME, DATA_TYPE, CAST(MIN_VALUE AS VARCHAR2(64)), CAST(INCREMENT_BY AS VARCHAR2(64)) \
                     FROM ALL_SEQUENCES \
                     WHERE SEQUENCE_OWNER = '{}' \
                     ORDER BY SEQUENCE_OWNER, SEQUENCE_NAME",
                    escape_literal(s)
                )
            } else {
                "SELECT SEQUENCE_OWNER, SEQUENCE_NAME, DATA_TYPE, CAST(MIN_VALUE AS VARCHAR2(64)), CAST(INCREMENT_BY AS VARCHAR2(64)) \
                 FROM ALL_SEQUENCES \
                 ORDER BY SEQUENCE_OWNER, SEQUENCE_NAME"
                    .to_string()
            };
            let rows = conn
                .query(&sql, &[] as &[&dyn oracle::sql_type::ToSql])
                .map_err(|e| AppError::query_failed(format!("{e}")))?;
            let mut result = Vec::new();
            for row_result in rows {
                let row = row_result.map_err(|e| AppError::query_failed(format!("{e}")))?;
                let schema_name: Option<String> = row.get(0).ok().flatten();
                let seq_name: Option<String> = row.get(1).ok().flatten();
                let data_type: Option<String> = row.get(2).ok().flatten();
                let start_value: Option<String> = row.get(3).ok().flatten();
                let increment: Option<String> = row.get(4).ok().flatten();
                if let (Some(s), Some(n)) = (schema_name, seq_name) {
                    result.push(SequenceInfo {
                        schema: s,
                        name: n,
                        data_type: data_type.unwrap_or_default(),
                        start_value,
                        increment,
                    });
                }
            }
            Ok(result)
        })
        .await
    }
}

#[async_trait]
impl TypeDriver for OracleDriver {
    async fn list_types(&self, schema: Option<String>) -> DriverResult<Vec<TypeInfo>> {
        let schema_upper = schema
            .map(|s| s.trim().to_uppercase())
            .filter(|s| !s.is_empty());
        self.run_blocking(move |conn| {
            let sql = if let Some(ref s) = schema_upper {
                format!(
                    "SELECT OWNER, TYPE_NAME, TYPECODE \
                     FROM ALL_TYPES \
                     WHERE OWNER = '{}' \
                     ORDER BY OWNER, TYPE_NAME",
                    escape_literal(s)
                )
            } else {
                "SELECT OWNER, TYPE_NAME, TYPECODE \
                 FROM ALL_TYPES \
                 ORDER BY OWNER, TYPE_NAME"
                    .to_string()
            };
            let rows = conn
                .query(&sql, &[] as &[&dyn oracle::sql_type::ToSql])
                .map_err(|e| AppError::query_failed(format!("{e}")))?;
            let mut result = Vec::new();
            for row_result in rows {
                let row = row_result.map_err(|e| AppError::query_failed(format!("{e}")))?;
                let schema_name: Option<String> = row.get(0).ok().flatten();
                let type_name: Option<String> = row.get(1).ok().flatten();
                let type_code: Option<String> = row.get(2).ok().flatten();
                if let (Some(s), Some(n)) = (schema_name, type_name) {
                    result.push(TypeInfo {
                        schema: s,
                        name: n,
                        category: type_code.unwrap_or_default(),
                    });
                }
            }
            Ok(result)
        })
        .await
    }
}

#[async_trait]
impl PackageDriver for OracleDriver {
    async fn list_packages(&self, schema: Option<String>) -> DriverResult<Vec<PackageInfo>> {
        let schema_upper = schema
            .map(|s| s.trim().to_uppercase())
            .filter(|s| !s.is_empty());
        self.run_blocking(move |conn| {
            let sql = if let Some(ref s) = schema_upper {
                format!(
                    "SELECT OWNER, OBJECT_NAME, OBJECT_TYPE \
                     FROM ALL_OBJECTS \
                     WHERE OBJECT_TYPE = 'PACKAGE' AND OWNER = '{}' \
                     ORDER BY OWNER, OBJECT_NAME",
                    escape_literal(s)
                )
            } else {
                "SELECT OWNER, OBJECT_NAME, OBJECT_TYPE \
                 FROM ALL_OBJECTS \
                 WHERE OBJECT_TYPE = 'PACKAGE' \
                 ORDER BY OWNER, OBJECT_NAME"
                    .to_string()
            };
            let rows = conn
                .query(&sql, &[] as &[&dyn oracle::sql_type::ToSql])
                .map_err(|e| AppError::query_failed(format!("{e}")))?;
            let mut result = Vec::new();
            for row_result in rows {
                let row = row_result.map_err(|e| AppError::query_failed(format!("{e}")))?;
                let schema_name: Option<String> = row.get(0).ok().flatten();
                let pkg_name: Option<String> = row.get(1).ok().flatten();
                let obj_type: Option<String> = row.get(2).ok().flatten();
                if let (Some(s), Some(n)) = (schema_name, pkg_name) {
                    result.push(PackageInfo {
                        schema: s,
                        name: n,
                        object_type: obj_type.unwrap_or_default(),
                    });
                }
            }
            Ok(result)
        })
        .await
    }
}

#[async_trait]
impl ForeignKeyDriver for OracleDriver {
    async fn get_schema_foreign_keys(
        &self,
        _database: Option<&str>,
    ) -> DriverResult<Vec<SchemaForeignKey>> {
        self.run_blocking(move |conn| {
            let sql = "SELECT c.CONSTRAINT_NAME, \
                            c.OWNER AS SRC_SCHEMA, \
                            cc.TABLE_NAME, \
                            cc.COLUMN_NAME, \
                            rc.OWNER AS TGT_SCHEMA, \
                            rc.TABLE_NAME AS REF_TABLE, \
                            rcc.COLUMN_NAME AS REF_COLUMN, \
                            c.DELETE_RULE \
                     FROM ALL_CONSTRAINTS c \
                     JOIN ALL_CONS_COLUMNS cc \
                       ON cc.CONSTRAINT_NAME = c.CONSTRAINT_NAME \
                      AND cc.OWNER = c.OWNER \
                     JOIN ALL_CONSTRAINTS rc \
                       ON rc.CONSTRAINT_NAME = c.R_CONSTRAINT_NAME \
                      AND rc.OWNER = c.R_OWNER \
                     JOIN ALL_CONS_COLUMNS rcc \
                       ON rcc.CONSTRAINT_NAME = rc.CONSTRAINT_NAME \
                      AND rcc.OWNER = rc.OWNER \
                      AND rcc.POSITION = cc.POSITION \
                     WHERE c.CONSTRAINT_TYPE = 'R' \
                     ORDER BY c.CONSTRAINT_NAME, cc.POSITION";
            let rows = conn
                .query(sql, &[] as &[&dyn oracle::sql_type::ToSql])
                .map_err(|e| AppError::query_failed(format!("{e}")))?;

            let mut foreign_keys = Vec::new();
            for row_result in rows {
                let row = row_result.map_err(|e| AppError::query_failed(format!("{e}")))?;
                let fk_name: Option<String> = row.get(0).ok().flatten();
                let src_schema: Option<String> = row.get(1).ok().flatten();
                let src_table: Option<String> = row.get(2).ok().flatten();
                let src_col: Option<String> = row.get(3).ok().flatten();
                let tgt_schema: Option<String> = row.get(4).ok().flatten();
                let tgt_table: Option<String> = row.get(5).ok().flatten();
                let tgt_col: Option<String> = row.get(6).ok().flatten();
                let delete_rule: Option<String> = row.get(7).ok().flatten();
                if let (
                    Some(name),
                    Some(src_table),
                    Some(src_col),
                    Some(tgt_table),
                    Some(tgt_col),
                ) = (fk_name, src_table, src_col, tgt_table, tgt_col)
                {
                    foreign_keys.push(SchemaForeignKey {
                        name,
                        source_schema: src_schema,
                        source_table: src_table,
                        source_column: src_col,
                        target_schema: tgt_schema,
                        target_table: tgt_table,
                        target_column: tgt_col,
                        on_update: None,
                        on_delete: delete_rule,
                    });
                }
            }
            Ok(foreign_keys)
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::{escape_literal, quote_ident};

    #[test]
    fn quote_ident_wraps_in_double_quotes() {
        assert_eq!(quote_ident("MY_TABLE"), "\"MY_TABLE\"");
    }

    #[test]
    fn quote_ident_escapes_embedded_double_quote() {
        assert_eq!(quote_ident("a\"b"), "\"a\"\"b\"");
    }

    #[test]
    fn escape_literal_escapes_single_quote() {
        assert_eq!(escape_literal("O'Brien"), "O''Brien");
    }

    #[test]
    fn first_sql_keyword_extracts_select() {
        assert_eq!(
            crate::db::drivers::first_sql_keyword("  SELECT id FROM t"),
            Some("SELECT".to_string())
        );
    }

    #[test]
    fn first_sql_keyword_skips_comments() {
        assert_eq!(
            crate::db::drivers::first_sql_keyword("-- comment\nINSERT INTO t VALUES(1)"),
            Some("INSERT".to_string())
        );
    }

    #[test]
    fn first_sql_keyword_identifies_with() {
        assert_eq!(
            crate::db::drivers::first_sql_keyword("WITH cte AS (SELECT 1) SELECT * FROM cte"),
            Some("WITH".to_string())
        );
    }
}
