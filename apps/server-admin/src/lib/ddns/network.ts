import { spawn } from "node:child_process";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { networkInterfaces, tmpdir } from "node:os";
import { join } from "node:path";
import type { DDNSHttpClient, DDNSNetworkInterfaceAddress, DDNSNetworkInterfaceOption } from "./types";

export const DDNS_NETWORK_INTERFACE_FIELD = "network_interface";
export const DEFAULT_DDNS_NETWORK_INTERFACE = "";

type DDNSAddressFamily = 4 | 6;

type DDNSFetchInit = RequestInit & {
  networkInterface?: string | null;
  preferredFamily?: DDNSAddressFamily;
};

function isUsableIPv4(address: string): boolean {
  return !(address.startsWith("127.") || address.startsWith("169.254."));
}

function isUsableIPv6(address: string): boolean {
  const normalized = address.replace(/:/g, "").toLowerCase();
  return !(
    normalized === "1"
    || normalized.startsWith("fe8")
    || normalized.startsWith("fe9")
    || normalized.startsWith("fea")
    || normalized.startsWith("feb")
  );
}

function toAddressFamily(value: string | number): DDNSAddressFamily | null {
  if (value === "IPv4" || value === 4) {
    return 4;
  }
  if (value === "IPv6" || value === 6) {
    return 6;
  }
  return null;
}

function formatAddressSummary(addresses: DDNSNetworkInterfaceAddress[]): string {
  return addresses
    .map((item) => `${item.family === "ipv4" ? "IPv4" : "IPv6"}: ${item.address}`)
    .join(" / ");
}

function toNetworkInterfaceOption(
  name: string,
  items: NonNullable<ReturnType<typeof networkInterfaces>[string]>,
): DDNSNetworkInterfaceOption | null {
  const addresses: DDNSNetworkInterfaceAddress[] = items.flatMap((item) => {
    const family = toAddressFamily(item.family);
    if (!family || item.internal) {
      return [];
    }

    if (family === 4 && !isUsableIPv4(item.address)) {
      return [];
    }

    if (family === 6 && !isUsableIPv6(item.address)) {
      return [];
    }

    return [{
      family: family === 4 ? "ipv4" : "ipv6",
      address: item.address,
      cidr: item.cidr ?? null,
      internal: item.internal,
    }];
  });

  if (addresses.length === 0) {
    return null;
  }

  const summary = formatAddressSummary(addresses);
  return {
    name,
    label: `${name} (${summary})`,
    summary,
    hasIpv4: addresses.some((item) => item.family === "ipv4"),
    hasIpv6: addresses.some((item) => item.family === "ipv6"),
    addresses,
  };
}

export function normalizeNetworkInterface(value: string | null | undefined): string {
  return value?.trim() || DEFAULT_DDNS_NETWORK_INTERFACE;
}

export function listDDNSNetworkInterfaces(): DDNSNetworkInterfaceOption[] {
  return Object.entries(networkInterfaces())
    .map(([name, items]) => {
      if (!items) {
        return null;
      }
      return toNetworkInterfaceOption(name, items);
    })
    .filter((item): item is DDNSNetworkInterfaceOption => item !== null)
    .sort((left, right) => left.name.localeCompare(right.name));
}

function ensureNetworkInterfaceExists(interfaceName: string): void {
  const selected = listDDNSNetworkInterfaces().find((item) => item.name === interfaceName);
  if (!selected) {
    throw new Error(`未找到可用网卡: ${interfaceName}`);
  }
}

function getPreferredFamilyArgs(preferredFamily?: DDNSAddressFamily): string[] {
  if (preferredFamily === 4) {
    return ["-4"];
  }
  if (preferredFamily === 6) {
    return ["-6"];
  }
  return [];
}

function parseStatusLine(line: string): { status: number; statusText: string } {
  const match = line.match(/^HTTP\/\S+\s+(\d{3})(?:\s+(.*))?$/i);
  if (!match) {
    throw new Error(`无法解析 curl 响应状态行: ${line}`);
  }

  return {
    status: Number(match[1]),
    statusText: (match[2] || "").trim(),
  };
}

function parseCurlHeaders(rawHeaders: string): {
  status: number;
  statusText: string;
  headers: Headers;
} {
  const normalized = rawHeaders.replace(/\r\n/g, "\n").trim();
  if (!normalized) {
    throw new Error("curl 未返回任何响应头");
  }

  const blocks = normalized
    .split(/\n\n(?=HTTP\/)/)
    .map((item) => item.trim())
    .filter(Boolean);
  const finalBlock = blocks.at(-1) || normalized;
  const lines = finalBlock.split("\n");
  const { status, statusText } = parseStatusLine(lines[0] || "");
  const headers = new Headers();

  for (const line of lines.slice(1)) {
    if (!line) {
      continue;
    }
    const separatorIndex = line.indexOf(":");
    if (separatorIndex <= 0) {
      continue;
    }
    const key = line.slice(0, separatorIndex).trim();
    const value = line.slice(separatorIndex + 1).trim();
    headers.append(key, value);
  }

  return { status, statusText, headers };
}

