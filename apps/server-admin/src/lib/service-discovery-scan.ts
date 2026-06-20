import type { ScanOptions } from "../plugins/scanner/types";
import {
  DISCOVER_COMMON_PORTS,
  buildSingletonPortRanges,
} from "./scan-discovery";

export const LOOPBACK_DISCOVERY_CIDR = "127.0.0.1/32";
export const LOOPBACK_DISCOVERY_HOST = "127.0.0.1";

export interface ServiceDiscoveryScanResult {
  host?: string;
  totalPortsScanned: number;
  foundServices: number;
  scannedHosts?: number;
  services: unknown[];
}

export interface ServiceDiscoveryScanner {
  scanAndAnalyze: (
    host: string,
    options?: ScanOptions,
  ) => Promise<ServiceDiscoveryScanResult>;
  scanAndAnalyzeMany: (
    hosts: string[],
    options?: ScanOptions,
  ) => Promise<ServiceDiscoveryScanResult>;
}

export const shouldUseFullLoopbackDiscovery = (
  isDockerRuntime: boolean,
  scanCidrs: readonly string[],
) => !isDockerRuntime && scanCidrs.includes(LOOPBACK_DISCOVERY_CIDR);

export const buildDiscoveryPortModeLabel = (
  isDockerRuntime: boolean,
  scanCidrs: readonly string[],
) => {
  if (!shouldUseFullLoopbackDiscovery(isDockerRuntime, scanCidrs)) {
    return String(DISCOVER_COMMON_PORTS.length);
  }

  return scanCidrs.length === 1
    ? "1000-60000"
    : `127.0.0.1=1000-60000, others=${DISCOVER_COMMON_PORTS.length}`;
};

const buildCommonPortScanOptions = (
  excludePorts: readonly number[],
): ScanOptions => ({
  skipPorts: [...excludePorts],
  timeout: 80,
  maxConcurrent: 64,
  hostConcurrency: 6,
  portRanges: buildSingletonPortRanges(DISCOVER_COMMON_PORTS),
});

const buildFullLoopbackScanOptions = (
  excludePorts: readonly number[],
): ScanOptions => ({
  skipPorts: [...excludePorts],
  maxConcurrent: 200,
});

const mergeDiscoveryScanResults = (
  primaryHost: string,
  results: ServiceDiscoveryScanResult[],
): ServiceDiscoveryScanResult => {
  const services = results.flatMap((result) => result.services || []);

  return {
    host: primaryHost,
    totalPortsScanned: results.reduce(
      (total, result) => total + result.totalPortsScanned,
      0,
    ),
    foundServices: services.length,
    scannedHosts: results.reduce(
      (total, result) => total + (result.scannedHosts || 0),
      0,
    ),
    services,
  };
};

export async function runServiceDiscoveryScan({
  excludePorts,
  isDockerRuntime,
  scanCidrs,
  scanHosts,
  scannerService,
}: {
  excludePorts: readonly number[];
  isDockerRuntime: boolean;
  scanCidrs: readonly string[];
  scanHosts: readonly string[];
  scannerService: ServiceDiscoveryScanner;
}): Promise<ServiceDiscoveryScanResult> {
  if (!shouldUseFullLoopbackDiscovery(isDockerRuntime, scanCidrs)) {
    return scannerService.scanAndAnalyzeMany(
      [...scanHosts],
      buildCommonPortScanOptions(excludePorts),
    );
  }

  const nonLoopbackHosts = scanHosts.filter(
    (host) => host !== LOOPBACK_DISCOVERY_HOST,
  );

  if (nonLoopbackHosts.length === 0) {
    return scannerService.scanAndAnalyze(
      LOOPBACK_DISCOVERY_HOST,
      buildFullLoopbackScanOptions(excludePorts),
    );
  }

  const [loopbackResult, networkResult] = await Promise.all([
    scannerService.scanAndAnalyze(
      LOOPBACK_DISCOVERY_HOST,
      buildFullLoopbackScanOptions(excludePorts),
    ),
    scannerService.scanAndAnalyzeMany(
      nonLoopbackHosts,
      buildCommonPortScanOptions(excludePorts),
    ),
  ]);

  return mergeDiscoveryScanResults(scanHosts[0] || "", [
    loopbackResult,
    networkResult,
  ]);
}
