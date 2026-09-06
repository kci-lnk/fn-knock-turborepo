import type { components, operations } from "@fn-knock/api-contract";

type RuntimeDebugSchemas = components["schemas"];
export type RuntimeDebugReport = RuntimeDebugSchemas["RuntimeDebugReportData"];
export type RuntimeDebugSample = RuntimeDebugSchemas["DebugSample"];
export type RuntimeDebugOperation = RuntimeDebugSchemas["OperationStats"];
export type RuntimeDebugMemoryCategory = RuntimeDebugSchemas["MemoryCategory"];
export type RuntimeDebugResponse =
  operations["get_api_admin_runtime_health_debug"]["responses"][200]["content"]["application/json"];
