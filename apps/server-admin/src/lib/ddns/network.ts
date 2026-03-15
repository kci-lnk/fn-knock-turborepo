import { networkInterfaces } from "node:os";
import { Agent } from "undici";
import type { DDNSHttpClient, DDNSNetworkInterfaceAddress, DDNSNetworkInterfaceOption } from "./types";

export const DDNS_NETWORK_INTERFACE_FIELD = "network_interface";
export const DEFAULT_DDNS_NETWORK_INTERFACE = "";

type DDNSAddressFamily = 4 | 6;

type DDNSFetchInit = RequestInit & {
  networkInterface?: string | null;
  preferredFamily?: DDNSAddressFamily;
};

type DDNSBinding = {
  family: DDNSAddressFamily;
  localAddress: string;
};

const agentCache = new Map<string, Agent>();

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

function getBindingCandidates(
  interfaceName: string,
  preferredFamily?: DDNSAddressFamily,
): DDNSBinding[] {
  const selected = listDDNSNetworkInterfaces().find((item) => item.name === interfaceName);
  if (!selected) {
    throw new Error(`未找到可用网卡: ${interfaceName}`);
  }

  const ipv4 = selected.addresses.find((item) => item.family === "ipv4")?.address ?? null;
  const ipv6 = selected.addresses.find((item) => item.family === "ipv6")?.address ?? null;

  const bindings: DDNSBinding[] = [];
  const pushBinding = (family: DDNSAddressFamily, localAddress: string | null) => {
    if (!localAddress) {
      return;
    }
    if (bindings.some((item) => item.family === family && item.localAddress === localAddress)) {
      return;
    }
    bindings.push({ family, localAddress });
  };

  if (preferredFamily === 4) {
    pushBinding(4, ipv4);
    pushBinding(6, ipv6);
  } else if (preferredFamily === 6) {
    pushBinding(6, ipv6);
    pushBinding(4, ipv4);
  } else {
    pushBinding(4, ipv4);
    pushBinding(6, ipv6);
  }

  if (bindings.length === 0) {
    throw new Error(`网卡 ${interfaceName} 没有可用于 DDNS 出站请求的地址`);
  }

  return bindings;
}

function getAgent(localAddress: string): Agent {
  const cached = agentCache.get(localAddress);
  if (cached) {
    return cached;
  }

  const agent = new Agent({
    connect: {
      localAddress,
    },
  });
  agentCache.set(localAddress, agent);
  return agent;
}

export async function ddnsFetch(
  input: RequestInfo | URL,
  init: DDNSFetchInit = {},
): Promise<Response> {
  const { networkInterface, preferredFamily, ...requestInit } = init;
  const normalizedInterface = normalizeNetworkInterface(networkInterface);
  const request = new Request(input, requestInit);

  if (!normalizedInterface) {
    return fetch(request);
  }

  const candidates = getBindingCandidates(normalizedInterface, preferredFamily);
  let lastError: unknown = null;

  for (const candidate of candidates) {
    try {
      return await fetch(request.clone(), {
        dispatcher: getAgent(candidate.localAddress),
      } as RequestInit & { dispatcher: Agent });
    } catch (error) {
      lastError = error;
    }
  }

  const message = lastError instanceof Error ? lastError.message : String(lastError);
  const tried = candidates.map((item) => item.localAddress).join(", ");
  throw new Error(`网卡 ${normalizedInterface} 出站请求失败（已尝试: ${tried}）: ${message}`);
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
