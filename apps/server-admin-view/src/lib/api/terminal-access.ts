import type { components } from "@fn-knock/api-contract";
import { apiClient } from "./client";

type Schemas = components["schemas"];
export type WebTerminalSettings = Schemas["WebTerminalSettings"];
export type WebTerminalSettingsInput = Schemas["WebTerminalSettingsInput"];

export const TerminalAccessAPI = {
  async settings(): Promise<WebTerminalSettings> {
    return (await apiClient.get("/terminal/settings")).data.data;
  },
  async update(input: WebTerminalSettingsInput): Promise<WebTerminalSettings> {
    return (await apiClient.patch("/terminal/settings", input)).data.data;
  },
};

export function terminalAccessErrorKey(error: unknown): string {
  const code = (error as { response?: { data?: { errorCode?: string } } })
    ?.response?.data?.errorCode;
  if (code === "feature_disabled") return "admin.webTerminalSettings.disabled";
  if (code === "conflict") return "admin.webTerminalSettings.conflict";
  return "admin.webTerminalSettings.requestFailed";
}
