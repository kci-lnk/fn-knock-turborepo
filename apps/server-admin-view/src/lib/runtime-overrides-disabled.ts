import type {
  DockerAdminBootstrapState,
  RuntimeCapabilities,
  RuntimeProfile,
} from "../types";

export type DockerAdminDebugStage = "setup" | "login" | "authenticated";

export const readDockerAdminDebugStage = (): DockerAdminDebugStage | null =>
  null;

export const writeDockerAdminDebugStage = (
  _stage: DockerAdminDebugStage | null,
) => {};

export const readDockerAdminDebugPassword = () => "";

export const writeDockerAdminDebugPassword = (_password: string | null) => {};

export const validateDockerAdminDebugPassword = (_password: string) => null;

export const createDockerAdminDebugState = (
  _stage: DockerAdminDebugStage,
  locale?: DockerAdminBootstrapState["locale"],
  appearance?: DockerAdminBootstrapState["appearance"],
  _rememberMe = false,
): DockerAdminBootstrapState => ({
  deployment_target: "fpk-lite",
  enabled: false,
  password_configured: false,
  authenticated: true,
  auth_source: null,
  session_expires_at: null,
  locale: locale ?? { default_locale: "zh-CN" },
  appearance: appearance ?? { theme_color_preset: "default" },
});

export const buildDockerAdminDebugState = (
  _backendState: DockerAdminBootstrapState,
): DockerAdminBootstrapState | null => null;

export const getEffectiveRuntimeProfile = (
  profile?: RuntimeProfile,
): RuntimeProfile | undefined => profile;

export const getEffectiveRuntimeCapabilities = (
  capabilities?: RuntimeCapabilities,
): RuntimeCapabilities | undefined => capabilities;
