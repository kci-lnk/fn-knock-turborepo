import { Elysia, t } from "elysia";
import { randomUUID } from "node:crypto";
import { lookup } from "node:dns/promises";
import { isIP } from "node:net";
import { portScannerPlugin } from "../plugins/scanner";
import { isAbortError } from "../plugins/scanner/scanner";
import { acmePlugin } from "../plugins/acme";
import { ConfigManager } from "../lib/redis";
import { DOCKER_ADMIN_DISCOVER_IP_HEADER_NAME } from "../lib/docker-admin-panel";
import { routeDoc, withRouteDoc } from "../lib/openapi";
import {
  getRuntimeProfile,
  isAdminPanelProtectedRuntime,
} from "../lib/runtime-profile";
import {
  SCAN_DISCOVERY_LIMITS,
  ScanDiscoveryValidationError,
  buildCustomDiscoverTargets,
  buildDockerDiscoverTarget,
  buildInterfaceDiscoverTargets,
  buildLoopbackDiscoverTarget,
  buildMappingDiscoverTargets,
  buildSavedDiscoverTargets,
  buildScanScope,
  dedupeTargets,
  expandScanCidrs,
  isAllowedScanIpv4,
  normalizeAllowedScanCidrs,
  validateScanCidrs,
} from "../lib/scan-discovery";
import {
  buildDiscoveryPortModeLabel,
  countDiscoveryScanPorts,
  type DiscoveryProgressEvent,
  LOOPBACK_DISCOVERY_CIDR,
  LOOPBACK_DISCOVERY_HOST,
  runServiceDiscoveryScan,
  type ServiceDiscoveryScanResult,
  type ServiceDiscoveryScanner,
} from "../lib/service-discovery-scan";
import { probeConfiguredHostMappings } from "../lib/host-mapping-probe";
import { listPrivateIpv4Candidates } from "../lib/local-network";
import { createRequestTranslator } from "../lib/i18n";

const runtimeProfile = getRuntimeProfile();
const adminPanelProtectedRuntime = isAdminPanelProtectedRuntime(runtimeProfile);
const defaultAdminViewPort = "7991";
const defaultBackendPort =
  runtimeProfile.deployment_target === "openwrt" ? "17998" : "7998";

const normalizeHostLike = (value: string): string => {
  const trimmed = value.trim();
  if (!trimmed) return "";

  try {
    return new URL(`http://${trimmed}`).hostname.trim().toLowerCase();
  } catch {
    return trimmed.replace(/^\[/, "").replace(/\]$/, "").trim().toLowerCase();
  }
};

const isUsablePrivateIpv4 = (value: string): boolean =>
  isIP(value) === 4 && isAllowedScanIpv4(value) && !value.startsWith("127.");

const DOCKER_DISCOVER_LAN_IP = (() => {
  const raw = process.env.DOCKER_DISCOVER_LAN_IP?.trim() || "";
  return isUsablePrivateIpv4(raw) ? raw : "";
})();

if (
  runtimeProfile.is_docker &&
  process.env.DOCKER_DISCOVER_LAN_IP &&
  !DOCKER_DISCOVER_LAN_IP
) {
  console.warn(
    `[scan] ignoring invalid DOCKER_DISCOVER_LAN_IP=${process.env.DOCKER_DISCOVER_LAN_IP}`,
  );
}

const resolveDockerDiscoverTargetHost = async (
  request: Request,
): Promise<string | null> => {
  const forwardedDiscoverIp = String(
    request.headers.get(DOCKER_ADMIN_DISCOVER_IP_HEADER_NAME) || "",
  ).trim();
  if (isUsablePrivateIpv4(forwardedDiscoverIp)) {
    return forwardedDiscoverIp;
  }

  if (DOCKER_DISCOVER_LAN_IP) {
    return DOCKER_DISCOVER_LAN_IP;
  }

  const candidateValues = [
    request.headers.get("x-forwarded-host"),
    request.headers.get("host"),
  ]
    .flatMap((value) => String(value ?? "").split(","))
    .map((value) => normalizeHostLike(value))
    .filter(Boolean);

  try {
    candidateValues.push(new URL(request.url).hostname.trim().toLowerCase());
  } catch {
    // ignore malformed request URL
  }

  for (const candidate of candidateValues) {
    if (!candidate || candidate === "localhost") {
      continue;
    }

    if (isUsablePrivateIpv4(candidate)) {
      return candidate;
    }

    if (isIP(candidate) !== 0) {
      continue;
    }

    try {
      const resolved = await lookup(candidate, { family: 4, all: true });
      const privateMatch = resolved.find((item) =>
        isUsablePrivateIpv4(item.address),
      );
      if (privateMatch) {
        return privateMatch.address;
      }
    } catch {
      // ignore resolution failures and continue
    }
  }

  return null;
};

