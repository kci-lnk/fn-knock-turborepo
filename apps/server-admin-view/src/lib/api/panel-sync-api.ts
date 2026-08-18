import type { components as ApiContractComponents } from "@fn-knock/api-contract";

import { apiClient } from "./client";

type Schemas = ApiContractComponents["schemas"];

export type PanelProvider = Schemas["PanelProvider"];
export type PanelProviderDescriptor = Schemas["ProviderDescriptor"];
export type PanelConnection = Schemas["PanelConnection"];
export type PanelSyncPreview = Schemas["SyncPreview"];
export type PanelSyncRun = Schemas["SyncRun"];
export type PanelProbeResult = Schemas["ProbeResult"];
export type PanelConnectionInput = Omit<
  Schemas["ConnectionInput"],
  | "allow_invalid_tls"
  | "api_path"
  | "auto_sync"
  | "clear_credential"
  | "credential"
  | "grouping"
> & {
  allow_invalid_tls: boolean;
  api_path: string;
  auto_sync: Required<Schemas["AutoSyncConfig"]>;
  clear_credential: boolean;
  credential?: string;
  grouping: Required<Schemas["GroupingConfig"]>;
};
export type PanelConnectionUpdateInput = Omit<PanelConnectionInput, "provider">;

const unwrap = <T>(response: { data: { data: T } }): T => response.data.data;

export const PanelSyncAPI = {
  async providers(): Promise<PanelProviderDescriptor[]> {
    return unwrap(await apiClient.get("/panel-sync/providers"));
  },

  async connections(): Promise<PanelConnection[]> {
    return unwrap(await apiClient.get("/panel-sync/connections"));
  },

  async create(input: PanelConnectionInput): Promise<PanelConnection> {
    return unwrap(await apiClient.post("/panel-sync/connections", input));
  },

  async update(
    id: string,
    input: PanelConnectionUpdateInput,
  ): Promise<PanelConnection> {
    return unwrap(
      await apiClient.put(
        `/panel-sync/connections/${encodeURIComponent(id)}`,
        input,
      ),
    );
  },

  async remove(id: string, cleanupPreview?: PanelSyncPreview): Promise<void> {
    await apiClient.delete(
      `/panel-sync/connections/${encodeURIComponent(id)}`,
      {
        params: cleanupPreview
          ? {
              cleanup_remote: true,
              plan_hash: cleanupPreview.plan_hash,
              source_revision: cleanupPreview.source_revision,
            }
          : undefined,
      },
    );
  },

  async testSaved(id: string): Promise<PanelProbeResult> {
    return unwrap(
      await apiClient.post("/panel-sync/test", { connection_id: id }),
    );
  },

  async testDraft(
    draft: PanelConnectionInput,
    connectionId?: string,
  ): Promise<PanelProbeResult> {
    return unwrap(
      await apiClient.post("/panel-sync/test", {
        connection_id: connectionId,
        draft,
      }),
    );
  },

  async preview(id: string, cleanupRemote = false): Promise<PanelSyncPreview> {
    return unwrap(
      await apiClient.post(
        `/panel-sync/connections/${encodeURIComponent(id)}/preview`,
        { cleanup_remote: cleanupRemote },
      ),
    );
  },

  async sync(
    id: string,
    preview: PanelSyncPreview,
  ): Promise<Schemas["SyncAccepted"]> {
    return unwrap(
      await apiClient.post(
        `/panel-sync/connections/${encodeURIComponent(id)}/sync`,
        {
          source_revision: preview.source_revision,
          plan_hash: preview.plan_hash,
        },
      ),
    );
  },

  async runs(id: string): Promise<PanelSyncRun[]> {
    return unwrap(
      await apiClient.get(
        `/panel-sync/connections/${encodeURIComponent(id)}/runs`,
      ),
    );
  },

  async run(runId: string): Promise<PanelSyncRun> {
    return unwrap(
      await apiClient.get(`/panel-sync/runs/${encodeURIComponent(runId)}`),
    );
  },
};
