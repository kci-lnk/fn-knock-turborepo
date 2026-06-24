import { spawn } from "node:child_process";
import { readFileSync } from "node:fs";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { request as httpRequest } from "node:http";
import { request as httpsRequest } from "node:https";
import { isIP } from "node:net";
import { networkInterfaces, tmpdir } from "node:os";
import { join } from "node:path";
import type {
  DDNSHttpTransport,
  DDNSHttpClient,
  DDNSNetworkInterfaceAddress,
  DDNSNetworkInterfaceOption,
} from "./types";
import { ddnsTranslate } from "./providers/helpers";

export const DDNS_NETWORK_INTERFACE_FIELD = "network_interface";
export const DEFAULT_DDNS_NETWORK_INTERFACE = "";

type DDNSAddressFamily = 4 | 6;

type DDNSFetchInit = RequestInit & {
  networkInterface?: string | null;
  preferredFamily?: DDNSAddressFamily;
  transport?: DDNSHttpTransport;
};

const CURL_PROXY_ENV_KEYS = [
  "http_proxy",
  "https_proxy",
  "all_proxy",
  "HTTP_PROXY",
  "HTTPS_PROXY",
  "ALL_PROXY",
  "no_proxy",
  "NO_PROXY",
] as const;

const DOCKER_HOST_INTERFACE_PREFIX = "docker-host:";
const DEFAULT_DOCKER_HOST_IF_INET6_PATH = "/host/proc/net/if_inet6";
const HOST_IF_INET6_PATH =
  process.env.DDNS_HOST_IF_INET6_PATH || DEFAULT_DOCKER_HOST_IF_INET6_PATH;

const ddnsT = ddnsTranslate;
const NODE_FETCH_MAX_REDIRECTS = 20;

const IFA_F_TEMPORARY = 0x01;
const IFA_F_DEPRECATED = 0x20;
const IFA_F_TENTATIVE = 0x40;
const IFA_F_DADFAILED = 0x08;

type HostIPv6Address = DDNSNetworkInterfaceAddress & {
  flags: number;
  prefixLength: number;
};

function isUsableIPv4(address: string): boolean {
  return !(address.startsWith("127.") || address.startsWith("169.254."));
}

function isUsableIPv6(address: string): boolean {
  const normalized = address.replace(/:/g, "").toLowerCase();
  return !(
    normalized === "1" ||
    normalized.startsWith("fe8") ||
    normalized.startsWith("fe9") ||
    normalized.startsWith("fea") ||
    normalized.startsWith("feb")
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

function formatAddressSummary(
  addresses: DDNSNetworkInterfaceAddress[],
): string {
  return addresses
    .map(
      (item) => `${item.family === "ipv4" ? "IPv4" : "IPv6"}: ${item.address}`,
    )
    .join(" / ");
}

function parseIPv4Octets(address: string): number[] | null {
  const parts = address.split(".");
  if (parts.length !== 4) {
    return null;
  }

  const octets = parts.map((part) => Number(part));
  if (
    octets.some((octet) => !Number.isInteger(octet) || octet < 0 || octet > 255)
  ) {
    return null;
  }

  return octets;
}

function isPrivateIPv4(address: string): boolean {
  const octets = parseIPv4Octets(address);
  if (!octets) {
    return false;
  }

  const first = octets[0];
  const second = octets[1];
  return (
    first === 10 ||
    (first === 172 && second !== undefined && second >= 16 && second <= 31) ||
    (first === 192 && second === 168)
  );
}

function isUniqueLocalIPv6(address: string): boolean {
  const normalized = address.split("%")[0]?.toLowerCase() || "";
  return normalized.startsWith("fc") || normalized.startsWith("fd");
}

function isDockerHostInterfaceName(value: string): boolean {
  return value.startsWith(DOCKER_HOST_INTERFACE_PREFIX);
}

function toCurlNetworkInterface(value: string): string | undefined {
  return isDockerHostInterfaceName(value) ? undefined : value || undefined;
}

function normalizeRequestFamily(
  value: DDNSAddressFamily | undefined,
): DDNSAddressFamily | undefined {
  return value === 4 || value === 6 ? value : undefined;
}

export function isSelectableDDNSInterfaceAddress(
  address: DDNSNetworkInterfaceAddress,
): boolean {
  if (address.family === "ipv4") {
    return !isPrivateIPv4(address.address);
  }

  return !isUniqueLocalIPv6(address.address);
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

    return [
      {
        family: family === 4 ? "ipv4" : "ipv6",
        address: item.address,
        cidr: item.cidr ?? null,
        internal: item.internal,
        source: "runtime",
      },
    ];
  });

  if (addresses.length === 0) {
    return null;
  }

  const selectableAddresses = addresses.filter(
    isSelectableDDNSInterfaceAddress,
  );
  const summary = formatAddressSummary(addresses);
  return {
    name,
    label: `${name} (${summary})`,
    summary,
    hasIpv4: addresses.some((item) => item.family === "ipv4"),
    hasIpv6: addresses.some((item) => item.family === "ipv6"),
    source: "runtime",
    addresses,
    selectableAddresses,
  };
}