const buildAutomaticDiscoverTargets = async (
  request: Request,
  config: Awaited<ReturnType<ConfigManager["getConfig"]>>,
) => {
  const primaryTarget = runtimeProfile.is_docker
    ? buildDockerDiscoverTarget(await resolveDockerDiscoverTargetHost(request))
    : buildLoopbackDiscoverTarget();

  return dedupeTargets([
    primaryTarget,
    ...buildInterfaceDiscoverTargets(),
    ...buildMappingDiscoverTargets(config),
  ]);
};

const buildDiscoverTargetsPayload = async (
  request: Request,
  config: Awaited<ReturnType<ConfigManager["getConfig"]>>,
) => {
  const automaticTargets = await buildAutomaticDiscoverTargets(request, config);
  const scanDiscovery = config.scan_discovery;
  const customTargets = buildCustomDiscoverTargets(
    scanDiscovery?.custom_cidrs || [],
  );
  const savedSelectedCidrs = normalizeAllowedScanCidrs(
    scanDiscovery?.selected_cidrs || [],
  );
  const automaticCidrs = automaticTargets.map((target) => target.cidr);
  const selectionMode: "automatic" | "custom" =
    savedSelectedCidrs.length > 0 ? "custom" : "automatic";
  const selectedCidrs =
    savedSelectedCidrs.length > 0 ? savedSelectedCidrs : automaticCidrs;
  const effectiveCidrs =
    selectedCidrs.length > 0 ? selectedCidrs : automaticCidrs;
  const selectedTargets = buildSavedDiscoverTargets(effectiveCidrs);

  return {
    automaticTargets,
    customTargets,
    selectedTargets,
    selectionMode,
    selectedCidrs: effectiveCidrs,
    effectiveCidrs,
    limits: SCAN_DISCOVERY_LIMITS,
  };
};

const resolveFullRangeDiscoverCidrs = async (
  request: Request,
  config: Awaited<ReturnType<ConfigManager["getConfig"]>>,
): Promise<string[]> => {
  const automaticTargets = await buildAutomaticDiscoverTargets(request, config);
  return normalizeAllowedScanCidrs([
    LOOPBACK_DISCOVERY_CIDR,
    ...automaticTargets
      .filter(
        (target) =>
          target.source === "loopback" ||
          target.source === "interface" ||
          target.source === "docker",
      )
      .map((target) => target.cidr),
  ]);
};

const normalizeDiscoverSelfHosts = (
  hosts: Iterable<string | null | undefined>,
): string[] => {
  const result: string[] = [];
  const seen = new Set<string>();

  for (const host of hosts) {
    const value = String(host || "").trim();
    if (isIP(value) !== 4 || !isAllowedScanIpv4(value) || seen.has(value)) {
      continue;
    }
    seen.add(value);
    result.push(value);
  }

  return result;
};

const resolveDiscoverSelfHosts = async (request: Request): Promise<string[]> =>
  normalizeDiscoverSelfHosts([
    LOOPBACK_DISCOVERY_HOST,
    ...listPrivateIpv4Candidates().map((candidate) => candidate.value),
    ...(runtimeProfile.is_docker
      ? [await resolveDockerDiscoverTargetHost(request)]
      : []),
  ]);

export const collectExcludedPorts = (
  _config: Pick<
    Awaited<ReturnType<ConfigManager["getConfig"]>>,
    "proxy_mappings"
  >,
): number[] => {
  const adminViewPort = parseInt(
    process.env.ADMIN_VIEW_PORT || defaultAdminViewPort,
    10,
  );
  const envPorts = [
    ...(adminPanelProtectedRuntime ? [] : [adminViewPort]),
    parseInt(process.env.BACKEND_PORT || defaultBackendPort, 10),
    parseInt(process.env.AUTH_PORT || "7997", 10),
    parseInt(process.env.GO_BACKEND_PORT || "7996", 10),
    parseInt(process.env.GO_REPROXY_PORT || "7999", 10),
    7995,
    8000,
  ].filter((port) => Number.isFinite(port) && port > 0);

  // Existing mapping targets are filtered after discovery by host:port.
  // Excluding their ports here would hide same-port services on other hosts.
  return Array.from(new Set([...envPorts, 8200, 30661, 30662]));
};

