import { DEFAULT_LOCALE, type LocaleConfig } from "@fn-knock/i18n/core";
import { browserT } from "@fn-knock/i18n/vue/admin";
import type {
  DockerAdminBootstrapState,
  RuntimeCapabilities,
  RuntimeProfile,
} from "../types";

export const DOCKER_MODE_DEBUG_STORAGE_KEY = "fn_knock:debug:docker-mode";
export const DOCKER_ADMIN_STAGE_DEBUG_STORAGE_KEY =
  "fn_knock:debug:docker-admin-stage";
export const DOCKER_ADMIN_PASSWORD_DEBUG_STORAGE_KEY =
  "fn_knock:debug:docker-admin-password";

export type DockerAdminDebugStage = "setup" | "login" | "authenticated";

const DEBUG_SESSION_TTL_MS = 12 * 60 * 60 * 1000;
const DEBUG_REMEMBER_ME_SESSION_TTL_MS = 30 * 24 * 60 * 60 * 1000;
const DEFAULT_DEBUG_LOCALE_CONFIG: LocaleConfig = {
  default_locale: DEFAULT_LOCALE,
};

const isBrowser = () => typeof window !== "undefined";

const readStorageValue = (key: string): string => {
  if (!isBrowser()) return "";
  return window.localStorage.getItem(key)?.trim() || "";
};

const writeStorageValue = (key: string, value: string | null) => {
  if (!isBrowser()) return;

  if (!value) {
    window.localStorage.removeItem(key);
    return;
  }

  window.localStorage.setItem(key, value);
};

const isTruthyDebugValue = (value: string) => {
  const normalized = value.trim().toLowerCase();
  return (
    normalized === "1" ||
    normalized === "true" ||
    normalized === "on" ||
    normalized === "yes" ||
    normalized === "docker"
  );
};

const createAuthenticatedSessionExpiry = (rememberMe = false) =>
  new Date(
    Date.now() +
      (rememberMe ? DEBUG_REMEMBER_ME_SESSION_TTL_MS : DEBUG_SESSION_TTL_MS),
  ).toISOString();

export const isDockerModeDebugEnabled = () =>
  isTruthyDebugValue(readStorageValue(DOCKER_MODE_DEBUG_STORAGE_KEY));

export const readDockerAdminDebugStage = (): DockerAdminDebugStage | null => {
  if (!isDockerModeDebugEnabled()) {
    return null;
  }

  const raw = readStorageValue(DOCKER_ADMIN_STAGE_DEBUG_STORAGE_KEY)
    .toLowerCase();

  if (
    raw === "setup" ||
    raw === "login" ||
    raw === "authenticated"
  ) {
    return raw;
  }

  return "setup";
};

export const writeDockerAdminDebugStage = (
  stage: DockerAdminDebugStage | null,
) => {
  writeStorageValue(DOCKER_ADMIN_STAGE_DEBUG_STORAGE_KEY, stage);
};

export const readDockerAdminDebugPassword = () =>
  readStorageValue(DOCKER_ADMIN_PASSWORD_DEBUG_STORAGE_KEY);

export const writeDockerAdminDebugPassword = (password: string | null) => {
  writeStorageValue(
    DOCKER_ADMIN_PASSWORD_DEBUG_STORAGE_KEY,
    password?.trim() || null,
  );
};

export const validateDockerAdminDebugPassword = (
  password: string,
): string | null => {
  if (password.length < 6) {
    return browserT("admin.dockerAdmin.passwordMin");
  }
  if (password.length > 128) {
    return browserT("admin.dockerAdmin.passwordMax");
  }
  if (/\s/.test(password)) {
    return browserT("admin.dockerAdmin.passwordNoWhitespace");
  }
  if (!/[A-Za-z]/.test(password) || !/\d/.test(password)) {
    return browserT("admin.dockerAdmin.passwordRequireLetterNumber");
  }

  return null;
};

export const createDockerAdminDebugState = (
  stage: DockerAdminDebugStage,
  locale: LocaleConfig = DEFAULT_DEBUG_LOCALE_CONFIG,
  rememberMe = false,
): DockerAdminBootstrapState => {
  if (stage === "setup") {
    return {
      deployment_target: "docker",
      enabled: true,
      password_configured: false,
      authenticated: false,
      session_expires_at: null,
      locale,
    };
  }

  if (stage === "login") {
    return {
      deployment_target: "docker",
      enabled: true,
      password_configured: true,
      authenticated: false,
      session_expires_at: null,
      locale,
    };
  }

  return {
    deployment_target: "docker",
    enabled: true,
    password_configured: true,
    authenticated: true,
    session_expires_at: createAuthenticatedSessionExpiry(rememberMe),
    locale,
  };
};

export const buildDockerAdminDebugState = (
  backendState: DockerAdminBootstrapState,
): DockerAdminBootstrapState | null => {
  if (!isDockerModeDebugEnabled() || backendState.enabled) {
    return null;
  }

  const stage = readDockerAdminDebugStage();
  if (!stage) {
    return null;
  }

  return createDockerAdminDebugState(stage, backendState.locale);
};

export const getEffectiveRuntimeProfile = (
  profile?: RuntimeProfile,
): RuntimeProfile | undefined => {
  if (!isDockerModeDebugEnabled()) {
    return profile;
  }

  return {
    deployment_target: "docker",
    is_docker: true,
    is_linux: profile?.is_linux ?? true,
    is_root_process: profile?.is_root_process ?? false,
  };
};

export const getEffectiveRuntimeCapabilities = (
  capabilities?: RuntimeCapabilities,
): RuntimeCapabilities | undefined => {
  if (!isDockerModeDebugEnabled()) {
    return capabilities;
  }

  return {
    direct_mode_available: false,
    host_firewall_available: false,
    smart_connect_available: false,
    system_clock_sync_available: false,
    self_update_available: false,
    terminal_available: false,
    shared_root_available: capabilities?.shared_root_available ?? false,
  };
};
