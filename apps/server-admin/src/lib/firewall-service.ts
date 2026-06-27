import {
  DEFAULT_GATEWAY_CRAWLER_BLOCKER_CONFIG,
  configManager,
  DEFAULT_REVERSE_PROXY_THROTTLE_CONFIG,
  type AppConfig,
} from "./redis";
import { goBackend, type GoResponse } from "./go-backend";
import { buildGatewayAuthConfig } from "./subdomain-mode";
import { syncGatewayProxyHeadersRuntimeForConfig } from "./gateway-proxy-headers";
import { syncGatewayHostResponseRuntimeForConfig } from "./gateway-host-response";
import { syncGatewayVisibilityToGateway } from "./gateway-visibility";
import { syncGatewayCrawlerBlockerToGateway } from "./gateway-crawler-blocker";
import { syncReverseProxyTrustedIPsNow } from "./reverse-proxy-trusted-ips";
import { whitelistManager } from "./whitelist-manager";
import { isReverseProxySubdomainMode } from "./reverse-proxy-submode";
import { shouldAutoManageFirewallForRunType } from "./firewall-automation";
import { SMART_CONNECT_DNS_PORT } from "./dnsmasq-manager";
import {
  getCapabilityUnavailableMessage,
  getRuntimeCapabilities,
} from "./runtime-profile";
import { tDefault } from "./i18n";

const DISABLED_DEFAULT_ROUTE = "/__select__";
const firewallT = (
  key: string,
  params?: Record<string, string | number | boolean | null | undefined>,
) => tDefault(`server.firewall.${key}`, params);

export class FirewallService {
  private readonly legacyRedirectedHttpPorts = [80, 443] as const;

  private ensureHostFirewallAvailable() {
    if (!getRuntimeCapabilities().host_firewall_available) {
      throw new Error(
        getCapabilityUnavailableMessage("host_firewall_available"),
      );
    }
  }

  private ensureDirectModeAvailable() {
    if (!getRuntimeCapabilities().direct_mode_available) {
      throw new Error(getCapabilityUnavailableMessage("direct_mode_available"));
    }
  }

  private assertGoBackendSuccess<T>(
    result: GoResponse<T>,
    fallbackMessage: string,
    acceptableCodes: number[] = [],
  ): GoResponse<T> {
    if (result.success) return result;
    if (result.code !== undefined && acceptableCodes.includes(result.code)) {
      return {
        ...result,
        success: true,
      } satisfies GoResponse<T>;
    }
    console.error(
      firewallT("goBackendCallFailed", { message: fallbackMessage }),
      result,
    );
    // throw new Error(fallbackMessage);
    return {
      success: false,
      code: result.code,
      message: fallbackMessage,
    } satisfies GoResponse<T>;
  }

  private async runGoBackend<T>(
    promise: Promise<GoResponse<T>>,
    fallbackMessage: string,
    acceptableCodes: number[] = [],
  ): Promise<GoResponse<T>> {
    const result = await promise;
    return this.assertGoBackendSuccess(
      result,
      fallbackMessage,
      acceptableCodes,
    );
  }

  private async runGoBackendOrThrow<T>(
    promise: Promise<GoResponse<T>>,
    fallbackMessage: string,
    acceptableCodes: number[] = [],
  ): Promise<GoResponse<T>> {
    const result = await this.runGoBackend(
      promise,
      fallbackMessage,
      acceptableCodes,
    );
    if (!result.success) {
      throw new Error(result.message || fallbackMessage);
    }
    return result;
  }

  private resolveGatewayPort(): number {
    const parsed = Number.parseInt(process.env.GO_REPROXY_PORT || "7999", 10);
    return Number.isFinite(parsed) && parsed > 0 ? parsed : 7999;
  }

