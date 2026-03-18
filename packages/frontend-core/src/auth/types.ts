import type { CaptchaPublicSettings } from "../captcha/types";

export type AuthClientInfo = {
  ip: string;
};

export type AuthClientLocationStatus =
  | "idle"
  | "queued"
  | "processing"
  | "success"
  | "failed"
  | "skipped";

export type AuthClientLocationData = {
  ip: string;
  location: string;
  status: AuthClientLocationStatus;
  attempts: number;
  maxAttempts: number;
  error?: string;
};

export type AuthAccessState = {
  authenticated: boolean;
  message: string;
};

export type AuthPasskeyState = {
  available: boolean;
  mode?: "auth_host" | "parent_domain";
  rp_id?: string;
};

export type AuthBootstrapData = {
  auth: AuthAccessState;
  client: AuthClientInfo;
  captcha: CaptchaPublicSettings;
  passkey: AuthPasskeyState;
  redirect_to?: string;
};

export type AuthSessionData = {
  auth: AuthAccessState;
  client: AuthClientInfo;
  passkey: AuthPasskeyState;
};
