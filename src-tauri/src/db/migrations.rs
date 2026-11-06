use crate::error::AppError;
use sqlx::{Pool, Sqlite, Transaction};

type Tx<'a> = Transaction<'a, Sqlite>;

async fn execute(tx: &mut Tx<'_>, sql: &str, version: &str) -> Result<(), AppError> {
    sqlx::query(sql)
        .execute(&mut **tx)
        .await
        .map_err(|e| AppError::internal(format!("迁移 {version} 执行失败: {e}")))?;
    Ok(())
}

async fn object_exists(tx: &mut Tx<'_>, kind: &str, name: &str) -> Result<bool, AppError> {
    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = ? AND name = ?)")
        .bind(kind)
        .bind(name)
        .fetch_one(&mut **tx)
        .await
        .map_err(|e| AppError::internal(format!("检查数据库结构 {name} 失败: {e}")))
}

async fn column_exists(tx: &mut Tx<'_>, table: &str, column: &str) -> Result<bool, AppError> {
    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pragma_table_info(?) WHERE name = ?)")
        .bind(table)
        .bind(column)
        .fetch_one(&mut **tx)
        .await
        .map_err(|e| AppError::internal(format!("检查数据库列 {table}.{column} 失败: {e}")))
}

async fn ensure_object(
    tx: &mut Tx<'_>,
    kind: &str,
    name: &str,
    sql: &str,
    version: &str,
) -> Result<(), AppError> {
    if !object_exists(tx, kind, name).await? {
        execute(tx, sql, version).await?;
    }
    Ok(())
}

async fn ensure_column(
    tx: &mut Tx<'_>,
    table: &str,
    column: &str,
    sql: &str,
    version: &str,
) -> Result<(), AppError> {
    if !column_exists(tx, table, column).await? {
        execute(tx, sql, version).await?;
    }
    Ok(())
}

async fn relaxed_provider_triggers_exist(tx: &mut Tx<'_>) -> Result<bool, AppError> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'trigger' \
         AND name IN ('trg_ai_providers_provider_type_insert', 'trg_ai_providers_provider_type_update') \
         AND sql LIKE '%provider_type must be lowercase and match%'",
    )
    .fetch_one(&mut **tx)
    .await
    .map_err(|e| AppError::internal(format!("检查 AI provider 触发器失败: {e}")))?;
    Ok(count == 2)
}

