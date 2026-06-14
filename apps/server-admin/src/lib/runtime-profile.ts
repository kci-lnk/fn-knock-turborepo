import { existsSync, readFileSync } from "node:fs";
import { tWithFallback } from "./i18n";
import {
  DEFAULT_LOCALE,
  type LocaleCode,
} from "../../../../packages/i18n/src";

export type DeploymentTarget = "fpk" | "docker" | "dev";

export interface RuntimeProfile {
  deployment_target: DeploymentTarget;
  is_docker: boolean;
  is_linux: boolean;
  is_root_process: boolean;
}

export interface RuntimeCapabilities {
  direct_mode_available: boolean;
  host_firewall_available: boolean;
  smart_connect_available: boolean;
  system_clock_sync_available: boolean;
  self_update_available: boolean;
  terminal_available: boolean;
  shared_root_available: boolean;
}

export type RuntimeCapabilityKey = keyof RuntimeCapabilities;

let cachedRuntimeProfile: RuntimeProfile | null = null;

const normalizeDeploymentTarget = (
  value: string | undefined,
): DeploymentTarget | null => {
  const normalized = value?.trim().toLowerCase();
  if (normalized === "docker") return "docker";
  if (normalized === "fpk") return "fpk";
  if (normalized === "dev" || normalized === "development") return "dev";
  return null;
};

const detectDockerByCgroup = (): boolean => {
  try {
    const cgroup = readFileSync("/proc/1/cgroup", "utf-8");
    return /(docker|containerd|kubepods|podman)/i.test(cgroup);
  } catch {
    return false;
  }
};

const detectDeploymentTarget = (): DeploymentTarget => {
  const explicitTarget = normalizeDeploymentTarget(
    process.env.FN_KNOCK_RUNTIME_TARGET,
  );
  if (explicitTarget) {
    return explicitTarget;
  }

  if (existsSync("/.dockerenv") || detectDockerByCgroup()) {
    return "docker";
  }

  if (
    process.env.TRIM_APPDEST ||
    process.env.TRIM_PKGVAR ||
    process.env.TRIM_SERVICE_PORT
  ) {
    return "fpk";
  }

  return "dev";
};

const isRootProcess = (): boolean => {
  if (typeof process.getuid !== "function") {
    return false;
  }

  try {
    return process.getuid() === 0;
  } catch {
    return false;
  }
};

const hasSharedRoot = (): boolean => {
  const candidates = [
    process.env.FN_KNOCK_ROOT_SHARE_DIR,
    process.env.FN_KNOCK_CERT_SHARE_DIR,
  ]
    .map((value) => value?.trim() || "")
    .filter(Boolean);

  return candidates.some((candidate) => existsSync(candidate));
};

export const getRuntimeProfile = (): RuntimeProfile => {
  if (cachedRuntimeProfile) {
    return cachedRuntimeProfile;
  }

  const deploymentTarget = detectDeploymentTarget();
  cachedRuntimeProfile = {
    deployment_target: deploymentTarget,
    is_docker: deploymentTarget === "docker",
    is_linux: process.platform === "linux",
    is_root_process: isRootProcess(),
  };

  return cachedRuntimeProfile;
};

export const getRuntimeCapabilities = (
  profile: RuntimeProfile = getRuntimeProfile(),
): RuntimeCapabilities => {
  const hostRuntimeAvailable =
    profile.deployment_target !== "docker" &&
    profile.is_linux &&
    profile.is_root_process;

  return {
    direct_mode_available: hostRuntimeAvailable,
    host_firewall_available: hostRuntimeAvailable,
    smart_connect_available: hostRuntimeAvailable,
    system_clock_sync_available: hostRuntimeAvailable,
    self_update_available: profile.deployment_target === "fpk",
    terminal_available: profile.deployment_target !== "docker",
    shared_root_available: hasSharedRoot(),
  };
};

export const getCapabilityUnavailableMessage = (
  capability: RuntimeCapabilityKey,
  profile: RuntimeProfile = getRuntimeProfile(),
  locale: LocaleCode = DEFAULT_LOCALE,
): string => {
  const message = (reason: string, fallback: string) =>
    tWithFallback(
      locale,
      `server.runtimeProfile.capabilities.${capability}.${reason}`,
      fallback,
    );

  switch (capability) {
    case "direct_mode_available":
      if (profile.is_docker) {
        return message(
          "docker",
          "Docker deployments do not support host direct firewall mode",
        );
      }
      if (!profile.is_linux) {
        return message(
          "platform",
          "The current runtime does not support host direct firewall mode",
        );
      }
      return message(
        "permission",
        "The current process does not have host direct firewall capability",
      );
    case "host_firewall_available":
      if (profile.is_docker) {
        return message(
          "docker",
          "Docker deployments do not support host firewall management",
        );
      }
      if (!profile.is_linux) {
        return message(
          "platform",
          "The current runtime does not support host firewall management",
        );
      }
      return message(
        "permission",
        "The current process does not have host firewall management capability",
      );
    case "smart_connect_available":
      if (profile.is_docker) {
        return message(
          "docker",
          "Docker deployments do not support Smart Connect yet. It depends on host dnsmasq and port 53",
        );
      }
      if (!profile.is_linux) {
        return message(
          "platform",
          "The current runtime does not support Smart Connect yet",
        );
      }
      return message(
        "permission",
        "The current process does not have the host management capability required by Smart Connect",
      );
    case "system_clock_sync_available":
      if (profile.is_docker) {
        return message(
          "docker",
          "Docker deployments do not support host system time sync",
        );
      }
      if (!profile.is_linux) {
        return message(
          "platform",
          "The current runtime does not support system time sync",
        );
      }
      return message(
        "permission",
        "The current process does not have the host permission required for system time sync",
      );
    case "self_update_available":
      if (profile.is_docker) {
        return message(
          "docker",
          "Docker deployments do not support in-app FPK updates. Upgrade by pulling a new image",
        );
      }
      return message(
        "deployment",
        "The current deployment type does not support in-app updates",
      );
    case "terminal_available":
      if (profile.is_docker) {
        return message("docker", "Docker deployments do not support Web terminal");
      }
      return message(
        "platform",
        "The current runtime does not support Web terminal",
      );
    case "shared_root_available":
      return message(
        "missing",
        "No shared directory mount is available in the current runtime",
      );
    default:
      return tWithFallback(
        locale,
        "server.runtimeProfile.capabilities.default",
        "The current runtime does not support this capability",
      );
  }
};
