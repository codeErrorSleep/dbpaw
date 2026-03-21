use crate::models::ConnectionForm;
use ssh2::Session;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

pub struct SshTunnel {
    pub local_port: u16,
    _guard: Arc<TunnelGuard>,
}

struct TunnelGuard {
    running: AtomicBool,
    local_port: u16,
}

impl Drop for TunnelGuard {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        // Connect to unblock accept
        let _ = TcpStream::connect(format!("127.0.0.1:{}", self.local_port));
    }
}

pub fn start_ssh_tunnel(config: &ConnectionForm) -> Result<SshTunnel, String> {
    // Validate config
    let ssh_host = config.ssh_host.clone().ok_or("SSH Host is required")?;
    let ssh_port = config.ssh_port.unwrap_or(22);
    if ssh_port < 1 || ssh_port > 65535 {
        return Err("SSH port must be between 1 and 65535".to_string());
    }
    let ssh_port = ssh_port as u16;

    let ssh_user = config
        .ssh_username
        .clone()
        .ok_or("SSH Username is required")?;
    let ssh_password = config.ssh_password.clone();
    let ssh_key_path = config
        .ssh_key_path
        .clone()
        .and_then(|v| if v.trim().is_empty() { None } else { Some(v) });

    let target_host = config.host.clone().unwrap_or("localhost".to_string());
    let target_port = config.port.unwrap_or(5432);
    if target_port < 1 || target_port > 65535 {
        return Err("Target port must be between 1 and 65535".to_string());
    }
    let target_port = target_port as u16;

    // Bind local listener
    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|e| format!("Failed to bind local port: {}", e))?;
    let local_port = listener.local_addr().unwrap().port();

    let guard = Arc::new(TunnelGuard {
        running: AtomicBool::new(true),
        local_port,
    });
    let guard_clone = guard.clone();

    // Spawn acceptor thread
    thread::spawn(move || {
        for stream in listener.incoming() {
            if !guard_clone.running.load(Ordering::Relaxed) {
                break;
            }

            if let Ok(local_stream) = stream {
                let ssh_host = ssh_host.clone();
                let ssh_user = ssh_user.clone();
                let ssh_password = ssh_password.clone();
                let ssh_key_path = ssh_key_path.clone();
                let target_host = target_host.clone();

                // Spawn handler thread per connection
                thread::spawn(move || {
                    if let Err(e) = handle_connection(
                        local_stream,
                        &ssh_host,
                        ssh_port,
                        &ssh_user,
                        ssh_password.as_deref(),
                        ssh_key_path.as_deref(),
                        &target_host,
                        target_port,
                    ) {
                        eprintln!("SSH Tunnel Error: {}", e);
                    }
                });
            }
        }
    });

    Ok(SshTunnel {
        local_port,
        _guard: guard,
    })
}

