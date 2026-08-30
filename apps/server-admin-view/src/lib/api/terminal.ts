import type { components as ApiContractComponents } from "@fn-knock/api-contract";
import { apiClient } from "./client";

type TerminalSchemas = ApiContractComponents["schemas"];
type TerminalWireEvent = TerminalSchemas["TerminalEvent"];

export type TerminalAuthMethod = TerminalSchemas["AuthMethod"];
export type TerminalSessionPhase = TerminalSchemas["SessionPhase"];
export type TerminalAttachmentRole = TerminalSchemas["AttachmentRole"];
export type TerminalErrorCode = TerminalSchemas["TerminalErrorCode"];
export type TerminalErrorEnvelope = TerminalSchemas["TerminalErrorEnvelope"];
export type TerminalTrustedHostKey = TerminalSchemas["TrustedHostKey"];
export type TerminalTargetRecord = TerminalSchemas["TerminalTarget"];
export type TerminalSecretAction = TerminalSchemas["SecretAction"];
export type TerminalCredentialMutation = TerminalSchemas["CredentialMutation"];
export type TerminalPassphraseMutation = TerminalSchemas["PassphraseMutation"];
export type TerminalTargetCreateInput = TerminalSchemas["TargetCreateInput"];
export type TerminalTargetUpdateInput = TerminalSchemas["TargetUpdateInput"];
export type TerminalTargetDraftInput = TerminalSchemas["TargetDraft"];
export type TerminalHostKeyProbeInput = TerminalSchemas["ProbeHostKeyInput"];
export type TerminalHostKeyProbeResult = TerminalSchemas["HostKeyProbeResult"];
export type TerminalConnectionTestInput =
  TerminalSchemas["TerminalTestConnectionInput"];
export type TerminalConnectionTestResult =
  TerminalSchemas["ConnectionTestResult"];
export type TerminalLocalStatus = TerminalSchemas["LocalTerminalStatus"];
export type TerminalLocalSettingsInput =
  TerminalSchemas["LocalTerminalSettingsInput"];
export type TerminalSessionBackend = TerminalSchemas["SessionBackend"];
export type TerminalSessionRecord = TerminalSchemas["TerminalSession"];
export type TerminalSessionListResult = TerminalSchemas["SessionListResult"];
export type TerminalAttachmentRecord = TerminalSchemas["TerminalAttachment"];

export type TerminalOutputEvent = Pick<
  TerminalWireEvent,
  "cursor" | "dataBase64" | "reset"
> & {
  type: "output";
};

export type TerminalSessionStateEvent = Pick<
  TerminalWireEvent,
  "cursor" | "errorCode" | "errorMessage" | "exitCode"
> & {
  type: "status";
  phase: TerminalSessionPhase;
};

export type TerminalControlEvent = Pick<TerminalWireEvent, "cursor"> & {
  type: "control";
  role: TerminalAttachmentRole;
  generation: number;
};

export type TerminalEvent =
  TerminalOutputEvent | TerminalSessionStateEvent | TerminalControlEvent;

export type TerminalEventsResult = Omit<
  TerminalSchemas["EventsResult"],
  "events"
> & { events: TerminalEvent[] };

export type TerminalCreateSessionInput = TerminalSchemas["CreateSessionInput"];
export type TerminalAttachmentInput = TerminalSchemas["CreateAttachmentInput"];

export interface TerminalAttachmentEventsQuery {
  after?: number;
  timeoutMs?: number;
}

export type TerminalInputBody = TerminalSchemas["InputRequest"];
export type TerminalResizeBody = TerminalSchemas["ResizeRequest"];
export type TerminalClaimControlBody = TerminalSchemas["ClaimControlRequest"];
export type TerminalRenameSessionInput = TerminalSchemas["RenameSessionInput"];

export type TerminalSshDestination = TerminalTargetRecord & { kind: "ssh" };
export type TerminalLocalDestination = TerminalLocalStatus & {
  id: "local";
  kind: "local";
  name: "Local";
};
export type TerminalDestination =
  TerminalLocalDestination | TerminalSshDestination;

const targetPath = (id: string) =>
  `/terminal/targets/${encodeURIComponent(id)}`;
const sessionPath = (id: string) =>
  `/terminal/sessions/${encodeURIComponent(id)}`;
const attachmentPath = (id: string) =>
  `/terminal/attachments/${encodeURIComponent(id)}`;