  private resolveExemptPorts(
    config: AppConfig,
    protocolMappingEnabled: boolean,
    runType: 0 | 1 | 3 = config.run_type,
  ): string[] {
    const ports = new Set<string>([String(this.resolveGatewayPort())]);

    if (runType === 3 && protocolMappingEnabled) {
      for (const mapping of config.stream_mappings ?? []) {
        if (
          Number.isInteger(mapping.listen_port) &&
          mapping.listen_port > 0 &&
          mapping.listen_port <= 65535
        ) {
          ports.add(String(mapping.listen_port));
        }
      }
    }

    if (
      runType === 3 &&
      config.smart_connect?.enabled === true &&
      config.smart_connect.selected_ipv4.trim()
    ) {
      ports.add(String(SMART_CONNECT_DNS_PORT));
    }

    return [...ports];
  }

  private async clearLegacyGatewayRedirects(
    targetPort: number,
    strict = false,
  ) {
    for (const listenPort of this.legacyRedirectedHttpPorts) {
      const fallbackMessage = firewallT("clearLegacyTcpRedirectFailed", {
        listenPort,
        targetPort,
      });
      if (strict) {
        await this.runGoBackendOrThrow(
          goBackend.clearTCPRedirect(listenPort, targetPort),
          fallbackMessage,
          [404],
        );
        continue;
      }

      await this.runGoBackend(
        goBackend.clearTCPRedirect(listenPort, targetPort),
        fallbackMessage,
        [404],
      );
    }
  }

  private async initDefaultFirewall(
    config: AppConfig,
    protocolMappingEnabled: boolean,
    runType: 0 | 1 | 3 = config.run_type,
    strict = false,
  ) {
    const request = goBackend.initIptables({
      chain_name: "FN-KNOCK-FW",
      parent_chain: ["INPUT", "DOCKER-USER"],
      exempt_ports: this.resolveExemptPorts(
        config,
        protocolMappingEnabled,
        runType,
      ),
    });

    if (strict) {
      await this.runGoBackendOrThrow(
        request,
        firewallT("initDefaultRulesFailed"),
      );
      return;
    }

    await this.runGoBackend(request, firewallT("initDefaultRulesFailed"));
  }

  private async syncActiveWhitelistRecords(
    strict = false,
    source?: "manual" | "auto",
  ): Promise<number> {
    const records = await whitelistManager.getAllActiveConcreteTargets(source);

    for (const record of records) {
      const fallbackMessage = firewallT("syncWhitelistTargetFailed", {
        target: record.target,
      });
      if (strict) {
        await this.runGoBackendOrThrow(
          goBackend.allowIP(record.target),
          fallbackMessage,
        );
        continue;
      }

      await this.runGoBackend(goBackend.allowIP(record.target), fallbackMessage);
    }

    return records.length;
  }

  async resetFirewallForRunType(runType: 0 | 1 | 3) {
    this.ensureHostFirewallAvailable();
    if (runType === 0) {
      this.ensureDirectModeAvailable();
    }

    const [config, protocolMappingFeature] = await Promise.all([
      configManager.getConfig(),
      configManager.getProtocolMappingFeatureConfig(),
    ]);
    const protocolMappingEnabled =
      runType === 3 && protocolMappingFeature.enabled === true;
    const gatewayPort = this.resolveGatewayPort();

    await this.clearLegacyGatewayRedirects(gatewayPort, true);
    await this.runGoBackendOrThrow(
      goBackend.cleanIptables(),
      firewallT("cleanRulesFailed"),
    );

    if (runType === 1) {
      return {
        runType,
        gatewayPort,
        exemptPorts: [] as string[],
        whitelistSynced: 0,
      };
    }

    await this.initDefaultFirewall(
      config,
      protocolMappingEnabled,
      runType,
      true,
    );

    const whitelistSynced =
      runType === 0 ? await this.syncActiveWhitelistRecords(true) : 0;

    return {
      runType,
      gatewayPort,
      exemptPorts: this.resolveExemptPorts(
        config,
        protocolMappingEnabled,
        runType,
      ),
      whitelistSynced,
    };
  }

