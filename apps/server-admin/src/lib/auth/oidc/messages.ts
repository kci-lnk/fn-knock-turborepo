import { tDefault } from "../../i18n";

export const oidcT = (
  key: string,
  params?: Record<string, string | number | boolean | null | undefined>,
): string => tDefault(`server.oidc.${key}`, params);

export const OIDC_CALLBACK_STATE_EXPIRED_MESSAGE =
  oidcT("callbackStateExpired");
