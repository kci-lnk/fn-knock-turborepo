import { Elysia, t } from "elysia";
import { lookup } from "node:dns/promises";
import { isIP } from "node:net";
import { portScannerPlugin } from "../plugins/scanner";
import { acmePlugin } from "../plugins/acme";
import { ConfigManager } from "../lib/redis";
import { DOCKER_ADMIN_DISCOVER_IP_HEADER_NAME } from "../lib/docker-admin-panel";
import { routeDoc, withRouteDoc } from "../lib/openapi";
import {
  getRuntimeProfile,
  isAdminPanelProtectedRuntime,
} from "../lib/runtime-profile";
import {
  DISCOVER_COMMON_PORTS,
  SCAN_DISCOVERY_LIMITS,
  ScanDiscoveryValidationError,
  buildCustomDiscoverTargets,
  buildDockerDiscoverTarget,
  buildInterfaceDiscoverTargets,
  buildLoopbackDiscoverTarget,
  buildMappingDiscoverTargets,
  buildSavedDiscoverTargets,
  buildScanScope,
  buildSingletonPortRanges,
  dedupeTargets,
  expandScanCidrs,
  isAllowedScanIpv4,
  normalizeAllowedScanCidrs,
  validateScanCidrs,
} from "../lib/scan-discovery";
import { createRequestTranslator } from "../lib/i18n";
import { probeConfiguredHostMappings } from "../lib/host-mapping-probe";

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

const resolveScanCidrs = async (
  request: Request,
  config: Awaited<ReturnType<ConfigManager["getConfig"]>>,
  targetCidrs?: string[],
): Promise<string[]> => {
  if (targetCidrs !== undefined) {
    return validateScanCidrs(targetCidrs);
  }

  const payload = await buildDiscoverTargetsPayload(request, config);
  return validateScanCidrs(payload.effectiveCidrs);
};

const collectExcludedPorts = (
  config: Awaited<ReturnType<ConfigManager["getConfig"]>>,
): number[] => {
  const envPorts = [
    parseInt(
      process.env.ADMIN_VIEW_PORT ||
        (adminPanelProtectedRuntime ? defaultAdminViewPort : ""),
      10,
    ),
    parseInt(process.env.BACKEND_PORT || defaultBackendPort, 10),
    parseInt(process.env.AUTH_PORT || "7997", 10),
    parseInt(process.env.GO_BACKEND_PORT || "7996", 10),
    parseInt(process.env.GO_REPROXY_PORT || "7999", 10),
    7995,
    8000,
  ].filter((port) => Number.isFinite(port) && port > 0);

  const mappingPorts: number[] = [];
  for (const mapping of config.proxy_mappings || []) {
    if (mapping.target) {
      try {
        const parsedUrl = new URL(mapping.target);
        if (parsedUrl.port) {
          mappingPorts.push(parseInt(parsedUrl.port, 10));
        } else if (parsedUrl.protocol === "http:") {
          mappingPorts.push(80);
        } else if (parsedUrl.protocol === "https:") {
          mappingPorts.push(443);
        }
      } catch (e) {
        console.warn(
          `[scan] failed to parse proxy mapping URL: ${mapping.target}`,
        );
      }
    }
  }

  return Array.from(
    new Set([...envPorts, ...mappingPorts, 8200, 30661, 30662]),
  );
};

const handleDiscover = async (
  {
    request,
    scannerService,
    set,
  }: {
    request: Request;
    scannerService: {
      scanAndAnalyzeMany: (
        hosts: string[],
        options?: Record<string, unknown>,
      ) => Promise<any>;
      scanAndAnalyze: (
        host: string,
        options?: Record<string, unknown>,
      ) => Promise<any>;
    };
    set: { status?: number | string };
  },
  targetCidrs?: string[],
) => {
  const configManager = new ConfigManager();
  const config = await configManager.getConfig();
  const { t } = createRequestTranslator(request, config.locale);
  const excludePorts = collectExcludedPorts(config);
  console.log("[scan] excluded ports:", excludePorts);

  try {
    const scanCidrs = await resolveScanCidrs(request, config, targetCidrs);
    if (scanCidrs.length === 0) {
      set.status = 400;
      return {
        success: false,
        message: t("server.scanDiscovery.selectAtLeastOneCidr"),
      };
    }

    const scanHosts = expandScanCidrs(scanCidrs);
    const scanScope = buildScanScope(scanCidrs);
    const useFullLocalhostScan =
      !runtimeProfile.is_docker &&
      scanCidrs.length === 1 &&
      scanCidrs[0] === "127.0.0.1/32";

    console.log(
      `[scan] scope=${scanScope} hosts=${scanHosts.length} ports=${
        useFullLocalhostScan ? "1000-60000" : DISCOVER_COMMON_PORTS.length
      }`,
    );

    const scanResult = useFullLocalhostScan
      ? await scannerService.scanAndAnalyze(scanHosts[0] || "127.0.0.1", {
          skipPorts: excludePorts,
          maxConcurrent: 200,
        })
      : await scannerService.scanAndAnalyzeMany(scanHosts, {
          skipPorts: excludePorts,
          timeout: 80,
          maxConcurrent: 64,
          hostConcurrency: 6,
          portRanges: buildSingletonPortRanges(DISCOVER_COMMON_PORTS),
        });

    return {
      success: true,
      data: {
        ...scanResult,
        host: scanHosts[0] || "",
        scannedHosts: scanHosts.length,
        scanHostCount: scanHosts.length,
        scanScope,
        scanCidrs,
      },
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
  .get(
    "/discover",
    async ({ request, scannerService, set }) =>
      handleDiscover({ request, scannerService, set }),
    routeDoc("扫描可发现服务"),
  )
  .post(
    "/discover",
    async ({ body, request, scannerService, set }) =>
      handleDiscover({ request, scannerService, set }, body.target_cidrs),
    withRouteDoc("按网段扫描可发现服务", {
      body: t.Object({
        target_cidrs: t.Optional(t.Array(t.String())),
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