const sanitizeDiscoveredService = (service: any) => ({
  serviceKey: service.serviceKey,
  host: service.host,
  port: service.port,
  httpStatus: service.httpStatus,
  ...(service.requiresBasicAuth ? { requiresBasicAuth: true } : {}),
  detail: {
    name: service.detail?.name || "",
    label: service.detail?.label || "",
    rule: service.detail?.rule,
    isDefault: Boolean(service.detail?.isDefault),
  },
});

type DiscoverJobState =
  | "queued"
  | "running"
  | "completed"
  | "cancelled"
  | "failed";

type DiscoveredJobService = ReturnType<typeof sanitizeDiscoveredService>;

interface DiscoverJobMeta {
  host: string;
  totalPortsScanned: number;
  foundServices: number;
  scannedHosts: number;
  scanHostCount: number;
  scanScope: string | null;
  scanCidrs: string[];
  portRange: string;
}

interface DiscoverJobResult extends ServiceDiscoveryScanResult {
  host: string;
  scannedHosts: number;
  scanHostCount: number;
  scanScope: string | null;
  scanCidrs: string[];
  services: DiscoveredJobService[];
}

interface DiscoverScanJob {
  id: string;
  abortController: AbortController;
  activeTimer?: ReturnType<typeof setTimeout>;
  cleanupTimer?: ReturnType<typeof setTimeout>;
  createdAt: number;
  updatedAt: number;
  state: DiscoverJobState;
  meta: DiscoverJobMeta | null;
  progress: DiscoveryProgressEvent | null;
  serviceEvents: DiscoveredJobService[];
  serviceMap: Map<string, DiscoveredJobService>;
  result: DiscoverJobResult | null;
  error: string | null;
}

const DISCOVER_JOB_ACTIVE_TTL_MS = 30 * 60 * 1000;
const DISCOVER_JOB_DONE_TTL_MS = 5 * 60 * 1000;
const DISCOVER_JOB_MAX_ACTIVE = 4;
const DISCOVER_JOB_MAX_RETAINED = 64;
const discoverJobs = new Map<string, DiscoverScanJob>();

const isTerminalDiscoverJobState = (state: DiscoverJobState): boolean =>
  state === "completed" || state === "cancelled" || state === "failed";

const touchDiscoverJob = (job: DiscoverScanJob) => {
  job.updatedAt = Date.now();
};

const clearDiscoverJobActiveTimer = (job: DiscoverScanJob) => {
  if (!job.activeTimer) return;
  clearTimeout(job.activeTimer);
  job.activeTimer = undefined;
};

const deleteDiscoverJob = (jobId: string) => {
  const job = discoverJobs.get(jobId);
  if (job) {
    clearDiscoverJobActiveTimer(job);
  }
  if (job?.cleanupTimer) {
    clearTimeout(job.cleanupTimer);
  }
  discoverJobs.delete(jobId);
};

const scheduleDiscoverJobRemoval = (job: DiscoverScanJob) => {
  clearDiscoverJobActiveTimer(job);
  if (job.cleanupTimer) return;
  job.cleanupTimer = setTimeout(() => {
    deleteDiscoverJob(job.id);
  }, DISCOVER_JOB_DONE_TTL_MS);
  job.cleanupTimer.unref?.();
};

const scheduleDiscoverJobActiveTimeout = (job: DiscoverScanJob) => {
  clearDiscoverJobActiveTimer(job);
  job.activeTimer = setTimeout(() => {
    const current = discoverJobs.get(job.id);
    if (current && !isTerminalDiscoverJobState(current.state)) {
      cancelDiscoverJob(current);
    }
  }, DISCOVER_JOB_ACTIVE_TTL_MS);
  job.activeTimer.unref?.();
};

const sortDiscoverJobsByAge = (jobs: DiscoverScanJob[]) =>
  [...jobs].sort((left, right) => left.createdAt - right.createdAt);

const cancelDiscoverJob = (job: DiscoverScanJob) => {
  if (isTerminalDiscoverJobState(job.state)) return;
  job.abortController.abort();
  job.state = "cancelled";
  job.serviceEvents = [];
  job.serviceMap.clear();
  touchDiscoverJob(job);
  scheduleDiscoverJobRemoval(job);
};

