import type { DeploymentTarget } from "../types";

export const protectedAdminPanelDeploymentTargets = [
  "docker",
  "openwrt",
  "linux",
  "macos",
  "windows",
] as const satisfies ReadonlyArray<DeploymentTarget>;

export type ProtectedAdminPanelDeploymentTarget =
  (typeof protectedAdminPanelDeploymentTargets)[number];

export const isProtectedAdminPanelDeploymentTarget = (
  target?: DeploymentTarget,
): target is ProtectedAdminPanelDeploymentTarget =>
  protectedAdminPanelDeploymentTargets.some(
    (candidate) => candidate === target,
  );
