import { Elysia } from "elysia";
import { ScannerLogic } from "./scanner";
import { analyzeService } from "./analyzers";
import { ScanOptions } from "./types";

export class ScannerService {

  async scanAndAnalyze(host: string, options: ScanOptions = {}) {
    const scanner = new ScannerLogic({
      timeout: options.timeout,
      maxConcurrent: options.maxConcurrent,
    });
    
    const rawResults = await scanner.runScan(host, options);
    const successfulResults = rawResults.filter((r) => r.httpStatus === 200 || r.httpStatus === 401);

    const processedServices = await Promise.all(
      successfulResults.map(async (result) => {
        const rule = await analyzeService(result);
        if (!rule) return null;
        
        return {
          port: result.port,
          httpStatus: result.httpStatus,
          detail: rule,
        };
      })
    );

    const uniqueServicesMap = new Map<string, any>();
    processedServices.forEach(service => {
      if (!service) return;
      const serviceName = service.detail.name;
      const mapKey = serviceName ? serviceName : `unknown-${service.port}`;
      
      const existing = uniqueServicesMap.get(mapKey);
      if (!existing || service.port < existing.port) {
        uniqueServicesMap.set(mapKey, service);
      }
    });

    const filteredServices = Array.from(uniqueServicesMap.values());
    
    return {
      host,
      totalPortsScanned: rawResults.length,
      foundServices: filteredServices.length,
      services: filteredServices,
    };
  }
}

export const portScannerPlugin = new Elysia({ name: "plugin-port-scanner" })
  .decorate("scannerService", new ScannerService());