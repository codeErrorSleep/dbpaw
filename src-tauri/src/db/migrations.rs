use crate::error::AppError;
use sqlx::{Pool, Sqlite};

const MIGRATIONS: &[(i64, &str)] = &[
    (1, include_str!("../../migrations/001_initial.sql")),
    (2, include_str!("../../migrations/002_saved_queries.sql")),
    (
        3,
        include_str!("../../migrations/003_add_database_to_saved_queries.sql"),
    ),
    (4, include_str!("../../migrations/004_add_ssh_fields.sql")),
    (5, include_str!("../../migrations/005_ai_providers.sql")),
    (6, include_str!("../../migrations/006_ai_conversations.sql")),
    (7, include_str!("../../migrations/007_ai_messages.sql")),
    (
        8,
        include_str!("../../migrations/008_ai_provider_vendor_unique.sql"),
    ),
    (
        9,
        include_str!("../../migrations/009_ai_provider_type_relaxed.sql"),
    ),
    (
        10,
        include_str!("../../migrations/010_sql_execution_logs.sql"),
    ),
    (11, include_str!("../../migrations/011_add_ssl_fields.sql")),
    (
        12,
        include_str!("../../migrations/012_add_redis_connection_options.sql"),
    ),
    (
        13,
        include_str!("../../migrations/013_add_elasticsearch_connection_options.sql"),
    ),
    (
        14,
        include_str!("../../migrations/014_add_sentinel_fields.sql"),
    ),
    (
        15,
        include_str!("../../migrations/015_add_mongodb_auth_source.sql"),
    ),
    (
        16,
        include_str!("../../migrations/016_redis_command_logs.sql"),
    ),
];

/// 迁移是否已实际生效（表/列已存在）。
/// 必须基于真实 schema 检查而不是 schema_migrations 记录：旧版本数据库
/// 曾被一次性标记为"全部已应用"却未执行（缺 auth_source 列、
/// redis_command_logs 表），只看记录会继续漏掉迁移。
async fn migration_applied(pool: &Pool<Sqlite>, version: i64) -> Result<bool, AppError> {
    let table_exists = |name: &str| {
        format!("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='{name}')")
    };
    let column_exists = |table: &str, column: &str| {
        format!(
            "SELECT EXISTS(SELECT 1 FROM pragma_table_info('{table}') WHERE name='{column}')"
        )
    };

    let sql = match version {
        1 => table_exists("connections"),
        2 => table_exists("saved_queries"),
        3 => column_exists("saved_queries", "database"),
        4 => column_exists("connections", "ssh_enabled"),
        5 => table_exists("ai_providers"),
        6 => table_exists("ai_conversations"),
        7 => table_exists("ai_messages"),
        8 => "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='index' AND name='idx_ai_providers_provider_type_unique')".to_string(),
        // 009 SQL 全幂等，直接执行（与旧逻辑一致）
        9 => return Ok(false),
        10 => table_exists("sql_execution_logs"),
        11 => column_exists("connections", "ssl_mode"),
        12 => column_exists("connections", "mode"),
        13 => column_exists("connections", "auth_mode"),
        14 => column_exists("connections", "service_name"),
        15 => column_exists("connections", "auth_source"),
        16 => table_exists("redis_command_logs"),
        _ => return Ok(true),
    };

    sqlx::query_scalar::<_, bool>(&sql)
        .fetch_one(pool)
        .await
        .map_err(|e| AppError::internal(format!("检查迁移 {version:03} 状态失败: {e}")))
}

pub async fn run_migrations(pool: &Pool<Sqlite>) -> Result<(), AppError> {
    for &(version, sql) in MIGRATIONS {
        if migration_applied(pool, version).await? {
            continue;
        }
        sqlx::query(sql)
            .execute(pool)
            .await
            .map_err(|e| AppError::internal(format!("迁移 {version:03} 执行失败: {e}")))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::run_migrations;

    async fn mem_pool() -> sqlx::Pool<sqlx::Sqlite> {
        sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("connect sqlite memory db")
    }

    #[tokio::test]
    async fn fresh_db_applies_everything_and_is_idempotent() {
        let pool = mem_pool().await;
        run_migrations(&pool).await.expect("fresh db migrations");
        let auth_source: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM pragma_table_info('connections') WHERE name='auth_source')",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(auth_source, "auth_source column should exist after fresh migration");
        let redis_logs: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='redis_command_logs')",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(redis_logs, "redis_command_logs table should exist after fresh migration");
        run_migrations(&pool).await.expect("rerun should be idempotent");
    }

    #[tokio::test]
    async fn legacy_db_without_migration_records_gets_missing_columns() {
        let pool = mem_pool().await;
        // 模拟旧版本数据库：只有早期 connections 表，没有 schema_migrations 表
        sqlx::query(
            "CREATE TABLE connections (
                id INTEGER PRIMARY KEY AUTOINCREMENT, uuid TEXT, type TEXT,
                name TEXT, host TEXT, port INTEGER, database TEXT, username TEXT,
                password TEXT, ssl INTEGER DEFAULT 0, file_path TEXT,
                created_at TEXT, updated_at TEXT
            )",
        )
        .execute(&pool)
        .await
        .expect("create legacy connections table");

        run_migrations(&pool).await.expect("legacy db migrations");
        let auth_source: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM pragma_table_info('connections') WHERE name='auth_source')",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(auth_source, "legacy db should be healed with auth_source column");
        let redis_logs: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='redis_command_logs')",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(redis_logs, "legacy db should be healed with redis_command_logs table");

    }

    #[tokio::test]
    async fn poisoned_db_with_all_records_marked_gets_healed() {
        let pool = mem_pool().await;
        // 复现旧 bug：schema_migrations 全标记，但 015 列 / 016 表缺失
        sqlx::query(
            "CREATE TABLE connections (
                id INTEGER PRIMARY KEY AUTOINCREMENT, uuid TEXT, type TEXT,
                name TEXT, host TEXT, port INTEGER, database TEXT, username TEXT,
                password TEXT, ssl INTEGER DEFAULT 0, file_path TEXT,
                ssl_mode TEXT, mode TEXT, auth_mode TEXT, service_name TEXT,
                created_at TEXT, updated_at TEXT
            )",
        )
        .execute(&pool)
        .await
        .expect("create poisoned connections table");
        sqlx::query(
            "CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL DEFAULT (datetime('now')))",
        )
        .execute(&pool)
        .await
        .expect("create schema_migrations");
        for v in 1..=16i64 {
            sqlx::query("INSERT INTO schema_migrations (version) VALUES (?)")
                .bind(v)
                .execute(&pool)
                .await
                .expect("mark all migrations applied");
        }

        run_migrations(&pool).await.expect("poisoned db migrations");
        let auth_source: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM pragma_table_info('connections') WHERE name='auth_source')",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(auth_source, "poisoned db should be healed with auth_source column");
        let redis_logs: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='redis_command_logs')",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(redis_logs, "poisoned db should be healed with redis_command_logs table");
    }
}

