import type {
  DeepMonitorEvent,
  DeepMonitorEventSummary,
  DeepMonitorSession,
} from "../../types";
import { adminApiBasePath, apiClient } from "./client";

const sessionPath = (sessionId: string) =>
  `/deep-monitor/sessions/${encodeURIComponent(sessionId)}`;

export const DeepMonitorAPI = {
  async list(): Promise<DeepMonitorSession[]> {
    const res = await apiClient.get("/deep-monitor/sessions");
    return res.data.data.items || [];
  },
  async get(sessionId: string): Promise<DeepMonitorSession> {
    const res = await apiClient.get(sessionPath(sessionId));
    return res.data.data;
  },
  async start(payload: {
    host: string;
    duration_seconds: number;
  }): Promise<DeepMonitorSession> {
    const res = await apiClient.post("/deep-monitor/sessions", payload);
    return res.data.data;
  },
  async extend(
    sessionId: string,
    durationSeconds: number,
  ): Promise<DeepMonitorSession> {
    const res = await apiClient.post(`${sessionPath(sessionId)}/extend`, {
      duration_seconds: durationSeconds,
    });
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
    params: {
      cursor?: string;
      limit?: number;
      type?: string;
      search?: string;
      direction?: string;
      method?: string;
      status?: number;
      client_ip?: string;
      identity?: string;
      path?: string;
    } = {},
  ): Promise<{
    items: DeepMonitorEventSummary[];
    next_cursor: string;
    has_more: boolean;
  }> {
    const res = await apiClient.get(`${sessionPath(sessionId)}/events`, {
      params,
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
      { params: { part, limit: 256 * 1024 }, responseType: "arraybuffer" },
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
