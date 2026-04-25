#[path = "common/redis_context.rs"]
mod redis_context;

use dbpaw_lib::datasources::redis;
use dbpaw_lib::datasources::redis::{RedisSetKeyPayload, RedisValue, RedisZSetMember};
use std::collections::BTreeMap;

// ── helpers ──────────────────────────────────────────────────────────────────

fn noauth() -> dbpaw_lib::models::ConnectionForm {
    redis_context::shared_redis_noauth_form()
}

fn auth() -> dbpaw_lib::models::ConnectionForm {
    redis_context::shared_redis_auth_form()
}

async fn cleanup(form: &dbpaw_lib::models::ConnectionForm, key: &str) {
    if let Ok(mut conn) = redis::connect(form, None).await {
        let _ = redis::delete_key(&mut conn, key.to_string()).await;
    }
}

// ── connection tests ──────────────────────────────────────────────────────────

#[tokio::test]
async fn noauth_ping() {
    let form = noauth();
    let mut conn = redis::connect(&form, None).await.expect("connect failed");
    let result = redis::ping(&mut conn).await;
    assert!(result.is_ok(), "no-auth ping failed: {:?}", result.err());
}

#[tokio::test]
async fn auth_ping() {
    let form = auth();
    let mut conn = redis::connect(&form, None).await.expect("connect failed");
    let result = redis::ping(&mut conn).await;
    assert!(result.is_ok(), "auth ping failed: {:?}", result.err());
}

#[tokio::test]
async fn list_databases_standalone() {
    let form = noauth();
    let dbs = redis::list_databases(&form, 16).unwrap();
    assert_eq!(dbs.len(), 16, "expected 16 databases for standalone");
    assert!(
        dbs.iter().any(|d| d.selected),
        "no database marked selected"
    );
}

// ── scan tests ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn scan_keys_cursor_works() {
    let form = noauth();
    let prefix = redis_context::unique_name("scan_test");

    let mut conn = redis::connect(&form, None).await.unwrap();
    for i in 0..150u32 {
        let payload = RedisSetKeyPayload {
            key: format!("{prefix}:{i}"),
            value: RedisValue::String(format!("v{i}")),
            ttl_seconds: Some(60),
        };
        redis::set_key(&mut conn, payload).await.unwrap();
    }

    let mut all_keys = Vec::new();
    let mut cursor = "0".to_string();
    let mut rounds = 0;
    loop {
        let resp = redis::scan_keys(
            &mut conn,
            Some(cursor.clone()),
            Some(format!("{prefix}:*")),
            Some(50),
        )
        .await
        .unwrap();
        all_keys.extend(resp.keys.iter().map(|k| k.key.clone()));
        cursor = resp.cursor.clone();
        rounds += 1;
        if cursor == "0" {
            break;
        }
        assert!(rounds < 50, "cursor loop did not terminate");
    }
    assert_eq!(all_keys.len(), 150, "expected 150 keys after full scan");

    for i in 0..150u32 {
        cleanup(&form, &format!("{prefix}:{i}")).await;
    }
}

// ── CRUD: string ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn crud_string() {
    let form = noauth();
    let key = redis_context::unique_name("crud_string");
    let mut conn = redis::connect(&form, None).await.unwrap();

    let payload = RedisSetKeyPayload {
        key: key.clone(),
        value: RedisValue::String("hello".to_string()),
        ttl_seconds: None,
    };
    redis::set_key(&mut conn, payload).await.unwrap();

    let got = redis::get_key(&mut conn, key.clone()).await.unwrap();
    assert_eq!(got.key_type, "string");
    assert!(matches!(&got.value, RedisValue::String(v) if v == "hello"));
    assert_eq!(got.value_total_len, None);

    let payload2 = RedisSetKeyPayload {
        key: key.clone(),
        value: RedisValue::String("world".to_string()),
        ttl_seconds: None,
    };
    redis::set_key(&mut conn, payload2).await.unwrap();
    let got2 = redis::get_key(&mut conn, key.clone()).await.unwrap();
    assert!(matches!(&got2.value, RedisValue::String(v) if v == "world"));

    let del = redis::delete_key(&mut conn, key.clone()).await.unwrap();
    assert_eq!(del.affected, 1);
    let gone = redis::get_key(&mut conn, key).await.unwrap();
    assert_eq!(gone.key_type, "none");
}

// ── CRUD: hash ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn crud_hash() {
    let form = noauth();
    let key = redis_context::unique_name("crud_hash");
    let mut conn = redis::connect(&form, None).await.unwrap();

    let mut fields = BTreeMap::new();
    fields.insert("f1".to_string(), "v1".to_string());
    fields.insert("f2".to_string(), "v2".to_string());
    fields.insert("f3".to_string(), "v3".to_string());

    let payload = RedisSetKeyPayload {
        key: key.clone(),
        value: RedisValue::Hash(fields),
        ttl_seconds: Some(60),
    };
    redis::set_key(&mut conn, payload).await.unwrap();

    let got = redis::get_key(&mut conn, key.clone()).await.unwrap();
    assert_eq!(got.key_type, "hash");
    assert_eq!(got.value_total_len, Some(3));
    if let RedisValue::Hash(map) = &got.value {
        assert_eq!(map.get("f1").map(String::as_str), Some("v1"));
    } else {
        panic!("expected Hash");
    }

    cleanup(&form, &key).await;
}

