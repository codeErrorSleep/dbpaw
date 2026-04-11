use crate::db::drivers::DatabaseDriver;
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::State;

const DEFAULT_CHUNK_SIZE: i64 = 2000;
const MAX_IMPORT_FILE_SIZE_BYTES: u64 = 20 * 1024 * 1024;
const MAX_IMPORT_STATEMENTS: usize = 50_000;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportFormat {
    Csv,
    Json,
    SqlDml,
    SqlDdl,
    SqlFull,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportScope {
    CurrentPage,
    Filtered,
    FullTable,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    pub file_path: String,
    pub row_count: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSqlResult {
    pub file_path: String,
    pub total_statements: i64,
    pub success_statements: i64,
    pub failed_at: Option<i64>,
    pub failed_batch: Option<i64>,
    pub failed_statement_preview: Option<String>,
    pub error: Option<String>,
    pub time_taken_ms: i64,
    pub rolled_back: bool,
}

#[derive(Debug, Clone)]
struct ImportExecutionUnit {
    sql: String,
    batch_index: usize,
    preview: String,
}

#[derive(Debug, Clone)]
struct PreparedImportPlan {
    units: Vec<ImportExecutionUnit>,
    script_managed_transaction: bool,
}

async fn write_table_export(
    db_driver: Arc<dyn DatabaseDriver>,
    writer: &mut ExportWriter,
    schema: String,
    table: String,
    driver: String,
    format: ExportFormat,
    scope: ExportScope,
    filter: Option<String>,
    order_by: Option<String>,
    sort_column: Option<String>,
    sort_direction: Option<String>,
    page: Option<i64>,
    limit: Option<i64>,
    chunk: i64,
) -> Result<i64, String> {
    let mut exported = 0i64;

    if matches!(format, ExportFormat::SqlDdl | ExportFormat::SqlFull) {
        let ddl = db_driver
            .get_table_ddl(schema.clone(), table.clone())
            .await?;
        writer.write_ddl(&ddl)?;
    }

    if !matches!(format, ExportFormat::SqlDdl) {
        let columns: Vec<String> = db_driver
            .get_table_metadata(schema.clone(), table.clone())
            .await?
            .columns
            .into_iter()
            .map(|c| c.name)
            .collect();

        writer.write_csv_header(&columns)?;

        match scope {
            ExportScope::CurrentPage => {
                let resp = db_driver
                    .get_table_data_chunk(
                        schema.clone(),
                        table.clone(),
                        page.unwrap_or(1).max(1),
                        limit.unwrap_or(50).max(1),
                        sort_column,
                        sort_direction,
                        filter,
                        order_by,
                    )
                    .await?;
                exported +=
                    writer.write_rows(&resp.data, &columns, Some(&schema), &table, &driver)?;
            }
            ExportScope::Filtered | ExportScope::FullTable => {
                let (eff_filter, eff_order, eff_sort_col, eff_sort_dir) =
                    if matches!(scope, ExportScope::Filtered) {
                        (filter, order_by, sort_column, sort_direction)
                    } else {
                        (None, None, None, None)
                    };
                let mut current_page = 1;
                loop {
                    let resp = db_driver
                        .get_table_data_chunk(
                            schema.clone(),
                            table.clone(),
                            current_page,
                            chunk,
                            eff_sort_col.clone(),
                            eff_sort_dir.clone(),
                            eff_filter.clone(),
                            eff_order.clone(),
                        )
                        .await?;
                    if resp.data.is_empty() {
                        break;
                    }
                    exported +=
                        writer.write_rows(&resp.data, &columns, Some(&schema), &table, &driver)?;
                    if exported >= resp.total {
                        break;
                    }
                    current_page += 1;
                }
            }
        }
    }

    Ok(exported)
}

async fn do_table_export(
    db_driver: Arc<dyn DatabaseDriver>,
    output_path: PathBuf,
    schema: String,
    table: String,
    driver: String,
    format: ExportFormat,
    scope: ExportScope,
    filter: Option<String>,
    order_by: Option<String>,
    sort_column: Option<String>,
    sort_direction: Option<String>,
    page: Option<i64>,
    limit: Option<i64>,
    chunk: i64,
) -> Result<ExportResult, String> {
    let mut writer = ExportWriter::new(output_path.clone(), format.clone())?;
    let exported = write_table_export(
        db_driver,
        &mut writer,
        schema,
        table,
        driver,
        format,
        scope,
        filter,
        order_by,
        sort_column,
        sort_direction,
        page,
        limit,
        chunk,
    )
    .await?;

    writer.finish()?;
    Ok(ExportResult {
        file_path: output_path.to_string_lossy().to_string(),
        row_count: exported,
    })
}

async fn do_database_export(
    db_driver: Arc<dyn DatabaseDriver>,
    output_path: PathBuf,
    driver: String,
    format: ExportFormat,
    chunk: i64,
) -> Result<ExportResult, String> {
    let mut tables = db_driver.list_tables(None).await?;
    tables.sort_by(|a, b| a.schema.cmp(&b.schema).then(a.name.cmp(&b.name)));

    let mut writer = ExportWriter::new(output_path.clone(), format.clone())?;
    let mut exported = 0i64;
    for table in tables {
        exported += write_table_export(
            db_driver.clone(),
            &mut writer,
            table.schema,
            table.name,
            driver.clone(),
            format.clone(),
            ExportScope::FullTable,
            None,
            None,
            None,
            None,
            None,
            None,
            chunk,
        )
        .await?;
    }
    writer.finish()?;

    Ok(ExportResult {
        file_path: output_path.to_string_lossy().to_string(),
        row_count: exported,
    })
}

#[tauri::command]
pub async fn export_table_data(
    state: State<'_, AppState>,
    id: i64,
    database: Option<String>,
    schema: String,
    table: String,
    driver: String,
    format: ExportFormat,
    scope: ExportScope,
    filter: Option<String>,
    order_by: Option<String>,
    sort_column: Option<String>,
    sort_direction: Option<String>,
    page: Option<i64>,
    limit: Option<i64>,
    file_path: Option<String>,
    chunk_size: Option<i64>,
) -> Result<ExportResult, String> {
    let output_path = resolve_output_path(file_path, &table, extension_for_format(&format))?;
    let chunk = chunk_size.unwrap_or(DEFAULT_CHUNK_SIZE).max(1);
    super::execute_with_retry(&state, id, database, |db_driver| {
        let output_path = output_path.clone();
        let schema = schema.clone();
        let table = table.clone();
        let driver = driver.clone();
        let filter = filter.clone();
        let order_by = order_by.clone();
        let sort_column = sort_column.clone();
        let sort_direction = sort_direction.clone();
        let scope = scope.clone();
        let format = format.clone();
        async move {
            do_table_export(
                db_driver,
                output_path,
                schema,
                table,
                driver,
                format,
                scope,
                filter,
                order_by,
                sort_column,
                sort_direction,
                page,
                limit,
                chunk,
            )
            .await
        }
    })
    .await
}

pub async fn export_table_data_direct(
    state: &AppState,
    id: i64,
    database: Option<String>,
    schema: String,
    table: String,
    driver: String,
    format: ExportFormat,
    scope: ExportScope,
    filter: Option<String>,
    order_by: Option<String>,
    sort_column: Option<String>,
    sort_direction: Option<String>,
    page: Option<i64>,
    limit: Option<i64>,
    file_path: Option<String>,
    chunk_size: Option<i64>,
) -> Result<ExportResult, String> {
    let output_path = resolve_output_path(file_path, &table, extension_for_format(&format))?;
    let chunk = chunk_size.unwrap_or(DEFAULT_CHUNK_SIZE).max(1);
    super::execute_with_retry_from_app_state(state, id, database, |db_driver| {
        let output_path = output_path.clone();
        let schema = schema.clone();
        let table = table.clone();
        let driver = driver.clone();
        let filter = filter.clone();
        let order_by = order_by.clone();
        let sort_column = sort_column.clone();
        let sort_direction = sort_direction.clone();
        let scope = scope.clone();
        let format = format.clone();
        async move {
            do_table_export(
                db_driver,
                output_path,
                schema,
                table,
                driver,
                format,
                scope,
                filter,
                order_by,
                sort_column,
                sort_direction,
                page,
                limit,
                chunk,
            )
            .await
        }
    })
    .await
}

#[tauri::command]
pub async fn export_database_sql(
    state: State<'_, AppState>,
    id: i64,
    database: String,
    driver: String,
    format: ExportFormat,
    file_path: Option<String>,
    chunk_size: Option<i64>,
) -> Result<ExportResult, String> {
    let output_path = resolve_output_path(file_path, &database, "sql")?;
    let chunk = chunk_size.unwrap_or(DEFAULT_CHUNK_SIZE).max(1);
    super::execute_with_retry(&state, id, Some(database), |db_driver| {
        let output_path = output_path.clone();
        let driver = driver.clone();
        let format = format.clone();
        async move { do_database_export(db_driver, output_path, driver, format, chunk).await }
    })
    .await
}

pub async fn export_database_sql_direct(
    state: &AppState,
    id: i64,
    database: String,
    driver: String,
    format: ExportFormat,
    file_path: Option<String>,
    chunk_size: Option<i64>,
) -> Result<ExportResult, String> {
    let output_path = resolve_output_path(file_path, &database, "sql")?;
    let chunk = chunk_size.unwrap_or(DEFAULT_CHUNK_SIZE).max(1);
    super::execute_with_retry_from_app_state(state, id, Some(database), |db_driver| {
        let output_path = output_path.clone();
        let driver = driver.clone();
        let format = format.clone();
        async move { do_database_export(db_driver, output_path, driver, format, chunk).await }
    })
    .await
}

#[tauri::command]
pub async fn export_query_result(
    state: State<'_, AppState>,
    id: i64,
    database: Option<String>,
    sql: String,
    driver: String,
    format: ExportFormat,
    file_path: Option<String>,
) -> Result<ExportResult, String> {
    let output_path =
        resolve_output_path(file_path, "query_result", extension_for_format(&format))?;

    super::execute_with_retry(&state, id, database, |db_driver| {
        let output_path = output_path.clone();
        let driver = driver.clone();
        let sql = sql.clone();
        let format = format.clone();
        async move {
            let result = db_driver.execute_query(sql).await?;
            let columns = result
                .columns
                .into_iter()
                .map(|c| c.name)
                .collect::<Vec<_>>();
            let mut writer = ExportWriter::new(output_path.clone(), format)?;
            writer.write_csv_header(&columns)?;
            let exported =
                writer.write_rows(&result.data, &columns, None, "query_result", &driver)?;
            writer.finish()?;
            Ok(ExportResult {
                file_path: output_path.to_string_lossy().to_string(),
                row_count: exported,
            })
        }
    })
    .await
}

pub async fn export_query_result_direct(
    state: &AppState,
    id: i64,
    database: Option<String>,
    sql: String,
    driver: String,
    format: ExportFormat,
    file_path: Option<String>,
) -> Result<ExportResult, String> {
    let output_path =
        resolve_output_path(file_path, "query_result", extension_for_format(&format))?;

    super::execute_with_retry_from_app_state(state, id, database, |db_driver| {
        let output_path = output_path.clone();
        let driver = driver.clone();
        let sql = sql.clone();
        let format = format.clone();
        async move {
            let result = db_driver.execute_query(sql).await?;
            let columns = result
                .columns
                .into_iter()
                .map(|c| c.name)
                .collect::<Vec<_>>();
            let mut writer = ExportWriter::new(output_path.clone(), format)?;
            writer.write_csv_header(&columns)?;
            let exported =
                writer.write_rows(&result.data, &columns, None, "query_result", &driver)?;
            writer.finish()?;
            Ok(ExportResult {
                file_path: output_path.to_string_lossy().to_string(),
                row_count: exported,
            })
        }
    })
    .await
}

#[tauri::command]
pub async fn import_sql_file(
    state: State<'_, AppState>,
    id: i64,
    database: Option<String>,
    file_path: String,
    driver: String,
) -> Result<ImportSqlResult, String> {
    let normalized_driver = normalize_driver_name(&driver);
    let (begin_sql, commit_sql, rollback_sql) =
        import_transaction_sql(&normalized_driver, &driver)?;

    let import_path = PathBuf::from(file_path.trim());
    validate_import_path(&import_path)?;
    validate_import_file_size(&import_path)?;

    let source = fs::read_to_string(&import_path)
        .map_err(|e| format!("[IMPORT_ERROR] failed to read sql file: {e}"))?;
    let source = source
        .strip_prefix('\u{feff}')
        .unwrap_or(&source)
        .to_string();

    let import_plan = prepare_import_plan(&source, &normalized_driver)?;
    if import_plan.units.is_empty() {
        return Err("[IMPORT_ERROR] SQL file does not contain executable statements".to_string());
    }
    if import_plan.units.len() > MAX_IMPORT_STATEMENTS {
        return Err(format!(
            "[IMPORT_ERROR] statement count exceeds limit ({} > {})",
            import_plan.units.len(),
            MAX_IMPORT_STATEMENTS
        ));
    }

    let started_at = std::time::Instant::now();
    let total_statements = import_plan.units.len() as i64;
    let use_outer_transaction = !import_plan.script_managed_transaction;

    super::execute_with_retry(&state, id, database, |db_driver| {
        let import_plan = import_plan.clone();
        let import_path = import_path.clone();
        async move {
            if use_outer_transaction {
                db_driver
                    .execute_query(begin_sql.to_string())
                    .await
                    .map_err(|e| format!("[IMPORT_ERROR] failed to start transaction: {e}"))?;
            }

            let mut success_statements = 0i64;
            for (idx, unit) in import_plan.units.iter().enumerate() {
                if let Err(e) = db_driver.execute_query(unit.sql.clone()).await {
                    if use_outer_transaction {
                        let _ = db_driver.execute_query(rollback_sql.to_string()).await;
                    }
                    return Ok(ImportSqlResult {
                        file_path: import_path.to_string_lossy().to_string(),
                        total_statements,
                        success_statements,
                        failed_at: Some((idx + 1) as i64),
                        failed_batch: Some(unit.batch_index as i64),
                        failed_statement_preview: Some(unit.preview.clone()),
                        error: Some(truncate_error_message(&e)),
                        time_taken_ms: started_at.elapsed().as_millis() as i64,
                        rolled_back: use_outer_transaction,
                    });
                }
                success_statements += 1;
            }

            if use_outer_transaction {
                if let Err(e) = db_driver.execute_query(commit_sql.to_string()).await {
                    let _ = db_driver.execute_query(rollback_sql.to_string()).await;
                    return Ok(ImportSqlResult {
                        file_path: import_path.to_string_lossy().to_string(),
                        total_statements,
                        success_statements,
                        failed_at: None,
                        failed_batch: None,
                        failed_statement_preview: None,
                        error: Some(format!(
                            "[IMPORT_ERROR] failed to commit transaction: {}",
                            truncate_error_message(&e)
                        )),
                        time_taken_ms: started_at.elapsed().as_millis() as i64,
                        rolled_back: true,
                    });
                }
            }

            Ok(ImportSqlResult {
                file_path: import_path.to_string_lossy().to_string(),
                total_statements,
                success_statements: total_statements,
                failed_at: None,
                failed_batch: None,
                failed_statement_preview: None,
                error: None,
                time_taken_ms: started_at.elapsed().as_millis() as i64,
                rolled_back: false,
            })
        }
    })
    .await
}

pub async fn import_sql_file_direct(
    state: &AppState,
    id: i64,
    database: Option<String>,
    file_path: String,
    driver: String,
) -> Result<ImportSqlResult, String> {
    let normalized_driver = normalize_driver_name(&driver);
    let (begin_sql, commit_sql, rollback_sql) =
        import_transaction_sql(&normalized_driver, &driver)?;

    let import_path = PathBuf::from(file_path.trim());
    validate_import_path(&import_path)?;
    validate_import_file_size(&import_path)?;

    let source = fs::read_to_string(&import_path)
        .map_err(|e| format!("[IMPORT_ERROR] failed to read sql file: {e}"))?;
    let source = source
        .strip_prefix('\u{feff}')
        .unwrap_or(&source)
        .to_string();

    let import_plan = prepare_import_plan(&source, &normalized_driver)?;
    if import_plan.units.is_empty() {
        return Err("[IMPORT_ERROR] SQL file does not contain executable statements".to_string());
    }
    if import_plan.units.len() > MAX_IMPORT_STATEMENTS {
        return Err(format!(
            "[IMPORT_ERROR] statement count exceeds limit ({} > {})",
            import_plan.units.len(),
            MAX_IMPORT_STATEMENTS
        ));
    }

    let started_at = std::time::Instant::now();
    let total_statements = import_plan.units.len() as i64;
    let use_outer_transaction = !import_plan.script_managed_transaction;

    super::execute_with_retry_from_app_state(state, id, database, |db_driver| {
        let import_plan = import_plan.clone();
        let import_path = import_path.clone();
        async move {
            if use_outer_transaction {
                db_driver
                    .execute_query(begin_sql.to_string())
                    .await
                    .map_err(|e| format!("[IMPORT_ERROR] failed to start transaction: {e}"))?;
            }

            let mut success_statements = 0i64;
            for (idx, unit) in import_plan.units.iter().enumerate() {
                if let Err(e) = db_driver.execute_query(unit.sql.clone()).await {
                    if use_outer_transaction {
                        let _ = db_driver.execute_query(rollback_sql.to_string()).await;
                    }
                    return Ok(ImportSqlResult {
                        file_path: import_path.to_string_lossy().to_string(),
                        total_statements,
                        success_statements,
                        failed_at: Some((idx + 1) as i64),
                        failed_batch: Some(unit.batch_index as i64),
                        failed_statement_preview: Some(unit.preview.clone()),
                        error: Some(truncate_error_message(&e)),
                        time_taken_ms: started_at.elapsed().as_millis() as i64,
                        rolled_back: use_outer_transaction,
                    });
                }
                success_statements += 1;
            }

            if use_outer_transaction {
                if let Err(e) = db_driver.execute_query(commit_sql.to_string()).await {
                    let _ = db_driver.execute_query(rollback_sql.to_string()).await;
                    return Ok(ImportSqlResult {
                        file_path: import_path.to_string_lossy().to_string(),
                        total_statements,
                        success_statements,
                        failed_at: None,
                        failed_batch: None,
                        failed_statement_preview: None,
                        error: Some(format!(
                            "[IMPORT_ERROR] failed to commit transaction: {}",
                            truncate_error_message(&e)
                        )),
                        time_taken_ms: started_at.elapsed().as_millis() as i64,
                        rolled_back: true,
                    });
                }
            }

            Ok(ImportSqlResult {
                file_path: import_path.to_string_lossy().to_string(),
                total_statements,
                success_statements: total_statements,
                failed_at: None,
                failed_batch: None,
                failed_statement_preview: None,
                error: None,
                time_taken_ms: started_at.elapsed().as_millis() as i64,
                rolled_back: false,
            })
        }
    })
    .await
}

fn import_transaction_sql<'a>(
    normalized_driver: &'a str,
    original_driver: &str,
) -> Result<(&'a str, &'a str, &'a str), String> {
    match normalized_driver {
        "mysql" | "mariadb" | "tidb" => Ok(("START TRANSACTION", "COMMIT", "ROLLBACK")),
        "starrocks" => Err(
            "[UNSUPPORTED] Driver starrocks does not support transactional SQL import in this flow"
                .to_string(),
        ),
        "postgres" | "sqlite" | "duckdb" => Ok(("BEGIN", "COMMIT", "ROLLBACK")),
        "mssql" => Ok((
            "BEGIN TRANSACTION",
            "COMMIT TRANSACTION",
            "ROLLBACK TRANSACTION",
        )),
        "oracle" => Ok(("SELECT 1 FROM DUAL", "COMMIT", "ROLLBACK")),
        "clickhouse" => {
            Err("[UNSUPPORTED] Driver clickhouse is read-only in this import flow".to_string())
        }
        _ => Err(format!(
            "[UNSUPPORTED] Driver {} is not supported for SQL import",
            original_driver
        )),
    }
}

fn normalize_driver_name(driver: &str) -> String {
    let normalized = driver.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "postgresql" | "pgsql" => "postgres".to_string(),
        _ => normalized,
    }
}