pub async fn run_migrations(pool: &Pool<Sqlite>) -> Result<(), AppError> {
    // A failed repair must not leave a half-migrated database behind.
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| AppError::internal(format!("开始本地数据库迁移失败: {e}")))?;

    for (name, sql, version) in [
        (
            "connections",
            include_str!("../../migrations/001_initial.sql"),
            "001",
        ),
        (
            "saved_queries",
            include_str!("../../migrations/002_saved_queries.sql"),
            "002",
        ),
    ] {
        ensure_object(&mut tx, "table", name, sql, version).await?;
    }
    ensure_column(
        &mut tx,
        "saved_queries",
        "database",
        include_str!("../../migrations/003_add_database_to_saved_queries.sql"),
        "003",
    )
    .await?;

    // Multi-column ALTER migrations are checked one column at a time. An
    // interrupted older run can have any subset of these columns already.
    for (column, sql, version) in [
        (
            "ssh_enabled",
            "ALTER TABLE connections ADD COLUMN ssh_enabled INTEGER DEFAULT 0",
            "004",
        ),
        (
            "ssh_host",
            "ALTER TABLE connections ADD COLUMN ssh_host TEXT",
            "004",
        ),
        (
            "ssh_port",
            "ALTER TABLE connections ADD COLUMN ssh_port INTEGER",
            "004",
        ),
        (
            "ssh_username",
            "ALTER TABLE connections ADD COLUMN ssh_username TEXT",
            "004",
        ),
        (
            "ssh_password",
            "ALTER TABLE connections ADD COLUMN ssh_password TEXT",
            "004",
        ),
        (
            "ssh_key_path",
            "ALTER TABLE connections ADD COLUMN ssh_key_path TEXT",
            "004",
        ),
    ] {
        ensure_column(&mut tx, "connections", column, sql, version).await?;
    }

    for (name, sql, version) in [
        (
            "ai_providers",
            include_str!("../../migrations/005_ai_providers.sql"),
            "005",
        ),
        (
            "ai_conversations",
            include_str!("../../migrations/006_ai_conversations.sql"),
            "006",
        ),
        (
            "ai_messages",
            include_str!("../../migrations/007_ai_messages.sql"),
            "007",
        ),
    ] {
        ensure_object(&mut tx, "table", name, sql, version).await?;
    }

    // 008 rewrites provider types. A current 009 trigger proves that 008 was
    // superseded; never replay 008 just because somebody removed its index.
    let provider_index =
        object_exists(&mut tx, "index", "idx_ai_providers_provider_type_unique").await?;
    let relaxed_triggers = relaxed_provider_triggers_exist(&mut tx).await?;
    if !provider_index && !relaxed_triggers {
        let has_custom_types: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM ai_providers WHERE lower(provider_type) \
             NOT IN ('openai', 'kimi', 'glm', 'openai_compat'))",
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| AppError::internal(format!("检查旧 AI provider 数据失败: {e}")))?;
        if !has_custom_types {
            execute(
                &mut tx,
                include_str!("../../migrations/008_ai_provider_vendor_unique.sql"),
                "008",
            )
            .await?;
        }
    }
    if !provider_index || !relaxed_triggers {
        execute(
            &mut tx,
            include_str!("../../migrations/009_ai_provider_type_relaxed.sql"),
            "009",
        )
        .await?;
    }

    ensure_object(
        &mut tx,
        "table",
        "sql_execution_logs",
        include_str!("../../migrations/010_sql_execution_logs.sql"),
        "010",
    )
    .await?;
    for (column, sql, version) in [
        (
            "ssl_mode",
            "ALTER TABLE connections ADD COLUMN ssl_mode TEXT",
            "011",
        ),
        (
            "ssl_ca_cert",
            "ALTER TABLE connections ADD COLUMN ssl_ca_cert TEXT",
            "011",
        ),
        (
            "mode",
            "ALTER TABLE connections ADD COLUMN mode TEXT",
            "012",
        ),
        (
            "seed_nodes",
            "ALTER TABLE connections ADD COLUMN seed_nodes TEXT",
            "012",
        ),
        (
            "sentinels",
            "ALTER TABLE connections ADD COLUMN sentinels TEXT",
            "012",
        ),
        (
            "connect_timeout_ms",
            "ALTER TABLE connections ADD COLUMN connect_timeout_ms INTEGER",
            "012",
        ),
        (
            "auth_mode",
            "ALTER TABLE connections ADD COLUMN auth_mode TEXT",
            "013",
        ),
        (
            "api_key_id",
            "ALTER TABLE connections ADD COLUMN api_key_id TEXT",
            "013",
        ),
        (
            "api_key_secret",
            "ALTER TABLE connections ADD COLUMN api_key_secret TEXT",
            "013",
        ),
        (
            "api_key_encoded",
            "ALTER TABLE connections ADD COLUMN api_key_encoded TEXT",
            "013",
        ),
        (
            "cloud_id",
            "ALTER TABLE connections ADD COLUMN cloud_id TEXT",
            "013",
        ),
        (
            "service_name",
            "ALTER TABLE connections ADD COLUMN service_name TEXT",
            "014",
        ),
        (
            "sentinel_password",
            "ALTER TABLE connections ADD COLUMN sentinel_password TEXT",
            "014",
        ),
        (
            "auth_source",
            include_str!("../../migrations/015_add_mongodb_auth_source.sql"),
            "015",
        ),
        (
            "proxy",
            "ALTER TABLE connections ADD COLUMN proxy TEXT",
            "017",
        ),
    ] {
        ensure_column(&mut tx, "connections", column, sql, version).await?;
    }
    ensure_object(
        &mut tx,
        "table",
        "redis_command_logs",
        include_str!("../../migrations/016_redis_command_logs.sql"),
        "016",
    )
    .await?;

    // A table may exist even though its CREATE INDEX statement never ran.
    for (name, sql, version) in [
        ("idx_ai_providers_default_true", "CREATE UNIQUE INDEX idx_ai_providers_default_true ON ai_providers(is_default) WHERE is_default = 1", "005"),
        ("idx_ai_conversations_updated_at", "CREATE INDEX idx_ai_conversations_updated_at ON ai_conversations(updated_at DESC)", "006"),
        ("idx_ai_messages_conversation_id", "CREATE INDEX idx_ai_messages_conversation_id ON ai_messages(conversation_id, created_at ASC)", "007"),
        ("idx_sql_execution_logs_executed_at", "CREATE INDEX idx_sql_execution_logs_executed_at ON sql_execution_logs(executed_at DESC)", "010"),
        ("idx_redis_command_logs_executed_at", "CREATE INDEX idx_redis_command_logs_executed_at ON redis_command_logs(executed_at DESC)", "016"),
    ] {
        ensure_object(&mut tx, "index", name, sql, version).await?;
    }

    execute(
        &mut tx,
        "DROP TABLE IF EXISTS schema_migrations",
        "旧版本记录清理",
    )
    .await?;
    tx.commit()
        .await
        .map_err(|e| AppError::internal(format!("提交本地数据库迁移失败: {e}")))
}

#[cfg(test)]
mod tests {
    use super::{column_exists, object_exists, run_migrations};
    use sqlx::{sqlite::SqlitePoolOptions, Pool, Sqlite};

    async fn pool() -> Pool<Sqlite> {
        SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("open SQLite test database")
    }

