#[test]
fn samples_official_ranges_deterministically_and_within_bounds() {
    let prefixes = parse_prefixes(&CLOUDFLARE_IPV4_FALLBACK.join("\n"));
    let first = sample_candidate_ips(&prefixes);
    let second = sample_candidate_ips(&prefixes);
    assert_eq!(first, second);
    assert!(!first.is_empty());
    assert!(first.len() <= MAX_CANDIDATES);
    assert!(
        first
            .iter()
            .all(|ip| prefixes.iter().any(|prefix| prefix.contains(ip)))
    );
}

#[test]
fn source_settings_include_builtins_and_normalize_custom_hostnames() {
    let defaults = OptimizationSourceSettings::default();
    assert!(defaults.official_ranges);
    assert_eq!(defaults.builtin_ids.len(), BUILTIN_CANDIDATE_SOURCES.len());

    let normalized = normalize_source_settings(OptimizationSourceSettings {
        official_ranges: true,
        builtin_ids: vec![
            "sweden-government".to_string(),
            "sweden-government".to_string(),
            "us-fbi".to_string(),
            "removed-source".to_string(),
        ],
        custom_hostnames: vec![
            " WWW.Example.org. ".to_string(),
            "www.example.org".to_string(),
        ],
    })
    .expect("settings should normalize");
    assert_eq!(normalized.builtin_ids, vec!["sweden-government"]);
    assert_eq!(normalized.custom_hostnames, vec!["www.example.org"]);
}

#[test]
fn domain_settings_normalize_and_deduplicate_external_hostnames() {
    let normalized = normalize_domain_settings(OptimizationDomainSettings {
        external_hostnames: vec![
            " App.Example.com. ".to_string(),
            "app.example.com".to_string(),
            "other.example.com".to_string(),
        ],
    })
    .expect("domain settings should normalize");
    assert_eq!(
        normalized.external_hostnames,
        vec!["app.example.com", "other.example.com"]
    );
    assert!(
        normalize_domain_settings(OptimizationDomainSettings {
            external_hostnames: vec!["https://app.example.com".to_string()],
        })
        .is_err()
    );
}

#[test]
fn external_hostname_partition_preserves_configured_order() {
    let settings = OptimizationDomainSettings {
        external_hostnames: vec![
            "external.example.com".to_string(),
            "stale.example.com".to_string(),
        ],
    };
    let (managed, external) = partition_optimization_hosts(
        vec![
            "auth.example.com".to_string(),
            "external.example.com".to_string(),
            "app.example.com".to_string(),
        ],
        &settings,
    );
    assert_eq!(managed, vec!["auth.example.com", "app.example.com"]);
    assert_eq!(external, vec!["external.example.com"]);
}

#[test]
fn dns_conflict_details_distinguish_instance_ownership() {
    let records = vec![
        json!({
            "type": "CNAME",
            "content": "current.example.com",
            "proxied": false,
            "comment": "Managed by fn-knock (instance-a)",
        }),
        json!({
            "type": "A",
            "content": "192.0.2.1",
            "proxied": true,
            "tags": ["fn-knock-instance:instance-b"],
        }),
        json!({
            "type": "TXT",
            "content": "external",
            "proxied": null,
        }),
    ];
    let details = dns_conflict_details(
        &records,
        "instance-a",
        "CNAME",
        "desired.example.com",
        false,
    );
    assert_eq!(details["records"][0]["ownerKind"], "current-instance");
    assert_eq!(
        details["records"][1]["ownerKind"],
        "other-fn-knock-instance"
    );
    assert_eq!(details["records"][2]["ownerKind"], "external");
    assert_eq!(details["desired"]["content"], "desired.example.com");
}

#[test]
fn exact_dns_cleanup_uses_the_tracked_origin_or_edge_target() {
    let ownership = json!({
        "optimization": {
            "originDns": { "name": "origin.example.com" },
            "edgeDns": { "name": "edge.example.com" },
        }
    });
    let origin = tracked_exact_dns_snapshot(
        "app.example.com",
        "dns-1",
        &json!({ "exactDnsTarget": "origin" }),
        &ownership,
        None,
    );
    let edge = tracked_exact_dns_snapshot(
        "app.example.com",
        "dns-2",
        &json!({ "exactDnsTarget": "edge" }),
        &ownership,
        None,
    );
    assert_eq!(origin["content"], "origin.example.com");
    assert_eq!(edge["content"], "edge.example.com");
}

#[test]
fn source_settings_reject_urls_ips_and_an_empty_source_set() {
    for value in [
        "https://www.example.org",
        "28.0.2.55",
        "*.example.org",
        "example.org/path",
    ] {
        assert!(normalize_candidate_hostname(value).is_err(), "{value}");
    }
    assert!(
        normalize_source_settings(OptimizationSourceSettings {
            official_ranges: false,
            builtin_ids: Vec::new(),
            custom_hostnames: Vec::new(),
        })
        .is_err()
    );
}

