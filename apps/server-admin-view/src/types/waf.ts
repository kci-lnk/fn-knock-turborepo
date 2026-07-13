export type WAFMode = "off" | "detection" | "blocking";

export interface WAFConfig {
  enabled: boolean;
  system_rules_auto_update_enabled: boolean;
  common_location_exempt_enabled: boolean;
  mode: WAFMode;
  active_bundle_id: string;
  rules_dir: string;
  paranoia_level: number;
  executing_paranoia_level: number;
  inbound_anomaly_threshold: number;
  outbound_anomaly_threshold: number;
  request_body_access: boolean;
  request_body_limit_bytes: number;
  request_body_in_memory_limit_bytes: number;
  response_body_access: boolean;
  disabled_hosts: string[];
  disabled_path_prefixes: string[];
  log_retention_days: number;
  drain_interval_seconds: number;
  updated_at: string | null;
}

export interface WAFStatus {
  enabled: boolean;
  mode: WAFMode | string;
  loaded: boolean;
  bundle_id?: string;
  bundle_hash?: string;
  loaded_at?: string;
  rules_dir?: string;
  pending_events: number;
  last_error?: string;
}

export interface WAFValidationResult {
  ok: boolean;
  bundle_id?: string;
  bundle_path?: string;
  bundle_hash?: string;
  error?: string;
}

export type WAFRuleSource = "system" | "custom";

export interface WAFManifestRule {
  filename: string;
  description: string;
}

export interface WAFRemoteManifest {
  rulesDescription?: {
    rules?: WAFManifestRule[];
  };
  packagingTime?: string;
  zipFile: string;
  zipHash: string;
  commitHash?: string;
  commitDate?: string;
}

export interface WAFRuleFile {
  source: WAFRuleSource;
  filename: string;
  description: string;
  recommended: boolean;
  enabled: boolean;
  size_bytes: number;
  updated_at: string;
}

export interface WAFRuleFileContent extends WAFRuleFile {
  content: string;
}

export interface WAFSystemSyncState {
  zip_file: string;
  zip_hash: string;
  synced_at: string;
  packaging_time?: string;
  commit_hash?: string;
  commit_date?: string;
}

export interface WAFDetails {
  config: WAFConfig;
  status: WAFStatus | null;
  rules_dir: string;
  system: {
    manifest: WAFRemoteManifest | null;
    manifest_cached_at: string | null;
    manifest_last_checked_at: string | null;
    manifest_last_error: string | null;
    synced: WAFSystemSyncState | null;
    update_available: boolean;
    rules: WAFRuleFile[];
  };
  custom: {
    rules: WAFRuleFile[];
  };
}

export interface WAFMatchedVariable {
  variable?: string;
  key?: string;
  value_preview?: string;
}

export interface WAFRuleMatch {
  id: number;
  message?: string;
  data?: string;
  severity?: string;
  phase?: number;
  file?: string;
  line?: number;
  tags?: string[];
  disruptive: boolean;
  matched_variables?: WAFMatchedVariable[];
}

export interface WAFInterruptionInfo {
  rule_id?: number;
  action?: string;
  status?: number;
}

export interface WAFEvent {
  trace_id: string;
  transaction_id?: string;
  time: string;
  mode: WAFMode | string;
  action: string;
  status?: number;
  client_ip?: string;
  remote_addr?: string;
  method?: string;
  scheme?: string;
  host?: string;
  path?: string;
  query?: string;
  request_uri?: string;
  user_agent?: string;
  referer?: string;
  route_type?: string;
  route_key?: string;
  upstream?: string;
  bundle_id?: string;
  bundle_hash?: string;
  rule_ids?: number[];
  rules?: WAFRuleMatch[];
  interruption?: WAFInterruptionInfo;
  error?: string;
}

export interface WAFDrainResult {
  events: WAFEvent[];
  drained: number;
  remaining: number;
}

export interface WAFLogEntriesPayload {
  date: string;
  available_dates: string[];
  cursor: string;
  next_cursor: string;
  has_more: boolean;
  limit: number;
  total: number;
  items: WAFEvent[];
}

export interface WAFLogDeletePayload {
  date: string;
  deleted: boolean;
  available_dates: string[];
}