const enforceDiscoverJobLimits = () => {
  const activeJobs = sortDiscoverJobsByAge(
    [...discoverJobs.values()].filter(
      (job) => !isTerminalDiscoverJobState(job.state),
    ),
  );
  const activeOverflow = Math.max(
    0,
    activeJobs.length - DISCOVER_JOB_MAX_ACTIVE,
  );
  for (const job of activeJobs.slice(0, activeOverflow)) {
    cancelDiscoverJob(job);
  }

  while (discoverJobs.size > DISCOVER_JOB_MAX_RETAINED) {
    const terminalJob = sortDiscoverJobsByAge(
      [...discoverJobs.values()].filter((job) =>
        isTerminalDiscoverJobState(job.state),
      ),
    )[0];
    if (terminalJob) {
      deleteDiscoverJob(terminalJob.id);
      continue;
    }

    const oldestActiveJob = sortDiscoverJobsByAge([
      ...discoverJobs.values(),
    ])[0];
    if (!oldestActiveJob) break;
    cancelDiscoverJob(oldestActiveJob);
    deleteDiscoverJob(oldestActiveJob.id);
  }
};

const cleanupDiscoverJobs = () => {
  const now = Date.now();
  for (const [jobId, job] of discoverJobs) {
    if (isTerminalDiscoverJobState(job.state)) {
      if (now - job.updatedAt > DISCOVER_JOB_DONE_TTL_MS) {
        deleteDiscoverJob(jobId);
      }
      continue;
    }

    if (now - job.createdAt > DISCOVER_JOB_ACTIVE_TTL_MS) {
      cancelDiscoverJob(job);
    }
  }
  enforceDiscoverJobLimits();
};

const normalizeServiceCursor = (value: unknown, max: number): number => {
  const parsed = Number.parseInt(String(value ?? "0"), 10);
  if (!Number.isFinite(parsed) || parsed < 0) return 0;
  return Math.min(parsed, max);
};

const serializeDiscoverJob = (job: DiscoverScanJob, cursor?: unknown) => {
  const serviceCursor = normalizeServiceCursor(
    cursor,
    job.serviceEvents.length,
  );

  return {
    jobId: job.id,
    state: job.state,
    createdAt: job.createdAt,
    updatedAt: job.updatedAt,
    meta: job.meta,
    progress: job.progress,
    services: job.serviceEvents.slice(serviceCursor),
    nextCursor: job.serviceEvents.length,
    result: job.result,
    error: job.error,
  };
};

const buildDiscoverJobResult = ({
  job,
  scanCidrs,
  scanHosts,
  scanResult,
  scanScope,
}: {
  job: DiscoverScanJob;
  scanCidrs: string[];
  scanHosts: string[];
  scanResult: ServiceDiscoveryScanResult;
  scanScope: string | null;
}): DiscoverJobResult => {
  const services = Array.from(job.serviceMap.values());
  return {
    ...scanResult,
    host: scanHosts[0] || "",
    foundServices: services.length,
    scannedHosts: scanHosts.length,
    scanHostCount: scanHosts.length,
    scanScope,
    scanCidrs,
    services,
  };
};

