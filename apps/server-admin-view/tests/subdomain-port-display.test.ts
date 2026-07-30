import assert from "node:assert/strict";
import test from "node:test";
import { computed, ref } from "vue";
import type { AppConfig, SubdomainModeConfig } from "../src/types";
import { createDefaultModeForm } from "../src/views/subdomain-proxy/model";
import { useSubdomainPortDisplay } from "../src/views/subdomain-proxy/useSubdomainPortDisplay";

const createConfig = (
  subdomainMode: SubdomainModeConfig,
  overrides: Partial<AppConfig> = {},
): AppConfig =>
  ({
    run_type: 3,
    reverse_proxy_submode: "path",
    default_tunnel: "frp",
    subdomain_mode: subdomainMode,
    ...overrides,
  }) as AppConfig;

const formatMappingHost = (
  subdomainMode: SubdomainModeConfig,
  overrides: Partial<AppConfig> = {},
): string => {
  const config = createConfig(subdomainMode, overrides);
  const { formatHostWithAccessEntryPort } = useSubdomainPortDisplay({
    accessEntryPort: ref("7999"),
    currentModeConfig: computed(() => config.subdomain_mode),
    getConfig: () => config,
    modeForm: { ...subdomainMode },
  });
  return formatHostWithAccessEntryPort("app.example.com");
};

test("edge ingress omits a stale configured gateway port from mapping hosts", () => {
  const subdomainMode = {
    ...createDefaultModeForm(),
    edge_client_ip_enabled: true,
    tencent_edgeone_enabled: true,
    public_https_port: 7999,
  };

  assert.equal(formatMappingHost(subdomainMode), "app.example.com");
});

test("non-edge ingress keeps an explicitly configured public port", () => {
  const subdomainMode = {
    ...createDefaultModeForm(),
    public_https_port: 8443,
  };

  assert.equal(formatMappingHost(subdomainMode), "app.example.com:8443");
});

test("cloudflared keeps an explicitly configured public port", () => {
  const subdomainMode = {
    ...createDefaultModeForm(),
    public_https_port: 8443,
  };

  assert.equal(
    formatMappingHost(subdomainMode, {
      run_type: 1,
      reverse_proxy_submode: "subdomain",
      default_tunnel: "cloudflared",
    }),
    "app.example.com:8443",
  );
});