#[test]
fn fake_ip_is_rejected_even_when_a_candidate_hostname_returns_it() {
    let prefixes = parse_prefixes(&CLOUDFLARE_IPV4_FALLBACK.join("\n"));
    assert!(!candidate_ip_is_cloudflare(
        "28.0.2.55".parse().expect("valid fake IP"),
        &prefixes
    ));
    assert!(candidate_ip_is_cloudflare(
        "104.18.26.94".parse().expect("valid Cloudflare IP"),
        &prefixes
    ));
}

#[test]
fn candidate_sources_merge_without_losing_provenance() {
    let ip = "104.18.26.94".parse().expect("valid IP");
    let mut seeds = Vec::new();
    let mut indexes = HashMap::new();
    merge_candidate_seed(&mut seeds, &mut indexes, ip, "builtin", Some("www.gov.se"));
    merge_candidate_seed(&mut seeds, &mut indexes, ip, "official-range", None);
    assert_eq!(seeds.len(), 1);
    assert_eq!(seeds[0].source_hostnames, vec!["www.gov.se"]);
    assert_eq!(seeds[0].source_types, vec!["builtin", "official-range"]);
}

#[test]
fn extracts_real_pop_from_cloudflare_ray_instead_of_geoip() {
    assert_eq!(cf_ray_colo("a261079199891d1c-SIN").as_deref(), Some("SIN"));
    assert_eq!(cf_ray_colo("bad"), None);
    assert_eq!(cf_ray_colo("ray-too-long"), None);
    assert_eq!(
        bounded_cf_ray(&reqwest::header::HeaderValue::from_static(
            "a261079199891d1c-SIN",
        ))
        .as_deref(),
        Some("a261079199891d1c-SIN")
    );
    assert_eq!(
        bounded_cf_ray(&reqwest::header::HeaderValue::from_static("   ")),
        None
    );
}

#[test]
fn score_penalizes_loss_latency_jitter_and_low_bandwidth() {
    let baseline = score_candidate(30.0, 2.0, 0.0, 100.0);
    assert!(score_candidate(60.0, 2.0, 0.0, 100.0) > baseline);
    assert!(score_candidate(30.0, 20.0, 0.0, 100.0) > baseline);
    assert!(score_candidate(30.0, 2.0, 0.2, 100.0) > baseline);
    assert!(score_candidate(30.0, 2.0, 0.0, 5.0) > baseline);
}

#[test]
fn automatic_switch_requires_a_full_fifteen_percent_lead() {
    assert!(score_is_15_percent_better(85.0, 100.0));
    assert!(!score_is_15_percent_better(85.01, 100.0));
    assert!(!score_is_15_percent_better(f64::NAN, 100.0));
    assert!(!score_is_15_percent_better(10.0, 0.0));
}

#[test]
fn automatic_first_round_uses_the_freshly_measured_current_candidate() {
    let candidate = |ip: &str, score: f64| OptimizationCandidate {
        ip: ip.to_string(),
        median_latency_ms: score,
        jitter_ms: 0.0,
        loss_ratio: 0.0,
        download_mbps: 100.0,
        score,
        verified_at: Some(time_utils::now_iso()),
        source_types: Vec::new(),
        source_hostnames: Vec::new(),
        colo: Some("SIN".to_string()),
        cf_ray: None,
        business_hostname: Some("app.example.com".to_string()),
        business_status: Some(200),
        business_colo: Some("SIN".to_string()),
        business_cf_ray: None,
        business_validated: true,
    };
    let mut ownership = json!({
        "optimization": {
            "selected": { "ip": "104.16.1.1", "score": 1.0 }
        }
    });
    let mut runtime = json!({});
    apply_automatic_scan_result(
        &mut ownership,
        &mut runtime,
        &[
            candidate("104.16.2.2", 80.0),
            candidate("104.16.1.1", 100.0),
        ],
    );
    assert_eq!(
        runtime.pointer("/pendingCandidate/candidate/ip"),
        Some(&json!("104.16.2.2"))
    );
}

#[test]
fn current_candidate_is_kept_inside_the_global_seed_limit() {
    let mut seeds = (0..MAX_CANDIDATES)
        .map(|index| CandidateSeed {
            ip: Ipv4Addr::new(104, 16, (index / 256) as u8, index as u8),
            source_types: vec!["official-range".to_string()],
            source_hostnames: Vec::new(),
        })
        .collect::<Vec<_>>();
    let current = Ipv4Addr::new(104, 17, 1, 1);
    merge_priority_candidate_seed(&mut seeds, current, "current");
    assert_eq!(seeds.len(), MAX_CANDIDATES);
    assert!(
        seeds
            .iter()
            .any(|seed| seed.ip == current && seed.source_types == vec!["current"])
    );
}

#[test]
fn current_and_preferred_seeds_survive_the_global_seed_limit() {
    let mut seeds = (0..MAX_CANDIDATES)
        .map(|index| CandidateSeed {
            ip: Ipv4Addr::new(104, 16, (index / 256) as u8, index as u8),
            source_types: vec!["official-range".to_string()],
            source_hostnames: Vec::new(),
        })
        .collect::<Vec<_>>();
    let current = Ipv4Addr::new(104, 17, 1, 1);
    let preferred = Ipv4Addr::new(104, 17, 1, 2);

    merge_priority_candidate_seed(&mut seeds, current, "current");
    merge_priority_candidate_seed(&mut seeds, preferred, "preferred-ip");

    assert_eq!(seeds.len(), MAX_CANDIDATES);
    assert!(seeds.iter().any(|seed| seed.ip == current));
    assert!(seeds.iter().any(|seed| seed.ip == preferred));
}

