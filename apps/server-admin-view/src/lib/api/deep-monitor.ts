import type { DeepMonitorEvent, DeepMonitorSession } from "../../types";
import type {
  components as ApiContractComponents,
  operations as ApiContractOperations,
} from "@fn-knock/api-contract";
import { adminApiBasePath, apiClient } from "./client";

type DeepMonitorSchemas = ApiContractComponents["schemas"];
type DeepMonitorStartRequest = DeepMonitorSchemas["DeepMonitorStartBodyData"];
type DeepMonitorExtendRequest = DeepMonitorSchemas["DeepMonitorExtendBodyData"];
type DeepMonitorEventList = DeepMonitorSchemas["DeepMonitorEventListData"];
type DeepMonitorEventsQuery = NonNullable<
  ApiContractOperations["get_api_admin_deep_monitor_sessions__session_id__events"]["parameters"]["query"]
>;
type DeepMonitorPayloadQuery =
  ApiContractOperations["get_api_admin_deep_monitor_sessions__session_id__events__event_id__payload"]["parameters"]["query"];

const sessionPath = (sessionId: string) =>
  `/deep-monitor/sessions/${encodeURIComponent(sessionId)}`;

export const DeepMonitorAPI = {
  async list(signal?: AbortSignal): Promise<DeepMonitorSession[]> {
    const res = await apiClient.get("/deep-monitor/sessions", { signal });
    return res.data.data.items || [];
  },
  async get(sessionId: string): Promise<DeepMonitorSession> {
    const res = await apiClient.get(sessionPath(sessionId));
    return res.data.data;
  },
  async start(payload: DeepMonitorStartRequest): Promise<DeepMonitorSession> {
    const res = await apiClient.post("/deep-monitor/sessions", payload);
    return res.data.data;
  },
  async extend(
    sessionId: string,
    durationSeconds: number,
  ): Promise<DeepMonitorSession> {
    const res = await apiClient.post(`${sessionPath(sessionId)}/extend`, {
      duration_seconds: durationSeconds,
    } satisfies DeepMonitorExtendRequest);
    return res.data.data;
  },
  async stop(sessionId: string): Promise<DeepMonitorSession> {
    const res = await apiClient.post(`${sessionPath(sessionId)}/stop`);
    return res.data.data;
  },
  async delete(sessionId: string): Promise<void> {
    await apiClient.delete(sessionPath(sessionId));
  },
  async events(
    sessionId: string,
    params: DeepMonitorEventsQuery = {},
    signal?: AbortSignal,
  ): Promise<DeepMonitorEventList> {
    const res = await apiClient.get(`${sessionPath(sessionId)}/events`, {
      params,
      signal,
    });
    return res.data.data;
  },
  async event(sessionId: string, eventId: string): Promise<DeepMonitorEvent> {
    const res = await apiClient.get(
      `${sessionPath(sessionId)}/events/${encodeURIComponent(eventId)}`,
    );
    return res.data.data;
  },
  async previewPayload(
    sessionId: string,
    eventId: string,
    part: string,
  ): Promise<ArrayBuffer> {
    const res = await apiClient.get(
      `${sessionPath(sessionId)}/events/${encodeURIComponent(eventId)}/payload`,
      {
        params: { part, limit: 256 * 1024 } satisfies DeepMonitorPayloadQuery,
        responseType: "arraybuffer",
      },
    );
    return res.data;
  },
  payloadUrl(sessionId: string, eventId: string, part: string): string {
    const path = `${adminApiBasePath}${sessionPath(sessionId)}/events/${encodeURIComponent(eventId)}/payload`;
    return `${path}?part=${encodeURIComponent(part)}`;
  },
  archiveUrl(sessionId: string): string {
    return `${adminApiBasePath}${sessionPath(sessionId)}/download`;
  },
  liveUrl(sessionId: string, afterSequence = 0): string {
    return `${adminApiBasePath}${sessionPath(sessionId)}/live?after_sequence=${afterSequence}`;
  },
};