#[tokio::test]
async fn crud_hash_pagination_uses_scan_cursor() {
    let form = noauth();
    let key = redis_context::unique_name("crud_hash_page");
    let mut conn = redis::connect(&form, None).await.unwrap();

    let fields: BTreeMap<String, String> = (0..3000)
        .map(|i| (format!("f{i}"), format!("v{i}")))
        .collect();

    let payload = RedisSetKeyPayload {
        key: key.clone(),
        value: RedisValue::Hash(fields),
        ttl_seconds: Some(60),
    };
    redis::set_key(&mut conn, payload).await.unwrap();

    let first = redis::get_key(&mut conn, key.clone()).await.unwrap();
    assert_eq!(first.value_total_len, Some(3000));
    assert!(first.value_offset != 0, "expected non-zero HSCAN cursor");

    let second = redis::get_key_page(&mut conn, key.clone(), first.value_offset, 200)
        .await
        .unwrap();
    assert_eq!(second.value_total_len, Some(3000));
    assert!(
        matches!(&second.value, RedisValue::Hash(v) if !v.is_empty()),
        "expected a non-empty second hash page"
    );

    cleanup(&form, &key).await;
}

// ── CRUD: list ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn crud_list_pagination() {
    let form = noauth();
    let key = redis_context::unique_name("crud_list");
    let mut conn = redis::connect(&form, None).await.unwrap();

    let items: Vec<String> = (0..300).map(|i| format!("item{i}")).collect();
    let payload = RedisSetKeyPayload {
        key: key.clone(),
        value: RedisValue::List(items),
        ttl_seconds: Some(120),
    };
    redis::set_key(&mut conn, payload).await.unwrap();

    let first = redis::get_key(&mut conn, key.clone()).await.unwrap();
    assert_eq!(first.value_total_len, Some(300));
    let first_count = if let RedisValue::List(v) = &first.value {
        v.len()
    } else {
        panic!("expected list");
    };
    assert_eq!(first_count, 200);

    let page2 = redis::get_key_page(&mut conn, key.clone(), 200, 200)
        .await
        .unwrap();
    assert_eq!(page2.value_total_len, Some(300));
    let page2_count = if let RedisValue::List(v) = &page2.value {
        v.len()
    } else {
        panic!("expected list");
    };
    assert_eq!(page2_count, 100);

    cleanup(&form, &key).await;
}

// ── CRUD: set ─────────────────────────────────────────────────────────────────

#[tokio::test]
async fn crud_set_pagination() {
    let form = noauth();
    let key = redis_context::unique_name("crud_set");
    let mut conn = redis::connect(&form, None).await.unwrap();

    let members: Vec<String> = (0..250).map(|i| format!("m{i}")).collect();
    let payload = RedisSetKeyPayload {
        key: key.clone(),
        value: RedisValue::Set(members),
        ttl_seconds: Some(120),
    };
    redis::set_key(&mut conn, payload).await.unwrap();

    let first = redis::get_key(&mut conn, key.clone()).await.unwrap();
    assert_eq!(first.value_total_len, Some(250));

    let page = redis::get_key_page(&mut conn, key.clone(), 0, 100)
        .await
        .unwrap();
    assert_eq!(page.value_total_len, Some(250));
    assert!(
        matches!(&page.value, RedisValue::Set(v) if !v.is_empty()),
        "expected non-empty set page"
    );

    cleanup(&form, &key).await;
}

#[tokio::test]
async fn crud_set_initial_page_exposes_scan_cursor() {
    let form = noauth();
    let key = redis_context::unique_name("crud_set_cursor");
    let mut conn = redis::connect(&form, None).await.unwrap();

    let members: Vec<String> = (0..250).map(|i| format!("m{i}")).collect();
    let payload = RedisSetKeyPayload {
        key: key.clone(),
        value: RedisValue::Set(members),
        ttl_seconds: Some(120),
    };
    redis::set_key(&mut conn, payload).await.unwrap();

    let first = redis::get_key(&mut conn, key.clone()).await.unwrap();
    assert_eq!(first.value_total_len, Some(250));
    assert!(first.value_offset != 0, "expected non-zero SSCAN cursor");

    let page = redis::get_key_page(&mut conn, key.clone(), first.value_offset, 100)
        .await
        .unwrap();
    assert!(
        matches!(&page.value, RedisValue::Set(v) if !v.is_empty()),
        "expected non-empty set page"
    );

    cleanup(&form, &key).await;
}

