import {
  goBackend,
  type ReverseProxyThrottleExemptIPsRuntime,
} from "./go-backend";
import { normalizeCidrLines } from "../../../../packages/admin-shared/src/utils/cidr";
import { isWhitelistExemptIp, normalizeIp } from "./ip-normalize";
import {
  configManager,
  type AppConfig,
  type ReverseProxyTrustedIPRuntimeState,
} from "./redis";
import { whitelistManager } from "./whitelist-manager";

const SYNC_DEBOUNCE_MS = 150;

let scheduledSyncTimer: ReturnType<typeof setTimeout> | null = null;
let scheduledSyncReason = "scheduled";
let syncInFlight: Promise<ReverseProxyTrustedIPRuntimeState> | null = null;
let rerunRequested = false;
let runtimeEndpointUnavailableLogged = false;

const addSourceForIp = (
  sourceMap: Map<string, Set<string>>,
  ip: string,
  source: string,
) => {
  const normalizedIp = normalizeIp(ip);
  if (!normalizedIp || isWhitelistExemptIp(normalizedIp)) {
    return;
  }

  const sources = sourceMap.get(normalizedIp) ?? new Set<string>();
  sources.add(source);
  sourceMap.set(normalizedIp, sources);
};

const toGatewayPayload = (
  runtime: ReverseProxyTrustedIPRuntimeState,
): ReverseProxyThrottleExemptIPsRuntime => ({
  enabled: runtime.enabled,
  ips: runtime.enabled ? runtime.items.map((item) => item.ip) : [],
  cidrs: runtime.enabled ? runtime.cidrs : [],
  updated_at: runtime.updated_at,
});

export const compileReverseProxyTrustedIPsRuntimeState = async (
  config?: Pick<AppConfig, "reverse_proxy_throttle">,
): Promise<ReverseProxyTrustedIPRuntimeState> => {
  const [reverseProxyThrottle, sessions, whitelistTargets, authSettings] =
    await Promise.all([
      config?.reverse_proxy_throttle
        ? Promise.resolve(config.reverse_proxy_throttle)
        : configManager.getReverseProxyThrottleConfig(),
      configManager.listSessions(),
      whitelistManager.getAllActiveConcreteTargets(),
      configManager.getAuthCredentialSettings(),
    ]);
  const sourceMap = new Map<string, Set<string>>();
  const sessionLinkedAutoWhitelistFinalIpByRecordId = new Map<string, string>();
  const cidrs: string[] = [];
  const mobilityEnabled = authSettings.session_ip_mobility_enabled === true;
  const mobilityManager = mobilityEnabled
    ? (await import("./auth-mobility-session")).authMobilitySessionManager
    : null;

  for (const session of sessions) {
    if (mobilityManager) {
      const activeIps = await mobilityManager.listEffectiveSessionIps(
        session.id,
        session.data,
      );
      for (const activeIp of activeIps) {
        addSourceForIp(sourceMap, activeIp, `session:${session.id}`);
      }
      continue;
    }

    const finalIp = normalizeIp(session.data.ip);
    if (!finalIp) continue;

    addSourceForIp(sourceMap, finalIp, `session:${session.id}`);
    if (session.data.postLoginIpGrantRecordId) {
      sessionLinkedAutoWhitelistFinalIpByRecordId.set(
        session.data.postLoginIpGrantRecordId,
        finalIp,
      );
    }
  }

  for (const entry of whitelistTargets) {
    if (entry.targetType === "cidr") {
      cidrs.push(entry.target);
      continue;
    }

    const compiledIp =
      !mobilityEnabled &&
      entry.source === "auto" &&
      entry.recordTargetType === "ip"
        ? (sessionLinkedAutoWhitelistFinalIpByRecordId.get(entry.recordId) ??
          entry.target)
        : entry.target;
    addSourceForIp(
      sourceMap,
      compiledIp,
      `whitelist:${entry.source}:${entry.recordId}`,
    );
  }

  return {
    enabled: reverseProxyThrottle.enabled,
    items: [...sourceMap.entries()]
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([ip, sources]) => ({
        ip,
        sources: [...sources].sort((left, right) => left.localeCompare(right)),
      })),
    cidrs: normalizeCidrLines(cidrs),
    updated_at: new Date().toISOString(),
  };
};

export const syncReverseProxyTrustedIPsToGateway = async (
  runtime?: ReverseProxyTrustedIPRuntimeState | null,
): Promise<ReverseProxyTrustedIPRuntimeState> => {
  const nextRuntime =
    runtime ?? (await configManager.getReverseProxyTrustedIPsRuntimeState());
  const response = await goBackend.setReverseProxyThrottleExemptIPs(
    toGatewayPayload(nextRuntime),
  );

  if (!response.success) {
    if (response.code === 404 || response.code === 501) {
      if (!runtimeEndpointUnavailableLogged) {
        runtimeEndpointUnavailableLogged = true;
        console.warn(
          "[reverse-proxy-trusted-ips] Go runtime endpoint /api/runtime/reverse-proxy-throttle-exempt-ips is unavailable; skipping gateway sync until the Go backend supports it.",
        );
      }
      return nextRuntime;
    }

    throw new Error(response.message || "同步反代节流豁免 IP 失败");
  }

  runtimeEndpointUnavailableLogged = false;
  return nextRuntime;
};

const rebuildReverseProxyTrustedIPsRuntime = async (
  config?: Pick<AppConfig, "reverse_proxy_throttle">,
): Promise<ReverseProxyTrustedIPRuntimeState> => {
  const runtime = await compileReverseProxyTrustedIPsRuntimeState(config);
  const savedRuntime =
    await configManager.saveReverseProxyTrustedIPsRuntimeState(runtime);
  await syncReverseProxyTrustedIPsToGateway(savedRuntime);
  return savedRuntime;
};

export const syncReverseProxyTrustedIPsNow = async ({
  config,
}: {
  config?: Pick<AppConfig, "reverse_proxy_throttle">;
} = {}): Promise<ReverseProxyTrustedIPRuntimeState> => {
  if (syncInFlight) {
    rerunRequested = true;
    return syncInFlight;
  }

  syncInFlight = (async () => {
    let lastRuntime = await rebuildReverseProxyTrustedIPsRuntime(config);

    while (rerunRequested) {
      rerunRequested = false;
      lastRuntime = await rebuildReverseProxyTrustedIPsRuntime();
    }

    return lastRuntime;
  })();

  try {
    return await syncInFlight;
  } finally {
    syncInFlight = null;
  }
};

export const scheduleSyncReverseProxyTrustedIPs = ({
  reason = "scheduled",
  delayMs = SYNC_DEBOUNCE_MS,
}: {
  reason?: string;
  delayMs?: number;
} = {}): void => {
  scheduledSyncReason = reason;

  if (scheduledSyncTimer) {
    clearTimeout(scheduledSyncTimer);
  }

  scheduledSyncTimer = setTimeout(
    () => {
      const nextReason = scheduledSyncReason;
      scheduledSyncTimer = null;
      scheduledSyncReason = "scheduled";
      void syncReverseProxyTrustedIPsNow().catch((error) => {
        console.error(
          `[reverse-proxy-trusted-ips] failed to sync runtime state (${nextReason}):`,
          error,
        );
      });
    },
    Math.max(0, Math.floor(delayMs)),
  );
};