fn prepare_import_plan(sql: &str, normalized_driver: &str) -> Result<PreparedImportPlan, String> {
    let units = if normalized_driver == "mssql" {
        let batches = parse_mssql_batches(sql)?;
        batches
            .into_iter()
            .enumerate()
            .map(|(idx, batch)| ImportExecutionUnit {
                preview: build_statement_preview(&batch),
                sql: batch,
                batch_index: idx + 1,
            })
            .collect::<Vec<_>>()
    } else {
        parse_sql_statements(sql, normalized_driver)?
            .into_iter()
            .enumerate()
            .map(|(idx, statement)| ImportExecutionUnit {
                preview: build_statement_preview(&statement),
                sql: statement,
                batch_index: idx + 1,
            })
            .collect::<Vec<_>>()
    };

    let script_managed_transaction = units
        .iter()
        .any(|unit| statement_controls_transaction(&unit.sql, normalized_driver));

    Ok(PreparedImportPlan {
        units,
        script_managed_transaction,
    })
}

fn build_statement_preview(statement: &str) -> String {
    let compact = statement.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut preview = String::new();
    for (idx, ch) in compact.chars().enumerate() {
        if idx >= 160 {
            preview.push_str("...");
            break;
        }
        preview.push(ch);
    }
    if preview.is_empty() {
        "<empty>".to_string()
    } else {
        preview
    }
}

