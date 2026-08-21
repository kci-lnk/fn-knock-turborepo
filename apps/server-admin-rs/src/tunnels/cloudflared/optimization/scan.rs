use super::*;

pub(super) async fn run_scan(
    state: &AppState,
    job_id: Option<&str>,
    preferred_ip: Option<Ipv4Addr>,
) -> Result<OptimizationScanResult, CloudflareApiError> {
    let settings = load_source_settings(state).await?;
    let source_fingerprint = source_settings_fingerprint(&settings);
    let prefixes = load_cloudflare_prefixes(state).await;
    let preferred_ip = preferred_ip.filter(|ip| candidate_ip_is_cloudflare(*ip, &prefixes));
    let ownership = load_managed_state(state).await;
    let current_ip = ownership
        .pointer("/optimization/selected/ip")
        .and_then(Value::as_str)
        .and_then(|value| value.parse::<Ipv4Addr>().ok())
        .filter(|ip| candidate_ip_is_cloudflare(*ip, &prefixes));
    let (mut seeds, source_warnings, resolver_diagnostics, mut resolution_path) =
        load_candidate_seeds(&settings, &prefixes).await;
    if let Some(ip) = current_ip {
        if seeds.is_empty() {
            resolution_path = "current-candidate".to_string();
        }
        merge_priority_candidate_seed(&mut seeds, ip, "current");
    }
    if let Some(ip) = preferred_ip {
        if seeds.is_empty() {
            resolution_path = "preferred-ip".to_string();
        }
        merge_priority_candidate_seed(&mut seeds, ip, "preferred-ip");
    }
    if seeds.is_empty() {
        let mut runtime = load_runtime(state).await;
        let runtime_object = ensure_object(&mut runtime);
        runtime_object.insert(
            "lastSourceWarnings".to_string(),
            json!(source_warnings.clone()),
        );
        runtime_object.insert(
            "lastResolverDiagnostics".to_string(),
            json!(resolver_diagnostics.clone()),
        );
        runtime_object.insert(
            "lastResolutionPath".to_string(),
            json!(resolution_path.clone()),
        );
        let _ = save_runtime(state, &runtime).await;
        if let Some(job_id) = job_id {
            update_job(
                state,
                job_id,
                json!({
                    "sourceWarnings": source_warnings,
                    "resolverDiagnostics": resolver_diagnostics,
                    "resolutionPath": resolution_path,
                    "candidateSourceCount": 0,
                    "sourceFingerprint": source_fingerprint,
                }),
            )
            .await;
        }
        return Err(local_error(CANDIDATE_RESOLUTION_UNAVAILABLE_SCAN_ERROR));
    }
    let vantage = probe_local_vantage(state).await;
    let business_hostname = scan_validation_hostname(&ownership)
        .ok_or_else(|| scan_validation_hostname_error(&ownership))?;
    if let Some(job_id) = job_id {
        update_job(
            state,
            job_id,
            json!({
                "vantage": vantage,
                "sourceWarnings": source_warnings,
                "resolverDiagnostics": resolver_diagnostics,
                "resolutionPath": resolution_path,
                "candidateSourceCount": seeds.len(),
                "businessValidationHostname": business_hostname,
                "sourceFingerprint": source_fingerprint,
            }),
        )
        .await;
    }
    let total = seeds.len().max(1);
    let mut join_set = JoinSet::new();
    let mut results = Vec::new();
    let mut processed_count = 0usize;
    for chunk in seeds.chunks(PROBE_CONCURRENCY) {
        if let Some(job_id) = job_id
            && is_job_cancelled(state, job_id).await
        {
            return Ok(OptimizationScanResult {
                candidates: Vec::new(),
                vantage,
                source_warnings,
                resolver_diagnostics,
                resolution_path,
                source_fingerprint,
            });
        }
        for seed in chunk.iter().cloned() {
            join_set.spawn(async move {
                let metrics = probe_latency_metrics(seed.ip).await;
                (seed, metrics)
            });
        }
        while let Some(result) = join_set.join_next().await {
            if let Ok((seed, Some(metrics))) = result
                && metrics.loss_ratio <= 1.0 / 3.0
            {
                results.push(OptimizationCandidate {
                    ip: seed.ip.to_string(),
                    median_latency_ms: metrics.median_latency_ms,
                    jitter_ms: metrics.jitter_ms,
                    loss_ratio: metrics.loss_ratio,
                    download_mbps: 0.0,
                    score: f64::MAX,
                    verified_at: Some(time_utils::now_iso()),
                    source_types: seed.source_types,
                    source_hostnames: seed.source_hostnames,
                    colo: metrics.colo,
                    cf_ray: metrics.cf_ray,
                    business_hostname: Some(business_hostname.clone()),
                    business_status: None,
                    business_colo: None,
                    business_cf_ray: None,
                    business_validated: false,
                });
            }
        }
        processed_count += chunk.len();
        if let Some(job_id) = job_id {
            let progress = 5 + ((processed_count.min(total) * 55) / total) as i64;
            update_job(
                state,
                job_id,
                json!({ "phase": "latency", "progress": progress }),
            )
            .await;
        }
    }
    results.sort_by(|left, right| {
        left.median_latency_ms
            .partial_cmp(&right.median_latency_ms)
            .unwrap_or(Ordering::Equal)
    });
    retain_shortlist_with_priority(
        &mut results,
        &[preferred_ip, current_ip]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>(),
        DOWNLOAD_SHORTLIST,
    );
    if let Some(job_id) = job_id {
        update_job(
            state,
            job_id,
            json!({ "phase": "download", "progress": 65 }),
        )
        .await;
    }
    let download_total = results.len().max(1);
    let mut download_tasks = JoinSet::new();
    for mut candidate in results {
        let validation_hostname = business_hostname.clone();
        download_tasks.spawn(async move {
            let ip = candidate.ip.parse::<Ipv4Addr>().ok()?;
            let mut samples = Vec::new();
            for _ in 0..2 {
                if let Some(mbps) = probe_download(ip, DOWNLOAD_BYTES).await {
                    samples.push(mbps);
                }
            }
            samples.sort_by(|left, right| left.partial_cmp(right).unwrap_or(Ordering::Equal));
            candidate.download_mbps = median(&samples).unwrap_or(0.0);
            candidate.score = score_candidate(
                candidate.median_latency_ms,
                candidate.jitter_ms,
                candidate.loss_ratio,
                candidate.download_mbps,
            );
            let probe = probe_custom_hostname_details(&validation_hostname, ip)
                .await
                .ok()?;
            candidate.business_hostname = Some(validation_hostname);
            candidate.business_status = Some(probe.status);
            candidate.business_colo = probe.colo;
            candidate.business_cf_ray = probe.cf_ray;
            candidate.business_validated = true;
            Some(candidate)
        });
    }
    let mut completed = 0usize;
    let mut measured = Vec::new();
    while let Some(result) = download_tasks.join_next().await {
        if let Some(job_id) = job_id
            && is_job_cancelled(state, job_id).await
        {
            download_tasks.abort_all();
            return Ok(OptimizationScanResult {
                candidates: Vec::new(),
                vantage,
                source_warnings,
                resolver_diagnostics,
                resolution_path,
                source_fingerprint,
            });
        }
        if let Ok(Some(candidate)) = result {
            measured.push(candidate);
        }
        completed += 1;
        if let Some(job_id) = job_id {
            let progress = 65 + ((completed * 30) / download_total) as i64;
            update_job(
                state,
                job_id,
                json!({ "phase": "download", "progress": progress }),
            )
            .await;
        }
    }
    results = measured;
    results.retain(|candidate| candidate.download_mbps > 0.0 && candidate.score.is_finite());
    results.sort_by(|left, right| {
        left.score
            .partial_cmp(&right.score)
            .unwrap_or(Ordering::Equal)
    });
    Ok(OptimizationScanResult {
        candidates: results,
        vantage,
        source_warnings,
        resolver_diagnostics,
        resolution_path,
        source_fingerprint,
    })
}

