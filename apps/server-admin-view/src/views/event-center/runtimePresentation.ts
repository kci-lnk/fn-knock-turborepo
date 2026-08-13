import type {
  RuntimeHealthStatus,
  RuntimeOperationalLogEntry,
  SystemEventRecord,
} from "@/types";

export const formatRuntimeDate = (value?: string | null) =>
  value ? new Date(value).toLocaleString() : "-";

export const formatRuntimeBytes = (bytes?: number | null) => {
  if (bytes === undefined || bytes === null) return "-";
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KiB`;
  return `${(bytes / 1024 / 1024).toFixed(2)} MiB`;
};

export const runtimeStatusClass = (status: RuntimeHealthStatus) => {
  if (status === "healthy") {
    return "border-emerald-500/30 bg-emerald-500/10 text-emerald-700";
  }
  if (status === "degraded" || status === "blocked") {
    return "border-amber-500/30 bg-amber-500/10 text-amber-700";
  }
  if (status === "unhealthy") {
    return "border-red-500/30 bg-red-500/10 text-red-700";
  }
  return "border-slate-400/30 bg-slate-400/10 text-slate-600";
};

export const getRuntimeEventComponent = (event: SystemEventRecord) =>
  String(event.payload?.component || event.subject?.id || "-");

export const formatRuntimeLogLine = (entry: RuntimeOperationalLogEntry) => {
  const fields =
    entry.fields && Object.keys(entry.fields).length
      ? ` ${JSON.stringify(entry.fields)}`
      : "";
  return `${entry.time} [${entry.level}] ${entry.component}/${entry.event}${
    entry.reason_code ? ` (${entry.reason_code})` : ""
  }${fields}`;
};