fn leading_sql_tokens(sql: &str, max_tokens: usize) -> Vec<String> {
    let chars: Vec<char> = sql.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0usize;

    while i < chars.len() && tokens.len() < max_tokens {
        let ch = chars[i];
        let next = chars.get(i + 1).copied();

        if ch.is_whitespace() || ch == ';' {
            i += 1;
            continue;
        }

        if ch == '-' && next == Some('-') {
            i += 2;
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }

        if ch == '/' && next == Some('*') {
            i += 2;
            while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                i += 1;
            }
            if i + 1 < chars.len() {
                i += 2;
            }
            continue;
        }

        if ch.is_ascii_alphabetic() {
            let start = i;
            i += 1;
            while i < chars.len() && (chars[i].is_ascii_alphabetic() || chars[i] == '_') {
                i += 1;
            }
            tokens.push(
                chars[start..i]
                    .iter()
                    .collect::<String>()
                    .to_ascii_lowercase(),
            );
            continue;
        }

        i += 1;
    }

    tokens
}

fn statement_controls_transaction(statement: &str, normalized_driver: &str) -> bool {
    let tokens = leading_sql_tokens(statement, 2);
    if tokens.is_empty() {
        return false;
    }

    let first = tokens[0].as_str();
    let second = tokens.get(1).map(|s| s.as_str()).unwrap_or("");

    match first {
        "commit" | "rollback" => true,
        "start" => second == "transaction",
        "begin" => {
            if normalized_driver == "mssql" {
                second == "transaction" || second == "tran"
            } else {
                true
            }
        }
        _ => false,
    }
}

fn parse_mssql_go_line_count(line: &str) -> Option<usize> {
    let trimmed = line.trim();
    let prefix = trimmed.get(..2)?;
    if !prefix.eq_ignore_ascii_case("go") {
        return None;
    }
    let rest = trimmed[2..].trim();
    if rest.is_empty() {
        return Some(1);
    }
    if rest.chars().all(|ch| ch.is_ascii_digit()) {
        let count = rest.parse::<usize>().ok()?;
        if count > 0 {
            return Some(count);
        }
    }
    None
}

fn update_mssql_line_state(state: &mut SqlScanState, line: &str) {
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0usize;

    while i < chars.len() {
        match state {
            SqlScanState::Normal => {
                let ch = chars[i];
                let next = chars.get(i + 1).copied();
                if ch == '-' && next == Some('-') {
                    *state = SqlScanState::LineComment;
                    break;
                }
                if ch == '/' && next == Some('*') {
                    *state = SqlScanState::BlockComment;
                    i += 2;
                    continue;
                }
                if ch == '\'' {
                    *state = SqlScanState::SingleQuoted;
                    i += 1;
                    continue;
                }
                if ch == '"' {
                    *state = SqlScanState::DoubleQuoted;
                    i += 1;
                    continue;
                }
                i += 1;
            }
            SqlScanState::SingleQuoted => {
                if chars[i] == '\'' {
                    if chars.get(i + 1) == Some(&'\'') {
                        i += 2;
                        continue;
                    }
                    *state = SqlScanState::Normal;
                }
                i += 1;
            }
            SqlScanState::DoubleQuoted => {
                if chars[i] == '"' {
                    if chars.get(i + 1) == Some(&'"') {
                        i += 2;
                        continue;
                    }
                    *state = SqlScanState::Normal;
                }
                i += 1;
            }
            SqlScanState::BlockComment => {
                if chars[i] == '*' && chars.get(i + 1) == Some(&'/') {
                    *state = SqlScanState::Normal;
                    i += 2;
                    continue;
                }
                i += 1;
            }
            SqlScanState::LineComment => {
                break;
            }
            SqlScanState::BacktickQuoted | SqlScanState::DollarQuoted(_) => {
                *state = SqlScanState::Normal;
            }
        }
    }

    if matches!(state, SqlScanState::LineComment) {
        *state = SqlScanState::Normal;
    }
}

fn parse_mssql_batches(sql: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut state = SqlScanState::Normal;

    for line in sql.split_inclusive('\n') {
        if matches!(state, SqlScanState::Normal) {
            let plain_line = line.trim_end_matches(|ch| ch == '\r' || ch == '\n');
            if let Some(go_count) = parse_mssql_go_line_count(plain_line) {
                let statement = current.trim();
                if !statement.is_empty() {
                    for _ in 0..go_count {
                        out.push(statement.to_string());
                    }
                }
                current.clear();
                continue;
            }
        }

        update_mssql_line_state(&mut state, line);
        current.push_str(line);
    }

    match state {
        SqlScanState::Normal | SqlScanState::LineComment => {}
        SqlScanState::BlockComment => {
            return Err("[IMPORT_ERROR] Unterminated block comment in SQL file".to_string());
        }
        SqlScanState::SingleQuoted
        | SqlScanState::DoubleQuoted
        | SqlScanState::BacktickQuoted
        | SqlScanState::DollarQuoted(_) => {
            return Err("[IMPORT_ERROR] Unterminated string literal in SQL file".to_string());
        }
    }

    let tail = current.trim();
    if !tail.is_empty() {
        out.push(tail.to_string());
    }
    Ok(out)
}

fn extension_for_format(format: &ExportFormat) -> &'static str {
    match format {
        ExportFormat::Csv => "csv",
        ExportFormat::Json => "json",
        ExportFormat::SqlDml | ExportFormat::SqlDdl | ExportFormat::SqlFull => "sql",
    }
}

fn resolve_output_path(
    explicit_path: Option<String>,
    base_name: &str,
    extension: &str,
) -> Result<PathBuf, String> {
    let path = if let Some(path) = explicit_path {
        let trimmed = path.trim().to_string();
        if trimmed.is_empty() {
            default_output_path(base_name, extension)
        } else {
            PathBuf::from(trimmed)
        }
    } else {
        default_output_path(base_name, extension)
    };

    validate_output_path(&path)?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("[EXPORT_ERROR] create dir failed: {e}"))?;
    }
    Ok(path)
}

fn validate_import_path(path: &Path) -> Result<(), String> {
    if path.as_os_str().is_empty() {
        return Err("[IMPORT_ERROR] Invalid import path".to_string());
    }
    if path.is_dir() {
        return Err("[IMPORT_ERROR] Import path points to a directory".to_string());
    }
    if !path.exists() {
        return Err("[IMPORT_ERROR] Import file does not exist".to_string());
    }
    let Some(ext) = path.extension().and_then(|v| v.to_str()) else {
        return Err("[IMPORT_ERROR] Import file must use .sql extension".to_string());
    };
    if !ext.eq_ignore_ascii_case("sql") {
        return Err("[IMPORT_ERROR] Import file must use .sql extension".to_string());
    }
    Ok(())
}

fn validate_import_file_size(path: &Path) -> Result<(), String> {
    let metadata = fs::metadata(path)
        .map_err(|e| format!("[IMPORT_ERROR] failed to read file metadata: {e}"))?;
    if metadata.len() > MAX_IMPORT_FILE_SIZE_BYTES {
        return Err(format!(
            "[IMPORT_ERROR] file is too large (max {} bytes)",
            MAX_IMPORT_FILE_SIZE_BYTES
        ));
    }
    Ok(())
}

#[derive(Debug, Clone)]
enum SqlScanState {
    Normal,
    SingleQuoted,
    DoubleQuoted,
    BacktickQuoted,
    DollarQuoted(String),
    LineComment,
    BlockComment,
}

fn starts_with_chars(chars: &[char], idx: usize, needle: &[char]) -> bool {
    if idx + needle.len() > chars.len() {
        return false;
    }
    for (offset, ch) in needle.iter().enumerate() {
        if chars[idx + offset] != *ch {
            return false;
        }
    }
    true
}

fn starts_with_chars_ignore_ascii_case(chars: &[char], idx: usize, needle: &str) -> bool {
    let mut needle_chars = needle.chars();
    let needle_len = needle.len();
    if idx + needle_len > chars.len() {
        return false;
    }
    for offset in 0..needle_len {
        let needle_ch = match needle_chars.next() {
            Some(c) => c,
            None => return false,
        };
        if !chars[idx + offset].eq_ignore_ascii_case(&needle_ch) {
            return false;
        }
    }
    true
}

fn line_start_index(chars: &[char], idx: usize) -> usize {
    let mut start = idx;
    while start > 0 && chars[start - 1] != '\n' {
        start -= 1;
    }
    start
}

