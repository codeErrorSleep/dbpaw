use crate::models::ConnectionForm;

fn trim_to_option(value: Option<String>) -> Option<String> {
    value
        .map(|v| v.trim().to_string())
        .and_then(|v| if v.is_empty() { None } else { Some(v) })
}

fn trim_preserve_empty(value: Option<String>) -> Option<String> {
    value.map(|v| v.trim().to_string())
}

fn parse_host_embedded_port(host: &str, fallback_port: Option<i64>) -> (String, Option<i64>) {
    if host.starts_with('[') || host.contains(' ') || host.matches(':').count() != 1 {
        return (host.to_string(), fallback_port);
    }
    let Some((host_part, port_part)) = host.rsplit_once(':') else {
        return (host.to_string(), fallback_port);
    };
    if host_part.is_empty() || !port_part.chars().all(|c| c.is_ascii_digit()) {
        return (host.to_string(), fallback_port);
    }
    let parsed_port = port_part.parse::<i64>().ok();
    (host_part.to_string(), parsed_port)
}

fn validate_port_range(field: &str, port: Option<i64>) -> Result<(), String> {
    if let Some(v) = port {
        if !(1..=65535).contains(&v) {
            return Err(format!(
                "[VALIDATION_ERROR] {} must be between 1 and 65535",
                field
            ));
        }
    }
    Ok(())
}

pub fn normalize_connection_form(mut form: ConnectionForm) -> Result<ConnectionForm, String> {
    form.name = trim_to_option(form.name);
    form.host = trim_to_option(form.host);
    form.database = trim_to_option(form.database);
    form.schema = trim_to_option(form.schema);
    form.username = trim_to_option(form.username);
    form.password = trim_preserve_empty(form.password);
    form.ssl_ca_cert = trim_preserve_empty(form.ssl_ca_cert);
    form.file_path = trim_to_option(form.file_path);
    form.ssh_host = trim_to_option(form.ssh_host);
    form.ssh_username = trim_to_option(form.ssh_username);
    form.ssh_password = trim_preserve_empty(form.ssh_password);
    form.ssh_key_path = trim_to_option(form.ssh_key_path);

    validate_port_range("port", form.port)?;
    validate_port_range("ssh port", form.ssh_port)?;

    let driver = form.driver.to_ascii_lowercase();
    if crate::db::drivers::is_mysql_family_driver(&driver) {
        if let Some(host) = form.host.clone() {
            let (normalized_host, normalized_port) = parse_host_embedded_port(&host, form.port);
            form.host = Some(normalized_host);
            form.port = normalized_port.or(form.port);
        }
    }

    if matches!(driver.as_str(), "sqlite" | "duckdb") {
        if form.file_path.is_none() {
            return Err("[VALIDATION_ERROR] file path cannot be empty".to_string());
        }
    } else if form.host.is_none() {
        return Err("[VALIDATION_ERROR] host cannot be empty".to_string());
    }

    if form.ssh_enabled.unwrap_or(false) {
        if form.ssh_host.is_none() {
            return Err("[VALIDATION_ERROR] ssh host cannot be empty".to_string());
        }
        if form.ssh_username.is_none() {
            return Err("[VALIDATION_ERROR] ssh username cannot be empty".to_string());
        }
        if form.ssh_port.is_none() {
            form.ssh_port = Some(22);
        }
        if form.ssh_password.is_none() && form.ssh_key_path.is_none() {
            return Err("[VALIDATION_ERROR] ssh password or ssh key path is required".to_string());
        }
    }

    Ok(form)
}

#[cfg(test)]
mod tests {
    use super::normalize_connection_form;
    use crate::models::ConnectionForm;

    #[test]
    fn normalize_trims_fields_and_parses_mysql_host_port() {
        let form = ConnectionForm {
            driver: "starrocks".to_string(),
            host: Some(" 127.0.0.1:3307 ".to_string()),
            port: None,
            username: Some(" root ".to_string()),
            password: Some(" pass ".to_string()),
            ..Default::default()
        };
        let normalized = normalize_connection_form(form).unwrap();
        assert_eq!(normalized.host, Some("127.0.0.1".to_string()));
        assert_eq!(normalized.port, Some(3307));
        assert_eq!(normalized.username, Some("root".to_string()));
    }

    #[test]
    fn normalize_prefers_embedded_starrocks_port_over_existing_port() {
        let form = ConnectionForm {
            driver: "starrocks".to_string(),
            host: Some("127.0.0.1:9031".to_string()),
            port: Some(9030),
            username: Some("root".to_string()),
            ..Default::default()
        };

        let normalized = normalize_connection_form(form).unwrap();
        assert_eq!(normalized.host, Some("127.0.0.1".to_string()));
        assert_eq!(normalized.port, Some(9031));
    }

    #[test]
    fn normalize_prefers_embedded_mysql_port_over_existing_port() {
        let form = ConnectionForm {
            driver: "mysql".to_string(),
            host: Some("127.0.0.1:3307".to_string()),
            port: Some(3306),
            username: Some("root".to_string()),
            ..Default::default()
        };

        let normalized = normalize_connection_form(form).unwrap();
        assert_eq!(normalized.host, Some("127.0.0.1".to_string()));
        assert_eq!(normalized.port, Some(3307));
    }

    #[test]
    fn normalize_preserves_empty_secret_fields_when_present() {
        let form = ConnectionForm {
            driver: "mysql".to_string(),
            host: Some(" localhost ".to_string()),
            username: Some(" root ".to_string()),
            password: Some("   ".to_string()),
            ssl_ca_cert: Some("   ".to_string()),
            ssh_password: Some("   ".to_string()),
            ..Default::default()
        };

        let normalized = normalize_connection_form(form).unwrap();
        assert_eq!(normalized.password, Some(String::new()));
        assert_eq!(normalized.ssl_ca_cert, Some(String::new()));
        assert_eq!(normalized.ssh_password, Some(String::new()));
    }

    #[test]
    fn normalize_rejects_out_of_range_ports() {
        let form = ConnectionForm {
            driver: "postgres".to_string(),
            host: Some("localhost".to_string()),
            port: Some(70000),
            username: Some("postgres".to_string()),
            ..Default::default()
        };
        assert!(normalize_connection_form(form).is_err());
    }
}
