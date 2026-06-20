import { tDefault } from "../i18n";

export const whitelistManagerT = (
  key: string,
  params?: Record<string, string | number | boolean | null | undefined>,
) => tDefault(`server.whitelistManager.${key}`, params);