function formatIPv6FromProcHex(value: string): string | null {
  if (!/^[0-9a-fA-F]{32}$/.test(value)) {
    return null;
  }

  const groups = value
    .match(/.{4}/g)
    ?.map((group) => Number.parseInt(group, 16).toString(16));
  if (!groups || groups.length !== 8) {
    return null;
  }

  let bestStart = -1;
  let bestLength = 0;
  let currentStart = -1;
  let currentLength = 0;

  for (let index = 0; index <= groups.length; index += 1) {
    if (index < groups.length && groups[index] === "0") {
      if (currentStart === -1) {
        currentStart = index;
        currentLength = 0;
      }
      currentLength += 1;
      continue;
    }

    if (currentLength > bestLength) {
      bestStart = currentStart;
      bestLength = currentLength;
    }
    currentStart = -1;
    currentLength = 0;
  }

  if (bestLength < 2) {
    return groups.join(":");
  }

  const before = groups.slice(0, bestStart).join(":");
  const after = groups.slice(bestStart + bestLength).join(":");
  if (!before && !after) {
    return "::";
  }
  if (!before) {
    return `::${after}`;
  }
  if (!after) {
    return `${before}::`;
  }
  return `${before}::${after}`;
}

function hostIPv6SortScore(address: HostIPv6Address): number {
  let score = 0;
  if (address.flags & IFA_F_DADFAILED) score += 200;
  if (address.flags & IFA_F_TENTATIVE) score += 150;
  if (address.flags & IFA_F_DEPRECATED) score += 100;
  if (address.flags & IFA_F_TEMPORARY) score += 25;
  if (address.prefixLength === 128) score -= 5;
  return score;
}

function parseHostIfInet6(content: string): DDNSNetworkInterfaceOption[] {
  const byInterface = new Map<string, HostIPv6Address[]>();

  for (const line of content.split(/\r?\n/)) {
    const [addressHex, , prefixHex, scopeHex, flagsHex, interfaceName] = line
      .trim()
      .split(/\s+/);
    if (!addressHex || !prefixHex || !scopeHex || !flagsHex || !interfaceName) {
      continue;
    }

    const address = formatIPv6FromProcHex(addressHex);
    const prefixLength = Number.parseInt(prefixHex, 16);
    const scope = Number.parseInt(scopeHex, 16);
    const flags = Number.parseInt(flagsHex, 16);
    if (
      !address ||
      !Number.isFinite(prefixLength) ||
      !Number.isFinite(scope) ||
      !Number.isFinite(flags) ||
      scope !== 0 ||
      !isUsableIPv6(address)
    ) {
      continue;
    }

    const items = byInterface.get(interfaceName) || [];
    items.push({
      family: "ipv6",
      address,
      cidr: `${address}/${prefixLength}`,
      internal: false,
      source: "docker_host",
      flags,
      prefixLength,
    });
    byInterface.set(interfaceName, items);
  }

  return [...byInterface.entries()]
    .map(([hostName, addresses]) => {
      const sortedAddresses = [...addresses]
        .sort((left, right) => {
          const scoreDelta = hostIPv6SortScore(left) - hostIPv6SortScore(right);
          if (scoreDelta !== 0) return scoreDelta;
          return left.address.localeCompare(right.address);
        })
        .map(({ flags, prefixLength, ...item }) => item);
      const selectableAddresses = sortedAddresses.filter(
        isSelectableDDNSInterfaceAddress,
      );
      const summary = formatAddressSummary(sortedAddresses);
      return {
        name: `${DOCKER_HOST_INTERFACE_PREFIX}${hostName}`,
        label: ddnsT("dockerHostInterfaceLabel", { name: hostName, summary }),
        summary,
        source: "docker_host" as const,
        hasIpv4: false,
        hasIpv6: sortedAddresses.length > 0,
        addresses: sortedAddresses,
        selectableAddresses,
      };
    })
    .filter((item) => item.selectableAddresses.length > 0);
}

