import type {
  TerminalAttachmentRecord,
  TerminalOutputChunk,
  TerminalRuntimeStatus,
  TerminalSessionRecord,
  TerminalTmuxInstallState,
} from "../../types";
import { apiClient } from "./client";

export type {
  TerminalAttachmentRecord,
  TerminalFeatureConfig,
  TerminalOutputChunk,
  TerminalRuntimeStatus,
  TerminalSessionRecord,
  TerminalTmuxInstallState,
} from "../../types";

export const TerminalAPI = {
  async getStatus(): Promise<TerminalRuntimeStatus> {
    const res = await apiClient.get("/terminal/status");
    return res.data.data;
  },
  async installTmux(): Promise<TerminalTmuxInstallState> {
    const res = await apiClient.post("/terminal/tmux/install");
    return res.data.data;
  },
  async listSessions(): Promise<TerminalSessionRecord[]> {
    const res = await apiClient.get("/terminal/sessions");
    return res.data.data;
  },
  async getSession(id: string): Promise<TerminalSessionRecord> {
    const res = await apiClient.get(
      `/terminal/sessions/${encodeURIComponent(id)}`,
    );
    return res.data.data;
  },
  async createSession(payload: {
    title?: string;
    shell?: string;
    cwd?: string;
    cols?: number;
    rows?: number;
  }): Promise<TerminalSessionRecord> {
    const res = await apiClient.post("/terminal/sessions", payload);
    return res.data.data;
  },
  async updateSessionTitle(
    id: string,
    title: string,
  ): Promise<TerminalSessionRecord> {
    const res = await apiClient.patch(
      `/terminal/sessions/${encodeURIComponent(id)}`,
      { title },
    );
    return res.data.data;
  },
  async deleteSession(id: string): Promise<void> {
    await apiClient.delete(`/terminal/sessions/${encodeURIComponent(id)}`);
  },
  async createAttachment(sessionId: string): Promise<TerminalAttachmentRecord> {
    const res = await apiClient.post(
      `/terminal/sessions/${encodeURIComponent(sessionId)}/attachments`,
    );
    return res.data.data;
  },
  async pollAttachment(
    attachmentId: string,
    params: { cursor?: number; timeout_ms?: number } = {},
  ): Promise<{ changed: boolean; chunk: TerminalOutputChunk | null }> {
    const res = await apiClient.get(
      `/terminal/attachments/${encodeURIComponent(attachmentId)}/poll`,
      { params },
    );
    return res.data.data;
  },
  async sendInput(attachmentId: string, dataBase64: string): Promise<void> {
    await apiClient.post(
      `/terminal/attachments/${encodeURIComponent(attachmentId)}/input`,
      { dataBase64 },
    );
  },
  async resizeAttachment(
    attachmentId: string,
    cols: number,
    rows: number,
  ): Promise<TerminalSessionRecord> {
    const res = await apiClient.post(
      `/terminal/attachments/${encodeURIComponent(attachmentId)}/resize`,
      { cols, rows },
    );
    return res.data.data;
  },
  async detachAttachment(attachmentId: string): Promise<void> {
    await apiClient.delete(
      `/terminal/attachments/${encodeURIComponent(attachmentId)}`,
    );
  },
};
