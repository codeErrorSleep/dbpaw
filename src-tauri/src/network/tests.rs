use super::*;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

fn config(port: u16, kind: ProxyKind) -> NetworkProxy {
    NetworkProxy {
        enabled: true,
        kind,
        host: "127.0.0.1".into(),
        port: port as i64,
        username: String::new(),
        password: String::new(),
    }
}

async fn read_header(stream: &mut TcpStream) -> String {
    let mut bytes = Vec::new();
    while !bytes.ends_with(b"\r\n\r\n") {
        bytes.push(stream.read_u8().await.unwrap());
    }
    String::from_utf8(bytes).unwrap()
}

#[tokio::test]
async fn http_proxy_preserves_server_greeting_half_close_and_releases_tunnel() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let mut proxy = config(listener.local_addr().unwrap().port(), ProxyKind::Http);
    proxy.username = "user".into();
    proxy.password = "pass".into();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let header = read_header(&mut stream).await;
        assert!(header.starts_with("CONNECT db.unresolvable.invalid:5432 HTTP/1.1\r\n"));
        assert!(header.contains("Proxy-Authorization: Basic dXNlcjpwYXNz\r\n"));
        stream
            .write_all(b"HTTP/1.1 200 Connection established\r\n\r\nGREETING")
            .await
            .unwrap();
        let mut payload = Vec::new();
        stream.read_to_end(&mut payload).await.unwrap();
        assert_eq!(payload, b"query");
        stream.write_all(b"result").await.unwrap();
        stream.shutdown().await.unwrap();
    });
    let form = ConnectionForm {
        driver: "postgres".into(),
        host: Some("db.unresolvable.invalid".into()),
        proxy: Some(proxy),
        ..Default::default()
    };
    let tunnel = start_network_tunnel(&form).await.unwrap();
    let mut local = TcpStream::connect(("127.0.0.1", tunnel.local_port))
        .await
        .unwrap();
    let mut greeting = [0; 8];
    local.read_exact(&mut greeting).await.unwrap();
    assert_eq!(&greeting, b"GREETING");
    local.write_all(b"query").await.unwrap();
    local.shutdown().await.unwrap();
    let mut result = Vec::new();
    tokio::time::timeout(Duration::from_secs(2), local.read_to_end(&mut result))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(result, b"result");
    server.await.unwrap();
    let port = tunnel.local_port;
    drop(tunnel);
    tokio::task::yield_now().await;
    assert!(TcpStream::connect(("127.0.0.1", port)).await.is_err());
}

#[tokio::test]
async fn socks5_auth_remote_dns_and_multiple_connections() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let mut proxy = config(listener.local_addr().unwrap().port(), ProxyKind::Socks5);
    proxy.username = "user".into();
    proxy.password = " pass ".into();
    let server = tokio::spawn(async move {
        for _ in 0..2 {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut greeting = [0; 3];
            stream.read_exact(&mut greeting).await.unwrap();
            assert_eq!(greeting, [5, 1, 2]);
            stream.write_all(&[5, 2]).await.unwrap();
            let mut auth = [0; 13];
            stream.read_exact(&mut auth).await.unwrap();
            assert_eq!(&auth, b"\x01\x04user\x06 pass ");
            stream.write_all(&[1, 0]).await.unwrap();
            let mut request = [0; 5];
            stream.read_exact(&mut request).await.unwrap();
            assert_eq!(&request[..4], &[5, 1, 0, 3]);
            let mut host = vec![0; request[4] as usize];
            stream.read_exact(&mut host).await.unwrap();
            assert_eq!(host, b"db.unresolvable.invalid");
            assert_eq!(stream.read_u16().await.unwrap(), 3306);
            stream
                .write_all(&[5, 0, 0, 1, 0, 0, 0, 0, 0, 0])
                .await
                .unwrap();
            let mut query = [0; 4];
            stream.read_exact(&mut query).await.unwrap();
            assert_eq!(&query, b"PING");
            stream.write_all(b"PONG").await.unwrap();
        }
    });
    let form = ConnectionForm {
        driver: "mysql".into(),
        host: Some("db.unresolvable.invalid".into()),
        proxy: Some(proxy),
        ..Default::default()
    };
    let tunnel = start_network_tunnel(&form).await.unwrap();
    for _ in 0..2 {
        let mut stream = TcpStream::connect(("127.0.0.1", tunnel.local_port))
            .await
            .unwrap();
        stream.write_all(b"PING").await.unwrap();
        let mut reply = [0; 4];
        stream.read_exact(&mut reply).await.unwrap();
        assert_eq!(&reply, b"PONG");
    }
    server.await.unwrap();
}

#[tokio::test]
async fn proxy_auth_failure_is_returned_before_database_connect() {
    for kind in [ProxyKind::Http, ProxyKind::Socks5] {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let proxy = config(listener.local_addr().unwrap().port(), kind);
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            if kind == ProxyKind::Http {
                read_header(&mut stream).await;
                stream
                    .write_all(b"HTTP/1.1 407 Authentication required\r\n\r\n")
                    .await
                    .unwrap();
            } else {
                let mut greeting = [0; 3];
                stream.read_exact(&mut greeting).await.unwrap();
                stream.write_all(&[5, 255]).await.unwrap();
            }
        });
        let form = ConnectionForm {
            driver: "mysql".into(),
            host: Some("db.invalid".into()),
            proxy: Some(proxy),
            ..Default::default()
        };
        let error = start_network_tunnel(&form).await.err().unwrap();
        assert_eq!(error.code(), crate::error::codes::CONN_AUTH_FAILED);
        server.await.unwrap();
    }
}

#[test]
fn proxy_validation_and_secret_reuse() {
    let mut form = ConnectionForm {
        driver: "mysql".into(),
        proxy: Some(config(1080, ProxyKind::Socks5)),
        ..Default::default()
    };
    assert!(validate_proxy(&form).is_ok());
    form.ssh_enabled = Some(true);
    assert!(validate_proxy(&form).is_err());
    form.ssh_enabled = None;
    form.proxy.as_mut().unwrap().port = 65536;
    assert!(validate_proxy(&form).is_err());
    form.proxy.as_mut().unwrap().port = 1080;
    form.proxy.as_mut().unwrap().username = "user".into();
    form.proxy.as_mut().unwrap().password = "proxy-private-test-value".into();
    assert!(!format!("{form:?}").contains("proxy-private-test-value"));
    let saved = form.clone();
    form.proxy.as_mut().unwrap().password.clear();
    merge_proxy_password(&mut form, &saved);
    assert_eq!(
        form.proxy.as_ref().unwrap().password,
        "proxy-private-test-value"
    );
    form.proxy.as_mut().unwrap().password.clear();
    form.proxy.as_mut().unwrap().host = "different".into();
    merge_proxy_password(&mut form, &saved);
    assert!(form.proxy.as_ref().unwrap().password.is_empty());
    form.driver = "redis".into();
    form.mode = Some("cluster".into());
    assert!(validate_proxy(&form).is_err());
    form.driver = "mongodb".into();
    form.ssl = Some(true);
    assert!(validate_proxy(&form).is_err());
    form.proxy.as_mut().unwrap().enabled = false;
    assert!(validate_proxy(&form).is_ok());
}
