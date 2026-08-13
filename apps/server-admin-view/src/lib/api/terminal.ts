import type {
  TerminalAttachmentRecord,
  TerminalRuntimeStatus,
  TerminalSessionRecord,
  TerminalTmuxInstallState,
} from "../../types";
import type {
  components as ApiContractComponents,
  operations as ApiContractOperations,
} from "@fn-knock/api-contract";
import { apiClient } from "./client";

type TerminalSchemas = ApiContractComponents["schemas"];
type TerminalCreateSessionBody =
  TerminalSchemas["TerminalCreateSessionBodyData"];
type TerminalRenameSessionBody =
  TerminalSchemas["TerminalRenameSessionBodyData"];
type TerminalInputBody = TerminalSchemas["TerminalInputBodyData"];
type TerminalResizeBody = TerminalSchemas["TerminalResizeBodyData"];
type TerminalPollResult = TerminalSchemas["TerminalPollResultData"];
type TerminalPollQuery = NonNullable<
  ApiContractOperations["get_api_admin_terminal_attachments__id__poll"]["parameters"]["query"]
>;

export type {
  TerminalAttachmentRecord,
  TerminalFeatureConfig,
  TerminalOutputChunk,
  TerminalRuntimeStatus,
  TerminalSessionRecord,
  TerminalTmuxInstallState,
} from "../../types";

export const TerminalAPI = {
  async getStatus(signal?: AbortSignal): Promise<TerminalRuntimeStatus> {
    const res = await apiClient.get("/terminal/status", { signal });
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
  async createSession(
    payload: TerminalCreateSessionBody,
  ): Promise<TerminalSessionRecord> {
    const res = await apiClient.post("/terminal/sessions", payload);
    return res.data.data;
  },
  async updateSessionTitle(
    id: string,
    title: string,
  ): Promise<TerminalSessionRecord> {
    const body = { title } satisfies TerminalRenameSessionBody;
    const res = await apiClient.patch(
      `/terminal/sessions/${encodeURIComponent(id)}`,
      body,
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
    params: TerminalPollQuery = {},
  ): Promise<TerminalPollResult> {
    const res = await apiClient.get(
      `/terminal/attachments/${encodeURIComponent(attachmentId)}/poll`,
      { params },
    );
    return res.data.data;
  },
  async sendInput(attachmentId: string, dataBase64: string): Promise<void> {
    const body = { dataBase64 } satisfies TerminalInputBody;
    await apiClient.post(
      `/terminal/attachments/${encodeURIComponent(attachmentId)}/input`,
      body,
    );
  },
  async resizeAttachment(
    attachmentId: string,
    cols: number,
    rows: number,
  ): Promise<TerminalSessionRecord> {
    const body = { cols, rows } satisfies TerminalResizeBody;
    const res = await apiClient.post(
      `/terminal/attachments/${encodeURIComponent(attachmentId)}/resize`,
      body,
    );
    return res.data.data;
  },
  async detachAttachment(attachmentId: string): Promise<void> {
    await apiClient.delete(
      `/terminal/attachments/${encodeURIComponent(attachmentId)}`,
    );
  },
};
