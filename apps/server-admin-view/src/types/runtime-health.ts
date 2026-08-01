import type { SystemEventRecord } from "./system-events";

export type RuntimeHealthStatus =
  "healthy" | "degraded" | "unhealthy" | "unknown" | "blocked";

export type RuntimeProcessState =
  "running" | "stopped" | "unknown" | "not_applicable";

export interface RuntimeComponentHealth {
  id: string;
  status: RuntimeHealthStatus;
  process_state: RuntimeProcessState;
  version?: string | null;
  commit?: string | null;
  pid?: number | null;
  instance_id?: string | null;
  started_at?: string | null;
  uptime_ms?: number | null;
  last_checked_at?: string | null;
  last_success_at?: string | null;
  consecutive_failures: number;
  reason_code?: string | null;
  cpu_percent?: number | null;
  rss_bytes?: number | null;
  go_version?: string | null;
  goroutines?: number | null;
  heap_alloc_bytes?: number | null;
  heap_sys_bytes?: number | null;
  latency_ms?: number | null;
}

export interface RuntimeLogStatus {
  directory: string;
  bytes_used: number;
  dropped_info: number;
  oldest_at?: string | null;
  newest_at?: string | null;
}

export type RuntimeLogComponent = "management" | "gateway_process";

export interface RuntimeOperationalLogEntry {
  time: string;
  level: "INFO" | "WARN" | "ERROR" | string;
  component: string;
  event: string;
  reason_code?: string | null;
  fields?: Record<string, unknown>;
}

export interface RuntimeComponentLogs {
  schema_version: number;
  component: RuntimeLogComponent;
  generated_at: string;
  entries: RuntimeOperationalLogEntry[];
}

export interface RuntimeLogClearResult {
  component: RuntimeLogComponent;
  cleared_at: string;
}

export interface RuntimeHealthSnapshot {
  schema_version: number;
  overall_status: RuntimeHealthStatus;
  last_checked_at?: string | null;
  components: Record<string, RuntimeComponentHealth>;
  logs: RuntimeLogStatus;
  supervisor: string;
}

export interface RuntimeDiagnostics {
  schema_version: number;
  generated_at: string;
  version: string;
  commit: string;
  platform: Record<string, string>;
  runtime: RuntimeHealthSnapshot;
  recent_runtime_events: SystemEventRecord[];
}