function listDockerHostIPv6Interfaces(): DDNSNetworkInterfaceOption[] {
  try {
    return parseHostIfInet6(readFileSync(HOST_IF_INET6_PATH, "utf8"));
  } catch {
    return [];
  }
}

export function normalizeNetworkInterface(
  value: string | null | undefined,
): string {
  return value?.trim() || DEFAULT_DDNS_NETWORK_INTERFACE;
}

export function listDDNSNetworkInterfaces(): DDNSNetworkInterfaceOption[] {
  const runtimeInterfaces = Object.entries(networkInterfaces())
    .map(([name, items]) => {
      if (!items) {
        return null;
      }
      return toNetworkInterfaceOption(name, items);
    })
    .filter((item): item is DDNSNetworkInterfaceOption => item !== null)
    .sort((left, right) => left.name.localeCompare(right.name));

  return [...listDockerHostIPv6Interfaces(), ...runtimeInterfaces].sort(
    (left, right) => {
      if (left.source !== right.source) {
        return left.source === "docker_host" ? -1 : 1;
      }
      return left.name.localeCompare(right.name);
    },
  );
}

export function findDDNSNetworkInterface(
  interfaceName: string | null | undefined,
): DDNSNetworkInterfaceOption | null {
  const normalizedName = normalizeNetworkInterface(interfaceName);
  if (!normalizedName) {
    return null;
  }

  return (
    listDDNSNetworkInterfaces().find((item) => item.name === normalizedName) ||
    null
  );
}

export function listSelectableDDNSInterfaceAddresses(
  interfaceName: string,
  family: DDNSNetworkInterfaceAddress["family"],
): DDNSNetworkInterfaceAddress[] {
  const selected = findDDNSNetworkInterface(interfaceName);
  if (!selected) {
    return [];
  }

  return selected.selectableAddresses.filter((item) => item.family === family);
}

