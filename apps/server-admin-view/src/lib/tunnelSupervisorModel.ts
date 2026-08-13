import type { TunnelSupervisorStatus } from "@/lib/api/tunnel";

export type TunnelSupervisorTone = "success" | "info" | "warning" | "muted";

export const supervisorTone = (
  supervisor: Pick<TunnelSupervisorStatus, "state">,
): TunnelSupervisorTone => {
  switch (supervisor.state) {
    case "running":
      return "success";
    case "starting":
      return "info";
    case "backoff":
      return "warning";
    default:
      return "muted";
  }
};

export const supervisorAllowsStart = (
  supervisor: Pick<TunnelSupervisorStatus, "desiredRunning" | "running">,
) => !supervisor.desiredRunning && !supervisor.running;

export const supervisorAllowsStop = (
  supervisor: Pick<TunnelSupervisorStatus, "desiredRunning" | "running">,
) => supervisor.desiredRunning || supervisor.running;