// ── CRUD: zset ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn crud_zset_pagination() {
    let form = noauth();
    let key = redis_context::unique_name("crud_zset");
    let mut conn = redis::connect(&form, None).await.unwrap();

    let members: Vec<RedisZSetMember> = (0..300)
        .map(|i| RedisZSetMember {
            member: format!("z{i}"),
            score: i as f64,
        })
        .collect();
    let payload = RedisSetKeyPayload {
        key: key.clone(),
        value: RedisValue::ZSet(members),
        ttl_seconds: Some(120),
    };
    redis::set_key(&mut conn, payload).await.unwrap();

    let first = redis::get_key(&mut conn, key.clone()).await.unwrap();
    assert_eq!(first.value_total_len, Some(300));
    let first_len = if let RedisValue::ZSet(v) = &first.value {
        v.len()
    } else {
        panic!("expected zset");
    };
    assert_eq!(first_len, 200);

    let page2 = redis::get_key_page(&mut conn, key.clone(), 200, 200)
        .await
        .unwrap();
    assert_eq!(page2.value_total_len, Some(300));
    let page2_len = if let RedisValue::ZSet(v) = &page2.value {
        v.len()
    } else {
        panic!("expected zset");
    };
    assert_eq!(page2_len, 100);

    cleanup(&form, &key).await;
}

// ── rename ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn rename_key() {
    let form = noauth();
    let old = redis_context::unique_name("rename_old");
    let new = redis_context::unique_name("rename_new");
    let mut conn = redis::connect(&form, None).await.unwrap();

    let payload = RedisSetKeyPayload {
        key: old.clone(),
        value: RedisValue::String("data".to_string()),
        ttl_seconds: Some(60),
    };
    redis::set_key(&mut conn, payload).await.unwrap();

    redis::rename_key(&mut conn, old.clone(), new.clone(), false)
        .await
        .unwrap();

    let gone = redis::get_key(&mut conn, old).await.unwrap();
    assert_eq!(gone.key_type, "none", "old key should be gone");

    let present = redis::get_key(&mut conn, new.clone()).await.unwrap();
    assert_eq!(present.key_type, "string", "new key should exist");

    cleanup(&form, &new).await;
}

// ── TTL ───────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn set_ttl_and_persist() {
    let form = noauth();
    let key = redis_context::unique_name("ttl_test");
    let mut conn = redis::connect(&form, None).await.unwrap();

    let payload = RedisSetKeyPayload {
        key: key.clone(),
        value: RedisValue::String("ephemeral".to_string()),
        ttl_seconds: None,
    };
    redis::set_key(&mut conn, payload).await.unwrap();

    redis::set_ttl(&mut conn, key.clone(), Some(3600))
        .await
        .unwrap();
    let got = redis::get_key(&mut conn, key.clone()).await.unwrap();
    assert!(got.ttl > 0, "expected positive TTL after EXPIRE");

    redis::set_ttl(&mut conn, key.clone(), None)
        .await
        .unwrap();
    let got2 = redis::get_key(&mut conn, key.clone()).await.unwrap();
    assert_eq!(got2.ttl, -1, "expected -1 (no expiry) after PERSIST");

    cleanup(&form, &key).await;
}

// ── cluster ───────────────────────────────────────────────────────────────────

/// Requires IT_REUSE_LOCAL_DB=1 and REDIS_CLUSTER_HOSTS=<host1>,<host2>,...
#[tokio::test]
async fn cluster_scan_is_partial() {
    let hosts = match std::env::var("REDIS_CLUSTER_HOSTS") {
        Ok(h) if !h.is_empty() => h,
        _ => {
            eprintln!("[skip] REDIS_CLUSTER_HOSTS not set; skipping cluster test");
            return;
        }
    };
    if !redis_context::should_reuse_local_db() {
        eprintln!("[skip] IT_REUSE_LOCAL_DB=1 required for cluster test");
        return;
    }

    let form = dbpaw_lib::models::ConnectionForm {
        driver: "redis".to_string(),
        host: Some(hosts),
        ..Default::default()
    };
    let mut conn = redis::connect(&form, None).await.unwrap();
    let resp = redis::scan_keys(&mut conn, None, None, Some(10))
        .await
        .unwrap();
    assert!(resp.is_partial, "cluster scan should always set is_partial");
    assert!(!resp.cursor.is_empty(), "cluster scan cursor should not be empty");
}

#[tokio::test]
async fn cluster_scan_requires_narrow_pattern() {
    let hosts = match std::env::var("REDIS_CLUSTER_HOSTS") {
        Ok(h) if !h.is_empty() => h,
        _ => {
            eprintln!("[skip] REDIS_CLUSTER_HOSTS not set; skipping cluster test");
            return;
        }
    };
    if !redis_context::should_reuse_local_db() {
        eprintln!("[skip] IT_REUSE_LOCAL_DB=1 required for cluster test");
        return;
    }

    let form = dbpaw_lib::models::ConnectionForm {
        driver: "redis".to_string(),
        host: Some(hosts),
        ..Default::default()
    };
    let mut conn = redis::connect(&form, None).await.unwrap();
    let err = redis::scan_keys(&mut conn, None, Some("*".to_string()), Some(10))
        .await
        .unwrap_err();
    assert!(
        err.contains("requires a non-wildcard pattern"),
        "unexpected error: {err}"
    );
}
