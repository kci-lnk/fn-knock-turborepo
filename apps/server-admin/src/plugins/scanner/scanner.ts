import { ScanOptions, ScanResult } from "./types";
import net from "node:net";
import { headersRequireBasicAuth } from "../../lib/basic-auth-probe";

const LUCI_LOGIN_REQUIRED_HEADER = "x-luci-login-required";
const DEFAULT_SCAN_PORT_START = 80;
const DEFAULT_SCAN_PORT_END = 60000;
const HTTP_PROBE_HEADERS = {
  "User-Agent": "Node-Elysia-Scanner/1.0",
  Connection: "close",
} as const;

const isLuciLoginRequiredResponse = (
  status: number,
  headers: Record<string, string>,
): boolean =>
  status === 403 &&
  headers[LUCI_LOGIN_REQUIRED_HEADER]?.trim().toLowerCase() === "yes";

export const buildScanPortList = (options: ScanOptions): number[] => {
  let portsToScan: number[] = [];
  if (options.portRanges && options.portRanges.length > 0) {
    for (const range of options.portRanges) {
      for (let port = range.start; port <= range.end; port++) {
        portsToScan.push(port);
      }
    }
  } else {
    portsToScan = Array.from(
      { length: DEFAULT_SCAN_PORT_END - DEFAULT_SCAN_PORT_START + 1 },
      (_, i) => i + DEFAULT_SCAN_PORT_START,
    );
  }

  const skipSet = new Set(options.skipPorts || []);
  return portsToScan.filter((port) => !skipSet.has(port));
};

export const isAbortError = (error: unknown): boolean =>
  error instanceof Error && error.name === "AbortError";

const createAbortError = (): Error => {
  const error = new Error("Scan aborted");
  error.name = "AbortError";
  return error;
};

const throwIfAborted = (signal?: AbortSignal) => {
  if (signal?.aborted) {
    throw createAbortError();
  }
};

export class ScannerLogic {
  private timeout: number;
  private maxConcurrent: number;

  constructor(options: ScanOptions = {}) {
    this.timeout = options.timeout || 70;
    this.maxConcurrent = options.maxConcurrent || 100;
  }

  // 1. TCP 端口检测 (第一阶段)
  private async checkPort(
    host: string,
    port: number,
    signal?: AbortSignal,
  ): Promise<boolean> {
    throwIfAborted(signal);

    return new Promise((resolve) => {
      const socket = net.createConnection({ host, port });
      let settled = false;
      let timer: ReturnType<typeof setTimeout>;
      const finish = (ok: boolean) => {
        if (settled) return;
        settled = true;
        clearTimeout(timer);
        signal?.removeEventListener("abort", onAbort);
        socket.destroy();
        resolve(ok);
      };

      const onAbort = () => finish(false);
      timer = setTimeout(() => finish(false), this.timeout);
      signal?.addEventListener("abort", onAbort, { once: true });
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

  private async fetchHttpInfo(
    host: string,
    port: number,
    signal?: AbortSignal,
  ): Promise<Partial<ScanResult>> {
    try {
      throwIfAborted(signal);
      const timeoutSignal = AbortSignal.timeout(2000);
      const requestSignal = signal
        ? AbortSignal.any([signal, timeoutSignal])
        : timeoutSignal;
      const url = `http://${host}:${port}`;
      const fetchHttp = (redirect: RequestRedirect) =>
        fetch(url, {
          signal: requestSignal,
          headers: HTTP_PROBE_HEADERS,
          redirect,
        });
      let response: Response;
      try {
        response = await fetchHttp("follow");
      } catch (error) {
        if (isAbortError(error) || signal?.aborted || timeoutSignal.aborted) {
          throw error;
        }
        response = await fetchHttp("manual");
      }

      const headers: Record<string, string> = {};
      response.headers.forEach((value, key) => {
        headers[key.toLowerCase()] = value;
      });

      let body = "";
      try {
        body = await response.text();
      } catch (error) {
        if (isAbortError(error) || signal?.aborted || timeoutSignal.aborted) {
          throw error;
        }
      }

      return {
        httpStatus: response.status,
        headers,
        requiresBasicAuth: headersRequireBasicAuth(headers),
        body,
      };
    } catch (error) {
      if (isAbortError(error)) {
        throw error;
      }
      return { error: (error as Error).message };
    }
  }

  async runScan(host: string, options: ScanOptions): Promise<ScanResult[]> {
    const portsToScan = buildScanPortList(options);
    const totalPorts = portsToScan.length;
    let scannedPorts = 0;

    const finalResults: ScanResult[] = [];
    const batchSize = this.maxConcurrent;

    for (let i = 0; i < portsToScan.length; i += batchSize) {
      throwIfAborted(options.signal);
      const batch = portsToScan.slice(i, i + batchSize);
      const tcpPromises = batch.map(async (port) => {
        const isOpen = await this.checkPort(host, port, options.signal);
        scannedPorts += 1;
        options.onPortScanned?.({
          host,
          port,
          scannedPorts,
          totalPorts,
        });
        return { port, open: isOpen };
      });

      const tcpResults = await Promise.all(tcpPromises);
      throwIfAborted(options.signal);
      const openPorts = tcpResults.filter((r) => r.open).map((r) => r.port);

      const httpPromises = openPorts.map(async (port) => {
        const httpInfo = await this.fetchHttpInfo(host, port, options.signal);
        return {
          host,
          port,
          open: true,
          ...httpInfo,
        } as ScanResult;
      });

      const httpResults = await Promise.all(httpPromises);
      if (options.onResult) {
        await Promise.all(
          httpResults.map((result) => options.onResult!(result)),
        );
      }
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

    for (
      let index = 0;
      index < normalizedHosts.length;
      index += hostBatchSize
    ) {
      const hostBatch = normalizedHosts.slice(index, index + hostBatchSize);
      const batchResults = await Promise.all(
        hostBatch.map((host) => this.runScan(host, options)),
      );
      results.push(...batchResults.flat());
    }

    return results;
  }
}