function ensureNetworkInterfaceExists(interfaceName: string): void {
  const selected = findDDNSNetworkInterface(interfaceName);
  if (!selected) {
    throw new Error(ddnsT("interfaceNotFound", { name: interfaceName }));
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
    throw new Error(ddnsT("curlStatusLineParseFailed", { line }));
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
    throw new Error(ddnsT("curlNoHeaders"));
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
  return new Error(ddnsT("requestCanceled"));
}

async function readRequestBody(request: Request): Promise<Buffer | null> {
  if (!request.body) {
    return null;
  }

  const body = Buffer.from(await request.arrayBuffer());
  return body.length > 0 ? body : null;
}

type NodeRequestAttempt = {
  family?: DDNSAddressFamily;
  localAddress?: string;
};

function getFamilyLabel(family: DDNSAddressFamily): string {
  return family === 4 ? "IPv4" : "IPv6";
}

function getNodeInterfaceAddresses(
  interfaceName: string,
  family: DDNSAddressFamily,
): string[] {
  const selected = findDDNSNetworkInterface(interfaceName);
  if (!selected || selected.source === "docker_host") {
    return [];
  }

  const expectedFamily = family === 4 ? "ipv4" : "ipv6";
  return selected.addresses
    .filter(
      (item) =>
        item.family === expectedFamily && isIP(item.address) === family,
    )
    .map((item) => item.address);
}

function buildNodeRequestAttempts(
  options: {
    networkInterface?: string;
    preferredFamily?: DDNSAddressFamily;
  },
): NodeRequestAttempt[] {
  const preferredFamily = normalizeRequestFamily(options.preferredFamily);
  const normalizedInterface = normalizeNetworkInterface(
    options.networkInterface,
  );

  if (!normalizedInterface || isDockerHostInterfaceName(normalizedInterface)) {
    return preferredFamily ? [{ family: preferredFamily }] : [{}];
  }

  if (preferredFamily) {
    const [localAddress] = getNodeInterfaceAddresses(
      normalizedInterface,
      preferredFamily,
    );
    if (!localAddress) {
      throw new Error(
        ddnsT("nodeTransportInterfaceAddressUnavailable", {
          name: normalizedInterface,
          family: getFamilyLabel(preferredFamily),
        }),
      );
    }

    return [{ family: preferredFamily, localAddress }];
  }

  const attempts: NodeRequestAttempt[] = [];
  const [ipv4LocalAddress] = getNodeInterfaceAddresses(normalizedInterface, 4);
  const [ipv6LocalAddress] = getNodeInterfaceAddresses(normalizedInterface, 6);
  if (ipv4LocalAddress) {
    attempts.push({ family: 4, localAddress: ipv4LocalAddress });
  }
  if (ipv6LocalAddress) {
    attempts.push({ family: 6, localAddress: ipv6LocalAddress });
  }

  if (attempts.length === 0) {
    throw new Error(
      ddnsT("nodeTransportInterfaceNoAddress", {
        name: normalizedInterface,
      }),
    );
  }

  return attempts;
}

function toNodeRequestHeaders(headers: Headers): Record<string, string> {
  const result: Record<string, string> = {};
  headers.forEach((value, key) => {
    result[key] = value;
  });
  return result;
}

function toFetchHeaders(headers: Record<string, string | string[] | undefined>) {
  const result = new Headers();
  for (const [key, value] of Object.entries(headers)) {
    if (typeof value === "string") {
      result.append(key, value);
      continue;
    }
    if (Array.isArray(value)) {
      for (const item of value) {
        result.append(key, item);
      }
    }
  }
  return result;
}

function isRedirectStatus(status: number): boolean {
  return (
    status === 301 ||
    status === 302 ||
    status === 303 ||
    status === 307 ||
    status === 308
  );
}

function shouldDropRedirectBody(status: number, method: string): boolean {
  return status === 303 && method !== "GET" && method !== "HEAD";
}

async function executeNodeRequest(
  url: URL,
  method: string,
  headers: Headers,
  body: Buffer | null,
  signal: AbortSignal | null | undefined,
  attempt: NodeRequestAttempt,
): Promise<Response> {
  if (url.protocol !== "http:" && url.protocol !== "https:") {
    throw new Error(
      ddnsT("nodeTransportUnsupportedProtocol", { protocol: url.protocol }),
    );
  }

  const transport = url.protocol === "https:" ? httpsRequest : httpRequest;
  return new Promise<Response>((resolve, reject) => {
    const req = transport(
      {
        protocol: url.protocol,
        hostname: url.hostname,
        port: url.port || undefined,
        path: `${url.pathname}${url.search}`,
        method,
        headers: toNodeRequestHeaders(headers),
        family: attempt.family,
        localAddress: attempt.localAddress,
        agent: false,
        signal: signal ?? undefined,
      },
      (res) => {
        const chunks: Buffer[] = [];
        res.on("data", (chunk) => {
          chunks.push(Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk));
        });
        res.on("error", reject);
        res.on("end", () => {
          resolve(
            new Response(Buffer.concat(chunks), {
              status: res.statusCode || 502,
              statusText: res.statusMessage || "",
              headers: toFetchHeaders(res.headers),
            }),
          );
        });
      },
    );

    req.on("error", (error) => {
      reject(error);
    });

    if (body) {
      req.end(body);
      return;
    }

    req.end();
  });
}