  async clearFirewall() {
    this.ensureHostFirewallAvailable();
    const gatewayPort = this.resolveGatewayPort();
    await this.clearLegacyGatewayRedirects(gatewayPort, true);
    await this.runGoBackendOrThrow(
      goBackend.cleanIptables(),
      firewallT("cleanRulesFailed"),
    );

    return {
      gatewayPort,
    };
  }

  private async syncGatewayRuntimeConfig(
    config: AppConfig,
    protocolMappingEnabled: boolean,
    runType: 0 | 1 | 3,
  ) {
    await this.runGoBackend(
      goBackend.setAuthConfig(buildGatewayAuthConfig(config)),
      firewallT("syncAuthGatewayConfigFailed"),
    );
    await this.runGoBackend(
      goBackend.setReverseProxyThrottle(
        config.reverse_proxy_throttle ?? DEFAULT_REVERSE_PROXY_THROTTLE_CONFIG,
      ),
      firewallT("syncReverseProxyThrottleFailed"),
    );
    await syncGatewayCrawlerBlockerToGateway(
      config.gateway_crawler_blocker ?? DEFAULT_GATEWAY_CRAWLER_BLOCKER_CONFIG,
    ).catch((error) => {
      console.error(
        firewallT("goBackendCallFailed", {
          message: firewallT("syncGatewayCrawlerBlockerConfigFailed"),
        }),
        error,
      );
    });
    await syncReverseProxyTrustedIPsNow({ config }).catch((error) => {
      console.error(
        "[reverse-proxy-trusted-ips] failed to sync runtime state during run type apply:",
        error,
      );
    });
    try {
      await syncGatewayVisibilityToGateway();
    } catch (error) {
      console.error(
        firewallT("goBackendCallFailed", {
          message: firewallT("syncGatewayVisibilityConfigFailed"),
        }),
        error,
      );
    }
    try {
      await syncGatewayProxyHeadersRuntimeForConfig(config);
    } catch (error) {
      console.error(
        firewallT("goBackendCallFailed", {
          message: firewallT("syncGatewayProxyHeadersConfigFailed"),
        }),
        error,
      );
    }
    try {
      await syncGatewayHostResponseRuntimeForConfig(config);
    } catch (error) {
      console.error(
        firewallT("goBackendCallFailed", {
          message: firewallT("syncGatewayHostResponseConfigFailed"),
        }),
        error,
      );
    }

    if (runType === 1) {
      await this.runGoBackend(
        goBackend.setProxyProtocolForce(true),
        firewallT("enableProxyProtocolForceFailed"),
      );
      await this.runGoBackend(
        goBackend.flushStreamRules(),
        firewallT("disableStreamRulesFailed"),
      );

      if (isReverseProxySubdomainMode(config)) {
        await this.runGoBackend(
          goBackend.flushRules(),
          firewallT("flushPathRoutesFailed"),
        );
        await this.runGoBackend(
          goBackend.setHostRules(config.host_mappings),
          firewallT("syncHostRoutesFailed"),
        );
        await this.runGoBackend(
          goBackend.setDefaultRoute(DISABLED_DEFAULT_ROUTE),
          firewallT("syncDefaultRouteFailed"),
        );
        return;
      }

      await this.runGoBackend(
        goBackend.flushHostRules(),
        firewallT("flushHostRoutesFailed"),
      );
      await this.runGoBackend(
        goBackend.setRules(config.proxy_mappings),
        firewallT("syncPathRoutesFailed"),
      );
      await this.runGoBackend(
        goBackend.setDefaultRoute(config.default_route),
        firewallT("syncDefaultRouteFailed"),
      );
      return;
    }

    if (runType === 3) {
    await this.runGoBackend(
      goBackend.setProxyProtocolForce(false),
      firewallT("disableProxyProtocolForceFailed"),
    );
    await this.runGoBackend(
      goBackend.flushRules(),
      firewallT("flushPathRoutesFailed"),
    );
    await this.runGoBackend(
      goBackend.setHostRules(config.host_mappings),
      firewallT("syncHostRoutesFailed"),
    );
    if (protocolMappingEnabled) {
      await this.runGoBackend(
        goBackend.setStreamRules(config.stream_mappings),
        firewallT("syncStreamRulesFailed"),
      );
    } else {
      await this.runGoBackend(
        goBackend.flushStreamRules(),
        firewallT("disableStreamRulesFailed"),
      );
    }
    await this.runGoBackend(
      goBackend.setDefaultRoute(config.default_route),
      firewallT("syncDefaultRouteFailed"),
    );
      return;
    }

    await this.runGoBackend(
      goBackend.setProxyProtocolForce(false),
      firewallT("disableProxyProtocolForceFailed"),
    );
    await this.runGoBackend(
      goBackend.flushHostRules(),
      firewallT("flushHostRoutesFailed"),
    );
    await this.runGoBackend(
      goBackend.flushStreamRules(),
      firewallT("disableStreamRulesFailed"),
    );

    if (runType === 0) {
      if (config.proxy_mappings) {
        await this.runGoBackend(
          goBackend.setRules(config.proxy_mappings),
          firewallT("syncPathRoutesFailed"),
        );
      }
      if (config.default_route) {
        await this.runGoBackend(
          goBackend.setDefaultRoute(config.default_route),
          firewallT("syncDefaultRouteFailed"),
        );
      }
      await this.runGoBackend(
        goBackend.setRules([
          {
            path: "/auth",
            target: `http://127.0.0.1:${process.env.AUTH_PORT}`,
            rewrite_html: false,
            use_auth: false,
            use_root_mode: false,
            strip_path: false,
          },
        ]),
        firewallT("syncAuthEntryRouteFailed"),
      );
      await this.runGoBackend(
        goBackend.setDefaultRoute("/auth"),
        firewallT("syncAuthDefaultRouteFailed"),
      );
    }
  }

