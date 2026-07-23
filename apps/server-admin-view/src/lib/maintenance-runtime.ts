import type { RuntimeCapabilities, RuntimeProfile } from "../types";

export const supportsSharedBackupForRuntime = (
  profile?: RuntimeProfile,
  capabilities?: RuntimeCapabilities,
): boolean => {
  const target = profile?.deployment_target;

  if (target === "docker" || target === "openwrt") {
    return false;
  }

  return (
    target === "fpk" ||
    target === "fpk-lite" ||
    capabilities?.self_update_available === true ||
    capabilities?.shared_root_available === true
  );
};