fn parse_mysql_delimiter_command(chars: &[char], idx: usize) -> Option<(String, usize)> {
    let line_start = line_start_index(chars, idx);
    let mut cursor = line_start;
    while cursor < chars.len() && matches!(chars[cursor], ' ' | '\t' | '\r') {
        cursor += 1;
    }
    if cursor != idx {
        return None;
    }

    if !starts_with_chars_ignore_ascii_case(chars, cursor, "DELIMITER") {
        return None;
    }

    let mut after_keyword = cursor + "DELIMITER".len();
    if after_keyword < chars.len() && chars[after_keyword] != ' ' && chars[after_keyword] != '\t' {
        return None;
    }
    while after_keyword < chars.len() && matches!(chars[after_keyword], ' ' | '\t') {
        after_keyword += 1;
    }
    if after_keyword >= chars.len() || matches!(chars[after_keyword], '\n' | '\r') {
        return None;
    }

    let mut line_end = after_keyword;
    while line_end < chars.len() && !matches!(chars[line_end], '\n' | '\r') {
        line_end += 1;
    }

    let delimiter: String = chars[after_keyword..line_end]
        .iter()
        .collect::<String>()
        .trim()
        .to_string();
    if delimiter.is_empty() {
        return None;
    }

    let mut next_idx = line_end;
    if next_idx < chars.len() && chars[next_idx] == '\r' {
        next_idx += 1;
    }
    if next_idx < chars.len() && chars[next_idx] == '\n' {
        next_idx += 1;
    }

    Some((delimiter, next_idx))
}

fn sqlite_trigger_state(sql: &str) -> (bool, bool) {
    let chars: Vec<char> = sql.chars().collect();
    let mut state = SqlScanState::Normal;
    let mut i = 0usize;
    let mut tokens = Vec::new();
    let mut trigger_begin_seen = false;
    let mut trigger_block_depth = 0i32;
    let mut case_depth = 0i32;
    let mut last_word: Option<String> = None;

    while i < chars.len() {
        match &state {
            SqlScanState::Normal => {
                let ch = chars[i];
                let next = chars.get(i + 1).copied();
                if ch == '-' && next == Some('-') {
                    state = SqlScanState::LineComment;
                    i += 2;
                    continue;
                }
                if ch == '/' && next == Some('*') {
                    state = SqlScanState::BlockComment;
                    i += 2;
                    continue;
                }
                if ch == '\'' {
                    state = SqlScanState::SingleQuoted;
                    i += 1;
                    continue;
                }
                if ch == '"' {
                    state = SqlScanState::DoubleQuoted;
                    i += 1;
                    continue;
                }
                if ch == '`' {
                    state = SqlScanState::BacktickQuoted;
                    i += 1;
                    continue;
                }
                if ch.is_ascii_alphabetic() || ch == '_' {
                    let start = i;
                    i += 1;
                    while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                        i += 1;
                    }
                    let token = chars[start..i]
                        .iter()
                        .collect::<String>()
                        .to_ascii_lowercase();
                    tokens.push(token.clone());

                    if trigger_begin_seen {
                        match token.as_str() {
                            "case" => case_depth += 1,
                            "begin" => trigger_block_depth += 1,
                            "end" => {
                                if case_depth > 0 {
                                    case_depth -= 1;
                                } else if trigger_block_depth > 0 {
                                    trigger_block_depth -= 1;
                                }
                            }
                            _ => {}
                        }
                    } else if token == "begin" {
                        let is_create_trigger = matches!(
                            tokens.as_slice(),
                            [first, second, ..] if first == "create"
                                && (second == "trigger"
                                    || ((second == "temp" || second == "temporary")
                                        && tokens.get(2).map(String::as_str) == Some("trigger")))
                        );
                        if is_create_trigger {
                            trigger_begin_seen = true;
                            trigger_block_depth = 1;
                        }
                    }

                    last_word = Some(token);
                    continue;
                }
                i += 1;
            }
            SqlScanState::SingleQuoted => {
                if chars[i] == '\\' && chars.get(i + 1).is_some() {
                    i += 2;
                    continue;
                }
                if chars[i] == '\'' {
                    if chars.get(i + 1) == Some(&'\'') {
                        i += 2;
                        continue;
                    }
                    state = SqlScanState::Normal;
                }
                i += 1;
            }
            SqlScanState::DoubleQuoted => {
                if chars[i] == '"' {
                    if chars.get(i + 1) == Some(&'"') {
                        i += 2;
                        continue;
                    }
                    state = SqlScanState::Normal;
                }
                i += 1;
            }
            SqlScanState::BacktickQuoted => {
                if chars[i] == '`' {
                    if chars.get(i + 1) == Some(&'`') {
                        i += 2;
                        continue;
                    }
                    state = SqlScanState::Normal;
                }
                i += 1;
            }
            SqlScanState::LineComment => {
                if chars[i] == '\n' {
                    state = SqlScanState::Normal;
                }
                i += 1;
            }
            SqlScanState::BlockComment => {
                if chars[i] == '*' && chars.get(i + 1) == Some(&'/') {
                    state = SqlScanState::Normal;
                    i += 2;
                } else {
                    i += 1;
                }
            }
            SqlScanState::DollarQuoted(_) => {
                state = SqlScanState::Normal;
            }
        }
    }

    let is_trigger = trigger_begin_seen;
    let ready_to_terminate = is_trigger
        && trigger_block_depth == 0
        && case_depth == 0
        && last_word.as_deref() == Some("end");
    (is_trigger, ready_to_terminate)
}

fn oracle_plsql_state(sql: &str) -> (bool, bool) {
    let chars: Vec<char> = sql.chars().collect();
    let mut state = SqlScanState::Normal;
    let mut i = 0usize;
    let mut tokens = Vec::new();
    let mut block_depth = 0i32;
    let mut case_depth = 0i32;
    let mut last_word: Option<String> = None;
    let mut is_oracle_block = false;

    while i < chars.len() {
        match &state {
            SqlScanState::Normal => {
                let ch = chars[i];
                let next = chars.get(i + 1).copied();
                if ch == '-' && next == Some('-') {
                    state = SqlScanState::LineComment;
                    i += 2;
                    continue;
                }
                if ch == '/' && next == Some('*') {
                    state = SqlScanState::BlockComment;
                    i += 2;
                    continue;
                }
                if ch == '\'' {
                    state = SqlScanState::SingleQuoted;
                    i += 1;
                    continue;
                }
                if ch == '"' {
                    state = SqlScanState::DoubleQuoted;
                    i += 1;
                    continue;
                }
                if ch == '`' {
                    state = SqlScanState::BacktickQuoted;
                    i += 1;
                    continue;
                }
                if ch.is_ascii_alphabetic() || ch == '_' {
                    let start = i;
                    i += 1;
                    while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                        i += 1;
                    }
                    let token = chars[start..i]
                        .iter()
                        .collect::<String>()
                        .to_ascii_lowercase();
                    tokens.push(token.clone());

                    if !is_oracle_block {
                        let second = tokens.get(1).map(String::as_str);
                        let third = tokens.get(2).map(String::as_str);
                        let fourth = tokens.get(3).map(String::as_str);
                        is_oracle_block = matches!(
                            tokens.first().map(String::as_str),
                            Some("declare") | Some("begin")
                        ) || (tokens.first().map(String::as_str)
                            == Some("create")
                            && second == Some("or")
                            && third == Some("replace")
                            && matches!(
                                fourth,
                                Some("function")
                                    | Some("procedure")
                                    | Some("trigger")
                                    | Some("package")
                                    | Some("type")
                            ));
                    }

                    if is_oracle_block {
                        match token.as_str() {
                            "case" => case_depth += 1,
                            "begin" => block_depth += 1,
                            "end" => {
                                if case_depth > 0 {
                                    case_depth -= 1;
                                } else if block_depth > 0 {
                                    block_depth -= 1;
                                }
                            }
                            _ => {}
                        }
                    }

                    last_word = Some(token);
                    continue;
                }
                i += 1;
            }
            SqlScanState::SingleQuoted => {
                if chars[i] == '\\' && chars.get(i + 1).is_some() {
                    i += 2;
                    continue;
                }
                if chars[i] == '\'' {
                    if chars.get(i + 1) == Some(&'\'') {
                        i += 2;
                        continue;
                    }
                    state = SqlScanState::Normal;
                }
                i += 1;
            }
            SqlScanState::DoubleQuoted => {
                if chars[i] == '"' {
                    if chars.get(i + 1) == Some(&'"') {
                        i += 2;
                        continue;
                    }
                    state = SqlScanState::Normal;
                }
                i += 1;
            }
            SqlScanState::BacktickQuoted => {
                if chars[i] == '`' {
                    if chars.get(i + 1) == Some(&'`') {
                        i += 2;
                        continue;
                    }
                    state = SqlScanState::Normal;
                }
                i += 1;
            }
            SqlScanState::LineComment => {
                if chars[i] == '\n' {
                    state = SqlScanState::Normal;
                }
                i += 1;
            }
            SqlScanState::BlockComment => {
                if chars[i] == '*' && chars.get(i + 1) == Some(&'/') {
                    state = SqlScanState::Normal;
                    i += 2;
                } else {
                    i += 1;
                }
            }
            SqlScanState::DollarQuoted(_) => {
                state = SqlScanState::Normal;
            }
        }
    }

    let ready_to_terminate = is_oracle_block
        && block_depth == 0
        && case_depth == 0
        && last_word.as_deref() == Some("end");
    (is_oracle_block, ready_to_terminate)
}

fn parse_oracle_slash_terminator(chars: &[char], idx: usize) -> Option<usize> {
    let line_start = line_start_index(chars, idx);
    let mut cursor = line_start;
    while cursor < chars.len() && matches!(chars[cursor], ' ' | '\t' | '\r') {
        cursor += 1;
    }
    if cursor != idx || chars.get(idx) != Some(&'/') {
        return None;
    }

    let mut line_end = idx + 1;
    while line_end < chars.len() && !matches!(chars[line_end], '\n' | '\r') {
        if !matches!(chars[line_end], ' ' | '\t') {
            return None;
        }
        line_end += 1;
    }

    let mut next_idx = line_end;
    if next_idx < chars.len() && chars[next_idx] == '\r' {
        next_idx += 1;
    }
    if next_idx < chars.len() && chars[next_idx] == '\n' {
        next_idx += 1;
    }

    Some(next_idx)
}

