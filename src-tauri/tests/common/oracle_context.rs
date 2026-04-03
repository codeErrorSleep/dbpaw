mod shared;

use dbpaw_lib::models::ConnectionForm;

pub use shared::{connect_with_retry, should_reuse_local_db};

/// Oracle has no freely-distributable Docker image (the Oracle Database Free image
/// at container-registry.oracle.com requires an Oracle account and terms acceptance).
/// Integration tests therefore only support IT_REUSE_LOCAL_DB=1 mode.
///
/// Required environment variables (all have defaults):
///   ORACLE_HOST       – defaults to "localhost"
///   ORACLE_PORT       – defaults to 1521
///   ORACLE_USER       – defaults to "system"
///   ORACLE_PASSWORD   – no default (required)
///   ORACLE_SERVICE    – defaults to "FREE"   (service name / SID)
///   ORACLE_SCHEMA     – defaults to "SYSTEM" (schema to use for test tables)
pub fn oracle_form_from_test_context() -> ConnectionForm {
    if !should_reuse_local_db() {
        panic!(
            "Oracle integration tests require a local Oracle instance. \
             Set IT_REUSE_LOCAL_DB=1 and provide ORACLE_HOST/PORT/USER/PASSWORD/SERVICE env vars."
        );
    }
    oracle_form_from_local_env()
}

fn oracle_form_from_local_env() -> ConnectionForm {
    ConnectionForm {
        driver: "oracle".to_string(),
        host: Some(shared::env_or("ORACLE_HOST", "localhost")),
        port: Some(shared::env_i64("ORACLE_PORT", 1521)),
        username: Some(shared::env_or("ORACLE_USER", "system")),
        password: Some(shared::env_or("ORACLE_PASSWORD", "")),
        database: Some(shared::env_or("ORACLE_SERVICE", "FREE")),
        schema: Some(shared::env_or("ORACLE_SCHEMA", "SYSTEM")),
        ..Default::default()
    }
}
