use super::*;

pub(super) struct CaPaths {
    pub(super) dir: PathBuf,
    pub(super) cert: PathBuf,
    pub(super) key: PathBuf,
}

pub(super) fn ca_paths(state: &AppState) -> CaPaths {
    let dir = state.settings.data_dir.join("ssl");
    CaPaths {
        cert: dir.join(CA_CERT_FILENAME),
        key: dir.join(CA_KEY_FILENAME),
        dir,
    }
}

pub(super) fn init_root_ca(state: &AppState) -> anyhow::Result<Value> {
    let paths = ca_paths(state);
    std::fs::create_dir_all(&paths.dir)?;
    let subject = "/CN=KCI-LNK Root Certificate Authority/O=KCI-LNK Corporation/OU=Information Security Department/C=TW/ST=Taiwan/L=Taipei";
    run_openssl(vec![
        "req".to_string(),
        "-x509".to_string(),
        "-newkey".to_string(),
        "rsa:2048".to_string(),
        "-sha256".to_string(),
        "-days".to_string(),
        (20 * 365).to_string(),
        "-nodes".to_string(),
        "-keyout".to_string(),
        paths.key.to_string_lossy().to_string(),
        "-out".to_string(),
        paths.cert.to_string_lossy().to_string(),
        "-subj".to_string(),
        subject.to_string(),
        "-addext".to_string(),
        "basicConstraints=critical,CA:TRUE,pathlen:0".to_string(),
        "-addext".to_string(),
        "keyUsage=critical,keyCertSign,cRLSign,digitalSignature".to_string(),
    ])?;
    chmod_private(&paths.cert);
    chmod_private(&paths.key);
    let cert = std::fs::read_to_string(&paths.cert)?;
    parse_cert_info(&cert).ok_or_else(|| anyhow!("generated root CA certificate is invalid"))
}

pub(super) fn issue_ca_server_cert(
    state: &AppState,
    hosts: &[String],
) -> anyhow::Result<(String, String)> {
    let paths = ca_paths(state);
    if !paths.cert.exists() || !paths.key.exists() {
        anyhow::bail!("Root CA not initialized");
    }
    let clean_hosts = hosts
        .iter()
        .map(|host| host.trim().to_string())
        .filter(|host| !host.is_empty())
        .collect::<Vec<_>>();
    if clean_hosts.is_empty() {
        anyhow::bail!("No hosts configured");
    }
    let temp_dir = std::env::temp_dir().join(format!("fn-knock-ca-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir)?;
    let result = (|| {
        let key_path = temp_dir.join("server-key.pem");
        let csr_path = temp_dir.join("server.csr");
        let cert_path = temp_dir.join("server-cert.pem");
        let config_path = temp_dir.join("openssl.cnf");
        std::fs::write(&config_path, openssl_server_cert_config(&clean_hosts))?;
        run_openssl(vec![
            "genrsa".to_string(),
            "-out".to_string(),
            key_path.to_string_lossy().to_string(),
            "2048".to_string(),
        ])?;
        run_openssl(vec![
            "req".to_string(),
            "-new".to_string(),
            "-key".to_string(),
            key_path.to_string_lossy().to_string(),
            "-out".to_string(),
            csr_path.to_string_lossy().to_string(),
            "-config".to_string(),
            config_path.to_string_lossy().to_string(),
        ])?;
        run_openssl(vec![
            "x509".to_string(),
            "-req".to_string(),
            "-in".to_string(),
            csr_path.to_string_lossy().to_string(),
            "-CA".to_string(),
            paths.cert.to_string_lossy().to_string(),
            "-CAkey".to_string(),
            paths.key.to_string_lossy().to_string(),
            "-CAcreateserial".to_string(),
            "-out".to_string(),
            cert_path.to_string_lossy().to_string(),
            "-days".to_string(),
            (20 * 365).to_string(),
            "-sha256".to_string(),
            "-extensions".to_string(),
            "v3_req".to_string(),
            "-extfile".to_string(),
            config_path.to_string_lossy().to_string(),
        ])?;
        let cert = std::fs::read_to_string(cert_path)?;
        let key = std::fs::read_to_string(key_path)?;
        validate_ssl_cert(&cert, &key)?;
        Ok((cert, key))
    })();
    let _ = std::fs::remove_dir_all(temp_dir);
    result
}

pub(super) fn openssl_server_cert_config(hosts: &[String]) -> String {
    let common_name = hosts
        .first()
        .map(|host| openssl_dn_value(host))
        .unwrap_or_else(|| "KCI-LNK Root Certificate".to_string());
    let mut dns_index = 1;
    let mut ip_index = 1;
    let mut alt_names = Vec::new();
    for host in hosts {
        if host.parse::<IpAddr>().is_ok() {
            alt_names.push(format!("IP.{ip_index} = {host}"));
            ip_index += 1;
        } else {
            alt_names.push(format!("DNS.{dns_index} = {host}"));
            dns_index += 1;
        }
    }
    format!(
        "[req]\nprompt = no\ndistinguished_name = req_distinguished_name\nreq_extensions = v3_req\n\n[req_distinguished_name]\nCN = {common_name}\n\n[v3_req]\nbasicConstraints = CA:FALSE\nkeyUsage = digitalSignature, keyEncipherment\nextendedKeyUsage = serverAuth\nsubjectAltName = @alt_names\n\n[alt_names]\n{}\n",
        alt_names.join("\n")
    )
}

pub(super) fn openssl_dn_value(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\n', "")
        .replace('\r', "")
}

pub(super) fn run_openssl(args: Vec<String>) -> anyhow::Result<()> {
    run_openssl_capture(args).map(|_| ())
}

pub(super) fn run_openssl_capture(args: Vec<String>) -> anyhow::Result<String> {
    let output = Command::new("openssl")
        .args(&args)
        .stdin(Stdio::null())
        .output()?;
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).to_string());
    }
    let detail = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    )
    .lines()
    .map(str::trim)
    .filter(|line| !line.is_empty())
    .rev()
    .take(8)
    .collect::<Vec<_>>()
    .into_iter()
    .rev()
    .collect::<Vec<_>>()
    .join(" | ");
    Err(anyhow!(
        "{}",
        if detail.is_empty() {
            "openssl command failed".to_string()
        } else {
            detail
        }
    ))
}

