import { normalizeCidrLines } from "../../../../../packages/admin-shared/src/utils/cidr";
import { goBackend, type GoResponse } from "../go-backend";
import { normalizeIp } from "../ip-normalize";
import {
  getCapabilityUnavailableMessage,
  getRuntimeCapabilities,
} from "../runtime-profile";
import { DEFAULT_SSH_PORTS, sshPortResolver } from "./port-resolver";
import { tDefault } from "../i18n";

const SSH_FIREWALL_CHAIN = "FN-KNOCK-SSH";
const SSH_FIREWALL_PARENT_CHAINS = ["INPUT", "DOCKER-USER"] as const;
const sshSecurityT = (
  key: string,
  params?: Record<string, string | number | boolean | null | undefined>,
) => tDefault(`server.sshSecurity.${key}`, params);

export interface SSHFirewallPolicyInput {
  allowedCidrs: string[];
  blockedIps?: string[];
  ports?: number[];
  refreshPorts?: boolean;
}

export interface SSHFirewallPolicyResult {
  allowedCidrs: string[];
  blockedIps: string[];
  ports: number[];
}

const goResponseOk = (response: GoResponse): boolean =>
  response.success === true;

const normalizePortList = (
  ports: Iterable<unknown> | null | undefined,
): number[] => {
  const normalized = [
    ...new Set(
      [...(ports ?? [])]
        .map((item) => Number.parseInt(String(item ?? ""), 10))
        .filter((port) => Number.isInteger(port) && port > 0 && port <= 65535),
    ),
  ].sort((left, right) => left - right);
  return normalized.length > 0 ? normalized : [...DEFAULT_SSH_PORTS];
};

const normalizeIpList = (ips: Iterable<unknown> | null | undefined) => [
  ...new Set(
    [...(ips ?? [])]
      .map((ip) => normalizeIp(String(ip ?? "")))
      .filter((ip): ip is string => Boolean(ip)),
  ),
];

export class SSHSecurityFirewall {
  ensureAvailable(): void {
    if (!getRuntimeCapabilities().host_firewall_available) {
      throw new Error(
        getCapabilityUnavailableMessage("host_firewall_available"),
      );
    }
  }

  async getSSHPorts(refresh = false): Promise<number[]> {
    return (await sshPortResolver.resolve({ refresh })).ports;
  }

  async syncSSHPolicy(
    input: SSHFirewallPolicyInput,
  ): Promise<SSHFirewallPolicyResult> {
    this.ensureAvailable();
    const ports = input.ports?.length
      ? normalizePortList(input.ports)
      : await this.getSSHPorts(input.refreshPorts === true);
    const allowedCidrs = normalizeCidrLines(input.allowedCidrs ?? []);
    const blockedIps = normalizeIpList(input.blockedIps ?? []);

    if (allowedCidrs.length === 0 && blockedIps.length === 0) {
      await this.clearSSHPolicy();
      return {
        allowedCidrs,
        blockedIps,
        ports,
      };
    }

    const response = await goBackend.syncSSHFirewall({
      chain_name: SSH_FIREWALL_CHAIN,
      parent_chain: [...SSH_FIREWALL_PARENT_CHAINS],
      ports,
      allowed_cidrs: allowedCidrs,
      blocked_ips: blockedIps,
      include_local_cidrs: true,
    });
    if (!goResponseOk(response)) {
      throw new Error(response.message || sshSecurityT("syncSshPolicyFailed"));
    }

    return {
      allowedCidrs,
      blockedIps,
      ports,
    };
  }

  async clearSSHPolicy(): Promise<void> {
    this.ensureAvailable();
    const response = await goBackend.clearSSHFirewall({
      chain_name: SSH_FIREWALL_CHAIN,
      parent_chain: [...SSH_FIREWALL_PARENT_CHAINS],
    });
    if (!goResponseOk(response)) {
      throw new Error(response.message || sshSecurityT("clearSshPolicyFailed"));
    }
  }
}

export const sshSecurityFirewall = new SSHSecurityFirewall();
