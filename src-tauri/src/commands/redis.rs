use crate::datasources::redis::{
    self, RedisDatabaseInfo, RedisKeyPatchPayload, RedisKeyValue, RedisMutationResult,
    RedisRawResult, RedisScanResponse, RedisSetKeyPayload,
};
use crate::datasources::redis::{connect, RedisConnection};
use crate::models::ConnectionForm;
use crate::state::AppState;
use tauri::State;

async fn connection_form(state: &State<'_, AppState>, id: i64) -> Result<ConnectionForm, String> {
    let local_db = {
        let lock = state.local_db.lock().await;
        lock.clone()
    };
    let db = local_db.ok_or("Local DB not initialized")?;
    let form = db.get_connection_form_by_id(id).await?;
    if form.driver != "redis" {
        return Err(format!(
            "[UNSUPPORTED] Connection {} is not a Redis connection",
            id
        ));
    }
    Ok(form)
}

/// Cache key: standalone uses "{id}:{db}" so different databases on the same
/// server each get their own persistent connection (SELECT is connection-level).
/// Cluster uses "{id}:cluster" since it only supports db0.
fn cache_key(id: i64, database: Option<&str>, is_cluster: bool) -> String {
    if is_cluster {
        format!("{id}:cluster")
    } else {
        format!("{id}:{}", database.unwrap_or(""))
    }
}

/// Returns true if the error string looks like a broken/dropped TCP connection.
fn is_io_error(e: &str) -> bool {
    e.contains("[REDIS_ERROR]") && {
        let lower = e.to_lowercase();
        lower.contains("broken pipe")
            || lower.contains("connection reset")
            || lower.contains("connection refused")
            || lower.contains("connection closed")
            || lower.contains("eof")
            || lower.contains("os error")
    }
}

/// Get a cached connection for (id, database), creating one if not present.
async fn acquire(
    state: &State<'_, AppState>,
    id: i64,
    form: &ConnectionForm,
    database: Option<&str>,
) -> Result<RedisConnection, String> {
    let is_cluster = form
        .host
        .as_deref()
        .map(|h| h.split(',').filter(|p| !p.trim().is_empty()).count() > 1)
        .unwrap_or(false);
    let key = cache_key(id, database, is_cluster);

    // Fast path: return a clone of the cached connection
    {
        let cache = state.redis_cache.lock().await;
        if let Some(conn) = cache.get(&key) {
            return Ok(conn);
        }
    }

    // Slow path: create a new connection and cache it
    let conn = connect(form, database).await?;
    {
        let mut cache = state.redis_cache.lock().await;
        // Another task might have raced in; prefer the one already in the cache
        if let Some(existing) = cache.get(&key) {
            return Ok(existing);
        }
        cache.insert(key, conn.clone());
    }
    Ok(conn)
}

/// Remove a stale connection from the cache (called after an IO error).
async fn evict(
    state: &State<'_, AppState>,
    id: i64,
    form: &ConnectionForm,
    database: Option<&str>,
) {
    let is_cluster = form
        .host
        .as_deref()
        .map(|h| h.split(',').filter(|p| !p.trim().is_empty()).count() > 1)
        .unwrap_or(false);
    let key = cache_key(id, database, is_cluster);
    let mut cache = state.redis_cache.lock().await;
    cache.remove(&key);
}

#[tauri::command]
pub async fn redis_list_databases(
    state: State<'_, AppState>,
    id: i64,
) -> Result<Vec<RedisDatabaseInfo>, String> {
    let form = connection_form(&state, id).await?;
    let mut conn = acquire(&state, id, &form, None).await?;
    if let Err(e) = redis::ping(&mut conn).await {
        if is_io_error(&e) {
            evict(&state, id, &form, None).await;
        }
        return Err(e);
    }

    let db_count = if conn.is_cluster() {
        1
    } else {
        let mut cmd = ::redis::cmd("CONFIG");
        cmd.arg("GET").arg("databases");
        match conn.query::<Vec<String>>(cmd).await {
            Ok(values) if values.len() >= 2 => values[1]
                .parse::<i64>()
                .unwrap_or(16)
                .clamp(1, 256),
            _ => 16,
        }
    };

    redis::list_databases(&form, db_count)
}

#[tauri::command]
pub async fn redis_scan_keys(
    state: State<'_, AppState>,
    id: i64,
    database: Option<String>,
    cursor: Option<String>,
    pattern: Option<String>,
    limit: Option<u32>,
) -> Result<RedisScanResponse, String> {
    let form = connection_form(&state, id).await?;
    let db = database.as_deref();
    let mut conn = acquire(&state, id, &form, db).await?;
    match redis::scan_keys(&mut conn, cursor.clone(), pattern.clone(), limit).await {
        Err(ref e) if is_io_error(e) => {
            evict(&state, id, &form, db).await;
            let mut conn = acquire(&state, id, &form, db).await?;
            redis::scan_keys(&mut conn, cursor, pattern, limit).await
        }
        r => r,
    }
}

#[tauri::command]
pub async fn redis_get_key(
    state: State<'_, AppState>,
    id: i64,
    database: Option<String>,
    key: String,
) -> Result<RedisKeyValue, String> {
    let form = connection_form(&state, id).await?;
    let db = database.as_deref();
    let mut conn = acquire(&state, id, &form, db).await?;
    match redis::get_key(&mut conn, key.clone()).await {
        Err(ref e) if is_io_error(e) => {
            evict(&state, id, &form, db).await;
            let mut conn = acquire(&state, id, &form, db).await?;
            redis::get_key(&mut conn, key).await
        }
        r => r,
    }
}