fn parse_sql_statements(sql: &str, driver: &str) -> Result<Vec<String>, String> {
    let mysql_style_hash_comment = matches!(driver, "mysql" | "mariadb" | "tidb");
    let mysql_style_delimiter = mysql_style_hash_comment;
    let sqlite_style_trigger = driver == "sqlite";
    let oracle_style_block = driver == "oracle";
    let chars: Vec<char> = sql.chars().collect();
    let mut out = Vec::new();
    let mut current = String::new();
    let mut state = SqlScanState::Normal;
    let mut delimiter = ";".to_string();
    let mut delimiter_chars: Vec<char> = vec![';'];
    let mut i = 0usize;

    while i < chars.len() {
        match &state {
            SqlScanState::Normal => {
                if mysql_style_delimiter {
                    if let Some((next_delimiter, next_idx)) =
                        parse_mysql_delimiter_command(&chars, i)
                    {
                        delimiter = next_delimiter;
                        delimiter_chars = delimiter.chars().collect();
                        i = next_idx;
                        continue;
                    }
                }
                if oracle_style_block {
                    if let Some(next_idx) = parse_oracle_slash_terminator(&chars, i) {
                        let (is_block, ready_to_terminate) = oracle_plsql_state(current.trim());
                        if is_block && ready_to_terminate {
                            let statement = current.trim();
                            if !statement.is_empty() {
                                out.push(statement.to_string());
                            }
                            current.clear();
                            i = next_idx;
                            continue;
                        }
                    }
                }

                let ch = chars[i];
                let next = chars.get(i + 1).copied();

                if starts_with_chars(&chars, i, &delimiter_chars) {
                    if sqlite_style_trigger && delimiter == ";" {
                        let (is_trigger, ready_to_terminate) = sqlite_trigger_state(current.trim());
                        if is_trigger && !ready_to_terminate {
                            current.push(ch);
                            i += delimiter_chars.len();
                            continue;
                        }
                    }
                    if oracle_style_block && delimiter == ";" {
                        let (is_block, _) = oracle_plsql_state(current.trim());
                        if is_block {
                            current.push(ch);
                            i += delimiter_chars.len();
                            continue;
                        }
                    }
                    let statement = current.trim();
                    if !statement.is_empty() {
                        out.push(statement.to_string());
                    }
                    current.clear();
                    i += delimiter_chars.len();
                    continue;
                }

                if ch == '-' && next == Some('-') {
                    state = SqlScanState::LineComment;
                    i += 2;
                    continue;
                }
                if mysql_style_hash_comment && ch == '#' {
                    state = SqlScanState::LineComment;
                    i += 1;
                    continue;
                }
                if ch == '/' && next == Some('*') {
                    state = SqlScanState::BlockComment;
                    i += 2;
                    continue;
                }
                if ch == '\'' {
                    current.push(ch);
                    state = SqlScanState::SingleQuoted;
                    i += 1;
                    continue;
                }
                if ch == '"' {
                    current.push(ch);
                    state = SqlScanState::DoubleQuoted;
                    i += 1;
                    continue;
                }
                if ch == '`' {
                    current.push(ch);
                    state = SqlScanState::BacktickQuoted;
                    i += 1;
                    continue;
                }
                if ch == '$' {
                    if let Some((tag, end_idx)) = parse_dollar_quote_tag(&chars, i) {
                        current.push_str(&tag);
                        state = SqlScanState::DollarQuoted(tag);
                        i = end_idx + 1;
                        continue;
                    }
                }
                current.push(ch);
                i += 1;
            }
            SqlScanState::SingleQuoted => {
                let ch = chars[i];
                current.push(ch);
                if ch == '\\' {
                    if let Some(next) = chars.get(i + 1) {
                        current.push(*next);
                        i += 2;
                        continue;
                    }
                }
                if ch == '\'' {
                    if chars.get(i + 1) == Some(&'\'') {
                        current.push('\'');
                        i += 2;
                        continue;
                    }
                    state = SqlScanState::Normal;
                }
                i += 1;
            }
            SqlScanState::DoubleQuoted => {
                let ch = chars[i];
                current.push(ch);
                if ch == '"' {
                    if chars.get(i + 1) == Some(&'"') {
                        current.push('"');
                        i += 2;
                        continue;
                    }
                    state = SqlScanState::Normal;
                }
                i += 1;
            }
            SqlScanState::BacktickQuoted => {
                let ch = chars[i];
                current.push(ch);
                if ch == '`' {
                    if chars.get(i + 1) == Some(&'`') {
                        current.push('`');
                        i += 2;
                        continue;
                    }
                    state = SqlScanState::Normal;
                }
                i += 1;
            }
            SqlScanState::DollarQuoted(tag) => {
                if starts_with_tag(&chars, i, tag) {
                    current.push_str(tag);
                    i += tag.chars().count();
                    state = SqlScanState::Normal;
                    continue;
                }
                current.push(chars[i]);
                i += 1;
            }
            SqlScanState::LineComment => {
                if chars[i] == '\n' {
                    current.push('\n');
                    state = SqlScanState::Normal;
                }
                i += 1;
            }
            SqlScanState::BlockComment => {
                if chars[i] == '*' && chars.get(i + 1) == Some(&'/') {
                    state = SqlScanState::Normal;
                    i += 2;
                } else {
                    i += 1;
                }
            }
        }
    }

    match state {
        SqlScanState::Normal | SqlScanState::LineComment => {}
        SqlScanState::BlockComment => {
            return Err("[IMPORT_ERROR] Unterminated block comment in SQL file".to_string());
        }
        SqlScanState::SingleQuoted
        | SqlScanState::DoubleQuoted
        | SqlScanState::BacktickQuoted
        | SqlScanState::DollarQuoted(_) => {
            return Err("[IMPORT_ERROR] Unterminated string literal in SQL file".to_string());
        }
    }

    let tail = current.trim();
    if !tail.is_empty() {
        out.push(tail.to_string());
    }
    Ok(out)
}

fn parse_dollar_quote_tag(chars: &[char], start: usize) -> Option<(String, usize)> {
    if chars.get(start) != Some(&'$') {
        return None;
    }
    let mut idx = start + 1;
    while idx < chars.len() && (chars[idx].is_ascii_alphanumeric() || chars[idx] == '_') {
        idx += 1;
    }
    if idx < chars.len() && chars[idx] == '$' {
        let tag: String = chars[start..=idx].iter().collect();
        return Some((tag, idx));
    }
    None
}

fn starts_with_tag(chars: &[char], idx: usize, tag: &str) -> bool {
    let tag_chars: Vec<char> = tag.chars().collect();
    if idx + tag_chars.len() > chars.len() {
        return false;
    }
    for (offset, ch) in tag_chars.iter().enumerate() {
        if chars[idx + offset] != *ch {
            return false;
        }
    }
    true
}

fn truncate_error_message(message: &str) -> String {
    const MAX_CHARS: usize = 500;
    let mut out = String::new();
    for (idx, ch) in message.chars().enumerate() {
        if idx >= MAX_CHARS {
            out.push_str("...");
            break;
        }
        out.push(ch);
    }
    out
}

fn validate_output_path(path: &PathBuf) -> Result<(), String> {
    if path.as_os_str().is_empty() {
        return Err("[EXPORT_ERROR] Invalid output path".to_string());
    }
    if path.file_name().is_none() {
        return Err("[EXPORT_ERROR] Output path must include a file name".to_string());
    }
    if path.exists() && path.is_dir() {
        return Err("[EXPORT_ERROR] Output path points to a directory".to_string());
    }
    Ok(())
}

fn default_output_path(base_name: &str, extension: &str) -> PathBuf {
    let home = std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."));
    let export_dir = home.join("Downloads").join("DbPawExports");
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    export_dir.join(format!(
        "{}_{}.{}",
        sanitize_filename(base_name),
        timestamp,
        extension
    ))
}

fn sanitize_filename(name: &str) -> String {
    let sanitized = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>();
    if sanitized.is_empty() {
        "export".to_string()
    } else {
        sanitized
    }
}

struct ExportWriter {
    format: ExportFormat,
    writer: BufWriter<File>,
    first_json_row: bool,
}

impl ExportWriter {
    fn new(path: PathBuf, format: ExportFormat) -> Result<Self, String> {
        let file =
            File::create(path).map_err(|e| format!("[EXPORT_ERROR] create file failed: {e}"))?;
        let mut writer = BufWriter::new(file);

        if matches!(format, ExportFormat::Json) {
            writer
                .write_all(b"[\n")
                .map_err(|e| format!("[EXPORT_ERROR] write json header failed: {e}"))?;
        }

        Ok(Self {
            format,
            writer,
            first_json_row: true,
        })
    }

    fn write_csv_header(&mut self, columns: &[String]) -> Result<(), String> {
        if !matches!(self.format, ExportFormat::Csv) {
            return Ok(());
        }
        let header = columns
            .iter()
            .map(|c| csv_escape(c))
            .collect::<Vec<_>>()
            .join(",");
        self.writer
            .write_all(format!("{header}\n").as_bytes())
            .map_err(|e| format!("[EXPORT_ERROR] write csv header failed: {e}"))
    }

    fn write_rows(
        &mut self,
        rows: &[Value],
        columns: &[String],
        schema: Option<&str>,
        table: &str,
        driver: &str,
    ) -> Result<i64, String> {
        let mut count = 0;
        for row in rows {
            let obj = row
                .as_object()
                .ok_or("[EXPORT_ERROR] row is not a JSON object")?;
            self.write_row(obj, columns, schema, table, driver)?;
            count += 1;
        }
        Ok(count)
    }

