import { configManager, type AppConfig } from "./redis";
import { goBackend, type GoResponse } from "./go-backend";
import { buildGatewayAuthConfig } from "./subdomain-mode";
import { whitelistManager } from "./whitelist-manager";

export class FirewallService {
  private readonly legacyRedirectedHttpPorts = [80, 443] as const;

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
    console.error(`Go 后端接口调用失败: ${fallbackMessage}`, result);
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

  private resolveGatewayPort(): number {
    const parsed = Number.parseInt(process.env.GO_REPROXY_PORT || "7999", 10);
    return Number.isFinite(parsed) && parsed > 0 ? parsed : 7999;
  }

  private resolveExemptPorts(
    config: AppConfig,
    protocolMappingEnabled: boolean,
  ): string[] {
    const ports = new Set<string>([String(this.resolveGatewayPort())]);

    if (config.run_type === 3 && protocolMappingEnabled) {
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

    return [...ports];
  }

  private async clearLegacyGatewayRedirects(targetPort: number) {
    for (const listenPort of this.legacyRedirectedHttpPorts) {
      await this.runGoBackend(
        goBackend.clearTCPRedirect(listenPort, targetPort),
        `清理历史 TCP 重定向 ${listenPort} -> ${targetPort} 失败`,
        [404],
      );
    }
  }

  private async initDefaultFirewall(
    config: AppConfig,
    protocolMappingEnabled: boolean,
  ) {
    await this.runGoBackend(
      goBackend.initIptables({
        chain_name: "FN-KNOCK-FW",
        parent_chain: ["INPUT", "DOCKER-USER"],
        exempt_ports: this.resolveExemptPorts(config, protocolMappingEnabled),
      }),
      "初始化默认防火墙规则失败",
    );
  }

  async applyRunTypeConfig(runType: 0 | 1 | 3, previousRunType?: 0 | 1 | 3) {
    void previousRunType;
    const [config, protocolMappingFeature] = await Promise.all([
      configManager.getConfig(),
      configManager.getProtocolMappingFeatureConfig(),
    ]);
    const protocolMappingEnabled =
      runType === 3 && protocolMappingFeature.enabled === true;
    const gatewayPort = this.resolveGatewayPort();
    await this.runGoBackend(
      goBackend.setAuthConfig(buildGatewayAuthConfig(config)),
      "同步鉴权网关配置失败",
    );

    if (runType === 1) {
      await this.clearLegacyGatewayRedirects(gatewayPort);
      await this.runGoBackend(
        goBackend.setProxyProtocolForce(true),
        "开启 Proxy Protocol 强制模式失败",
      );
      await this.runGoBackend(goBackend.cleanIptables(), "清理防火墙规则失败");
      await this.runGoBackend(goBackend.flushHostRules(), "清空 Host 路由失败");
      await this.runGoBackend(
        goBackend.flushStreamRules(),
        "关闭 协议映射监听失败",
      );
      await this.runGoBackend(
        goBackend.setRules(config.proxy_mappings),
        "同步路径路由失败",
      );
      await this.runGoBackend(
        goBackend.setDefaultRoute(config.default_route),
        "同步默认路由失败",
      );
      return;
    }

    if (runType === 3) {
      await this.runGoBackend(
        goBackend.setProxyProtocolForce(false),
        "关闭 Proxy Protocol 强制模式失败",
      );
      await this.initDefaultFirewall(config, protocolMappingEnabled);
      await this.clearLegacyGatewayRedirects(gatewayPort);
      await this.runGoBackend(goBackend.flushRules(), "清空路径路由失败");
      await this.runGoBackend(
        goBackend.setHostRules(config.host_mappings),
        "同步 Host 路由失败",
      );
      if (protocolMappingEnabled) {
        await this.runGoBackend(
          goBackend.setStreamRules(config.stream_mappings),
          "同步 协议映射失败",
        );
      } else {
        await this.runGoBackend(
          goBackend.flushStreamRules(),
          "关闭 协议映射监听失败",
        );
      }
      await this.runGoBackend(
        goBackend.setDefaultRoute(config.default_route),
        "同步默认路由失败",
      );
      return;
    }

    await this.clearLegacyGatewayRedirects(gatewayPort);
    await this.runGoBackend(
      goBackend.setProxyProtocolForce(false),
      "关闭 Proxy Protocol 强制模式失败",
    );
    await this.runGoBackend(goBackend.flushHostRules(), "清空 Host 路由失败");
    await this.runGoBackend(
      goBackend.flushStreamRules(),
      "关闭 协议映射监听失败",
    );
    await this.initDefaultFirewall(config, false);

    if (runType === 0) {
      const records = await whitelistManager.getAllActiveRecords();
      for (const record of records) {
        await this.runGoBackend(
          goBackend.allowIP(record.ip),
          `同步白名单 IP ${record.ip} 失败`,
        );
      }
      if (config.proxy_mappings) {
        await this.runGoBackend(
          goBackend.setRules(config.proxy_mappings),
          "同步路径路由失败",
        );
      }
      if (config.default_route) {
        await this.runGoBackend(
          goBackend.setDefaultRoute(config.default_route),
          "同步默认路由失败",
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
        "同步鉴权入口路由失败",
      );
      await this.runGoBackend(
        goBackend.setDefaultRoute("/auth"),
        "同步鉴权默认路由失败",
      );
    }
  }
}

export const firewallService = new FirewallService();