    async fn assert_current_schema(pool: &Pool<Sqlite>) {
        let mut tx = pool.begin().await.unwrap();
        for table in [
            "connections",
            "saved_queries",
            "ai_providers",
            "ai_conversations",
            "ai_messages",
            "sql_execution_logs",
            "redis_command_logs",
        ] {
            assert!(
                object_exists(&mut tx, "table", table).await.unwrap(),
                "missing {table}"
            );
        }
        for column in [
            "ssh_enabled",
            "ssh_host",
            "ssh_port",
            "ssh_username",
            "ssh_password",
            "ssh_key_path",
            "ssl_mode",
            "ssl_ca_cert",
            "mode",
            "seed_nodes",
            "sentinels",
            "connect_timeout_ms",
            "auth_mode",
            "api_key_id",
            "api_key_secret",
            "api_key_encoded",
            "cloud_id",
            "service_name",
            "sentinel_password",
            "auth_source",
            "proxy",
        ] {
            assert!(
                column_exists(&mut tx, "connections", column).await.unwrap(),
                "missing {column}"
            );
        }
        assert!(column_exists(&mut tx, "saved_queries", "database")
            .await
            .unwrap());
        for index in [
            "idx_ai_providers_default_true",
            "idx_ai_providers_provider_type_unique",
            "idx_ai_conversations_updated_at",
            "idx_ai_messages_conversation_id",
            "idx_sql_execution_logs_executed_at",
            "idx_redis_command_logs_executed_at",
        ] {
            assert!(
                object_exists(&mut tx, "index", index).await.unwrap(),
                "missing {index}"
            );
        }
        assert!(!object_exists(&mut tx, "table", "schema_migrations")
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn fresh_database_migrates_and_reruns() {
        let pool = pool().await;
        run_migrations(&pool).await.unwrap();
        assert_current_schema(&pool).await;
        run_migrations(&pool).await.unwrap();
        assert_current_schema(&pool).await;
    }

    #[tokio::test]
    async fn legacy_database_keeps_connections_and_adds_missing_schema() {
        let pool = pool().await;
        sqlx::query(include_str!("../../migrations/001_initial.sql"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO connections (uuid, name, host, port, database, username, password) VALUES ('old', 'Legacy', 'localhost', 5432, 'db', 'user', 'secret')")
            .execute(&pool).await.unwrap();

        run_migrations(&pool).await.unwrap();
        assert_current_schema(&pool).await;
        let name: String = sqlx::query_scalar("SELECT name FROM connections WHERE uuid = 'old'")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(name, "Legacy");
        run_migrations(&pool).await.unwrap();
    }

    #[tokio::test]
    async fn poisoned_version_records_do_not_hide_missing_schema() {
        let pool = pool().await;
        sqlx::query(include_str!("../../migrations/001_initial.sql"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY)")
            .execute(&pool)
            .await
            .unwrap();
        for version in 1..=16 {
            sqlx::query("INSERT INTO schema_migrations (version) VALUES (?)")
                .bind(version)
                .execute(&pool)
                .await
                .unwrap();
        }

        run_migrations(&pool).await.unwrap();
        assert_current_schema(&pool).await;
        sqlx::query("SELECT auth_source FROM connections")
            .fetch_all(&pool)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn repairs_partial_column_and_index_migrations() {
        let pool = pool().await;
        run_migrations(&pool).await.unwrap();
        sqlx::query("DROP INDEX idx_redis_command_logs_executed_at")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DROP TABLE connections")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(include_str!("../../migrations/001_initial.sql"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("ALTER TABLE connections ADD COLUMN ssh_enabled INTEGER DEFAULT 0")
            .execute(&pool)
            .await
            .unwrap();

        run_migrations(&pool).await.unwrap();
        assert_current_schema(&pool).await;
    }

    #[tokio::test]
    async fn repairing_provider_schema_preserves_custom_provider_types() {
        let pool = pool().await;
        run_migrations(&pool).await.unwrap();
        sqlx::query("INSERT INTO ai_providers (name, provider_type, base_url, model, api_key) VALUES ('Custom', 'custom_api', 'https://example.com', 'model', '')")
            .execute(&pool).await.unwrap();
        sqlx::query("DROP INDEX idx_ai_providers_provider_type_unique")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DROP TRIGGER trg_ai_providers_provider_type_insert")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DROP TRIGGER trg_ai_providers_provider_type_update")
            .execute(&pool)
            .await
            .unwrap();

        run_migrations(&pool).await.unwrap();
        assert_current_schema(&pool).await;
        let provider_type: String =
            sqlx::query_scalar("SELECT provider_type FROM ai_providers WHERE name = 'Custom'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(provider_type, "custom_api");
    }

    #[tokio::test]
    async fn legacy_provider_data_receives_vendor_migration() {
        let pool = pool().await;
        sqlx::query(include_str!("../../migrations/005_ai_providers.sql"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO ai_providers (name, provider_type, base_url, model, api_key) VALUES ('Kimi', 'openai_compat', 'https://api.moonshot.cn', 'model', '')")
            .execute(&pool).await.unwrap();

        run_migrations(&pool).await.unwrap();
        let provider_type: String =
            sqlx::query_scalar("SELECT provider_type FROM ai_providers WHERE name = 'Kimi'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(provider_type, "kimi");
    }
}
