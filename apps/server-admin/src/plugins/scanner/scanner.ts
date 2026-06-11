import { ScanOptions, ScanResult } from "./types";
import net from "node:net";
import { headersRequireBasicAuth } from "../../lib/basic-auth-probe";

export const buildScanPortList = (options: ScanOptions): number[] => {
  let portsToScan: number[] = [];
  if (options.portRanges && options.portRanges.length > 0) {
    for (const range of options.portRanges) {
      for (let port = range.start; port <= range.end; port++) {
        portsToScan.push(port);
      }
    }
  } else {
    portsToScan = Array.from({ length: 59001 }, (_, i) => i + 1000);
  }

  const skipSet = new Set(options.skipPorts || []);
  return portsToScan.filter((port) => !skipSet.has(port));
};

export class ScannerLogic {
  private timeout: number;
  private maxConcurrent: number;

  constructor(options: ScanOptions = {}) {
    this.timeout = options.timeout || 70;
    this.maxConcurrent = options.maxConcurrent || 100;
  }

  // 1. TCP 端口检测 (第一阶段)
  private async checkPort(host: string, port: number): Promise<boolean> {
    return new Promise((resolve) => {
      const socket = net.createConnection({ host, port });
      let settled = false;
      const finish = (ok: boolean) => {
        if (settled) return;
        settled = true;
        clearTimeout(timer);
        socket.destroy();
        resolve(ok);
      };

      const timer = setTimeout(() => finish(false), this.timeout);
      socket.setTimeout(this.timeout);
      socket.once("connect", () => finish(true));
      socket.once("timeout", () => finish(false));
      socket.once("error", () => finish(false));
      socket.once("close", () => finish(false));
      socket.on("data", () => {
        // no-op: we only care if the TCP port accepts a connection.
      });
    });
  }

  private async fetchHttpInfo(host: string, port: number): Promise<Partial<ScanResult>> {
    try {
      const response = await fetch(`http://${host}:${port}`, {
        signal: AbortSignal.timeout(2000),
        headers: {
          "User-Agent": "Node-Elysia-Scanner/1.0",
          Connection: "close",
        },
      });

      const headers: Record<string, string> = {};
      response.headers.forEach((value, key) => {
        headers[key.toLowerCase()] = value;
      });

      let body = "";
      if (response.status === 200 || response.status === 401) {
        body = await response.text();
      }

      return {
        httpStatus: response.status,
        headers,
        requiresBasicAuth: headersRequireBasicAuth(headers),
        body,
      };
    } catch (error) {
      return { error: (error as Error).message };
    }
  }

  async runScan(host: string, options: ScanOptions): Promise<ScanResult[]> {
    const portsToScan = buildScanPortList(options);

    const finalResults: ScanResult[] = [];
    const batchSize = this.maxConcurrent;

    for (let i = 0; i < portsToScan.length; i += batchSize) {
      const batch = portsToScan.slice(i, i + batchSize);
      const tcpPromises = batch.map(async (port) => {
        const isOpen = await this.checkPort(host, port);
        return { port, open: isOpen };
      });

      const tcpResults = await Promise.all(tcpPromises);
      const openPorts = tcpResults.filter((r) => r.open).map((r) => r.port);

      const httpPromises = openPorts.map(async (port) => {
        const httpInfo = await this.fetchHttpInfo(host, port);
        return {
          host,
          port,
          open: true,
          ...httpInfo,
        } as ScanResult;
      });

      const httpResults = await Promise.all(httpPromises);
      finalResults.push(...httpResults);
    }

    return finalResults;
  }

  async runScanMany(
    hosts: string[],
    options: ScanOptions = {},
  ): Promise<ScanResult[]> {
    const normalizedHosts = hosts.map((host) => host.trim()).filter(Boolean);
    if (normalizedHosts.length === 0) {
      return [];
    }

    const hostBatchSize = Math.max(1, options.hostConcurrency || 1);
    const results: ScanResult[] = [];

    for (let index = 0; index < normalizedHosts.length; index += hostBatchSize) {
      const hostBatch = normalizedHosts.slice(index, index + hostBatchSize);
      const batchResults = await Promise.all(
        hostBatch.map((host) => this.runScan(host, options)),
      );
      results.push(...batchResults.flat());
    }

    return results;
  }
}
