use super::*;

pub(super) async fn run_discover_job(
    job: DiscoverJobHandle,
    scan_cidrs: Vec<String>,
    full_range_cidrs: Vec<String>,
    self_scan_hosts: Vec<String>,
    exclude_ports: Vec<u16>,
    translator: Translator,
) {
    let scan_hosts = expand_scan_cidrs(&scan_cidrs);
    let scan_scope = build_scan_scope(&scan_cidrs);
    let groups = build_discovery_host_groups(
        &scan_cidrs,
        &full_range_cidrs,
        Some(&scan_hosts),
        &self_scan_hosts,
    );
    let total_ports = count_discovery_scan_ports_for_groups(&groups, &exclude_ports);
    let port_mode_label = build_discovery_port_mode_label(&scan_cidrs, &full_range_cidrs);
    update_discover_job(&job, |job| {
        job.state = "running".to_string();
        job.meta = Some(json!({
            "host": scan_hosts.first().cloned().unwrap_or_default(),
            "totalPortsScanned": total_ports,
            "foundServices": 0,
            "scannedHosts": scan_hosts.len(),
            "scanHostCount": scan_hosts.len(),
            "scanScope": scan_scope,
            "scanCidrs": scan_cidrs,
            "portRange": port_mode_label
        }));
        job.progress = Some(json!({
            "scannedPorts": 0,
            "totalPorts": total_ports,
            "scannedHosts": 0,
            "totalHosts": scan_hosts.len(),
            "currentHost": scan_hosts.first().cloned().unwrap_or_default(),
        }));
    });

    let client = match discovery_http_client_builder()
        .redirect(reqwest::redirect::Policy::limited(20))
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            fail_discover_job(&job, error.to_string());
            return;
        }
    };
    let manual_redirect_client = match discovery_http_client_builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            fail_discover_job(&job, error.to_string());
            return;
        }
    };

    let scanned_ports = Arc::new(AtomicUsize::new(0));
    let completed_hosts = Arc::new(AtomicUsize::new(0));
    let total_hosts = scan_hosts.len();

    for group in groups {
        let skip_ports = merge_discovery_skip_ports(&exclude_ports, &group.skip_ports);
        let ports = Arc::new(build_port_list(group.port_range, &skip_ports));
        if ports.is_empty() || group.hosts.is_empty() {
            continue;
        }
        let is_loopback_only = group.hosts.len() == 1
            && group
                .hosts
                .first()
                .is_some_and(|host| host == LOOPBACK_DISCOVERY_HOST);
        let host_concurrency = if is_loopback_only {
            1
        } else {
            NETWORK_HOST_CONCURRENCY.min(group.hosts.len()).max(1)
        };
        let max_concurrent = if is_loopback_only {
            LOOPBACK_MAX_CONCURRENT
        } else {
            NETWORK_MAX_CONCURRENT
        };

        for hosts in group.hosts.chunks(host_concurrency) {
            if job_cancelled(&job) {
                return;
            }
            let mut tasks = JoinSet::new();
            for host in hosts {
                tasks.spawn(scan_discovery_host(
                    job.clone(),
                    client.clone(),
                    manual_redirect_client.clone(),
                    host.clone(),
                    ports.clone(),
                    total_ports,
                    total_hosts,
                    scanned_ports.clone(),
                    completed_hosts.clone(),
                    max_concurrent,
                    translator.clone(),
                ));
            }
            while let Some(result) = tasks.join_next().await {
                if let Err(error) = result {
                    tracing::warn!(%error, "scan discovery host task failed");
                }
            }
        }
    }

    if job_cancelled(&job) {
        return;
    }
    complete_discover_job(
        &job,
        scan_cidrs,
        scan_hosts,
        scan_scope,
        scanned_ports.load(Ordering::SeqCst),
    );
}

