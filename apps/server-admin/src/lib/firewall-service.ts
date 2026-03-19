import { configManager } from "./redis";
import { goBackend } from "./go-backend";
import { buildGatewayAuthConfig } from "./subdomain-mode";

export class FirewallService {
  private async initDefaultFirewall() {
    await goBackend.initIptables({
      chain_name: "FN-KNOCK-FW",
      parent_chain: ["INPUT", "DOCKER-USER"],
      exempt_ports: [process.env.GO_REPROXY_PORT || "7999"],
    });
  }

  async applyRunTypeConfig(runType: 0 | 1 | 3, previousRunType?: 0 | 1 | 3) {
    void previousRunType;
    const config = await configManager.getConfig();
    await goBackend.setAuthConfig(buildGatewayAuthConfig(config));

    if (runType === 1) {
      await goBackend.setProxyProtocolForce(true);
      await goBackend.cleanIptables();
      await goBackend.flushHostRules();
      await goBackend.setRules(config.proxy_mappings);
      await goBackend.setDefaultRoute(config.default_route);
      return;
    }

    if (runType === 3) {
      await goBackend.setProxyProtocolForce(false);
      await this.initDefaultFirewall();
      await goBackend.flushRules();
      await goBackend.setHostRules(config.host_mappings);
      await goBackend.setDefaultRoute(config.default_route);
      return;
    }

    await goBackend.setProxyProtocolForce(false);
    await goBackend.flushHostRules();
    await this.initDefaultFirewall();

    if (runType === 0) {
      if (config.whitelist_ips && config.whitelist_ips.length > 0) {
        for (const ip of config.whitelist_ips) {
          await goBackend.allowIP(ip);
        }
      }
      if (config.proxy_mappings) {
        await goBackend.setRules(config.proxy_mappings);
      }
      if (config.default_route) {
        await goBackend.setDefaultRoute(config.default_route);
      }
      await goBackend.setRules([
        {
          path: "/auth",
          target: `http://127.0.0.1:${process.env.AUTH_PORT}`,
          rewrite_html: false,
          use_auth: false,
          use_root_mode: false,
          strip_path: false,
        },
      ]);
      await goBackend.setDefaultRoute("/auth");
    }
  }
}

export const firewallService = new FirewallService();