export const TerminalAPI = {
  async getLocalStatus(signal?: AbortSignal): Promise<TerminalLocalStatus> {
    const response = await apiClient.get("/terminal/local", { signal });
    return response.data.data;
  },

  async updateLocalStatus(
    payload: TerminalLocalSettingsInput,
    force = false,
    confirmationToken?: string,
    signal?: AbortSignal,
  ): Promise<TerminalLocalStatus> {
    const response = await apiClient.patch("/terminal/local", payload, {
      params: {
        force,
        ...(confirmationToken ? { confirmationToken } : {}),
      },
      signal,
    });
    return response.data.data;
  },

  async listTargets(signal?: AbortSignal): Promise<TerminalTargetRecord[]> {
    const response = await apiClient.get("/terminal/targets", { signal });
    return response.data.data;
  },

  async getTarget(
    id: string,
    signal?: AbortSignal,
  ): Promise<TerminalTargetRecord> {
    const response = await apiClient.get(targetPath(id), { signal });
    return response.data.data;
  },

  async createTarget(
    payload: TerminalTargetCreateInput,
    signal?: AbortSignal,
  ): Promise<TerminalTargetRecord> {
    const response = await apiClient.post("/terminal/targets", payload, {
      signal,
    });
    return response.data.data;
  },

  async updateTarget(
    id: string,
    payload: TerminalTargetUpdateInput,
    force = false,
    confirmationToken?: string,
    signal?: AbortSignal,
  ): Promise<TerminalTargetRecord> {
    const response = await apiClient.patch(targetPath(id), payload, {
      params: {
        force,
        ...(confirmationToken ? { confirmationToken } : {}),
      },
      signal,
    });
    return response.data.data;
  },

  async deleteTarget(
    id: string,
    revision: number,
    terminateActiveSessions = false,
    confirmationToken?: string,
    signal?: AbortSignal,
  ): Promise<void> {
    await apiClient.delete(targetPath(id), {
      params: {
        force: terminateActiveSessions,
        revision,
        ...(confirmationToken ? { confirmationToken } : {}),
      },
      signal,
    });
  },

  async probeHostKey(
    payload: TerminalHostKeyProbeInput,
    signal?: AbortSignal,
  ): Promise<TerminalHostKeyProbeResult> {
    const response = await apiClient.post(
      "/terminal/targets/probe-host-key",
      payload,
      { signal },
    );
    return response.data.data;
  },

  async testConnection(
    payload: TerminalConnectionTestInput,
    signal?: AbortSignal,
  ): Promise<TerminalConnectionTestResult> {
    const response = await apiClient.post(
      "/terminal/targets/test-connection",
      payload,
      { signal },
    );
    return response.data.data;
  },

  async listSessions(signal?: AbortSignal): Promise<TerminalSessionListResult> {
    const response = await apiClient.get("/terminal/sessions", { signal });
    return response.data.data;
  },

  async createSession(
    targetId: string,
    payload: TerminalCreateSessionInput,
    signal?: AbortSignal,
  ): Promise<TerminalSessionRecord> {
    if (targetId === "local") {
      const response = await apiClient.post(
        "/terminal/local/sessions",
        payload,
        { signal },
      );
      return response.data.data;
    }
    const response = await apiClient.post(
      `${targetPath(targetId)}/sessions`,
      payload,
      { signal },
    );
    return response.data.data;
  },

  async createLocalSession(
    payload: TerminalCreateSessionInput,
    signal?: AbortSignal,
  ): Promise<TerminalSessionRecord> {
    const response = await apiClient.post("/terminal/local/sessions", payload, {
      signal,
    });
    return response.data.data;
  },

  async updateSessionTitle(
    id: string,
    title: string,
    signal?: AbortSignal,
  ): Promise<TerminalSessionRecord> {
    const payload: TerminalRenameSessionInput = { title };
    const response = await apiClient.patch(sessionPath(id), payload, {
      signal,
    });
    return response.data.data;
  },

  async deleteSession(id: string, signal?: AbortSignal): Promise<void> {
    await apiClient.delete(sessionPath(id), { signal });
  },

  async createAttachment(
    sessionId: string,
    payload: TerminalAttachmentInput,
    signal?: AbortSignal,
  ): Promise<TerminalAttachmentRecord> {
    const response = await apiClient.post(
      `${sessionPath(sessionId)}/attachments`,
      payload,
      { signal },
    );
    return response.data.data;
  },

  async pollAttachmentEvents(
    attachmentId: string,
    params: TerminalAttachmentEventsQuery = {},
    signal?: AbortSignal,
  ): Promise<TerminalEventsResult> {
    const response = await apiClient.get(
      `${attachmentPath(attachmentId)}/events`,
      { params, signal },
    );
    return response.data.data;
  },

  async sendInput(
    attachmentId: string,
    payload: TerminalInputBody,
    signal?: AbortSignal,
  ): Promise<void> {
    await apiClient.post(`${attachmentPath(attachmentId)}/input`, payload, {
      signal,
    });
  },

  async resizeAttachment(
    attachmentId: string,
    payload: TerminalResizeBody,
    signal?: AbortSignal,
  ): Promise<void> {
    await apiClient.post(`${attachmentPath(attachmentId)}/resize`, payload, {
      signal,
    });
  },

  async claimControl(
    attachmentId: string,
    generation?: number,
    signal?: AbortSignal,
  ): Promise<TerminalAttachmentRecord> {
    const payload: TerminalClaimControlBody =
      generation === undefined ? {} : { generation };
    const response = await apiClient.post(
      `${attachmentPath(attachmentId)}/control`,
      payload,
      { signal },
    );
    return response.data.data;
  },

  async detachAttachment(
    attachmentId: string,
    signal?: AbortSignal,
  ): Promise<void> {
    await apiClient.delete(attachmentPath(attachmentId), { signal });
  },
};
