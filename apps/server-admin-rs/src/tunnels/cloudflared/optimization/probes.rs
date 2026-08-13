use std::{
    cmp::Ordering,
    collections::HashSet,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    time::{Duration, Instant},
};

use ipnet::Ipv4Net;

use crate::crypto_utils;

use super::*;

pub(super) async fn load_cloudflare_prefixes(state: &AppState) -> Vec<Ipv4Net> {
    let remote = state
        .fallback_client
        .get("https://www.cloudflare.com/ips-v4")
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .ok()
        .and_then(|response| response.error_for_status().ok());
    let text = match remote {
        Some(response) => response.text().await.ok(),
        None => None,
    };
    let parsed = parse_prefixes(text.as_deref().unwrap_or(""));
    if parsed.is_empty() {
        bundled_cloudflare_prefixes()
    } else {
        parsed
    }
}

pub(super) fn bundled_cloudflare_prefixes() -> Vec<Ipv4Net> {
    parse_prefixes(&CLOUDFLARE_IPV4_FALLBACK.join("\n"))
}

pub(super) fn parse_prefixes(value: &str) -> Vec<Ipv4Net> {
    let mut seen = HashSet::new();
    value
        .lines()
        .filter_map(|line| line.trim().parse::<Ipv4Net>().ok())
        .filter(|network| seen.insert(*network))
        .collect()
}

pub(super) fn sample_candidate_ips(prefixes: &[Ipv4Net]) -> Vec<Ipv4Addr> {
    let mut output = Vec::new();
    let mut seen = HashSet::new();
    for prefix in prefixes {
        let host_bits = 32u32.saturating_sub(prefix.prefix_len() as u32);
        let address_count = 1u64 << host_bits.min(32);
        if address_count <= 2 {
            continue;
        }
        let base = u32::from(prefix.network()) as u64;
        let usable = address_count - 2;
        for index in 0..CANDIDATES_PER_PREFIX {
            let seed = crypto_utils::sha256_hex_str(&format!("{prefix}:{index}:fn-knock"));
            let sample = u64::from_str_radix(seed.get(..16).unwrap_or("0"), 16).unwrap_or(0);
            let offset = 1 + sample % usable;
            let ip = Ipv4Addr::from((base + offset) as u32);
            if prefix.contains(&ip) && seen.insert(ip) {
                output.push(ip);
            }
            if output.len() >= MAX_CANDIDATES {
                return output;
            }
        }
    }
    output
}

pub(super) async fn probe_latency(ip: Ipv4Addr) -> Option<(f64, f64, f64)> {
    let metrics = probe_latency_metrics(ip).await?;
    Some((
        metrics.median_latency_ms,
        metrics.jitter_ms,
        metrics.loss_ratio,
    ))
}

pub(super) async fn probe_latency_metrics(ip: Ipv4Addr) -> Option<LatencyProbeMetrics> {
    let client = speedtest_client(SPEEDTEST_HOST, ip, Duration::from_secs(4)).ok()?;
    let mut samples = Vec::new();
    let mut cf_ray = None;
    for _ in 0..LATENCY_PROBES {
        let started = Instant::now();
        let response = client
            .get(format!("https://{SPEEDTEST_HOST}{SPEEDTEST_PATH}?bytes=0"))
            .header(reqwest::header::CACHE_CONTROL, "no-store")
            .send()
            .await;
        if let Ok(response) = response
            && response.status().is_success()
            && let Some(response_cf_ray) = response.headers().get("cf-ray").and_then(bounded_cf_ray)
        {
            if cf_ray.is_none() {
                cf_ray = Some(response_cf_ray);
            }
            samples.push(started.elapsed().as_secs_f64() * 1000.0);
        }
    }
    let loss = 1.0 - samples.len() as f64 / LATENCY_PROBES as f64;
    if samples.len() < 2 {
        return None;
    }
    samples.sort_by(|left, right| left.partial_cmp(right).unwrap_or(Ordering::Equal));
    let latency = median(&samples)?;
    let jitter =
        samples.last().copied().unwrap_or(latency) - samples.first().copied().unwrap_or(latency);
    let colo = cf_ray.as_deref().and_then(cf_ray_colo);
    Some(LatencyProbeMetrics {
        median_latency_ms: latency,
        jitter_ms: jitter,
        loss_ratio: loss,
        colo,
        cf_ray,
    })
}

pub(super) async fn probe_download(ip: Ipv4Addr, bytes: usize) -> Option<f64> {
    let client = speedtest_client(SPEEDTEST_HOST, ip, Duration::from_secs(12)).ok()?;
    let started = Instant::now();
    let mut response = client
        .get(format!(
            "https://{SPEEDTEST_HOST}{SPEEDTEST_PATH}?bytes={bytes}"
        ))
        .header(reqwest::header::CACHE_CONTROL, "no-store")
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?;
    let mut received = 0usize;
    while received < bytes {
        let chunk = response.chunk().await.ok()??;
        received = received.saturating_add(chunk.len().min(bytes - received));
    }
    if received < bytes / 2 {
        return None;
    }
    let seconds = started.elapsed().as_secs_f64().max(0.001);
    Some(received as f64 * 8.0 / seconds / 1_000_000.0)
}

