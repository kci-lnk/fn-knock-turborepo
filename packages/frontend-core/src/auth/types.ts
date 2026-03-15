import type { CaptchaPublicSettings } from "../captcha/types";

export type AuthClientInfo = {
  ip: string;
  location: string;
};

export type AuthAccessState = {
  authenticated: boolean;
  message: string;
};

export type AuthPasskeyState = {
  available: boolean;
};

export type AuthBootstrapData = {
  auth: AuthAccessState;
  client: AuthClientInfo;
  captcha: CaptchaPublicSettings;
  passkey: AuthPasskeyState;
};

export type AuthSessionData = {
  auth: AuthAccessState;
  client: AuthClientInfo;
  passkey: AuthPasskeyState;
};