pub(super) fn validate_ssl_cert_pair(cert: &str, key: &str) -> Result<(), SslValidationError> {
    let temp_dir = std::env::temp_dir().join(format!("fn-knock-ssl-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir)
        .map_err(|error| SslValidationError::CertKeyCheckFailed(error.to_string()))?;
    let result = (|| {
        let cert_path = temp_dir.join("cert.pem");
        let key_path = temp_dir.join("key.pem");
        std::fs::write(&cert_path, cert)
            .map_err(|error| SslValidationError::CertKeyCheckFailed(error.to_string()))?;
        std::fs::write(&key_path, key)
            .map_err(|error| SslValidationError::CertKeyCheckFailed(error.to_string()))?;

        let cert_public_key = run_openssl_capture(vec![
            "x509".to_string(),
            "-in".to_string(),
            cert_path.to_string_lossy().to_string(),
            "-noout".to_string(),
            "-pubkey".to_string(),
        ])
        .map_err(|error| SslValidationError::CertFormatInvalid(error.to_string()))?;
        let key_public_key = run_openssl_capture(vec![
            "pkey".to_string(),
            "-in".to_string(),
            key_path.to_string_lossy().to_string(),
            "-pubout".to_string(),
        ])
        .map_err(|error| SslValidationError::KeyFormatInvalid(error.to_string()))?;

        if normalize_public_key_pem(&cert_public_key) != normalize_public_key_pem(&key_public_key) {
            return Err(SslValidationError::CertKeyMismatch);
        }
        Ok(())
    })();
    let _ = std::fs::remove_dir_all(temp_dir);
    result
}

pub(super) fn normalize_public_key_pem(value: &str) -> String {
    value
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) async fn get_ca_hosts(state: &AppState) -> crate::storage::StorageResult<Vec<String>> {
    Ok(state
        .store
        .get_json_value(CA_HOSTS_KEY)
        .await?
        .and_then(|value| {
            value.as_array().map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
        })
        .unwrap_or_default())
}

pub(super) async fn save_ca_hosts(
    state: &AppState,
    hosts: &[String],
) -> crate::storage::StorageResult<()> {
    state
        .store
        .set_json_value(CA_HOSTS_KEY, &json!(hosts))
        .await
}

pub(super) async fn add_ca_host_inner(
    state: &AppState,
    host: &str,
) -> crate::storage::StorageResult<Vec<String>> {
    let mut hosts = get_ca_hosts(state).await?;
    let host = host.trim();
    if !host.is_empty() && !hosts.iter().any(|item| item == host) {
        hosts.push(host.to_string());
        save_ca_hosts(state, &hosts).await?;
    }
    Ok(hosts)
}

pub(super) async fn remove_ca_host_inner(
    state: &AppState,
    host: &str,
) -> crate::storage::StorageResult<Vec<String>> {
    let mut hosts = get_ca_hosts(state).await?;
    let before = hosts.len();
    hosts.retain(|item| item != host.trim());
    if hosts.len() != before {
        save_ca_hosts(state, &hosts).await?;
    }
    Ok(hosts)
}

#[cfg(unix)]
pub(super) fn chmod_private(path: &Path) {
    use std::os::unix::fs::PermissionsExt;

    if let Ok(metadata) = std::fs::metadata(path) {
        let mut permissions = metadata.permissions();
        permissions.set_mode(0o600);
        let _ = std::fs::set_permissions(path, permissions);
    }
}

#[cfg(not(unix))]
pub(super) fn chmod_private(_path: &Path) {}