#[test]
fn preferred_ip_must_be_a_cloudflare_ipv4_address() {
    let prefixes = parse_prefixes(&CLOUDFLARE_IPV4_FALLBACK.join("\n"));
    assert_eq!(
        normalize_preferred_ip(Some(" 104.18.26.94 "), &prefixes),
        Ok(Some("104.18.26.94".parse().expect("valid Cloudflare IP")))
    );
    assert_eq!(normalize_preferred_ip(Some("  "), &prefixes), Ok(None));
    assert!(
        normalize_preferred_ip(Some("192.0.2.1"), &prefixes)
            .expect_err("non-Cloudflare IP must be rejected")
            .contains("official Cloudflare IPv4 range")
    );
    assert!(
        normalize_preferred_ip(Some("not-an-ip"), &prefixes)
            .expect_err("invalid IPv4 must be rejected")
            .contains("valid IPv4")
    );
}

#[test]
fn current_and_preferred_ips_are_kept_in_the_download_shortlist() {
    let candidate = |index: u8| OptimizationCandidate {
        ip: Ipv4Addr::new(104, 16, 0, index).to_string(),
        median_latency_ms: f64::from(index),
        jitter_ms: 0.0,
        loss_ratio: 0.0,
        download_mbps: 0.0,
        score: f64::MAX,
        verified_at: None,
        source_types: Vec::new(),
        source_hostnames: Vec::new(),
        colo: None,
        cf_ray: None,
        business_hostname: None,
        business_status: None,
        business_colo: None,
        business_cf_ray: None,
        business_validated: false,
    };
    let mut candidates = (0..12).map(candidate).collect::<Vec<_>>();
    let current = Ipv4Addr::new(104, 16, 0, 10);
    let preferred = Ipv4Addr::new(104, 16, 0, 11);

    retain_shortlist_with_priority(&mut candidates, &[current, preferred], DOWNLOAD_SHORTLIST);

    assert_eq!(candidates.len(), DOWNLOAD_SHORTLIST);
    assert!(
        candidates
            .iter()
            .any(|candidate| candidate.ip == current.to_string())
    );
    assert!(
        candidates
            .iter()
            .any(|candidate| candidate.ip == preferred.to_string())
    );

    let mut one_slot = vec![candidate(0), candidate(11)];
    retain_shortlist_with_priority(&mut one_slot, &[preferred, current], 1);
    assert_eq!(one_slot[0].ip, preferred.to_string());
}

#[test]
fn a_preferred_ip_is_recommended_only_after_business_validation() {
    let candidate = |ip: &str, business_validated: bool| OptimizationCandidate {
        ip: ip.to_string(),
        median_latency_ms: 10.0,
        jitter_ms: 0.0,
        loss_ratio: 0.0,
        download_mbps: 100.0,
        score: 1.0,
        verified_at: None,
        source_types: Vec::new(),
        source_hostnames: Vec::new(),
        colo: None,
        cf_ray: None,
        business_hostname: None,
        business_status: None,
        business_colo: None,
        business_cf_ray: None,
        business_validated,
    };
    let preferred = Ipv4Addr::new(104, 16, 0, 2);
    let automatic = candidate("104.16.0.1", true);

    assert_eq!(
        select_recommended_candidate(
            &[automatic.clone(), candidate("104.16.0.2", true)],
            Some(preferred),
        ),
        (Some("104.16.0.2".to_string()), Some(true))
    );
    assert_eq!(
        select_recommended_candidate(
            &[automatic.clone(), candidate("104.16.0.2", false)],
            Some(preferred),
        ),
        (None, Some(false))
    );
    assert_eq!(
        select_recommended_candidate(&[automatic], None),
        (Some("104.16.0.1".to_string()), None)
    );
}

#[test]
fn completed_scans_expire_after_ten_minutes() {
    let completed = 1_000_000;
    assert!(scan_is_fresh(completed, completed));
    assert!(scan_is_fresh(completed, completed + SCAN_APPLY_TTL_MS));
    assert!(!scan_is_fresh(completed, completed + SCAN_APPLY_TTL_MS + 1));
    assert!(!scan_is_fresh(completed, completed - 1));
    assert!(!scan_is_fresh(0, completed));
}

#[test]
fn source_fingerprint_changes_with_the_effective_configuration() {
    let defaults = OptimizationSourceSettings::default();
    let mut changed = defaults.clone();
    changed.official_ranges = false;
    assert_eq!(
        source_settings_fingerprint(&defaults),
        source_settings_fingerprint(&defaults.clone())
    );
    assert_ne!(
        source_settings_fingerprint(&defaults),
        source_settings_fingerprint(&changed)
    );
}
use super::*;