const runDiscoverScanJob = async ({
  fullRangeCidrs,
  job,
  scanCidrs,
  selfScanHosts,
  scannerService,
}: {
  fullRangeCidrs: string[];
  job: DiscoverScanJob;
  scanCidrs: string[];
  selfScanHosts: string[];
  scannerService: ServiceDiscoveryScanner;
}) => {
  try {
    job.state = "running";
    touchDiscoverJob(job);

    const configManager = new ConfigManager();
    const config = await configManager.getConfig();
    const excludePorts = collectExcludedPorts(config);
    const scanHosts = expandScanCidrs(scanCidrs);
    const scanScope = buildScanScope(scanCidrs);
    const totalPortsScanned = countDiscoveryScanPorts({
      excludePorts,
      fullRangeCidrs,
      scanCidrs,
      scanHosts,
      selfScanHosts,
    });
    const portModeLabel = buildDiscoveryPortModeLabel(
      runtimeProfile.is_docker,
      scanCidrs,
      fullRangeCidrs,
    );

    job.meta = {
      host: scanHosts[0] || "",
      totalPortsScanned,
      foundServices: 0,
      scannedHosts: scanHosts.length,
      scanHostCount: scanHosts.length,
      scanScope,
      scanCidrs,
      portRange: portModeLabel,
    };
    job.progress = {
      scannedPorts: 0,
      totalPorts: totalPortsScanned,
      scannedHosts: 0,
      totalHosts: scanHosts.length,
      currentHost: scanHosts[0],
    };
    touchDiscoverJob(job);

    console.log(
      `[scan] job=${job.id} scope=${scanScope} hosts=${scanHosts.length} ports=${portModeLabel}`,
    );

    const scanResult = await runServiceDiscoveryScan({
      excludePorts,
      isDockerRuntime: runtimeProfile.is_docker,
      onProgress: (progress) => {
        job.progress = progress;
        touchDiscoverJob(job);
      },
      onService: (service) => {
        const sanitized = sanitizeDiscoveredService(service);
        const serviceKey =
          sanitized.serviceKey || `${sanitized.host || ""}:${sanitized.port}`;
        job.serviceMap.set(serviceKey, sanitized);
        job.serviceEvents.push(sanitized);
        if (job.meta) {
          job.meta = {
            ...job.meta,
            foundServices: job.serviceMap.size,
          };
        }
        touchDiscoverJob(job);
      },
      scanCidrs,
      fullRangeCidrs,
      scanHosts,
      selfScanHosts,
      signal: job.abortController.signal,
      scannerService,
    });

    if (job.abortController.signal.aborted) {
      cancelDiscoverJob(job);
      return;
    }

    job.result = buildDiscoverJobResult({
      job,
      scanCidrs,
      scanHosts,
      scanResult,
      scanScope,
    });
    job.progress = {
      scannedPorts: totalPortsScanned,
      totalPorts: totalPortsScanned,
      scannedHosts: scanHosts.length,
      totalHosts: scanHosts.length,
    };
    job.state = "completed";
    touchDiscoverJob(job);
    scheduleDiscoverJobRemoval(job);
  } catch (error) {
    if (isAbortError(error) || job.abortController.signal.aborted) {
      cancelDiscoverJob(job);
      return;
    }

    job.error = (error as Error).message;
    job.state = "failed";
    touchDiscoverJob(job);
    scheduleDiscoverJobRemoval(job);
  }
};

const createDiscoverScanJob = (
  fullRangeCidrs: string[],
  scanCidrs: string[],
  selfScanHosts: string[],
  scannerService: ServiceDiscoveryScanner,
): DiscoverScanJob => {
  cleanupDiscoverJobs();

  const now = Date.now();
  const job: DiscoverScanJob = {
    id: randomUUID(),
    abortController: new AbortController(),
    createdAt: now,
    updatedAt: now,
    state: "queued",
    meta: null,
    progress: null,
    serviceEvents: [],
    serviceMap: new Map(),
    result: null,
    error: null,
  };

  discoverJobs.set(job.id, job);
  scheduleDiscoverJobActiveTimeout(job);
  enforceDiscoverJobLimits();
  queueMicrotask(() => {
    void runDiscoverScanJob({
      fullRangeCidrs,
      job,
      scanCidrs,
      selfScanHosts,
      scannerService,
    });
  });
  return job;
};

type Translate = ReturnType<typeof createRequestTranslator>["t"];

const validateExplicitDiscoverCidrs = (
  values: string[] | undefined,
  translate: Translate,
): string[] => {
  if (!Array.isArray(values)) {
    throw new ScanDiscoveryValidationError(
      translate("server.scanDiscovery.selectAtLeastOneCidr"),
    );
  }

  const scanCidrs = validateScanCidrs(values);
  if (scanCidrs.length === 0) {
    throw new ScanDiscoveryValidationError(
      translate("server.scanDiscovery.selectAtLeastOneCidr"),
    );
  }

  return scanCidrs;
};

