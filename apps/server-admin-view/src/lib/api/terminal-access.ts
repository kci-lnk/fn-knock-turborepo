import type { components } from "@fn-knock/api-contract";
import { apiClient } from "./client";

type Schemas = components["schemas"];
export type WebTerminalSettings = Schemas["WebTerminalSettings"];
export type WebTerminalSettingsInput = Schemas["WebTerminalSettingsInput"];
export type WebTerminalAccessStatus = Schemas["WebTerminalAccessStatus"];

export const TerminalAccessAPI = {
  async settings(): Promise<WebTerminalSettings> {
    return (await apiClient.get("/terminal/settings")).data.data;
  },
  async update(input: WebTerminalSettingsInput): Promise<WebTerminalSettings> {
    return (await apiClient.patch("/terminal/settings", input)).data.data;
  },
  async status(): Promise<WebTerminalAccessStatus> {
    return (await apiClient.get("/terminal/access")).data.data;
  },
  async verify(password: string): Promise<void> {
    await apiClient.post("/terminal/access/verify", { password });
  },
};

export function terminalAccessErrorKey(error: unknown): string {
  const code = (error as { response?: { data?: { errorCode?: string } } })
    ?.response?.data?.errorCode;
  if (code === "access_rate_limited")
    return "admin.webTerminalSettings.rateLimited";
  if (code === "access_password_required")
    return "admin.webTerminalSettings.wrongPassword";
  if (code === "feature_disabled") return "admin.webTerminalSettings.disabled";
  if (code === "conflict") return "admin.webTerminalSettings.conflict";
  return "admin.webTerminalSettings.requestFailed";
}
