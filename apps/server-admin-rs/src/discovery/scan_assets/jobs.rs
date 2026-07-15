use super::*;

pub(super) fn discover_jobs() -> &'static Mutex<HashMap<String, DiscoverJobHandle>> {
    DISCOVER_JOBS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn discover_jobs_guard() -> MutexGuard<'static, HashMap<String, DiscoverJobHandle>> {
    discover_jobs()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
}

pub(super) fn discover_job_guard(job: &DiscoverJobHandle) -> MutexGuard<'_, DiscoverJob> {
    job.lock().unwrap_or_else(|error| error.into_inner())
}

pub(super) fn create_discover_job(
    scan_cidrs: Vec<String>,
    self_scan_hosts: Vec<String>,
    exclude_ports: Vec<u16>,
    runtime_settings: ScanRuntimeSettings,
    translator: Translator,
) -> DiscoverJobHandle {
    cleanup_discover_jobs();
    let now = now_millis();
    let id = uuid::Uuid::new_v4().to_string();
    let cancel = Arc::new(AtomicBool::new(false));
    let job = Arc::new(Mutex::new(DiscoverJob {
        id: id.clone(),
        cancel: cancel.clone(),
        created_at: now,
        updated_at: now,
        state: "queued".to_string(),
        meta: None,
        progress: None,
        service_events: Vec::new(),
        service_map: Vec::new(),
        result: None,
        error: None,
    }));
    discover_jobs_guard().insert(id, job.clone());
    enforce_discover_job_limits();

    let job_for_task = job.clone();
    tokio::spawn(async move {
        run_discover_job(
            job_for_task,
            scan_cidrs,
            self_scan_hosts,
            exclude_ports,
            runtime_settings,
            translator,
        )
        .await;
    });
    job
}

pub(super) fn get_discover_job_handle(job_id: &str) -> Option<DiscoverJobHandle> {
    discover_jobs_guard().get(job_id).cloned()
}

pub(super) fn is_terminal_discover_state(state: &str) -> bool {
    matches!(state, "completed" | "cancelled" | "failed")
}

pub(super) fn cancel_discover_job(job: &DiscoverJobHandle) {
    let mut locked = discover_job_guard(job);
    if is_terminal_discover_state(&locked.state) {
        return;
    }
    locked.cancel.store(true, Ordering::SeqCst);
    locked.state = "cancelled".to_string();
    locked.service_events.clear();
    locked.service_map.clear();
    locked.updated_at = now_millis();
}

pub(super) fn cleanup_discover_jobs() {
    let now = now_millis();
    let jobs = discover_jobs();
    let handles = jobs
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .iter()
        .map(|(id, job)| (id.clone(), job.clone()))
        .collect::<Vec<_>>();
    let mut delete_ids = Vec::new();
    for (id, job) in handles {
        let mut locked = discover_job_guard(&job);
        if is_terminal_discover_state(&locked.state) {
            if now - locked.updated_at > DISCOVER_JOB_DONE_TTL_MS {
                delete_ids.push(id);
            }
            continue;
        }
        if discover_job_inactive_expired(&locked, now) {
            locked.cancel.store(true, Ordering::SeqCst);
            locked.state = "cancelled".to_string();
            locked.service_events.clear();
            locked.service_map.clear();
            locked.updated_at = now;
        }
    }
    if !delete_ids.is_empty() {
        let mut locked = jobs.lock().unwrap_or_else(|error| error.into_inner());
        for id in delete_ids {
            locked.remove(&id);
        }
    }
    enforce_discover_job_limits();
}

pub(super) fn discover_job_inactive_expired(job: &DiscoverJob, now: i64) -> bool {
    now.saturating_sub(job.updated_at) > DISCOVER_JOB_ACTIVE_TTL_MS
}

pub(super) fn enforce_discover_job_limits() {
    let jobs = discover_jobs();
    let handles = jobs
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .iter()
        .map(|(id, job)| (id.clone(), job.clone()))
        .collect::<Vec<_>>();
    let mut active = Vec::new();
    for (_id, job) in &handles {
        let locked = discover_job_guard(job);
        if !is_terminal_discover_state(&locked.state) {
            active.push((locked.created_at, job.clone()));
        }
    }
    active.sort_by_key(|(created_at, _)| *created_at);
    for (_, job) in active
        .iter()
        .take(active.len().saturating_sub(DISCOVER_JOB_MAX_ACTIVE))
    {
        cancel_discover_job(job);
    }

    let mut ids_by_age = Vec::new();
    for (id, job) in handles {
        let locked = discover_job_guard(&job);
        ids_by_age.push((
            locked.created_at,
            id,
            is_terminal_discover_state(&locked.state),
        ));
    }
    ids_by_age.sort_by_key(|(created_at, _, _)| *created_at);
    let overflow = ids_by_age.len().saturating_sub(DISCOVER_JOB_MAX_RETAINED);
    if overflow == 0 {
        return;
    }
    let mut locked_jobs = jobs.lock().unwrap_or_else(|error| error.into_inner());
    for (_, id, terminal) in ids_by_age.into_iter().take(overflow) {
        if terminal {
            locked_jobs.remove(&id);
        } else if let Some(job) = locked_jobs.get(&id) {
            cancel_discover_job(job);
        }
    }
}

