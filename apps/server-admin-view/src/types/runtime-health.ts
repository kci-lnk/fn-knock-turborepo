import type { components as ApiContractComponents } from "@fn-knock/api-contract";

type RuntimeSchemas = ApiContractComponents["schemas"];

export type RuntimeHealthStatus =
  RuntimeSchemas["RuntimeComponentHealthData"]["status"];
export type RuntimeProcessState =
  RuntimeSchemas["RuntimeComponentHealthData"]["process_state"];
export type RuntimeComponentHealth =
  RuntimeSchemas["RuntimeComponentHealthData"];
export type RuntimeLogStatus = RuntimeSchemas["RuntimeLogStatusData"];
export type RuntimeLogComponent =
  RuntimeSchemas["RuntimeComponentLogsData"]["component"];
export type RuntimeOperationalLogEntry =
  RuntimeSchemas["RuntimeOperationalLogEntryData"];
export type RuntimeComponentLogs =
  RuntimeSchemas["RuntimeComponentLogsData"];
export type RuntimeLogClearResult = RuntimeSchemas["RuntimeLogClearData"];
export type RuntimeHealthSnapshot =
  RuntimeSchemas["RuntimeHealthSnapshotData"];
export type RuntimeDiagnostics = RuntimeSchemas["RuntimeDiagnosticsData"];