export const assetsRoutes = new Elysia({
  prefix: "/api/admin/scan",
  tags: ["Assets"],
})
  .use(portScannerPlugin)
  .use(acmePlugin)
  .get(
    "/discover-targets",
    async ({ request }) => {
      const configManager = new ConfigManager();
      const config = await configManager.getConfig();
      return {
        success: true,
        data: await buildDiscoverTargetsPayload(request, config),
      };
    },
    routeDoc("获取服务发现扫描网段"),
  )
  .post(
    "/discover-targets",
    async ({ body, request, set }) => {
      const configManager = new ConfigManager();
      const customCidrs = normalizeAllowedScanCidrs(body.custom_cidrs || []);
      const selectedCidrs = normalizeAllowedScanCidrs(
        body.selected_cidrs || [],
      );

      try {
        if (selectedCidrs.length > 0) {
          validateScanCidrs(selectedCidrs);
        }
      } catch (error) {
        if (error instanceof ScanDiscoveryValidationError) {
          set.status = 400;
        }
        return {
          success: false,
          message: (error as Error).message,
        };
      }

      const config = await configManager.getConfig();
      config.scan_discovery = {
        custom_cidrs: customCidrs,
        selected_cidrs: selectedCidrs,
      };
      await configManager.saveConfig(config);

      return {
        success: true,
        data: await buildDiscoverTargetsPayload(request, config),
      };
    },
    withRouteDoc("保存服务发现扫描网段", {
      body: t.Object({
        custom_cidrs: t.Optional(t.Array(t.String())),
        selected_cidrs: t.Optional(t.Array(t.String())),
      }),
    }),
  )
  .post(
    "/discover/jobs",
    async ({ body, request, scannerService, set }) => {
      try {
        const configManager = new ConfigManager();
        const config = await configManager.getConfig();
        const { t: translate } = createRequestTranslator(
          request,
          config.locale,
        );
        const scanCidrs = validateExplicitDiscoverCidrs(
          body.target_cidrs,
          translate,
        );
        const fullRangeCidrs = await resolveFullRangeDiscoverCidrs(
          request,
          config,
        );
        const selfScanHosts = await resolveDiscoverSelfHosts(request);
        const job = createDiscoverScanJob(
          fullRangeCidrs,
          scanCidrs,
          selfScanHosts,
          scannerService,
        );
        return {
          success: true,
          data: serializeDiscoverJob(job),
        };
      } catch (error) {
        if (error instanceof ScanDiscoveryValidationError) {
          set.status = 400;
        }
        return {
          success: false,
          message: (error as Error).message,
        };
      }
    },
    withRouteDoc("创建服务发现扫描任务", {
      body: t.Object({
        target_cidrs: t.Array(t.String()),
      }),
    }),
  )
  .get(
    "/discover/jobs/:jobId",
    async ({ params, query, request, set }) => {
      cleanupDiscoverJobs();
      const job = discoverJobs.get(params.jobId);
      if (!job) {
        const configManager = new ConfigManager();
        const config = await configManager.getConfig();
        const { t: translate } = createRequestTranslator(
          request,
          config.locale,
        );
        set.status = 404;
        return {
          success: false,
          message: translate("server.scanDiscovery.scanJobNotFound"),
        };
      }

      return {
        success: true,
        data: serializeDiscoverJob(job, query.cursor),
      };
    },
    withRouteDoc("获取服务发现扫描任务状态", {
      params: t.Object({
        jobId: t.String(),
      }),
      query: t.Object({
        cursor: t.Optional(t.String()),
      }),
    }),
  )
  .delete(
    "/discover/jobs/:jobId",
    async ({ params, request, set }) => {
      cleanupDiscoverJobs();
      const job = discoverJobs.get(params.jobId);
      if (!job) {
        const configManager = new ConfigManager();
        const config = await configManager.getConfig();
        const { t: translate } = createRequestTranslator(
          request,
          config.locale,
        );
        set.status = 404;
        return {
          success: false,
          message: translate("server.scanDiscovery.scanJobNotFound"),
        };
      }

      if (!isTerminalDiscoverJobState(job.state)) {
        cancelDiscoverJob(job);
      }

      return {
        success: true,
        data: serializeDiscoverJob(job),
      };
    },
    withRouteDoc("取消服务发现扫描任务", {
      params: t.Object({
        jobId: t.String(),
      }),
    }),
  )
  .post(
    "/host-mappings/probe",
    async ({ body }) => {
      const configManager = new ConfigManager();
      const config = await configManager.getConfig();
      const results = await probeConfiguredHostMappings(
        config.host_mappings,
        body.hosts,
      );

      return {
        success: true,
        data: {
          results,
        },
      };
    },
    withRouteDoc("探测 Host 映射目标可达性", {
      body: t.Object({
        hosts: t.Optional(t.Array(t.String())),
      }),
    }),
  );