pub(super) fn serialize_discover_job(job: &DiscoverJobHandle, cursor: Option<&str>) -> Value {
    let locked = discover_job_guard(job);
    let service_cursor = normalize_service_cursor(cursor, locked.service_events.len());
    json!({
        "jobId": locked.id,
        "state": locked.state,
        "createdAt": locked.created_at,
        "updatedAt": locked.updated_at,
        "meta": locked.meta,
        "progress": locked.progress,
        "services": locked.service_events[service_cursor..].to_vec(),
        "nextCursor": locked.service_events.len(),
        "result": locked.result,
        "error": locked.error,
    })
}

pub(super) fn normalize_service_cursor(value: Option<&str>, max: usize) -> usize {
    parse_js_parse_int_radix_10(value.unwrap_or("0"))
        .filter(|cursor| *cursor >= 0)
        .map(|cursor| (cursor as usize).min(max))
        .unwrap_or(0)
}

pub(super) fn discovery_port_range() -> DiscoveryPortRange {
    DiscoveryPortRange {
        start: DISCOVERY_PORT_RANGE_START,
        end: DISCOVERY_PORT_RANGE_END,
    }
}

pub(super) fn count_ports_in_range(range: DiscoveryPortRange, skip_ports: &[u16]) -> usize {
    let skipped = skip_ports
        .iter()
        .copied()
        .filter(|port| *port >= range.start && *port <= range.end)
        .collect::<BTreeSet<_>>()
        .len();
    usize::from(range.end - range.start + 1).saturating_sub(skipped)
}

pub(super) fn build_port_list(range: DiscoveryPortRange, skip_ports: &[u16]) -> Vec<u16> {
    let skip = skip_ports.iter().copied().collect::<BTreeSet<_>>();
    (range.start..=range.end)
        .filter(|port| !skip.contains(port))
        .collect()
}

pub(super) fn merge_discovery_skip_ports(base: &[u16], extra: &[u16]) -> Vec<u16> {
    base.iter()
        .chain(extra)
        .copied()
        .filter(|port| *port > 0)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub(super) fn build_discovery_port_mode_label() -> String {
    format!("{DISCOVERY_PORT_RANGE_START}-{DISCOVERY_PORT_RANGE_END}")
}

pub(super) fn count_discovery_scan_ports_for_groups(
    groups: &[DiscoveryHostGroup],
    exclude_ports: &[u16],
) -> usize {
    groups
        .iter()
        .map(|group| {
            let skip_ports = merge_discovery_skip_ports(exclude_ports, &group.skip_ports);
            count_ports_in_range(discovery_port_range(), &skip_ports) * group.hosts.len()
        })
        .sum()
}

pub(super) fn build_discovery_host_groups(
    scan_cidrs: &[String],
    scan_hosts: Option<&[String]>,
    self_scan_hosts: &[String],
) -> Vec<DiscoveryHostGroup> {
    let allowed_hosts = scan_hosts.map(|hosts| hosts.iter().cloned().collect::<BTreeSet<_>>());
    let self_scan_hosts = build_self_scan_host_set(self_scan_hosts);
    let mut seen_hosts = BTreeSet::new();
    let mut groups = Vec::new();

    for cidr in normalize_allowed_scan_cidrs(scan_cidrs.iter().cloned()) {
        let hosts = expand_scan_cidrs(std::slice::from_ref(&cidr))
            .into_iter()
            .filter(|host| {
                if allowed_hosts
                    .as_ref()
                    .is_some_and(|allowed_hosts| !allowed_hosts.contains(host))
                {
                    return false;
                }
                seen_hosts.insert(host.clone())
            })
            .collect::<Vec<_>>();
        if hosts.is_empty() {
            continue;
        }

        push_discovery_host_groups(&mut groups, hosts, &self_scan_hosts);
    }

    groups
}

pub(super) fn build_self_scan_host_set(self_scan_hosts: &[String]) -> BTreeSet<String> {
    std::iter::once(LOOPBACK_DISCOVERY_HOST.to_string())
        .chain(self_scan_hosts.iter().map(|host| host.trim().to_string()))
        .filter(|host| !host.is_empty())
        .collect()
}

pub(super) fn push_discovery_host_groups(
    groups: &mut Vec<DiscoveryHostGroup>,
    hosts: Vec<String>,
    self_scan_hosts: &BTreeSet<String>,
) {
    if hosts.is_empty() {
        return;
    }
    let mut regular_hosts = Vec::new();
    let mut local_self_hosts = Vec::new();
    for host in hosts {
        if self_scan_hosts.contains(&host) {
            local_self_hosts.push(host);
        } else {
            regular_hosts.push(host);
        }
    }
    if !regular_hosts.is_empty() {
        groups.push(build_discovery_host_group(regular_hosts, Vec::new()));
    }
    if !local_self_hosts.is_empty() {
        groups.push(build_discovery_host_group(
            local_self_hosts,
            LOCAL_SELF_DISCOVERY_SKIP_PORTS.to_vec(),
        ));
    }
}

pub(super) fn build_discovery_host_group(
    hosts: Vec<String>,
    skip_ports: Vec<u16>,
) -> DiscoveryHostGroup {
    DiscoveryHostGroup { hosts, skip_ports }
}
