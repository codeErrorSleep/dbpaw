use super::*;
use crate::models::{NetworkProxy, ProxyKind};
use sqlx::sqlite::SqlitePoolOptions;

#[tokio::test]
async fn proxy_config_round_trip_redacts_and_preserves_credentials_on_edit() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    crate::db::migrations::run_migrations(&pool).await.unwrap();
    let db = LocalDb {
        pool,
        ai_master_key: [0; 32],
    };
    let form = ConnectionForm {
        driver: "mysql".into(),
        name: Some("proxied".into()),
        host: Some("db.invalid".into()),
        proxy: Some(NetworkProxy {
            enabled: true,
            kind: ProxyKind::Socks5,
            host: "127.0.0.1".into(),
            port: 1080,
            username: "user".into(),
            password: " pass ".into(),
        }),
        ..Default::default()
    };
    let created = db.create_connection(form).await.unwrap();
    assert!(created.proxy.as_ref().unwrap().enabled);
    assert!(created.proxy.as_ref().unwrap().password.is_empty());
    assert!(db.list_connections().await.unwrap()[0]
        .proxy
        .as_ref()
        .unwrap()
        .password
        .is_empty());
    let mut loaded = db.get_connection_form_by_id(created.id).await.unwrap();
    assert_eq!(loaded.ssh_port, None, "an unset SSH port must stay absent");
    assert_eq!(loaded.proxy.as_ref().unwrap().password, " pass ");
    loaded.proxy.as_mut().unwrap().password.clear();
    loaded.proxy.as_mut().unwrap().enabled = false;
    db.update_connection(created.id, loaded).await.unwrap();
    let mut loaded = db.get_connection_form_by_id(created.id).await.unwrap();
    assert!(!loaded.proxy.as_ref().unwrap().enabled);
    assert_eq!(loaded.proxy.as_ref().unwrap().password, " pass ");
    loaded.proxy.as_mut().unwrap().host = "another-proxy".into();
    loaded.proxy.as_mut().unwrap().password.clear();
    db.update_connection(created.id, loaded).await.unwrap();
    assert!(db
        .get_connection_form_by_id(created.id)
        .await
        .unwrap()
        .proxy
        .unwrap()
        .password
        .is_empty());
}