    fn write_row(
        &mut self,
        row: &Map<String, Value>,
        columns: &[String],
        schema: Option<&str>,
        table: &str,
        driver: &str,
    ) -> Result<(), String> {
        match self.format {
            ExportFormat::Csv => {
                let line = columns
                    .iter()
                    .map(|c| row.get(c).map(csv_value).unwrap_or_else(|| "".to_string()))
                    .collect::<Vec<_>>()
                    .join(",");
                self.writer
                    .write_all(format!("{line}\n").as_bytes())
                    .map_err(|e| format!("[EXPORT_ERROR] write csv row failed: {e}"))?;
            }
            ExportFormat::Json => {
                if !self.first_json_row {
                    self.writer
                        .write_all(b",\n")
                        .map_err(|e| format!("[EXPORT_ERROR] write json separator failed: {e}"))?;
                }
                self.first_json_row = false;
                let text = serde_json::to_string(row)
                    .map_err(|e| format!("[EXPORT_ERROR] serialize json row failed: {e}"))?;
                self.writer
                    .write_all(text.as_bytes())
                    .map_err(|e| format!("[EXPORT_ERROR] write json row failed: {e}"))?;
            }
            ExportFormat::SqlDml | ExportFormat::SqlFull => {
                let quoted_cols = columns
                    .iter()
                    .map(|c| quote_ident(c, driver))
                    .collect::<Vec<_>>()
                    .join(", ");
                let values = columns
                    .iter()
                    .map(|c| {
                        row.get(c)
                            .map(sql_value)
                            .unwrap_or_else(|| "NULL".to_string())
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                let statement = format!(
                    "INSERT INTO {} ({}) VALUES ({});\n",
                    quote_target(schema, table, driver),
                    quoted_cols,
                    values
                );
                self.writer
                    .write_all(statement.as_bytes())
                    .map_err(|e| format!("[EXPORT_ERROR] write sql row failed: {e}"))?;
            }
            ExportFormat::SqlDdl => unreachable!("SqlDdl rows are never written"),
        }
        Ok(())
    }

    fn write_ddl(&mut self, ddl: &str) -> Result<(), String> {
        let content = format!("{}\n\n", ddl.trim_end());
        self.writer
            .write_all(content.as_bytes())
            .map_err(|e| format!("[EXPORT_ERROR] write ddl failed: {e}"))?;
        Ok(())
    }

    fn finish(&mut self) -> Result<(), String> {
        if matches!(self.format, ExportFormat::Json) {
            self.writer
                .write_all(b"\n]\n")
                .map_err(|e| format!("[EXPORT_ERROR] write json end failed: {e}"))?;
        }
        self.writer
            .flush()
            .map_err(|e| format!("[EXPORT_ERROR] flush file failed: {e}"))?;
        Ok(())
    }
}

fn csv_escape(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') || value.contains('\r') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn csv_value(value: &Value) -> String {
    if value.is_null() {
        return "".to_string();
    }
    let raw = match value {
        Value::String(s) => s.clone(),
        _ => value.to_string(),
    };
    csv_escape(&raw)
}

fn sql_value(value: &Value) -> String {
    match value {
        Value::Null => "NULL".to_string(),
        Value::Bool(v) => {
            if *v {
                "TRUE".to_string()
            } else {
                "FALSE".to_string()
            }
        }
        Value::Number(n) => n.to_string(),
        Value::String(s) => format!("'{}'", s.replace('\'', "''")),
        _ => format!("'{}'", value.to_string().replace('\'', "''")),
    }
}

fn quote_ident(name: &str, driver: &str) -> String {
    if driver.eq_ignore_ascii_case("mysql")
        || driver.eq_ignore_ascii_case("tidb")
        || driver.eq_ignore_ascii_case("mariadb")
        || driver.eq_ignore_ascii_case("clickhouse")
    {
        format!("`{}`", name.replace('`', "``"))
    } else if driver.eq_ignore_ascii_case("mssql") {
        format!("[{}]", name.replace(']', "]]"))
    } else {
        format!("\"{}\"", name.replace('"', "\"\""))
    }
}

fn quote_target(schema: Option<&str>, table: &str, driver: &str) -> String {
    let normalized_schema = schema
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .and_then(|s| {
            if driver.eq_ignore_ascii_case("duckdb")
                && (s.eq_ignore_ascii_case("main") || s.eq_ignore_ascii_case("public"))
            {
                None
            } else {
                Some(s)
            }
        });

    match normalized_schema {
        Some(schema_name) => format!(
            "{}.{}",
            quote_ident(schema_name, driver),
            quote_ident(table, driver)
        ),
        None => quote_ident(table, driver),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{
        QueryResult, SchemaOverview, TableDataResponse, TableInfo, TableMetadata, TableStructure,
    };
    use async_trait::async_trait;
    use std::collections::HashMap;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct FakeExportDriver {
        tables: Vec<TableInfo>,
        ddls: HashMap<(String, String), String>,
        rows: HashMap<(String, String), Vec<Value>>,
    }

    #[async_trait]
    impl DatabaseDriver for FakeExportDriver {
        async fn test_connection(&self) -> Result<(), String> {
            Ok(())
        }

        async fn list_databases(&self) -> Result<Vec<String>, String> {
            Ok(vec!["db".to_string()])
        }

        async fn list_tables(&self, _schema: Option<String>) -> Result<Vec<TableInfo>, String> {
            Ok(self.tables.clone())
        }

        async fn get_table_structure(
            &self,
            _schema: String,
            _table: String,
        ) -> Result<TableStructure, String> {
            Err("not used".to_string())
        }

        async fn get_table_metadata(
            &self,
            schema: String,
            table: String,
        ) -> Result<TableMetadata, String> {
            let key = (schema, table);
            let has_rows = self.rows.contains_key(&key);
            let columns = if has_rows {
                vec![crate::models::ColumnInfo {
                    name: "id".to_string(),
                    r#type: "INT".to_string(),
                    nullable: false,
                    default_value: None,
                    primary_key: true,
                    comment: None,
                }]
            } else {
                Vec::new()
            };
            Ok(TableMetadata {
                columns,
                indexes: Vec::new(),
                foreign_keys: Vec::new(),
                clickhouse_extra: None,
            })
        }

        async fn get_table_ddl(&self, schema: String, table: String) -> Result<String, String> {
            self.ddls
                .get(&(schema, table))
                .cloned()
                .ok_or_else(|| "missing ddl".to_string())
        }

        async fn get_table_data(
            &self,
            _schema: String,
            _table: String,
            _page: i64,
            _limit: i64,
            _sort_column: Option<String>,
            _sort_direction: Option<String>,
            _filter: Option<String>,
            _order_by: Option<String>,
        ) -> Result<TableDataResponse, String> {
            Err("not used".to_string())
        }

        async fn get_table_data_chunk(
            &self,
            schema: String,
            table: String,
            page: i64,
            limit: i64,
            _sort_column: Option<String>,
            _sort_direction: Option<String>,
            _filter: Option<String>,
            _order_by: Option<String>,
        ) -> Result<TableDataResponse, String> {
            let key = (schema, table);
            let all_rows = self.rows.get(&key).cloned().unwrap_or_default();
            let offset = ((page.max(1) - 1) * limit.max(1)) as usize;
            let chunk = all_rows
                .into_iter()
                .skip(offset)
                .take(limit.max(1) as usize)
                .collect::<Vec<_>>();
            Ok(TableDataResponse {
                total: self
                    .rows
                    .get(&key)
                    .map(|rows| rows.len() as i64)
                    .unwrap_or(0),
                data: chunk,
                page,
                limit,
                execution_time_ms: 0,
            })
        }

        async fn execute_query(&self, _sql: String) -> Result<QueryResult, String> {
            Err("not used".to_string())
        }

        async fn get_schema_overview(
            &self,
            _schema: Option<String>,
        ) -> Result<SchemaOverview, String> {
            Err("not used".to_string())
        }

        async fn close(&self) {}
    }

    #[test]
    fn csv_escape_works() {
        assert_eq!(csv_escape("simple"), "simple");
        assert_eq!(csv_escape("a,b"), "\"a,b\"");
        assert_eq!(csv_escape("a\"b"), "\"a\"\"b\"");
        assert_eq!(csv_escape("a\nb"), "\"a\nb\"");
        assert_eq!(csv_escape("a,\nb"), "\"a,\nb\"");
    }

    #[test]
    fn sql_value_works() {
        assert_eq!(sql_value(&Value::Null), "NULL");
        assert_eq!(sql_value(&Value::Bool(true)), "TRUE");
        assert_eq!(
            sql_value(&Value::String("O'Reilly".to_string())),
            "'O''Reilly'"
        );
        assert_eq!(
            sql_value(&Value::Number(serde_json::Number::from(42))),
            "42"
        );
        assert_eq!(sql_value(&Value::Bool(false)), "FALSE");
    }

    #[test]
    fn quote_target_uses_schema_when_present() {
        assert_eq!(
            quote_target(Some("public"), "users", "postgres"),
            "\"public\".\"users\""
        );
        assert_eq!(
            quote_target(Some("analytics"), "events", "mysql"),
            "`analytics`.`events`"
        );
        assert_eq!(
            quote_target(Some("analytics"), "events", "tidb"),
            "`analytics`.`events`"
        );
        assert_eq!(
            quote_target(Some("analytics"), "events", "mariadb"),
            "`analytics`.`events`"
        );
        assert_eq!(
            quote_target(Some("analytics"), "events", "clickhouse"),
            "`analytics`.`events`"
        );
        assert_eq!(
            quote_target(Some("dbo"), "events", "mssql"),
            "[dbo].[events]"
        );
    }

    #[test]
    fn quote_target_ignores_empty_schema() {
        assert_eq!(quote_target(Some("  "), "users", "postgres"), "\"users\"");
        assert_eq!(quote_target(None, "users", "mysql"), "`users`");
        assert_eq!(quote_target(None, "users", "tidb"), "`users`");
        assert_eq!(quote_target(None, "users", "mariadb"), "`users`");
    }

    #[test]
    fn quote_target_uses_unqualified_main_for_duckdb() {
        assert_eq!(quote_target(Some("main"), "users", "duckdb"), "\"users\"");
        assert_eq!(
            quote_target(Some("analytics"), "events", "duckdb"),
            "\"analytics\".\"events\""
        );
    }

    #[test]
    fn quote_ident_escapes_driver_specific_chars() {
        assert_eq!(quote_ident("a`b", "mysql"), "`a``b`");
        assert_eq!(quote_ident("a`b", "clickhouse"), "`a``b`");
        assert_eq!(quote_ident("a]b", "mssql"), "[a]]b]");
        assert_eq!(quote_ident("a\"b", "postgres"), "\"a\"\"b\"");
    }

    #[test]
    fn validate_output_path_rejects_empty_path() {
        assert_eq!(
            validate_output_path(&PathBuf::new()).unwrap_err(),
            "[EXPORT_ERROR] Invalid output path"
        );
    }

    #[test]
    fn validate_output_path_rejects_directory_path() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("dbpaw-transfer-test-dir-{unique}"));
        fs::create_dir_all(&dir).unwrap();
        let err = validate_output_path(&dir).unwrap_err();
        assert_eq!(err, "[EXPORT_ERROR] Output path points to a directory");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn validate_output_path_rejects_path_without_filename() {
        let err = validate_output_path(&PathBuf::from("/")).unwrap_err();
        assert_eq!(err, "[EXPORT_ERROR] Output path must include a file name");
    }

    #[test]
    fn write_rows_rejects_non_object_rows() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("dbpaw-transfer-test-{unique}.json"));
        let mut writer = ExportWriter::new(path.clone(), ExportFormat::Json).unwrap();
        let err = writer
            .write_rows(
                &[Value::String("not-object".to_string())],
                &["a".to_string()],
                None,
                "t",
                "postgres",
            )
            .unwrap_err();
        assert_eq!(err, "[EXPORT_ERROR] row is not a JSON object");
        let _ = fs::remove_file(path);
    }