fn handle_connection(
    mut local_stream: TcpStream,
    ssh_host: &str,
    ssh_port: u16,
    ssh_user: &str,
    ssh_password: Option<&str>,
    ssh_key_path: Option<&str>,
    target_host: &str,
    target_port: u16,
) -> Result<(), String> {
    // 1. Connect to SSH server
    let tcp = TcpStream::connect(format!("{}:{}", ssh_host, ssh_port))
        .map_err(|e| format!("Failed to connect to SSH server: {}", e))?;

    let mut sess = Session::new().map_err(|e| format!("Failed to create SSH session: {}", e))?;
    sess.set_tcp_stream(tcp);
    sess.handshake()
        .map_err(|e| format!("SSH handshake failed: {}", e))?;

    // 2. Authenticate
    if let Some(key_path) = ssh_key_path {
        sess.userauth_pubkey_file(ssh_user, None, std::path::Path::new(key_path), None)
            .map_err(|e| format!("SSH key auth failed: {}", e))?;
    } else if let Some(password) = ssh_password {
        sess.userauth_password(ssh_user, password)
            .map_err(|e| format!("SSH password auth failed: {}", e))?;
    } else {
        return Err("SSH authentication requires password or key".to_string());
    }

    // 3. Open Channel
    let mut channel = sess
        .channel_direct_tcpip(target_host, target_port, None)
        .map_err(|e| format!("Failed to create SSH channel: {}", e))?;

    // 4. Bidirectional Copy
    // We need non-blocking I/O or two threads.
    // Since we are already in a spawned thread, we can spawn another one for the read-loop,
    // and use the current one for write-loop.
    // BUT we can't move `channel` or `sess` to another thread easily because of lifetime.
    // So we use non-blocking mode with polling.

    local_stream
        .set_nonblocking(true)
        .map_err(|e| format!("Failed to set non-blocking: {}", e))?;
    // ssh2 channel is blocking by default. set_blocking(false) makes read/write return WouldBlock.
    sess.set_blocking(false);

    let mut buf_local = [0u8; 8192];
    let mut buf_remote = [0u8; 8192];

    // We need to keep track of closed ends
    let mut local_closed = false;
    let mut remote_closed = false;

    loop {
        if local_closed && remote_closed {
            break;
        }

        let mut activity = false;

        // Local -> Remote
        if !local_closed {
            match local_stream.read(&mut buf_local) {
                Ok(0) => {
                    local_closed = true;
                    let _ = channel.send_eof();
                    activity = true;
                }
                Ok(n) => {
                    // Write to channel
                    // Handle partial writes? ssh2 write_all handles it?
                    // write_all in ssh2 might block if we don't handle WouldBlock.
                    // channel.write returns bytes written.
                    let mut written = 0;
                    while written < n {
                        match channel.write(&buf_local[written..n]) {
                            Ok(w) => written += w,
                            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                                // Busy wait? No, sleep a bit
                                thread::sleep(std::time::Duration::from_millis(1));
                            }
                            Err(_) => {
                                local_closed = true;
                                break;
                            }
                        }
                    }
                    activity = true;
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(_) => {
                    local_closed = true;
                    activity = true;
                }
            }
        }

        // Remote -> Local
        if !remote_closed {
            match channel.read(&mut buf_remote) {
                Ok(0) => {
                    remote_closed = true;
                    activity = true;
                }
                Ok(n) => {
                    // Write to local
                    let mut written = 0;
                    while written < n {
                        match local_stream.write(&buf_remote[written..n]) {
                            Ok(w) => written += w,
                            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                                thread::sleep(std::time::Duration::from_millis(1));
                            }
                            Err(_) => {
                                remote_closed = true;
                                break;
                            }
                        }
                    }
                    activity = true;
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(_) => {
                    remote_closed = true;
                    activity = true;
                }
            }
        }

        if !activity {
            thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ConnectionForm;

    #[test]
    fn test_ssh_port_validation() {
        let mut config = ConnectionForm::default();
        config.ssh_host = Some("example.com".to_string());
        config.ssh_username = Some("user".to_string());

        // Test negative port
        config.ssh_port = Some(-1);
        let result = start_ssh_tunnel(&config);
        assert!(result.is_err());
        assert_eq!(
            result.err().unwrap(),
            "SSH port must be between 1 and 65535"
        );

        // Test out of range port
        config.ssh_port = Some(70000);
        let result = start_ssh_tunnel(&config);
        assert!(result.is_err());
        assert_eq!(
            result.err().unwrap(),
            "SSH port must be between 1 and 65535"
        );
    }

    #[test]
    fn test_target_port_validation() {
        let mut config = ConnectionForm::default();
        config.ssh_host = Some("example.com".to_string());
        config.ssh_username = Some("user".to_string());
        config.ssh_port = Some(22);

        // Test negative port
        config.port = Some(-1);
        let result = start_ssh_tunnel(&config);
        assert!(result.is_err());
        assert_eq!(
            result.err().unwrap(),
            "Target port must be between 1 and 65535"
        );

        // Test out of range
        config.port = Some(70000);
        let result = start_ssh_tunnel(&config);
        assert!(result.is_err());
        assert_eq!(
            result.err().unwrap(),
            "Target port must be between 1 and 65535"
        );
    }
}