pub(super) async fn load_candidate_seeds(
    settings: &OptimizationSourceSettings,
    prefixes: &[Ipv4Net],
) -> (
    Vec<CandidateSeed>,
    Vec<String>,
    Vec<ResolverDiagnostic>,
    String,
) {
    let selected_builtins = settings
        .builtin_ids
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let mut hostname_sources = BUILTIN_CANDIDATE_SOURCES
        .iter()
        .filter(|source| selected_builtins.contains(source.id))
        .map(|source| {
            (
                source.hostname.to_string(),
                "builtin".to_string(),
                source.id.to_string(),
            )
        })
        .collect::<Vec<_>>();
    hostname_sources.extend(
        settings
            .custom_hostnames
            .iter()
            .cloned()
            .map(|hostname| (hostname.clone(), "custom".to_string(), hostname)),
    );

    let mut resolved = Vec::new();
    let doh_client = build_doh_client();
    for chunk in hostname_sources.chunks(8) {
        let mut tasks = JoinSet::new();
        for (index, (hostname, source_type, source_id)) in chunk.iter().cloned().enumerate() {
            let client = doh_client.clone();
            let prefixes = prefixes.to_vec();
            tasks.spawn(async move {
                let result = match client {
                    Ok(client) => resolve_candidate_hostname(&client, &hostname, &prefixes).await,
                    Err(failure) => CandidateResolution::failed_for_all_providers(failure),
                };
                (index, hostname, source_type, source_id, result)
            });
        }
        let mut chunk_results = Vec::new();
        while let Some(task) = tasks.join_next().await {
            if let Ok(value) = task {
                chunk_results.push(value);
            }
        }
        chunk_results.sort_by_key(|value| value.0);
        resolved.extend(chunk_results);
    }

    let mut seeds = Vec::new();
    let mut indexes = HashMap::new();
    let mut warnings = Vec::new();
    let mut resolver_attempts = Vec::new();
    let mut doh_candidates_available = false;
    for (_, hostname, source_type, source_id, result) in resolved {
        let all_failed_summary = result.all_failed_summary();
        resolver_attempts.extend(result.attempts);
        let ips = result.ips;
        if ips.is_empty() {
            if let Some(summary) = all_failed_summary {
                warnings.push(format!("{hostname}: {summary}"));
            } else {
                warnings.push(format!(
                    "{hostname} ({source_id}) did not resolve to a verified Cloudflare IPv4 address"
                ));
            }
            continue;
        }
        doh_candidates_available = true;
        for ip in ips {
            merge_candidate_seed(&mut seeds, &mut indexes, ip, &source_type, Some(&hostname));
        }
    }
    if settings.official_ranges {
        for ip in sample_candidate_ips(prefixes) {
            merge_candidate_seed(&mut seeds, &mut indexes, ip, "official-range", None);
            if seeds.len() >= MAX_CANDIDATES {
                break;
            }
        }
    }
    seeds.truncate(MAX_CANDIDATES);
    let resolution_path =
        initial_resolution_path(doh_candidates_available, settings.official_ranges);
    (
        seeds,
        warnings,
        aggregate_resolver_diagnostics(&resolver_attempts),
        resolution_path.to_string(),
    )
}