    #[test]
    fn parse_sql_statements_handles_quotes_and_comments() {
        let sql = r#"
            -- comment 1
            INSERT INTO users (name, note) VALUES ('alice', 'hello;world');
            /* block comment ; ; */
            INSERT INTO users (name) VALUES ("bob");
            # mysql style comment
            INSERT INTO users(name) VALUES ($tag$semi;inside$tag$);
        "#;

        let statements = parse_sql_statements(sql, "mysql").unwrap();
        assert_eq!(statements.len(), 3);
        assert!(statements[0].starts_with("INSERT INTO users"));
        assert!(statements[1].contains("\"bob\""));
        assert!(statements[2].contains("$tag$semi;inside$tag$"));
    }

    #[test]
    fn parse_sql_statements_rejects_unterminated_block_comment() {
        let err = parse_sql_statements("INSERT INTO t VALUES (1); /*", "mysql").unwrap_err();
        assert!(err.contains("Unterminated block comment"));
    }

    #[test]
    fn parse_sql_statements_preserves_hash_for_postgres() {
        let sql = "SELECT 1 # 2;\nSELECT '#not_comment';";
        let statements = parse_sql_statements(sql, "postgres").unwrap();
        assert_eq!(statements.len(), 2);
        assert_eq!(statements[0], "SELECT 1 # 2");
        assert_eq!(statements[1], "SELECT '#not_comment'");
    }

    #[test]
    fn parse_sql_statements_supports_mysql_delimiter_blocks() {
        let sql = r#"
            DELIMITER $$
            CREATE PROCEDURE p_demo()
            BEGIN
                SELECT 1;
                SELECT 'semi;inside';
            END$$
            DELIMITER ;
            SELECT 2;
        "#;

        let statements = parse_sql_statements(sql, "mysql").unwrap();
        assert_eq!(statements.len(), 2);
        assert!(statements[0].starts_with("CREATE PROCEDURE p_demo()"));
        assert!(statements[0].contains("SELECT 1;"));
        assert!(statements[0].contains("SELECT 'semi;inside';"));
        assert_eq!(statements[1], "SELECT 2");
    }

    #[test]
    fn parse_sql_statements_ignores_mysql_delimiter_inside_strings() {
        let sql = r#"
            DELIMITER //
            CREATE TRIGGER trg_demo BEFORE INSERT ON demo
            FOR EACH ROW
            BEGIN
                SET @note = 'DELIMITER // should stay';
            END//
            DELIMITER ;
        "#;

        let statements = parse_sql_statements(sql, "mysql").unwrap();
        assert_eq!(statements.len(), 1);
        assert!(statements[0].contains("DELIMITER // should stay"));
        assert!(statements[0].contains("END"));
    }

    #[test]
    fn parse_sql_statements_supports_sqlite_trigger_blocks() {
        let sql = r#"
            CREATE TRIGGER trg_demo
            AFTER INSERT ON demo
            BEGIN
                INSERT INTO audit_log(message) VALUES ('first;value');
                UPDATE demo SET touched_at = CURRENT_TIMESTAMP WHERE rowid = NEW.rowid;
            END;
            SELECT 1;
        "#;

        let statements = parse_sql_statements(sql, "sqlite").unwrap();
        assert_eq!(statements.len(), 2);
        assert!(statements[0].starts_with("CREATE TRIGGER trg_demo"));
        assert!(statements[0].contains("VALUES ('first;value');"));
        assert!(statements[0].contains("UPDATE demo SET touched_at = CURRENT_TIMESTAMP"));
        assert_eq!(statements[1], "SELECT 1");
    }

    #[test]
    fn parse_sql_statements_keeps_sqlite_case_end_inside_trigger_body() {
        let sql = r#"
            CREATE TRIGGER trg_case
            AFTER UPDATE ON demo
            BEGIN
                UPDATE demo
                SET status = CASE WHEN NEW.id > 10 THEN 'big' ELSE 'small' END;
            END;
        "#;

        let statements = parse_sql_statements(sql, "sqlite").unwrap();
        assert_eq!(statements.len(), 1);
        assert!(statements[0].contains("CASE WHEN NEW.id > 10 THEN 'big' ELSE 'small' END;"));
        assert!(statements[0].ends_with("END"));
    }

    #[test]
    fn parse_sql_statements_supports_oracle_create_or_replace_blocks() {
        let sql = r#"
            CREATE OR REPLACE PROCEDURE p_demo IS
            BEGIN
                INSERT INTO audit_log(message) VALUES ('first;value');
                UPDATE audit_log SET message = 'done' WHERE message = 'first;value';
            END;
            /
            SELECT 1 FROM DUAL;
        "#;

        let statements = parse_sql_statements(sql, "oracle").unwrap();
        assert_eq!(statements.len(), 2);
        assert!(statements[0].starts_with("CREATE OR REPLACE PROCEDURE p_demo IS"));
        assert!(statements[0].contains("VALUES ('first;value');"));
        assert!(statements[0].contains("END;"));
        assert_eq!(statements[1], "SELECT 1 FROM DUAL");
    }

    #[test]
    fn parse_sql_statements_supports_oracle_case_end_inside_block() {
        let sql = r#"
            CREATE OR REPLACE FUNCTION f_demo RETURN VARCHAR2 IS
                v_result VARCHAR2(10);
            BEGIN
                v_result := CASE WHEN 1 = 1 THEN 'yes' ELSE 'no' END;
                RETURN v_result;
            END;
            /
        "#;

        let statements = parse_sql_statements(sql, "oracle").unwrap();
        assert_eq!(statements.len(), 1);
        assert!(statements[0].contains("CASE WHEN 1 = 1 THEN 'yes' ELSE 'no' END;"));
        assert!(statements[0].ends_with("END;"));
    }

    #[test]
    fn parse_mssql_batches_splits_on_go_lines_only() {
        let sql = r#"
            SELECT 1;
            GO
            SELECT 'GO should stay in string';
            -- GO in comment should not split
            SELECT 2;
            GO
            /* GO in block comment
               GO
            */
            SELECT 3;
        "#;

        let batches = parse_mssql_batches(sql).unwrap();
        assert_eq!(batches.len(), 3);
        assert!(batches[0].contains("SELECT 1"));
        assert!(batches[1].contains("SELECT 'GO should stay in string'"));
        assert!(batches[2].contains("SELECT 3"));
    }

    #[test]
    fn parse_mssql_batches_supports_go_repeat_count() {
        let sql = "SELECT 1\nGO 3\nSELECT 2\nGO";
        let batches = parse_mssql_batches(sql).unwrap();
        assert_eq!(batches.len(), 4);
        assert_eq!(batches[0], "SELECT 1");
        assert_eq!(batches[1], "SELECT 1");
        assert_eq!(batches[2], "SELECT 1");
        assert_eq!(batches[3], "SELECT 2");
    }

    #[test]
    fn statement_controls_transaction_detects_driver_specific_tokens() {
        assert!(statement_controls_transaction("BEGIN TRANSACTION", "mssql"));
        assert!(!statement_controls_transaction("BEGIN TRY", "mssql"));
        assert!(statement_controls_transaction("BEGIN", "sqlite"));
        assert!(statement_controls_transaction("START TRANSACTION", "mysql"));
        assert!(statement_controls_transaction("ROLLBACK", "postgres"));
    }

    #[test]
    fn prepare_import_plan_disables_outer_tx_when_script_controls_it() {
        let sqlite_plan =
            prepare_import_plan("BEGIN;\nCREATE TABLE t(id INTEGER);\nCOMMIT;", "sqlite").unwrap();
        assert_eq!(sqlite_plan.units.len(), 3);
        assert!(sqlite_plan.script_managed_transaction);

        let mssql_plan = prepare_import_plan("SELECT 1\nGO\nSELECT 2", "mssql").unwrap();
        assert_eq!(mssql_plan.units.len(), 2);
        assert!(!mssql_plan.script_managed_transaction);
    }

    #[test]
    fn import_transaction_sql_maps_per_driver() {
        assert_eq!(
            import_transaction_sql("mysql", "mysql").unwrap(),
            ("START TRANSACTION", "COMMIT", "ROLLBACK")
        );
        assert_eq!(
            import_transaction_sql("postgres", "postgres").unwrap(),
            ("BEGIN", "COMMIT", "ROLLBACK")
        );
        assert_eq!(
            import_transaction_sql("postgres", "postgresql").unwrap(),
            ("BEGIN", "COMMIT", "ROLLBACK")
        );
        assert_eq!(
            import_transaction_sql("mssql", "mssql").unwrap(),
            (
                "BEGIN TRANSACTION",
                "COMMIT TRANSACTION",
                "ROLLBACK TRANSACTION"
            )
        );
        assert_eq!(
            import_transaction_sql("oracle", "oracle").unwrap(),
            ("SELECT 1 FROM DUAL", "COMMIT", "ROLLBACK")
        );
        assert!(import_transaction_sql("clickhouse", "clickhouse").is_err());
        assert!(import_transaction_sql("starrocks", "starrocks").is_err());
    }