async function fetchViaNodeWithBody(
  url: URL,
  method: string,
  headers: Headers,
  body: Buffer | null,
  signal: AbortSignal | null | undefined,
  options: {
    networkInterface?: string;
    preferredFamily?: DDNSAddressFamily;
  },
  redirectCount: number,
): Promise<Response> {
  if (signal?.aborted) {
    throw createAbortError(signal);
  }
  if (url.protocol !== "http:" && url.protocol !== "https:") {
    throw new Error(
      ddnsT("nodeTransportUnsupportedProtocol", { protocol: url.protocol }),
    );
  }

  const attempts = buildNodeRequestAttempts(options);
  let lastError: unknown = null;
  for (const attempt of attempts) {
    let response: Response;
    try {
      response = await executeNodeRequest(
        url,
        method,
        headers,
        body,
        signal,
        attempt,
      );
    } catch (error) {
      if (signal?.aborted) {
        throw createAbortError(signal);
      }
      lastError = error;
      continue;
    }

    if (!isRedirectStatus(response.status)) {
      return response;
    }

    const location = response.headers.get("location");
    if (!location) {
      return response;
    }
    if (redirectCount >= NODE_FETCH_MAX_REDIRECTS) {
      throw new Error(
        ddnsT("nodeTransportRedirectLimitExceeded", {
          max: NODE_FETCH_MAX_REDIRECTS,
        }),
      );
    }

    const nextUrl = new URL(location, url);
    const nextHeaders = new Headers(headers);
    const dropBody = shouldDropRedirectBody(response.status, method);
    if (dropBody) {
      nextHeaders.delete("content-length");
      nextHeaders.delete("content-type");
    }

    return fetchViaNodeWithBody(
      nextUrl,
      dropBody ? "GET" : method,
      nextHeaders,
      dropBody ? null : body,
      signal,
      options,
      redirectCount + 1,
    );
  }

  throw lastError instanceof Error ? lastError : new Error(String(lastError));
}

async function fetchViaNode(
  request: Request,
  options: {
    networkInterface?: string;
    preferredFamily?: DDNSAddressFamily;
  },
): Promise<Response> {
  const body = await readRequestBody(request.clone());
  return fetchViaNodeWithBody(
    new URL(request.url),
    request.method,
    new Headers(request.headers),
    body,
    request.signal,
    options,
    0,
  );
}

async function executeCurl(
  args: string[],
  body: Buffer | null,
  signal: AbortSignal | null | undefined,
): Promise<void> {
  await new Promise<void>((resolve, reject) => {
    const env = { ...process.env };
    for (const key of CURL_PROXY_ENV_KEYS) {
      delete env[key];
    }

    const child = spawn("curl", args, {
      env,
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
        const detail =
          stderr.trim() || closeSignal || `exit ${code ?? "unknown"}`;
        reject(new Error(ddnsT("curlRequestFailed", { detail })));
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
      "-q",
      "--silent",
      "--show-error",
      "--location",
      "--proxy",
      "",
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
  const {
    networkInterface,
    preferredFamily,
    transport = "curl",
    ...requestInit
  } = init;
  const normalizedInterface = normalizeNetworkInterface(networkInterface);
  const request = new Request(input, requestInit);

  if (normalizedInterface) {
    ensureNetworkInterfaceExists(normalizedInterface);
  }

  if (transport === "node") {
    return fetchViaNode(request, {
      networkInterface: normalizedInterface,
      preferredFamily,
    });
  }

  return fetchViaCurl(request, {
    networkInterface: toCurlNetworkInterface(normalizedInterface),
    preferredFamily,
  });
}

export function createDDNSHttpClient(
  options: {
    networkInterface?: string | null;
    transport?: DDNSHttpTransport;
  } = {},
): DDNSHttpClient {
  return {
    fetch(input: RequestInfo | URL, init?: RequestInit) {
      return ddnsFetch(input, {
        ...init,
        networkInterface: options.networkInterface,
        transport: options.transport,
      });
    },
  };
}
