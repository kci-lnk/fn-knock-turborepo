import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { computed, reactive, ref } from "vue";
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
  accessEntryPort = "7999",
) => {
  const config = createConfig(subdomainMode, overrides);
  return useSubdomainPortDisplay({
    accessEntryPort: ref(accessEntryPort),
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
    public_auth_base_url: "https://auth.example.com:9443",
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

for (const [port, suffix] of [
  [443, ""],
  [8443, ":8443"],
  [0, ":15101"],
] as const) {
  test(`FRP uses public port ${port} with remote-entry fallback only when unset`, () => {
    const subdomainMode = {
      ...createDefaultModeForm(),
      public_https_port: port,
      edge_client_ip_enabled: true,
      tencent_edgeone_enabled: true,
    };
    const display = createHostFormatters(
      subdomainMode,
      {
        run_type: 1,
        reverse_proxy_submode: "subdomain",
        default_tunnel: "frp",
      },
      "15101",
    );
    assert.equal(display.omitPublicPortConfiguration.value, false);
    assert.equal(display.authServicePublicPort.value, port || 15101);
    assert.equal(
      display.formatHostWithAccessEntryPort("app.example.com"),
      `app.example.com${suffix}`,
    );
    assert.equal(
      display.formatAuthServiceHostWithPublicPort("auth.example.com"),
      `auth.example.com${suffix}`,
    );
  });
}

test("editing the FRP HTTPS port removes a stale port from the auth base URL", () => {
  const modeForm = reactive({
    ...createDefaultModeForm(),
    public_auth_base_url: "https://auth.example.com:7999",
    public_https_port: 7999,
  });
  const config = createConfig(modeForm, {
    run_type: 1,
    reverse_proxy_submode: "subdomain",
    default_tunnel: "frp",
  });
  const display = useSubdomainPortDisplay({
    accessEntryPort: ref("15101"),
    currentModeConfig: computed(() => modeForm),
    getConfig: () => config,
    modeForm,
  });
  assert.equal(display.authServicePublicPort.value, 7999);
  display.authServicePublicPort.value = 443;
  assert.equal(modeForm.public_https_port, 443);
  assert.equal(modeForm.public_auth_base_url, "https://auth.example.com");
  assert.equal(display.authServicePublicPort.value, 443);
  assert.equal(
    display.formatAuthServiceHostWithPublicPort("auth.example.com"),
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

test("public auth port description displays a concise destructive warning", () => {
  const component = readFileSync(
    new URL(
      "../src/views/subdomain-proxy/SubdomainModeConfigCard.vue",
      import.meta.url,
    ),
    "utf8",
  );

  assert.match(
    component,
    /authServicePortHint[\s\S]*id="auth-service-public-port-warning"\s+class="text-xs font-medium leading-5 text-destructive"[\s\S]*<\/div>\s+<Input/u,
  );
  assert.match(
    component,
    /t\("admin\.subdomainProxy\.authServicePortWarning"\)/u,
  );
});

test("FRP preserves an explicit standard HTTPS port ahead of stale configured ports", () => {
  const display = createHostFormatters(
    {
      ...createDefaultModeForm(),
      public_auth_base_url: "https://auth.example.com:443",
      public_https_port: 7999,
    },
    { run_type: 1, reverse_proxy_submode: "subdomain", default_tunnel: "frp" },
  );
  assert.equal(display.authServicePublicPort.value, 443);
  assert.equal(
    display.formatAuthServiceHostWithPublicPort("auth.example.com"),
    "auth.example.com",
  );
  assert.equal(
    display.formatHostWithAccessEntryPort("app.example.com"),
    "app.example.com",
  );
});

test("clearing the FRP port also clears an explicit URL port and restores the remote entry", () => {
  const modeForm = reactive({
    ...createDefaultModeForm(),
    public_auth_base_url: "https://auth.example.com:8443",
    public_https_port: 8443,
  });
  const config = createConfig(modeForm, {
    run_type: 1,
    reverse_proxy_submode: "subdomain",
    default_tunnel: "frp",
  });
  const display = useSubdomainPortDisplay({
    accessEntryPort: ref("15101"),
    currentModeConfig: computed(() => modeForm),
    getConfig: () => config,
    modeForm,
  });
  display.authServicePublicPort.value = "";
  assert.equal(modeForm.public_https_port, 0);
  assert.equal(modeForm.public_auth_base_url, "https://auth.example.com");
  assert.equal(display.authServicePublicPort.value, 15101);
  assert.equal(
    display.formatAuthServiceHostWithPublicPort("auth.example.com"),
    "auth.example.com:15101",
  );
});

test("HTTPS auth preview keeps port 80 when it is explicitly configured", () => {
  const display = createHostFormatters({
    ...createDefaultModeForm(),
    public_https_port: 80,
  });
  assert.equal(
    display.formatAuthServiceHostWithPublicPort("auth.example.com"),
    "auth.example.com:80",
  );
});

test("FRP HTTPS previews do not borrow a legacy HTTP URL port", () => {
  for (const port of [0, 80, 443, 8443]) {
    const display = createHostFormatters(
      {
        ...createDefaultModeForm(),
        public_auth_base_url: "http://auth.example.com:9080",
        public_http_port: 9080,
        public_https_port: port,
      },
      {
        run_type: 1,
        reverse_proxy_submode: "subdomain",
        default_tunnel: "frp",
      },
      "15101",
    );
    const expectedPort = port || 15101;
    const suffix = expectedPort === 443 ? "" : `:${expectedPort}`;
    assert.equal(display.authServicePublicPort.value, expectedPort);
    assert.equal(
      display.formatAuthServiceHostWithPublicPort("auth.example.com"),
      `auth.example.com${suffix}`,
    );
    assert.equal(
      display.formatHostWithAccessEntryPort("app.example.com"),
      `app.example.com${suffix}`,
    );
  }
});