    #[test]
    fn normalize_driver_name_maps_aliases() {
        assert_eq!(normalize_driver_name("postgres"), "postgres");
        assert_eq!(normalize_driver_name("postgresql"), "postgres");
        assert_eq!(normalize_driver_name("pgsql"), "postgres");
        assert_eq!(normalize_driver_name("mysql"), "mysql");
    }

    #[test]
    fn truncate_error_message_caps_length() {
        let source = "x".repeat(600);
        let truncated = truncate_error_message(&source);
        assert!(truncated.len() <= 503);
        assert!(truncated.ends_with("..."));
    }

    fn tmp_path(suffix: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("dbpaw-transfer-test-{unique}-{suffix}"))
    }

    fn make_row(pairs: &[(&str, Value)]) -> Value {
        let mut map = serde_json::Map::new();
        for (k, v) in pairs {
            map.insert(k.to_string(), v.clone());
        }
        Value::Object(map)
    }

    #[test]
    fn extension_for_format_sql_variants_all_return_sql() {
        assert_eq!(extension_for_format(&ExportFormat::SqlDml), "sql");
        assert_eq!(extension_for_format(&ExportFormat::SqlDdl), "sql");
        assert_eq!(extension_for_format(&ExportFormat::SqlFull), "sql");
        assert_eq!(extension_for_format(&ExportFormat::Csv), "csv");
        assert_eq!(extension_for_format(&ExportFormat::Json), "json");
    }

    #[test]
    fn export_writer_csv_writes_header_then_rows() {
        let path = tmp_path("csv_header.csv");
        let cols = vec!["id".to_string(), "name".to_string()];
        let mut writer = ExportWriter::new(path.clone(), ExportFormat::Csv).unwrap();
        writer.write_csv_header(&cols).unwrap();
        let rows = vec![make_row(&[
            ("id", Value::Number(1.into())),
            ("name", Value::String("alice".to_string())),
        ])];
        writer
            .write_rows(&rows, &cols, None, "t", "postgres")
            .unwrap();
        writer.finish().unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.starts_with("id,name\n"));
        assert!(content.contains("1,alice"));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn write_csv_header_is_noop_for_sql_formats() {
        let path = tmp_path("sql_noop_header.sql");
        let mut writer = ExportWriter::new(path.clone(), ExportFormat::SqlDml).unwrap();
        writer.write_csv_header(&["id".to_string()]).unwrap();
        writer.finish().unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content, "");
        let _ = fs::remove_file(path);
    }

    #[test]
    fn export_writer_sql_dml_writes_insert_statements() {
        let path = tmp_path("sql_dml.sql");
        let cols = vec!["id".to_string(), "name".to_string()];
        let mut writer = ExportWriter::new(path.clone(), ExportFormat::SqlDml).unwrap();
        let rows = vec![
            make_row(&[
                ("id", Value::Number(1.into())),
                ("name", Value::String("alice".to_string())),
            ]),
            make_row(&[("id", Value::Number(2.into())), ("name", Value::Null)]),
        ];
        let count = writer
            .write_rows(&rows, &cols, Some("public"), "users", "postgres")
            .unwrap();
        writer.finish().unwrap();
        assert_eq!(count, 2);
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("INSERT INTO \"public\".\"users\""));
        assert!(content.contains("VALUES (1, 'alice')"));
        assert!(content.contains("VALUES (2, NULL)"));
        assert!(!content.contains("CREATE TABLE"));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn export_writer_sql_ddl_writes_only_ddl() {
        let path = tmp_path("sql_ddl.sql");
        let mut writer = ExportWriter::new(path.clone(), ExportFormat::SqlDdl).unwrap();
        writer
            .write_ddl("CREATE TABLE users (id INTEGER);")
            .unwrap();
        writer.finish().unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("CREATE TABLE users (id INTEGER);"));
        assert!(!content.contains("INSERT INTO"));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn export_writer_sql_full_writes_ddl_then_inserts() {
        let path = tmp_path("sql_full.sql");
        let cols = vec!["id".to_string(), "val".to_string()];
        let mut writer = ExportWriter::new(path.clone(), ExportFormat::SqlFull).unwrap();
        writer
            .write_ddl("CREATE TABLE t (id INT, val TEXT);")
            .unwrap();
        let rows = vec![make_row(&[
            ("id", Value::Number(1.into())),
            ("val", Value::String("x".to_string())),
        ])];
        let count = writer
            .write_rows(&rows, &cols, None, "t", "postgres")
            .unwrap();
        writer.finish().unwrap();
        assert_eq!(count, 1);
        let content = fs::read_to_string(&path).unwrap();
        let ddl_pos = content.find("CREATE TABLE").unwrap();
        let dml_pos = content.find("INSERT INTO").unwrap();
        assert!(ddl_pos < dml_pos, "DDL should appear before DML");
        assert!(content.contains("VALUES (1, 'x')"));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn database_export_writes_all_tables_in_schema_then_name_order() {
        let path = tmp_path("database_export.sql");
        let driver = Arc::new(FakeExportDriver {
            tables: vec![
                TableInfo {
                    schema: "zeta".to_string(),
                    name: "logs".to_string(),
                    r#type: "table".to_string(),
                },
                TableInfo {
                    schema: "alpha".to_string(),
                    name: "users".to_string(),
                    r#type: "table".to_string(),
                },
                TableInfo {
                    schema: "alpha".to_string(),
                    name: "accounts".to_string(),
                    r#type: "table".to_string(),
                },
            ],
            ddls: HashMap::from([
                (
                    ("alpha".to_string(), "accounts".to_string()),
                    "CREATE TABLE accounts (id INT);".to_string(),
                ),
                (
                    ("alpha".to_string(), "users".to_string()),
                    "CREATE TABLE users (id INT);".to_string(),
                ),
                (
                    ("zeta".to_string(), "logs".to_string()),
                    "CREATE TABLE logs (id INT);".to_string(),
                ),
            ]),
            rows: HashMap::from([
                (
                    ("alpha".to_string(), "accounts".to_string()),
                    vec![make_row(&[("id", Value::Number(1.into()))])],
                ),
                (
                    ("alpha".to_string(), "users".to_string()),
                    vec![make_row(&[("id", Value::Number(2.into()))])],
                ),
                (
                    ("zeta".to_string(), "logs".to_string()),
                    vec![make_row(&[("id", Value::Number(3.into()))])],
                ),
            ]),
        });

        let result = tauri::async_runtime::block_on(do_database_export(
            driver,
            path.clone(),
            "postgres".to_string(),
            ExportFormat::SqlFull,
            2000,
        ))
        .unwrap();

        assert_eq!(result.row_count, 3);
        let content = fs::read_to_string(&path).unwrap();
        let accounts_pos = content.find("CREATE TABLE accounts").unwrap();
        let users_pos = content.find("CREATE TABLE users").unwrap();
        let logs_pos = content.find("CREATE TABLE logs").unwrap();
        assert!(accounts_pos < users_pos);
        assert!(users_pos < logs_pos);
        assert!(content.contains("INSERT INTO \"alpha\".\"accounts\""));
        assert!(content.contains("INSERT INTO \"alpha\".\"users\""));
        assert!(content.contains("INSERT INTO \"zeta\".\"logs\""));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn database_export_respects_sql_ddl_mode() {
        let path = tmp_path("database_export_ddl.sql");
        let driver = Arc::new(FakeExportDriver {
            tables: vec![TableInfo {
                schema: "public".to_string(),
                name: "users".to_string(),
                r#type: "table".to_string(),
            }],
            ddls: HashMap::from([(
                ("public".to_string(), "users".to_string()),
                "CREATE TABLE users (id INT);".to_string(),
            )]),
            rows: HashMap::from([(
                ("public".to_string(), "users".to_string()),
                vec![make_row(&[("id", Value::Number(1.into()))])],
            )]),
        });

        let result = tauri::async_runtime::block_on(do_database_export(
            driver,
            path.clone(),
            "postgres".to_string(),
            ExportFormat::SqlDdl,
            2000,
        ))
        .unwrap();

        assert_eq!(result.row_count, 0);
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("CREATE TABLE users"));
        assert!(!content.contains("INSERT INTO"));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn database_export_respects_sql_dml_mode() {
        let path = tmp_path("database_export_dml.sql");
        let driver = Arc::new(FakeExportDriver {
            tables: vec![TableInfo {
                schema: "public".to_string(),
                name: "users".to_string(),
                r#type: "table".to_string(),
            }],
            ddls: HashMap::from([(
                ("public".to_string(), "users".to_string()),
                "CREATE TABLE users (id INT);".to_string(),
            )]),
            rows: HashMap::from([(
                ("public".to_string(), "users".to_string()),
                vec![make_row(&[("id", Value::Number(1.into()))])],
            )]),
        });

        let result = tauri::async_runtime::block_on(do_database_export(
            driver,
            path.clone(),
            "postgres".to_string(),
            ExportFormat::SqlDml,
            2000,
        ))
        .unwrap();

        assert_eq!(result.row_count, 1);
        let content = fs::read_to_string(&path).unwrap();
        assert!(!content.contains("CREATE TABLE"));
        assert!(content.contains("INSERT INTO \"public\".\"users\""));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn write_ddl_trims_trailing_whitespace_and_adds_blank_line() {
        let path = tmp_path("ddl_trim.sql");
        let mut writer = ExportWriter::new(path.clone(), ExportFormat::SqlDdl).unwrap();
        writer.write_ddl("CREATE TABLE t (id INT);   \n\n").unwrap();
        writer.finish().unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.starts_with("CREATE TABLE t (id INT);"));
        assert!(content.ends_with("\n\n"));
        let _ = fs::remove_file(path);
    }
}