pub(super) fn speedtest_client(
    hostname: &str,
    ip: Ipv4Addr,
    timeout: Duration,
) -> Result<reqwest::Client, reqwest::Error> {
    reqwest::Client::builder()
        // Candidate measurements must reach the supplied IP directly. A
        // process-level HTTPS proxy would otherwise handle CONNECT by hostname
        // and silently bypass this explicit resolver override.
        .no_proxy()
        .connect_timeout(Duration::from_secs(2))
        .timeout(timeout)
        .https_only(true)
        .resolve(hostname, SocketAddr::new(IpAddr::V4(ip), 443))
        .build()
}

pub(super) async fn probe_custom_hostname(hostname: &str, ip: Ipv4Addr) -> Result<(), String> {
    probe_custom_hostname_details(hostname, ip)
        .await
        .map(|_| ())
}

pub(super) async fn probe_custom_hostname_details(
    hostname: &str,
    ip: Ipv4Addr,
) -> Result<BusinessProbeResult, String> {
    let client = reqwest::Client::builder()
        .no_proxy()
        .connect_timeout(Duration::from_secs(3))
        .timeout(Duration::from_secs(8))
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .resolve(hostname, SocketAddr::new(IpAddr::V4(ip), 443))
        .build()
        .map_err(|error| error.to_string())?;
    let response = client
        .get(format!("https://{hostname}/"))
        .header(reqwest::header::CACHE_CONTROL, "no-store")
        .send()
        .await
        .map_err(|error| format!("Preferred edge TLS probe failed: {error}"))?;
    let status = response.status();
    let cf_ray = response.headers().get("cf-ray").and_then(bounded_cf_ray);
    let mut response = response;
    let mut body = Vec::new();
    while body.len() < 32 * 1024 {
        let chunk = match response.chunk().await {
            Ok(Some(chunk)) => chunk,
            Ok(None) => break,
            Err(error) => return Err(format!("Preferred edge response failed: {error}")),
        };
        let remaining = 32 * 1024 - body.len();
        body.extend_from_slice(&chunk[..chunk.len().min(remaining)]);
    }
    let body = String::from_utf8_lossy(&body).to_ascii_lowercase();
    if let Some(error) = cloudflare_route_rejection_message(status.as_u16(), &body) {
        return Err(error);
    }
    if cf_ray.is_none() {
        return Err(format!(
            "Preferred edge returned HTTP {status} without a Cloudflare Ray ID"
        ));
    }
    Ok(BusinessProbeResult {
        status: status.as_u16(),
        colo: cf_ray.as_deref().and_then(cf_ray_colo),
        cf_ray,
    })
}

pub(super) fn cloudflare_route_rejection_message(status: u16, body: &str) -> Option<String> {
    if body.contains("error 1000")
        || body.contains("error code: 1000")
        || body.contains("error code 1000")
        || body.contains("dns points to prohibited ip")
    {
        return Some("Cloudflare Error 1000: DNS points to a prohibited Cloudflare IP".to_string());
    }
    if body.contains("error 1016")
        || body.contains("error code: 1016")
        || body.contains("error code 1016")
    {
        return Some("Cloudflare Error 1016: origin DNS resolution failed".to_string());
    }
    if matches!(status, 520..=527 | 530) {
        return Some(format!("Cloudflare edge returned HTTP {status}"));
    }
    if body.contains("cloudflare") && body.contains("error code") {
        return Some(format!(
            "Cloudflare returned an edge error page (HTTP {status})"
        ));
    }
    None
}

pub(super) fn cf_ray_colo(value: &str) -> Option<String> {
    let colo = value.rsplit_once('-')?.1.trim().to_ascii_uppercase();
    (colo.len() == 3 && colo.bytes().all(|byte| byte.is_ascii_alphanumeric())).then_some(colo)
}

pub(super) fn bounded_cf_ray(value: &reqwest::header::HeaderValue) -> Option<String> {
    let value = value.to_str().ok()?.trim();
    (!value.is_empty() && value.len() <= 128).then(|| value.to_string())
}

pub(super) fn score_candidate(latency: f64, jitter: f64, loss: f64, download_mbps: f64) -> f64 {
    latency + 2.0 * jitter + 1500.0 * loss + 800.0 / download_mbps.max(1.0)
}

pub(super) fn score_is_15_percent_better(candidate: f64, current: f64) -> bool {
    candidate.is_finite()
        && current.is_finite()
        && candidate >= 0.0
        && current > 0.0
        && candidate <= current * 0.85
}

pub(super) fn scan_is_fresh(completed_at_ms: i64, now_ms: i64) -> bool {
    completed_at_ms > 0
        && now_ms >= completed_at_ms
        && now_ms.saturating_sub(completed_at_ms) <= SCAN_APPLY_TTL_MS
}

pub(super) fn median(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        None
    } else if values.len() % 2 == 1 {
        values.get(values.len() / 2).copied()
    } else {
        let right = values.len() / 2;
        Some((values[right - 1] + values[right]) / 2.0)
    }
}
