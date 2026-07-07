use super::*;

pub(super) fn build_summary(content: &str) -> FrpcInstanceSummary {
    let proxy = first_proxy_block(content);
    FrpcInstanceSummary {
        server_addr: extract_toml_value(content, "serverAddr")
            .or_else(|| extract_toml_value(content, "server_addr"))
            .unwrap_or_default(),
        server_port: extract_toml_value(content, "serverPort")
            .or_else(|| extract_toml_value(content, "server_port"))
            .unwrap_or_else(|| "7000".to_string()),
        local_port: extract_toml_value(&proxy, "localPort")
            .or_else(|| extract_toml_value(&proxy, "local_port"))
            .unwrap_or_default(),
        remote_port: extract_toml_value(&proxy, "remotePort")
            .or_else(|| extract_toml_value(&proxy, "remote_port"))
            .unwrap_or_default(),
    }
}

pub(super) fn first_proxy_block(content: &str) -> String {
    let mut in_block = false;
    let mut lines = Vec::new();
    for line in content.lines() {
        if line.trim() == "[[proxies]]" {
            in_block = true;
            continue;
        }
        if in_block && line.trim_start().starts_with("[[") {
            break;
        }
        if in_block {
            lines.push(line);
        }
    }
    lines.join("\n")
}

pub(super) fn extract_toml_value(content: &str, key: &str) -> Option<String> {
    for line in content.lines() {
        let line = line.trim();
        let Some((left, right)) = line.split_once('=') else {
            continue;
        };
        if left.trim() != key {
            continue;
        }
        let value = right.trim();
        if (value.starts_with('"') && value.ends_with('"'))
            || (value.starts_with('\'') && value.ends_with('\''))
        {
            return Some(value[1..value.len().saturating_sub(1)].to_string());
        }
        if !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()) {
            return Some(value.to_string());
        }
        return None;
    }
    None
}