pub(super) fn candidate_ip_is_cloudflare(ip: Ipv4Addr, prefixes: &[Ipv4Net]) -> bool {
    prefixes.iter().any(|prefix| prefix.contains(&ip))
}

pub(super) fn merge_candidate_seed(
    seeds: &mut Vec<CandidateSeed>,
    indexes: &mut HashMap<Ipv4Addr, usize>,
    ip: Ipv4Addr,
    source_type: &str,
    source_hostname: Option<&str>,
) {
    if let Some(index) = indexes.get(&ip).copied() {
        let seed = &mut seeds[index];
        if !seed.source_types.iter().any(|value| value == source_type) {
            seed.source_types.push(source_type.to_string());
        }
        if let Some(hostname) = source_hostname
            && !seed.source_hostnames.iter().any(|value| value == hostname)
        {
            seed.source_hostnames.push(hostname.to_string());
        }
        return;
    }
    if seeds.len() >= MAX_CANDIDATES {
        return;
    }
    indexes.insert(ip, seeds.len());
    seeds.push(CandidateSeed {
        ip,
        source_types: vec![source_type.to_string()],
        source_hostnames: source_hostname
            .map(|value| vec![value.to_string()])
            .unwrap_or_default(),
    });
}

pub(super) fn merge_priority_candidate_seed(
    seeds: &mut Vec<CandidateSeed>,
    ip: Ipv4Addr,
    source_type: &str,
) {
    if let Some(seed) = seeds.iter_mut().find(|seed| seed.ip == ip) {
        if !seed.source_types.iter().any(|value| value == source_type) {
            seed.source_types.push(source_type.to_string());
        }
        return;
    }
    if seeds.len() >= MAX_CANDIDATES {
        let Some(position) = seeds.iter().rposition(|seed| {
            !seed
                .source_types
                .iter()
                .any(|source_type| matches!(source_type.as_str(), "current" | "preferred-ip"))
        }) else {
            return;
        };
        seeds.remove(position);
    }
    seeds.push(CandidateSeed {
        ip,
        source_types: vec![source_type.to_string()],
        source_hostnames: Vec::new(),
    });
}

