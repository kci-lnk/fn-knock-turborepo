import { Elysia } from "elysia";
import { buildScanPortList, ScannerLogic } from "./scanner";
import { analyzeService } from "./analyzers";
import { AnalyzedScanService, ScanOptions, ScanResult } from "./types";

const PLAIN_HTTP_TO_HTTPS_PATTERNS = [
  /plain HTTP request was sent to HTTPS port/i,
  /client sent an HTTP request to an HTTPS server/i,
] as const;

const isPlainHttpToHttpsPortResult = (result: {
  body?: string;
  httpStatus?: number;
}): boolean =>
  result.httpStatus === 400 &&
  PLAIN_HTTP_TO_HTTPS_PATTERNS.some((pattern) =>
    pattern.test(result.body || ""),
  );

export const isDiscoverableServiceResult = (result: {
  body?: string;
  open?: boolean;
  httpStatus?: number;
}): boolean =>
  result.open === true &&
  Number.isInteger(result.httpStatus) &&
  !isPlainHttpToHttpsPortResult(result);

const buildServiceKey = (
  service: Pick<AnalyzedScanService, "detail" | "host" | "port">,
) => {
  const serviceName = service.detail.name;
  return serviceName
    ? `${service.host}::${serviceName}`
    : `${service.host}::unknown-${service.port}`;
};

const toPublicService = (service: AnalyzedScanService) => {
  const { serviceKey: _serviceKey, ...publicService } = service;
  return publicService;
};

export class ScannerService {
  async scanAndAnalyze(host: string, options: ScanOptions = {}) {
    return this.scanAndAnalyzeMany([host], options);
  }

  async scanAndAnalyzeMany(hosts: string[], options: ScanOptions = {}) {
    const normalizedHosts = [
      ...new Set(hosts.map((host) => host.trim())),
    ].filter(Boolean);
    if (normalizedHosts.length === 0) {
      return {
        host: "",
        totalPortsScanned: 0,
        foundServices: 0,
        scannedHosts: 0,
        services: [],
      };
    }

    const scanner = new ScannerLogic({
      timeout: options.timeout,
      maxConcurrent: options.maxConcurrent,
    });
    const uniqueServicesMap = new Map<string, AnalyzedScanService>();
    const processResult = async (result: ScanResult) => {
      if (!isDiscoverableServiceResult(result)) return;

      const rule = await analyzeService(result);
      if (!rule) return;

      const service: AnalyzedScanService = {
        host: result.host,
        port: result.port,
        httpStatus: result.httpStatus,
        ...(result.requiresBasicAuth ? { requiresBasicAuth: true } : {}),
        detail: rule,
        serviceKey: "",
      };
      service.serviceKey = buildServiceKey(service);

      const existing = uniqueServicesMap.get(service.serviceKey);
      if (existing && existing.port <= service.port) {
        return;
      }

      uniqueServicesMap.set(service.serviceKey, service);
      await options.onService?.(service);
    };
    const scannerOptions: ScanOptions = options.onService
      ? {
          ...options,
          onResult: async (result) => {
            await options.onResult?.(result);
            await processResult(result);
          },
        }
      : options;

    const rawResults =
      normalizedHosts.length === 1
        ? await scanner.runScan(normalizedHosts[0]!, scannerOptions)
        : await scanner.runScanMany(normalizedHosts, scannerOptions);
    if (!options.onService) {
      await Promise.all(rawResults.map(processResult));
    }
    const filteredServices = Array.from(uniqueServicesMap.values()).map(
      toPublicService,
    );

    return {
      host: normalizedHosts[0],
      totalPortsScanned:
        buildScanPortList(options).length * normalizedHosts.length,
      foundServices: filteredServices.length,
      scannedHosts: normalizedHosts.length,
      services: filteredServices,
    };
  }
}

export const portScannerPlugin = new Elysia({
  name: "plugin-port-scanner",
}).decorate("scannerService", new ScannerService());