#[tauri::command]
pub async fn redis_set_key(
    state: State<'_, AppState>,
    id: i64,
    database: Option<String>,
    payload: RedisSetKeyPayload,
) -> Result<RedisMutationResult, String> {
    let form = connection_form(&state, id).await?;
    let db = database.as_deref();
    let mut conn = acquire(&state, id, &form, db).await?;
    match redis::set_key(&mut conn, payload.clone()).await {
        Err(ref e) if is_io_error(e) => {
            evict(&state, id, &form, db).await;
            let mut conn = acquire(&state, id, &form, db).await?;
            redis::set_key(&mut conn, payload).await
        }
        r => r,
    }
}

#[tauri::command]
pub async fn redis_update_key(
    state: State<'_, AppState>,
    id: i64,
    database: Option<String>,
    payload: RedisSetKeyPayload,
) -> Result<RedisMutationResult, String> {
    let form = connection_form(&state, id).await?;
    let db = database.as_deref();
    let mut conn = acquire(&state, id, &form, db).await?;
    match redis::set_key(&mut conn, payload.clone()).await {
        Err(ref e) if is_io_error(e) => {
            evict(&state, id, &form, db).await;
            let mut conn = acquire(&state, id, &form, db).await?;
            redis::set_key(&mut conn, payload).await
        }
        r => r,
    }
}

#[tauri::command]
pub async fn redis_delete_key(
    state: State<'_, AppState>,
    id: i64,
    database: Option<String>,
    key: String,
) -> Result<RedisMutationResult, String> {
    let form = connection_form(&state, id).await?;
    let db = database.as_deref();
    let mut conn = acquire(&state, id, &form, db).await?;
    match redis::delete_key(&mut conn, key.clone()).await {
        Err(ref e) if is_io_error(e) => {
            evict(&state, id, &form, db).await;
            let mut conn = acquire(&state, id, &form, db).await?;
            redis::delete_key(&mut conn, key).await
        }
        r => r,
    }
}

#[tauri::command]
pub async fn redis_patch_key(
    state: State<'_, AppState>,
    id: i64,
    database: Option<String>,
    payload: RedisKeyPatchPayload,
) -> Result<RedisMutationResult, String> {
    let form = connection_form(&state, id).await?;
    let db = database.as_deref();
    let mut conn = acquire(&state, id, &form, db).await?;
    match redis::patch_key(&mut conn, payload.clone()).await {
        Err(ref e) if is_io_error(e) => {
            evict(&state, id, &form, db).await;
            let mut conn = acquire(&state, id, &form, db).await?;
            redis::patch_key(&mut conn, payload).await
        }
        r => r,
    }
}

#[tauri::command]
pub async fn redis_rename_key(
    state: State<'_, AppState>,
    id: i64,
    database: Option<String>,
    old_key: String,
    new_key: String,
    force: Option<bool>,
) -> Result<RedisMutationResult, String> {
    let form = connection_form(&state, id).await?;
    let db = database.as_deref();
    let force = force.unwrap_or(false);
    let mut conn = acquire(&state, id, &form, db).await?;
    match redis::rename_key(&mut conn, old_key.clone(), new_key.clone(), force).await {
        Err(ref e) if is_io_error(e) => {
            evict(&state, id, &form, db).await;
            let mut conn = acquire(&state, id, &form, db).await?;
            redis::rename_key(&mut conn, old_key, new_key, force).await
        }
        r => r,
    }
}

#[tauri::command]
pub async fn redis_get_key_page(
    state: State<'_, AppState>,
    id: i64,
    database: Option<String>,
    key: String,
    offset: u64,
    limit: u32,
) -> Result<RedisKeyValue, String> {
    let form = connection_form(&state, id).await?;
    let db = database.as_deref();
    let mut conn = acquire(&state, id, &form, db).await?;
    match redis::get_key_page(&mut conn, key.clone(), offset, limit).await {
        Err(ref e) if is_io_error(e) => {
            evict(&state, id, &form, db).await;
            let mut conn = acquire(&state, id, &form, db).await?;
            redis::get_key_page(&mut conn, key, offset, limit).await
        }
        r => r,
    }
}

#[tauri::command]
pub async fn redis_set_ttl(
    state: State<'_, AppState>,
    id: i64,
    database: Option<String>,
    key: String,
    ttl_seconds: Option<i64>,
) -> Result<RedisMutationResult, String> {
    let form = connection_form(&state, id).await?;
    let db = database.as_deref();
    let mut conn = acquire(&state, id, &form, db).await?;
    match redis::set_ttl(&mut conn, key.clone(), ttl_seconds).await {
        Err(ref e) if is_io_error(e) => {
            evict(&state, id, &form, db).await;
            let mut conn = acquire(&state, id, &form, db).await?;
            redis::set_ttl(&mut conn, key, ttl_seconds).await
        }
        r => r,
    }
}

#[tauri::command]
pub async fn redis_execute_raw(
    state: State<'_, AppState>,
    id: i64,
    database: Option<String>,
    command: String,
) -> Result<RedisRawResult, String> {
    let form = connection_form(&state, id).await?;
    let db = database.as_deref();
    let mut conn = acquire(&state, id, &form, db).await?;
    match redis::execute_raw(&mut conn, command.clone()).await {
        Err(ref e) if is_io_error(e) => {
            evict(&state, id, &form, db).await;
            let mut conn = acquire(&state, id, &form, db).await?;
            redis::execute_raw(&mut conn, command).await
        }
        r => r,
    }
}