pub(super) fn discovery_http_client_builder() -> reqwest::ClientBuilder {
    reqwest::Client::builder()
        .timeout(Duration::from_millis(DISCOVERY_HTTP_TIMEOUT_MS))
        .danger_accept_invalid_certs(true)
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn scan_discovery_host(
    job: DiscoverJobHandle,
    client: reqwest::Client,
    manual_redirect_client: reqwest::Client,
    host: String,
    ports: Arc<Vec<u16>>,
    total_ports: usize,
    total_hosts: usize,
    scanned_ports: Arc<AtomicUsize>,
    completed_hosts: Arc<AtomicUsize>,
    max_concurrent: usize,
    translator: Translator,
) {
    for chunk in ports.chunks(max_concurrent.max(1)) {
        if job_cancelled(&job) {
            return;
        }
        let mut tcp_tasks = JoinSet::new();
        for port in chunk.iter().copied() {
            let host = host.clone();
            tcp_tasks.spawn(async move { (port, check_tcp_port(&host, port).await) });
        }

        let mut open_ports = Vec::new();
        while let Some(result) = tcp_tasks.join_next().await {
            let Ok((port, open)) = result else {
                continue;
            };
            let scanned = scanned_ports.fetch_add(1, Ordering::SeqCst) + 1;
            update_discover_progress(
                &job,
                scanned,
                total_ports,
                completed_hosts.load(Ordering::SeqCst),
                total_hosts,
                &host,
            );
            if open {
                open_ports.push(port);
            }
        }

        if open_ports.is_empty() {
            continue;
        }
        let mut http_tasks = JoinSet::new();
        for port in open_ports {
            let client = client.clone();
            let manual_redirect_client = manual_redirect_client.clone();
            let host = host.clone();
            let translator = translator.clone();
            http_tasks.spawn(async move {
                probe_discovery_service(&client, &manual_redirect_client, &host, port, &translator)
                    .await
            });
        }
        while let Some(result) = http_tasks.join_next().await {
            if let Ok(Some(service)) = result {
                push_discovered_service(&job, service);
            }
        }
    }

    let completed = completed_hosts.fetch_add(1, Ordering::SeqCst) + 1;
    update_discover_progress(
        &job,
        scanned_ports.load(Ordering::SeqCst),
        total_ports,
        completed,
        total_hosts,
        &host,
    );
}

pub(super) async fn check_tcp_port(host: &str, port: u16) -> bool {
    timeout(
        Duration::from_millis(DISCOVERY_TIMEOUT_MS),
        TcpStream::connect((host, port)),
    )
    .await
    .is_ok_and(|result| result.is_ok())
}

pub(super) fn update_discover_progress(
    job: &DiscoverJobHandle,
    scanned_ports: usize,
    total_ports: usize,
    scanned_hosts: usize,
    total_hosts: usize,
    current_host: &str,
) {
    update_discover_job(job, |job| {
        if is_terminal_discover_state(&job.state) {
            return;
        }
        job.progress = Some(json!({
            "scannedPorts": scanned_ports,
            "totalPorts": total_ports,
            "scannedHosts": scanned_hosts,
            "totalHosts": total_hosts,
            "currentHost": current_host,
        }));
    });
}

pub(super) fn update_discover_job(job: &DiscoverJobHandle, update: impl FnOnce(&mut DiscoverJob)) {
    let mut locked = discover_job_guard(job);
    update(&mut locked);
    locked.updated_at = now_millis();
}

pub(super) fn fail_discover_job(job: &DiscoverJobHandle, message: String) {
    update_discover_job(job, |job| {
        if is_terminal_discover_state(&job.state) {
            return;
        }
        job.state = "failed".to_string();
        job.error = Some(message);
    });
}

pub(super) fn job_cancelled(job: &DiscoverJobHandle) -> bool {
    let locked = discover_job_guard(job);
    locked.cancel.load(Ordering::SeqCst) || locked.state == "cancelled"
}

pub(super) fn push_discovered_service(job: &DiscoverJobHandle, service: Value) {
    let service_key = discovered_service_key(&service);
    update_discover_job(job, |job| {
        if let Some((_, existing)) = job
            .service_map
            .iter_mut()
            .find(|(key, _)| key == &service_key)
        {
            if discovered_service_port(existing) <= discovered_service_port(&service) {
                return;
            }
            *existing = service.clone();
        } else {
            job.service_map.push((service_key, service.clone()));
        }
        job.service_events.push(service);
        if let Some(meta) = job.meta.as_mut().and_then(Value::as_object_mut) {
            meta.insert("foundServices".to_string(), json!(job.service_map.len()));
        }
    });
}

pub(super) fn discovered_service_key(service: &Value) -> String {
    if let Some(key) = service.get("serviceKey").and_then(Value::as_str)
        && !key.is_empty()
    {
        return key.to_string();
    }
    let host = service.get("host").and_then(Value::as_str).unwrap_or("");
    let port = service
        .get("port")
        .and_then(Value::as_u64)
        .map(|port| port.to_string())
        .unwrap_or_default();
    format!("{host}:{port}")
}

pub(super) fn discovered_service_port(service: &Value) -> u64 {
    service
        .get("port")
        .and_then(Value::as_u64)
        .unwrap_or(u64::MAX)
}

pub(super) fn complete_discover_job(
    job: &DiscoverJobHandle,
    scan_cidrs: Vec<String>,
    scan_hosts: Vec<String>,
    scan_scope: Option<String>,
    scanned_ports: usize,
) {
    update_discover_job(job, |job| {
        if is_terminal_discover_state(&job.state) {
            return;
        }
        let services = job
            .service_map
            .iter()
            .map(|(_, service)| service.clone())
            .collect::<Vec<_>>();
        job.progress = Some(json!({
            "scannedPorts": scanned_ports,
            "totalPorts": scanned_ports,
            "scannedHosts": scan_hosts.len(),
            "totalHosts": scan_hosts.len(),
        }));
        job.result = Some(json!({
            "host": scan_hosts.first().cloned().unwrap_or_default(),
            "totalPortsScanned": scanned_ports,
            "foundServices": services.len(),
            "scannedHosts": scan_hosts.len(),
            "scanHostCount": scan_hosts.len(),
            "scanScope": scan_scope,
            "scanCidrs": scan_cidrs,
            "services": services,
        }));
        job.state = "completed".to_string();
    });
}

pub(super) async fn probe_discovery_service(
    client: &reqwest::Client,
    manual_redirect_client: &reqwest::Client,
    host: &str,
    port: u16,
    translator: &Translator,
) -> Option<Value> {
    let target = format!("http://{host}:{port}");
    let response = match send_discovery_probe_request(client, &target).await {
        Ok(response) => response,
        Err(error) => {
            if error.is_timeout() {
                return None;
            }
            tracing::debug!(%error, %target, "discovery HTTP follow probe failed; retrying without redirects");
            send_discovery_probe_request(manual_redirect_client, &target)
                .await
                .ok()?
        }
    };
    let status = response.status().as_u16();
    let headers = collect_response_headers(response.headers());
    let body = response.text().await.unwrap_or_default();
    analyze_discovered_http_service(
        client,
        DiscoveryHttpResult {
            host: host.to_string(),
            port,
            status,
            headers,
            body,
        },
        translator,
    )
    .await
}

pub(super) async fn send_discovery_probe_request(
    client: &reqwest::Client,
    target: &str,
) -> Result<reqwest::Response, reqwest::Error> {
    client
        .get(target)
        .header(reqwest::header::USER_AGENT, DISCOVERY_HTTP_USER_AGENT)
        .header(reqwest::header::CONNECTION, "close")
        .send()
        .await
}

pub(super) fn collect_response_headers(
    headers: &reqwest::header::HeaderMap,
) -> HashMap<String, String> {
    let mut output = HashMap::new();
    for (name, value) in headers {
        let key = name.as_str().to_ascii_lowercase();
        let value = value.to_str().unwrap_or("").to_string();
        output
            .entry(key)
            .and_modify(|existing: &mut String| {
                if !existing.is_empty() && !value.is_empty() {
                    existing.push_str(", ");
                }
                existing.push_str(&value);
            })
            .or_insert(value);
    }
    output
}