  private async applyHostFirewallConfig(
    config: AppConfig,
    protocolMappingEnabled: boolean,
    runType: 0 | 1 | 3,
    autoManageFirewall: boolean,
  ) {
    if (!autoManageFirewall) {
      return;
    }

    const gatewayPort = this.resolveGatewayPort();

    if (runType === 1) {
      await this.clearLegacyGatewayRedirects(gatewayPort);
      await this.runGoBackend(
        goBackend.cleanIptables(),
        firewallT("cleanRulesFailed"),
      );
      return;
    }

    if (runType === 3) {
      await this.initDefaultFirewall(config, protocolMappingEnabled, runType);
      await this.clearLegacyGatewayRedirects(gatewayPort);
      return;
    }

    await this.clearLegacyGatewayRedirects(gatewayPort);
    await this.initDefaultFirewall(config, false, runType);
    await this.syncActiveWhitelistRecords();
  }

  async applyRunTypeConfig(runType: 0 | 1 | 3, previousRunType?: 0 | 1 | 3) {
    void previousRunType;
    if (runType === 0) {
      this.ensureDirectModeAvailable();
    }

    const [config, protocolMappingFeature] = await Promise.all([
      configManager.getConfig(),
      configManager.getProtocolMappingFeatureConfig(),
    ]);
    const protocolMappingEnabled =
      runType === 3 && protocolMappingFeature.enabled === true;
    const autoManageFirewall =
      getRuntimeCapabilities().host_firewall_available &&
      shouldAutoManageFirewallForRunType(runType, config);

    await this.syncGatewayRuntimeConfig(
      config,
      protocolMappingEnabled,
      runType,
    );
    await this.applyHostFirewallConfig(
      config,
      protocolMappingEnabled,
      runType,
      autoManageFirewall,
    );
  }
}

export const firewallService = new FirewallService();
