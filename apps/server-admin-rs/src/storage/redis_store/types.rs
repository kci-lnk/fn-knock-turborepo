use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TotpCredential {
    pub id: String,
    pub secret: String,
    #[serde(default)]
    pub comment: String,
    #[serde(default, rename = "createdAt")]
    pub created_at: String,
    #[serde(default)]
    pub access_scopes: Value,
    #[serde(default)]
    pub subdomain_access: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuthAccount {
    pub id: String,
    pub username: String,
    #[serde(default, rename = "displayName")]
    pub display_name: String,
    #[serde(default, rename = "sourceTotpId")]
    pub source_totp_id: String,
    #[serde(default, rename = "createdAt")]
    pub created_at: String,
    #[serde(default, rename = "updatedAt")]
    pub updated_at: String,
    #[serde(default)]
    pub access_scopes: Value,
    #[serde(default)]
    pub subdomain_access: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuthPasswordCredential {
    #[serde(rename = "accountId")]
    pub account_id: String,
    pub algorithm: String,
    pub salt: String,
    pub hash: String,
    pub n: u32,
    pub r: u32,
    pub p: u32,
    pub key_length: usize,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LoginSession {
    #[serde(rename = "totpId")]
    pub totp_id: String,
    pub method: String,
    #[serde(rename = "credentialId")]
    pub credential_id: String,
    #[serde(rename = "credentialName")]
    pub credential_name: String,
    #[serde(rename = "linkedTotpName", skip_serializing_if = "Option::is_none")]
    pub linked_totp_name: Option<String>,
    #[serde(
        default,
        rename = "accessScopes",
        skip_serializing_if = "Option::is_none"
    )]
    pub access_scopes: Option<Value>,
    #[serde(
        default,
        rename = "subdomainAccess",
        skip_serializing_if = "Option::is_none"
    )]
    pub subdomain_access: Option<Value>,
    #[serde(rename = "grantType", skip_serializing_if = "Option::is_none")]
    pub grant_type: Option<String>,
    #[serde(
        default,
        rename = "postLoginIpGrantMode",
        skip_serializing_if = "Option::is_none"
    )]
    pub post_login_ip_grant_mode: Option<String>,
    #[serde(
        default,
        rename = "postLoginIpGrantRecordId",
        skip_serializing_if = "Option::is_none"
    )]
    pub post_login_ip_grant_record_id: Option<String>,
    #[serde(
        default,
        rename = "streamAccessExpiresAt",
        skip_serializing_if = "Option::is_none"
    )]
    pub stream_access_expires_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    pub ip: String,
    #[serde(rename = "userAgent")]
    pub user_agent: String,
    #[serde(rename = "loginTime")]
    pub login_time: String,
    #[serde(rename = "expiresAt", skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
    #[serde(rename = "ipLocation", skip_serializing_if = "Option::is_none")]
    pub ip_location: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DockerAdminPasswordRecord {
    pub algorithm: String,
    pub salt: String,
    pub hash: String,
    pub n: u32,
    pub r: u32,
    pub p: u32,
    pub key_length: usize,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DockerAdminSessionRecord {
    pub id: String,
    pub created_at: String,
    pub updated_at: String,
    pub expires_at: String,
    #[serde(default)]
    pub ttl_seconds: i64,
    #[serde(default)]
    pub password_revision: String,
    pub ip: String,
    pub user_agent: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LoginAttemptRecord {
    pub ip: String,
    pub attempts: u32,
    pub last_attempt_at: String,
    pub blocked_until: i64,
}

#[derive(Clone, Debug, Serialize)]
pub struct DockerAdminResetSummary {
    pub password_cleared: bool,
    pub sessions_cleared: usize,
    pub login_failures_cleared: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct LoginBackoffStatus {
    pub ip: String,
    pub attempts: i64,
    pub blocked: bool,
    #[serde(skip_serializing_if = "Option::is_none", rename = "retryAfter")]
    pub retry_after: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "blockedUntil")]
    pub blocked_until: Option<i64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct WhitelistRecord {
    pub id: String,
    pub ip: String,
    #[serde(default = "default_whitelist_target_type", rename = "targetType")]
    pub target_type: String,
    #[serde(rename = "expireAt")]
    pub expire_at: Option<i64>,
    #[serde(default = "default_whitelist_source")]
    pub source: String,
    #[serde(default, rename = "createdAt")]
    pub created_at: i64,
    #[serde(default = "default_whitelist_status")]
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "ipLocation")]
    pub ip_location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "resolvedTargets")]
    pub resolved_targets: Option<Vec<String>>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        rename = "checkIntervalMinutes"
    )]
    pub check_interval_minutes: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "lastCheckedAt")]
    pub last_checked_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "lastResolvedAt")]
    pub last_resolved_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "resolveStatus")]
    pub resolve_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "resolveMessage")]
    pub resolve_message: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WhitelistConcreteTarget {
    #[serde(rename = "recordId")]
    pub record_id: String,
    #[serde(rename = "recordTarget")]
    pub record_target: String,
    #[serde(rename = "recordTargetType")]
    pub record_target_type: String,
    pub source: String,
    pub target: String,
    #[serde(rename = "targetType")]
    pub target_type: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct WhitelistRegionInput {
    pub province: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query_city: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operator: Option<crate::cidr::CidrOperator>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct WhitelistRegionGroupRecord {
    pub id: String,
    #[serde(default)]
    pub regions: Vec<WhitelistRegionInput>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cidrs: Vec<String>,
    #[serde(default, rename = "policyId", skip_serializing_if = "String::is_empty")]
    pub policy_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy: Option<Value>,
    #[serde(default, rename = "sourceCidrCount")]
    pub source_cidr_count: usize,
    #[serde(default, rename = "rangeCount")]
    pub range_count: usize,
    #[serde(rename = "expireAt")]
    pub expire_at: Option<i64>,
    #[serde(default = "default_whitelist_source")]
    pub source: String,
    #[serde(default, rename = "createdAt")]
    pub created_at: i64,
    #[serde(default, rename = "updatedAt")]
    pub updated_at: i64,
    #[serde(default = "default_whitelist_status")]
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct WhitelistRegionGroupSummary {
    pub id: String,
    pub regions: Vec<WhitelistRegionInput>,
    #[serde(rename = "expireAt")]
    pub expire_at: Option<i64>,
    pub source: String,
    #[serde(rename = "createdAt")]
    pub created_at: i64,
    #[serde(rename = "updatedAt")]
    pub updated_at: i64,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    #[serde(rename = "cidrCount")]
    pub cidr_count: usize,
}

impl WhitelistRecord {
    pub fn target_type(&self) -> &str {
        match self.target_type.as_str() {
            "cidr" => "cidr",
            "cname" => "cname",
            _ => "ip",
        }
    }

    pub fn is_active(&self) -> bool {
        self.status == "active"
    }

    pub fn concrete_targets(&self) -> Vec<WhitelistConcreteTarget> {
        match self.target_type() {
            "cidr" => vec![self.concrete_target(&self.ip, "cidr")],
            "cname" => self
                .resolved_targets
                .clone()
                .unwrap_or_default()
                .into_iter()
                .filter_map(|target| {
                    let normalized = normalize_ip(&target);
                    (!normalized.is_empty()).then_some(normalized)
                })
                .collect::<BTreeSet<_>>()
                .into_iter()
                .map(|target| self.concrete_target(&target, "ip"))
                .collect(),
            _ => vec![self.concrete_target(&self.ip, "ip")],
        }
    }

    fn concrete_target(&self, target: &str, target_type: &str) -> WhitelistConcreteTarget {
        WhitelistConcreteTarget {
            record_id: self.id.clone(),
            record_target: self.ip.clone(),
            record_target_type: self.target_type().to_string(),
            source: if self.source == "auto" {
                "auto".to_string()
            } else {
                "manual".to_string()
            },
            target: target.to_string(),
            target_type: target_type.to_string(),
        }
    }
}

impl WhitelistRegionGroupRecord {
    pub fn is_active(&self) -> bool {
        self.status == "active"
    }

    pub fn summary(&self) -> WhitelistRegionGroupSummary {
        WhitelistRegionGroupSummary {
            id: self.id.clone(),
            regions: self.regions.clone(),
            expire_at: self.expire_at,
            source: self.source.clone(),
            created_at: self.created_at,
            updated_at: self.updated_at,
            status: self.status.clone(),
            comment: self.comment.clone(),
            cidr_count: if self.source_cidr_count > 0 {
                self.source_cidr_count
            } else {
                self.cidrs.len()
            },
        }
    }

    pub fn concrete_targets(&self) -> Vec<WhitelistConcreteTarget> {
        self.policy()
            .map(|policy| policy.to_cidrs())
            .unwrap_or_else(|| self.cidrs.clone())
            .into_iter()
            .map(|cidr| WhitelistConcreteTarget {
                record_id: self.id.clone(),
                record_target: self.id.clone(),
                record_target_type: "cidr".to_string(),
                source: self.source.clone(),
                target: cidr,
                target_type: "cidr".to_string(),
            })
            .collect()
    }

    pub fn policy(&self) -> Option<crate::cidr::CompiledIpSet> {
        self.policy_result().ok()
    }

    pub fn policy_result(&self) -> Result<crate::cidr::CompiledIpSet, String> {
        if let Some(value) = self.policy.as_ref() {
            let policy =
                crate::cidr::CompiledIpSet::from_transport_value(value)?.into_current_format();
            if !self.policy_id.trim().is_empty() && self.policy_id != policy.id {
                return Err(format!(
                    "policy reference mismatch: expected {}, got {}",
                    self.policy_id, policy.id
                ));
            }
            return Ok(policy);
        }
        crate::cidr::compile_ip_set(&self.cidrs)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TrafficDeltaPoint {
    pub ts: i64,
    pub delta: f64,
}

#[derive(Clone, Debug)]
pub struct TrafficSnapshotRecord {
    pub host: Option<String>,
    pub stream: Option<String>,
    pub total_in: f64,
    pub total_out: f64,
    pub error_5xx: f64,
}