pub(super) fn retain_shortlist_with_priority(
    candidates: &mut Vec<OptimizationCandidate>,
    priority_ips: &[Ipv4Addr],
    limit: usize,
) {
    let mut priorities = Vec::new();
    for ip in priority_ips {
        let ip = ip.to_string();
        if priorities
            .iter()
            .any(|candidate: &OptimizationCandidate| candidate.ip == ip)
        {
            continue;
        }
        if let Some(position) = candidates.iter().position(|candidate| candidate.ip == ip) {
            priorities.push(candidates.remove(position));
        }
    }
    priorities.truncate(limit);
    candidates.truncate(limit.saturating_sub(priorities.len()));
    candidates.extend(priorities);
}

pub(super) fn normalize_preferred_ip(
    value: Option<&str>,
    prefixes: &[Ipv4Net],
) -> Result<Option<Ipv4Addr>, String> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let ip = value
        .parse::<Ipv4Addr>()
        .map_err(|_| "Preferred IP must be a valid IPv4 address".to_string())?;
    if !candidate_ip_is_cloudflare(ip, prefixes) {
        return Err("Preferred IP must belong to an official Cloudflare IPv4 range".to_string());
    }
    Ok(Some(ip))
}

pub(super) fn select_recommended_candidate(
    candidates: &[OptimizationCandidate],
    preferred_ip: Option<Ipv4Addr>,
) -> (Option<String>, Option<bool>) {
    let Some(preferred_ip) = preferred_ip.map(|ip| ip.to_string()) else {
        return (
            candidates.first().map(|candidate| candidate.ip.clone()),
            None,
        );
    };
    let validated = candidates
        .iter()
        .any(|candidate| candidate.ip == preferred_ip && candidate.business_validated);
    (validated.then_some(preferred_ip), Some(validated))
}

pub(super) async fn probe_local_vantage(state: &AppState) -> Value {
    let measured_at = time_utils::now_iso();
    let response = state
        .fallback_client
        .get("https://www.cloudflare.com/cdn-cgi/trace")
        .timeout(Duration::from_secs(8))
        .send()
        .await;
    let Ok(response) = response else {
        return json!({
            "id": "local-server",
            "label": "fn-knock server",
            "publicIp": Value::Null,
            "defaultColo": Value::Null,
            "measuredAt": measured_at,
        });
    };
    let text = response.text().await.unwrap_or_default();
    let trace = parse_trace(&text);
    json!({
        "id": "local-server",
        "label": "fn-knock server",
        "publicIp": trace.get("ip").cloned().unwrap_or_default(),
        "defaultColo": trace.get("colo").cloned().unwrap_or_default(),
        "measuredAt": measured_at,
    })
}

pub(super) fn scan_validation_hostname_error(ownership: &Value) -> CloudflareApiError {
    let capability_probe = ownership.pointer("/optimization/capabilityProbe");
    if capability_probe
        .and_then(|probe| probe.get("reasonCode"))
        .and_then(Value::as_str)
        == Some(CLOUDFLARE_SAAS_REQUIRED_ERROR_CODE)
    {
        return local_error(CLOUDFLARE_SAAS_REQUIRED_SCAN_ERROR);
    }

    let business_hostname_conflict = ownership
        .pointer("/optimization/customHostnames")
        .and_then(Value::as_object)
        .is_some_and(|items| {
            items
                .values()
                .any(|state| state.get("status").and_then(Value::as_str) == Some("conflict"))
        });
    if business_hostname_conflict {
        return local_error(CLOUDFLARE_RESOURCE_CONFLICT_SCAN_ERROR);
    }

    let capability_pending = capability_probe
        .and_then(|probe| probe.get("status"))
        .and_then(Value::as_str)
        .is_some_and(|status| matches!(status, "pending" | "awaiting-candidate"));
    let business_hostname_pending = ownership
        .pointer("/optimization/customHostnames")
        .and_then(Value::as_object)
        .is_some_and(|items| {
            items.values().any(|state| {
                matches!(
                    state.get("status").and_then(Value::as_str),
                    Some("queued" | "pending" | "active" | "ready")
                ) && !scan_business_hostname_is_ready(state)
            })
        });

    if capability_pending || business_hostname_pending {
        local_error(CLOUDFLARE_SAAS_VALIDATION_PENDING_SCAN_ERROR)
    } else {
        local_error(OPTIMIZATION_NOT_READY_SCAN_ERROR)
    }
}
