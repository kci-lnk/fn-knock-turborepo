import { Elysia } from "elysia";
import { buildScanPortList, ScannerLogic } from "./scanner";
import { analyzeService } from "./analyzers";
import { ScanOptions } from "./types";

export class ScannerService {
  async scanAndAnalyze(host: string, options: ScanOptions = {}) {
    return this.scanAndAnalyzeMany([host], options);
  }

  async scanAndAnalyzeMany(hosts: string[], options: ScanOptions = {}) {
    const normalizedHosts = [...new Set(hosts.map((host) => host.trim()))].filter(
      Boolean,
    );
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

    const rawResults =
      normalizedHosts.length === 1
        ? await scanner.runScan(normalizedHosts[0]!, options)
        : await scanner.runScanMany(normalizedHosts, options);
    const successfulResults = rawResults.filter(
      (result) => result.httpStatus === 200 || result.httpStatus === 401,
    );

    const processedServices = await Promise.all(
      successfulResults.map(async (result) => {
        const rule = await analyzeService(result);
        if (!rule) return null;

        return {
          host: result.host,
          port: result.port,
          httpStatus: result.httpStatus,
          ...(result.requiresBasicAuth ? { requiresBasicAuth: true } : {}),
          detail: rule,
        };
      }),
    );

    const uniqueServicesMap = new Map<string, any>();
    processedServices.forEach((service) => {
      if (!service) return;
      const serviceName = service.detail.name;
      const mapKey = serviceName
        ? `${service.host}::${serviceName}`
        : `${service.host}::unknown-${service.port}`;

      const existing = uniqueServicesMap.get(mapKey);
      if (!existing || service.port < existing.port) {
        uniqueServicesMap.set(mapKey, service);
      }
    });

    const filteredServices = Array.from(uniqueServicesMap.values());

    return {
      host: normalizedHosts[0],
      totalPortsScanned: buildScanPortList(options).length * normalizedHosts.length,
      foundServices: filteredServices.length,
      scannedHosts: normalizedHosts.length,
      services: filteredServices,
    };
  }
}

export const portScannerPlugin = new Elysia({ name: "plugin-port-scanner" })
  .decorate("scannerService", new ScannerService());
