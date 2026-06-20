import type { SessionMobilityDetails, SessionRecord } from "../../types";
import { apiClient } from "./client";

export type { SessionMobilityDetails, SessionRecord } from "../../types";

export const SessionAPI = {
  async list(): Promise<SessionRecord[]> {
    const res = await apiClient.get("/sessions");
    return Array.isArray(res.data?.data) ? res.data.data : [];
  },
  async get(id: string): Promise<SessionRecord> {
    const res = await apiClient.get(`/sessions/${encodeURIComponent(id)}`);
    return res.data.data;
  },
  async getMobility(id: string): Promise<SessionMobilityDetails> {
    const res = await apiClient.get(
      `/sessions/${encodeURIComponent(id)}/mobility`,
    );
    return res.data.data;
  },
  async updateComment(id: string, comment: string): Promise<SessionRecord> {
    const res = await apiClient.patch(
      `/sessions/${encodeURIComponent(id)}/comment`,
      { comment },
    );
    return res.data.data;
  },
  async kick(id: string): Promise<void> {
    await apiClient.delete(`/sessions/${encodeURIComponent(id)}`);
  },
};
