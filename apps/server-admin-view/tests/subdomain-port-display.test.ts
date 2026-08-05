import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
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

const createHostFormatters = (
  subdomainMode: SubdomainModeConfig,
  overrides: Partial<AppConfig> = {},
) => {
  const config = createConfig(subdomainMode, overrides);
  return useSubdomainPortDisplay({
    accessEntryPort: ref("7999"),
    currentModeConfig: computed(() => config.subdomain_mode),
    getConfig: () => config,
    modeForm: { ...subdomainMode },
  });
};

test("edge ingress omits a stale configured gateway port from mapping hosts", () => {
  const subdomainMode = {
    ...createDefaultModeForm(),
    edge_client_ip_enabled: true,
    tencent_edgeone_enabled: true,
    public_https_port: 7999,
  };

  const { formatHostWithAccessEntryPort } = createHostFormatters(subdomainMode);

  assert.equal(
    formatHostWithAccessEntryPort("app.example.com"),
    "app.example.com",
  );
});

test("edge ingress omits a stale configured gateway port from the current auth service", () => {
  const subdomainMode = {
    ...createDefaultModeForm(),
    edge_client_ip_enabled: true,
    tencent_edgeone_enabled: true,
    public_https_port: 7999,
  };
  const { formatAuthServiceHostWithPublicPort } =
    createHostFormatters(subdomainMode);

  assert.equal(
    formatAuthServiceHostWithPublicPort("auth.example.com"),
    "auth.example.com",
  );
});

test("non-edge ingress keeps an explicitly configured public port", () => {
  const subdomainMode = {
    ...createDefaultModeForm(),
    public_https_port: 8443,
  };
  const { formatAuthServiceHostWithPublicPort, formatHostWithAccessEntryPort } =
    createHostFormatters(subdomainMode);

  assert.equal(
    formatHostWithAccessEntryPort("app.example.com"),
    "app.example.com:8443",
  );
  assert.equal(
    formatAuthServiceHostWithPublicPort("auth.example.com"),
    "auth.example.com:8443",
  );
});

test("cloudflared omits a stale explicitly configured public port", () => {
  const subdomainMode = {
    ...createDefaultModeForm(),
    public_https_port: 8443,
  };
  const {
    formatAuthServiceHostWithPublicPort,
    formatHostWithAccessEntryPort,
    omitPublicPortConfiguration,
  } = createHostFormatters(subdomainMode, {
    run_type: 1,
    reverse_proxy_submode: "subdomain",
    default_tunnel: "cloudflared",
  });

  assert.equal(omitPublicPortConfiguration.value, true);
  assert.equal(
    formatHostWithAccessEntryPort("app.example.com"),
    "app.example.com",
  );
  assert.equal(
    formatAuthServiceHostWithPublicPort("auth.example.com"),
    "auth.example.com",
  );
});

test("edge provider state is hidden when the current run mode cannot use it", () => {
  const subdomainMode = {
    ...createDefaultModeForm(),
    edge_client_ip_enabled: true,
    tencent_edgeone_enabled: true,
  };
  const { activeEdgeClientIpProvider, savedEdgeClientIpProvider } =
    createHostFormatters(subdomainMode, {
      run_type: 1,
      reverse_proxy_submode: "subdomain",
      default_tunnel: "cloudflared",
    });

  assert.equal(activeEdgeClientIpProvider.value, null);
  assert.equal(savedEdgeClientIpProvider.value, null);
});

test("edge network controls are not rendered when the mode is unavailable", () => {
  const component = readFileSync(
    new URL(
      "../src/views/subdomain-proxy/SubdomainModeConfigCard.vue",
      import.meta.url,
    ),
    "utf8",
  );

  assert.match(
    component,
    /v-if="isEdgeClientIpModeEditable"\s+class="rounded-lg border px-4 py-4"/u,
  );
});
