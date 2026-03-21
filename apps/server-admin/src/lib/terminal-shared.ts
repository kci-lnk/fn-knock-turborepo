import { basename } from "node:path";
import { dataPath } from "./AppDirManager";

export type TerminalResumeBackend = "tmux";
export type TerminalTransport = "http-polling";
export type TerminalTmuxDetectionSource = "env-path" | "absolute-path";
export type TerminalTmuxInstallStatus =
  | "uninstalled"
  | "installing"
  | "installed"
  | "error";
export type TerminalSessionStatus =
  | "created"
  | "attached"
  | "detached"
  | "stopped"
  | "error";

export interface TerminalTmuxInstallState {
  status: TerminalTmuxInstallStatus;
  progress: number;
  message: string;
  executablePath: string;
  detectionSource: TerminalTmuxDetectionSource | null;
  version: string;
}

export interface TerminalFeatureConfig {
  enabled: boolean;
  default_shell: string;
  default_cwd: string;
  max_sessions: number;
  idle_timeout_seconds: number;
  resume_backend: TerminalResumeBackend;
  allow_mobile_toolbar: boolean;
  dangerously_run_as_current_user: boolean;
}

export interface TerminalSessionRecord {
  id: string;
  title: string;
  status: TerminalSessionStatus;
  created_at: string;
  updated_at: string;
  last_attached_at: string;
  last_detached_at: string;
  last_client_ip: string;
  shell: string;
  cwd: string;
  cols: number;
  rows: number;
  resume_backend: TerminalResumeBackend;
  backend_session_name: string;
  pane_tty_path: string;
  input_pipe_path: string;
  output_log_path: string;
  expires_at: string;
  last_frame_revision?: string;
}

export interface TerminalAttachmentRecord {
  id: string;
  session_id: string;
  transport: TerminalTransport;
  created_at: string;
  updated_at: string;
  expires_at: string;
}

export interface TerminalOutputChunk {
  cursor: number;
  data_base64: string;
  reset: boolean;
  updatedAt: string;
}

export interface TerminalRuntimeStatus {
  enabled: boolean;
  tmuxAvailable: boolean;
  tmuxExecutablePath: string;
  tmuxDetectionSource: TerminalTmuxDetectionSource | null;
  tmuxVersion: string;
  tmuxInstallState: TerminalTmuxInstallState;
  httpPollingAvailable: boolean;
  runningAsRoot: boolean;
  blockedReason: string;
}

export const DEFAULT_TERMINAL_POLL_TIMEOUT_MS = 15_000;
export const DEFAULT_TERMINAL_POLL_INTERVAL_MS = 300;
export const DEFAULT_TERMINAL_ATTACHMENT_TTL_SECONDS = 120;

export const DEFAULT_TERMINAL_FEATURE_CONFIG: TerminalFeatureConfig = {
  enabled: false,
  default_shell: "",
  default_cwd: dataPath,
  max_sessions: 3,
  idle_timeout_seconds: 24 * 60 * 60,
  resume_backend: "tmux",
  allow_mobile_toolbar: true,
  dangerously_run_as_current_user: true,
};

const clampInteger = (
  value: unknown,
  fallback: number,
  min: number,
  max: number,
): number => {
  const parsed = Number.parseInt(String(value ?? ""), 10);
  if (!Number.isFinite(parsed)) return fallback;
  return Math.min(max, Math.max(min, parsed));
};

const normalizeString = (value: unknown, fallback = ""): string => {
  if (typeof value !== "string") return fallback;
  const trimmed = value.trim();
  return trimmed || fallback;
};

const normalizeIsoString = (value: unknown): string => {
  if (typeof value !== "string") return "";
  const trimmed = value.trim();
  if (!trimmed) return "";
  const parsed = Date.parse(trimmed);
  return Number.isFinite(parsed) ? new Date(parsed).toISOString() : "";
};

export const normalizeTerminalFeatureConfig = (
  value?: Partial<TerminalFeatureConfig> | null,
): TerminalFeatureConfig => {
  const raw = value ?? {};

  return {
    enabled: raw.enabled === true,
    default_shell: normalizeString(raw.default_shell),
    default_cwd: normalizeString(
      raw.default_cwd,
      DEFAULT_TERMINAL_FEATURE_CONFIG.default_cwd,
    ),
    max_sessions: clampInteger(
      raw.max_sessions,
      DEFAULT_TERMINAL_FEATURE_CONFIG.max_sessions,
      1,
      12,
    ),
    idle_timeout_seconds: clampInteger(
      raw.idle_timeout_seconds,
      DEFAULT_TERMINAL_FEATURE_CONFIG.idle_timeout_seconds,
      60,
      7 * 24 * 60 * 60,
    ),
    resume_backend: "tmux",
    allow_mobile_toolbar:
      raw.allow_mobile_toolbar !== undefined
        ? raw.allow_mobile_toolbar === true
        : DEFAULT_TERMINAL_FEATURE_CONFIG.allow_mobile_toolbar,
    dangerously_run_as_current_user:
      raw.dangerously_run_as_current_user !== undefined
        ? raw.dangerously_run_as_current_user === true
        : DEFAULT_TERMINAL_FEATURE_CONFIG.dangerously_run_as_current_user,
  };
};

export const normalizeTerminalSessionRecord = (
  value: Partial<TerminalSessionRecord>,
): TerminalSessionRecord => {
  const now = new Date().toISOString();
  const cwd = normalizeString(
    value.cwd,
    DEFAULT_TERMINAL_FEATURE_CONFIG.default_cwd,
  );

  return {
    id: normalizeString(value.id),
    title: normalizeString(value.title, basename(cwd) || "Web终端"),
    status:
      value.status === "attached" ||
      value.status === "detached" ||
      value.status === "stopped" ||
      value.status === "error"
        ? value.status
        : "created",
    created_at: normalizeIsoString(value.created_at) || now,
    updated_at: normalizeIsoString(value.updated_at) || now,
    last_attached_at: normalizeIsoString(value.last_attached_at),
    last_detached_at: normalizeIsoString(value.last_detached_at),
    last_client_ip: normalizeString(value.last_client_ip),
    shell: normalizeString(value.shell, process.env.SHELL || "/bin/sh"),
    cwd,
    cols: clampInteger(value.cols, 120, 20, 400),
    rows: clampInteger(value.rows, 32, 8, 200),
    resume_backend: "tmux",
    backend_session_name: normalizeString(value.backend_session_name),
    pane_tty_path: normalizeString(value.pane_tty_path),
    input_pipe_path: normalizeString(value.input_pipe_path),
    output_log_path: normalizeString(value.output_log_path),
    expires_at: normalizeIsoString(value.expires_at),
    last_frame_revision: normalizeString(value.last_frame_revision),
  };
};

export const normalizeTerminalAttachmentRecord = (
  value: Partial<TerminalAttachmentRecord>,
): TerminalAttachmentRecord => {
  const now = new Date().toISOString();
  return {
    id: normalizeString(value.id),
    session_id: normalizeString(value.session_id),
    transport: "http-polling",
    created_at: normalizeIsoString(value.created_at) || now,
    updated_at: normalizeIsoString(value.updated_at) || now,
    expires_at: normalizeIsoString(value.expires_at) || now,
  };
};

export const isTerminalSessionExpired = (
  session: Pick<TerminalSessionRecord, "expires_at">,
  now = Date.now(),
): boolean => {
  const expiresAt = Date.parse(session.expires_at || "");
  return Number.isFinite(expiresAt) && expiresAt <= now;
};
