import type {
  components as ApiContractComponents,
  operations as ApiContractOperations,
} from "@fn-knock/api-contract";
import { apiClient } from "./client";

export type TraceLookupPayload =
  ApiContractComponents["schemas"]["TraceLookupData"];
type TraceLookupResponse =
  ApiContractOperations["get_api_admin_traces_trace_id"]["responses"][200]["content"]["application/json"];

export const TraceAPI = {
  async get(
    traceId: string,
    signal?: AbortSignal,
  ): Promise<TraceLookupResponse> {
    const response = await apiClient.get(
      `/traces/${encodeURIComponent(traceId)}`,
      { signal },
    );
    return response.data;
  },
};
