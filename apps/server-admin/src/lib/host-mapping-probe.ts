import {
  fetchWithRelaxedTls,
  type RelaxedTlsFetchInit,
} from "./relaxed-tls-fetch";
import type { HostMapping } from "./redis";
import { isAuthServiceTarget } from "./auth-service";

export type HostMappingProbeStatus = "online" | "stale" | "unsupported";

export interface HostMappingProbeResult {
  host: string;
  target: string;
  status: HostMappingProbeStatus;
  httpStatus?: number;
  error?: string;
  latencyMs?: number;
}

type ProbeFetch = (
  input: string | URL,
  init?: RelaxedTlsFetchInit,
) => Promise<Response>;

export interface ProbeHostMappingOptions {
  timeoutMs?: number;
  fetcher?: ProbeFetch;
}

const DEFAULT_PROBE_TIMEOUT_MS = 2500;

const normalizeHostKey = (value: string): string =>
  value
    .trim()
    .toLowerCase()
    .replace(/^[a-z]+:\/\//i, "")
    .replace(/\/.*$/, "")
    .replace(/\.+$/, "");

const normalizeProbeUrl = (target: string): URL | null => {
  try {
    const url = new URL(target.trim());
    if (url.protocol !== "http:" && url.protocol !== "https:") {
      return null;
    }
    return url;
  } catch {
    return null;
  }
};

const getErrorMessage = (error: unknown): string =>
  error instanceof Error ? error.message : String(error);

const requestTarget = async (
  url: URL,
  method: "HEAD" | "GET",
  options: Required<ProbeHostMappingOptions>,
): Promise<Response> => {
  return options.fetcher(url, {
    method,
    redirect: "manual",
    signal: AbortSignal.timeout(options.timeoutMs),
    headers: {
      "User-Agent": "fn-knock-host-mapping-probe/1.0",
      Connection: "close",
    },
  });
};

export const probeHostMappingTarget = async (
  target: string,
  options: ProbeHostMappingOptions = {},
): Promise<Omit<HostMappingProbeResult, "host" | "target">> => {
  const startedAt = Date.now();
  const url = normalizeProbeUrl(target);
  if (!url) {
    return {
      status: "unsupported",
      error: "Only http:// and https:// targets can be probed",
      latencyMs: Date.now() - startedAt,
    };
  }

  const probeOptions: Required<ProbeHostMappingOptions> = {
    timeoutMs: options.timeoutMs ?? DEFAULT_PROBE_TIMEOUT_MS,
    fetcher: options.fetcher ?? fetchWithRelaxedTls,
  };

  let headError: unknown = null;
  try {
    const response = await requestTarget(url, "HEAD", probeOptions);
    return {
      status: "online",
      httpStatus: response.status,
      latencyMs: Date.now() - startedAt,
    };
  } catch (error) {
    headError = error;
  }

  try {
    const response = await requestTarget(url, "GET", probeOptions);
    return {
      status: "online",
      httpStatus: response.status,
      latencyMs: Date.now() - startedAt,
    };
  } catch (error) {
    return {
      status: "stale",
      error: getErrorMessage(error || headError),
      latencyMs: Date.now() - startedAt,
    };
  }
};

export const probeConfiguredHostMappings = async (
  mappings: Array<Pick<HostMapping, "host" | "target">>,
  hosts?: string[],
  options: ProbeHostMappingOptions = {},
): Promise<HostMappingProbeResult[]> => {
  const requestedHosts =
    hosts && hosts.length > 0
      ? new Set(hosts.map(normalizeHostKey).filter(Boolean))
      : null;
  const targetProbeCache = new Map<
    string,
    Promise<Omit<HostMappingProbeResult, "host" | "target">>
  >();
  const pendingResults: Array<Promise<HostMappingProbeResult>> = [];

  for (const mapping of mappings) {
    const host = mapping.host.trim();
    const target = mapping.target.trim();
    if (!host || !target || isAuthServiceTarget(target)) {
      continue;
    }

    if (requestedHosts && !requestedHosts.has(normalizeHostKey(host))) {
      continue;
    }

    const url = normalizeProbeUrl(target);
    const targetKey = url ? url.toString() : target;
    let probe = targetProbeCache.get(targetKey);
    if (!probe) {
      probe = probeHostMappingTarget(target, options);
      targetProbeCache.set(targetKey, probe);
    }

    pendingResults.push(
      probe.then((result) => ({
        host,
        target,
        ...result,
      })),
    );
  }

  return Promise.all(pendingResults);
};
