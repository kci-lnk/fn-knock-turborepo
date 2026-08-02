import type { DeploymentTarget } from "../types";
import {
  isProtectedAdminPanelDeploymentTarget,
  type ProtectedAdminPanelDeploymentTarget,
} from "./admin-panel-runtime";

export const dockerAdminPanelResetCommands = {
  ssh: "ssh root@<docker-host>",
  compose:
    "cd /opt/fn-knock-docker && docker compose exec -T fn-knock fn-knock-reset-panel-password",
  dockerExec:
    "docker exec -it \"$(docker ps --filter label=com.docker.compose.service=fn-knock --format '{{.Names}}' | head -n 1)\" fn-knock-reset-panel-password",
} as const;

export const openWrtAdminPanelResetCommands = {
  ssh: "ssh root@<openwrt-host>",
  reset: "fn-knock-reset-panel-password",
} as const;

export const linuxAdminPanelResetCommands = {
  reset: "sudo knock reset-panel-password",
} as const;

export const windowsAdminPanelResetCommands = {
  reset:
    '& "$env:ProgramFiles\\Knock 敲门\\fn-knock-service.exe" reset-panel-password',
} as const;

type AdminPanelResetDescriptionKey =
  | "admin.components.dockerAdminGate.resetDescription"
  | "admin.components.dockerAdminGate.resetDescriptionDevice"
  | "admin.components.dockerAdminGate.resetDescriptionWindows";

type AdminPanelResetStepLabelKey =
  | "admin.components.dockerAdminGate.resetStepSsh"
  | "admin.components.dockerAdminGate.resetStepCompose"
  | "admin.components.dockerAdminGate.resetStepDockerExec"
  | "admin.components.dockerAdminGate.resetStepOpenWrtSsh"
  | "admin.components.dockerAdminGate.resetStepOpenWrtCommand"
  | "admin.components.dockerAdminGate.resetStepLinux"
  | "admin.components.dockerAdminGate.resetStepWindows";

export type AdminPanelResetGuide = {
  descriptionKey: AdminPanelResetDescriptionKey;
  steps: ReadonlyArray<{
    labelKey: AdminPanelResetStepLabelKey;
    command: string;
  }>;
};

const resetGuides = {
  docker: {
    descriptionKey: "admin.components.dockerAdminGate.resetDescription",
    steps: [
      {
        labelKey: "admin.components.dockerAdminGate.resetStepSsh",
        command: dockerAdminPanelResetCommands.ssh,
      },
      {
        labelKey: "admin.components.dockerAdminGate.resetStepCompose",
        command: dockerAdminPanelResetCommands.compose,
      },
      {
        labelKey: "admin.components.dockerAdminGate.resetStepDockerExec",
        command: dockerAdminPanelResetCommands.dockerExec,
      },
    ],
  },
  openwrt: {
    descriptionKey: "admin.components.dockerAdminGate.resetDescriptionDevice",
    steps: [
      {
        labelKey: "admin.components.dockerAdminGate.resetStepOpenWrtSsh",
        command: openWrtAdminPanelResetCommands.ssh,
      },
      {
        labelKey: "admin.components.dockerAdminGate.resetStepOpenWrtCommand",
        command: openWrtAdminPanelResetCommands.reset,
      },
    ],
  },
  linux: {
    descriptionKey: "admin.components.dockerAdminGate.resetDescriptionDevice",
    steps: [
      {
        labelKey: "admin.components.dockerAdminGate.resetStepLinux",
        command: linuxAdminPanelResetCommands.reset,
      },
    ],
  },
  windows: {
    descriptionKey: "admin.components.dockerAdminGate.resetDescriptionWindows",
    steps: [
      {
        labelKey: "admin.components.dockerAdminGate.resetStepWindows",
        command: windowsAdminPanelResetCommands.reset,
      },
    ],
  },
} as const satisfies Record<
  ProtectedAdminPanelDeploymentTarget,
  AdminPanelResetGuide
>;

export const resolveAdminPanelResetGuide = (
  deploymentTarget?: DeploymentTarget,
): AdminPanelResetGuide | null => {
  if (!isProtectedAdminPanelDeploymentTarget(deploymentTarget)) {
    return null;
  }
  return resetGuides[deploymentTarget];
};