function createAbortError(signal: AbortSignal | null | undefined): Error {
  const reason = signal?.reason;
  if (reason instanceof Error) {
    return reason;
  }
  if (typeof reason === "string" && reason) {
    return new Error(reason);
  }
  return new Error("请求已取消");
}

async function readRequestBody(request: Request): Promise<Buffer | null> {
  if (!request.body) {
    return null;
  }

  const body = Buffer.from(await request.arrayBuffer());
  return body.length > 0 ? body : null;
}

async function executeCurl(
  args: string[],
  body: Buffer | null,
  signal: AbortSignal | null | undefined,
): Promise<void> {
  await new Promise<void>((resolve, reject) => {
    const child = spawn("curl", args, {
      stdio: ["pipe", "ignore", "pipe"],
    });
    let stderr = "";
    let settled = false;
    let killTimer: NodeJS.Timeout | null = null;

    const cleanup = () => {
      if (killTimer) {
        clearTimeout(killTimer);
        killTimer = null;
      }
      signal?.removeEventListener("abort", onAbort);
    };

    const finish = (callback: () => void) => {
      if (settled) {
        return;
      }
      settled = true;
      cleanup();
      callback();
    };

    const onAbort = () => {
      child.kill("SIGTERM");
      killTimer = setTimeout(() => {
        child.kill("SIGKILL");
      }, 250);
      killTimer.unref();
      finish(() => reject(createAbortError(signal)));
    };

    if (signal?.aborted) {
      onAbort();
      return;
    }

    signal?.addEventListener("abort", onAbort, { once: true });
    child.stderr.setEncoding("utf8");
    child.stderr.on("data", (chunk: string) => {
      stderr += chunk;
    });

    child.on("error", (error) => {
      finish(() => reject(error));
    });

    child.on("close", (code, closeSignal) => {
      finish(() => {
        if (code === 0) {
          resolve();
          return;
        }
        const detail = stderr.trim() || closeSignal || `exit ${code ?? "unknown"}`;
        reject(new Error(`curl 请求失败: ${detail}`));
      });
    });

    if (body) {
      child.stdin.end(body);
      return;
    }

    child.stdin.end();
  });
}

async function fetchViaCurl(
  request: Request,
  options: {
    networkInterface?: string;
    preferredFamily?: DDNSAddressFamily;
  },
): Promise<Response> {
  const tempDir = await mkdtemp(join(tmpdir(), "ddns-curl-"));
  const headerPath = join(tempDir, "headers.txt");
  const bodyPath = join(tempDir, "body.bin");

  try {
    const requestForCurl = request.clone();
    const body = await readRequestBody(requestForCurl);
    const args = [
      "--silent",
      "--show-error",
      "--location",
      ...getPreferredFamilyArgs(options.preferredFamily),
      "--dump-header",
      headerPath,
      "--output",
      bodyPath,
      "--request",
      request.method,
    ];

    if (options.networkInterface) {
      args.push("--interface", options.networkInterface);
    }

    request.headers.forEach((value, key) => {
      args.push("--header", `${key}: ${value}`);
    });

    if (body) {
      args.push("--data-binary", "@-");
    }

    args.push(request.url);

    await executeCurl(args, body, request.signal);

    const [rawHeaders, responseBody] = await Promise.all([
      readFile(headerPath, "utf8"),
      readFile(bodyPath).catch(() => Buffer.alloc(0)),
    ]);
    const { status, statusText, headers } = parseCurlHeaders(rawHeaders);

    return new Response(responseBody, {
      status,
      statusText,
      headers,
    });
  } finally {
    await rm(tempDir, { recursive: true, force: true });
  }
}

export async function ddnsFetch(
  input: RequestInfo | URL,
  init: DDNSFetchInit = {},
): Promise<Response> {
  const { networkInterface, preferredFamily, ...requestInit } = init;
  const normalizedInterface = normalizeNetworkInterface(networkInterface);
  const request = new Request(input, requestInit);

  if (normalizedInterface) {
    ensureNetworkInterfaceExists(normalizedInterface);
  }

  return fetchViaCurl(request, {
    networkInterface: normalizedInterface || undefined,
    preferredFamily,
  });
}

export function createDDNSHttpClient(options: {
  networkInterface?: string | null;
} = {}): DDNSHttpClient {
  return {
    fetch(input: RequestInfo | URL, init?: RequestInit) {
      return ddnsFetch(input, {
        ...init,
        networkInterface: options.networkInterface,
      });
    },
  };
}
