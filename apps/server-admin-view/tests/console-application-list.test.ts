import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";
import type { AppConfig, HostMapping } from "../src/types";
import {
  buildConsoleApplicationItems,
  buildConsoleHostApplicationHref,
  buildConsolePathApplicationHref,
  shouldShowConsoleApplicationList,
} from "../src/views/layout/console-application-list";

const hostMapping = (
  host: string,
  overrides: Partial<HostMapping> = {},
): HostMapping => ({
  host,
  group_id: null,
  target_type: "proxy",
  target: `http://${host}:8080`,
  static_serve: null,
  target_path_mode: "entry",
  waf_enabled: true,
  use_auth: true,
  access_mode: "login_first",
  suppress_toolbar: false,
  preserve_host: true,
  is_default: false,
  disabled: false,
  availability: null,
  visibility: {
    mode: "inherit",
    selections: [],
    custom_cidrs: [],
    cidrs: [],
  },
  protocol_mode: "auto",
  basic_auth: { enabled: false, username: "", password: "" },
  locations: [],
  service_role: "app",
  title: "",
  title_override: "",
  favicon: "",
  favicon_override: "",
  ...overrides,
});

const config = (overrides: Partial<AppConfig> = {}): AppConfig =>
  ({
    run_type: 3,
    reverse_proxy_submode: "path",
    host_mappings: [],
    host_mapping_groups: [],
    host_mapping_grouped_view: false,
    proxy_mappings: [],
    subdomain_mode: {
      public_auth_base_url: "",
      public_http_port: 0,
      public_https_port: 0,
      edge_client_ip_enabled: false,
      aliyun_esa_enabled: false,
      tencent_edgeone_enabled: false,
    },
    ssl: { enabled: true },
    gateway_portal: {
      enabled: true,
      display_style: "title",
      show_app_icon: true,
      show_wol: true,
      icon_drag_mode: "corners",
      version: "v1",
    },
    ...overrides,
  }) as AppConfig;

const location = { protocol: "https:", hostname: "nas.example.test" };

describe("console application list", () => {
  it("is mounted only on the console dashboard route", () => {
    const layoutSource = readFileSync(
      new URL("../src/views/Layout.vue", import.meta.url),
      "utf8",
    );
    assert.match(
      layoutSource,
      /<ConsoleApplicationBar v-if="route\.name === 'Dashboard'" \/>/u,
    );
  });

  it("is only visible for enabled FPK deployments", () => {
    assert.equal(
      shouldShowConsoleApplicationList({
        deploymentTarget: "fpk",
        enabled: true,
      }),
      true,
    );
    assert.equal(
      shouldShowConsoleApplicationList({
        deploymentTarget: "fpk-lite",
        enabled: true,
      }),
      true,
    );
    assert.equal(
      shouldShowConsoleApplicationList({
        deploymentTarget: "docker",
        enabled: true,
      }),
      false,
    );
    assert.equal(
      shouldShowConsoleApplicationList({
        deploymentTarget: "fpk",
        enabled: false,
      }),
      false,
    );
  });

  it("prioritizes eligible Host apps and follows grouped ordering", () => {
    const mediaGroup = "11111111-1111-4111-8111-111111111111";
    const toolsGroup = "22222222-2222-4222-8222-222222222222";
    const autoIcon = "data:image/png;base64,YXV0bw==";
    const customIcon = "data:image/webp;base64,Y3VzdG9t";
    const items = buildConsoleApplicationItems({
      accessEntryPort: "8443",
      location,
      config: config({
        host_mapping_grouped_view: true,
        host_mapping_groups: [
          { id: mediaGroup, name: "Media" },
          { id: toolsGroup, name: "Tools" },
        ],
        host_mappings: [
          hostMapping("tool.example.test", {
            group_id: toolsGroup,
            title: "Tool",
          }),
          hostMapping("loose.example.test"),
          hostMapping("auth.example.test", { service_role: "auth" }),
          hostMapping("disabled.example.test", { disabled: true }),
          hostMapping("media.example.test", {
            group_id: mediaGroup,
            title: "Collected title",
            title_override: "Media center",
            favicon: autoIcon,
            favicon_override: customIcon,
          }),
        ],
        proxy_mappings: [
          {
            path: "/fallback",
            target: "http://fallback:8080",
            rewrite_html: false,
            use_auth: true,
            use_root_mode: false,
            strip_path: false,
          },
        ],
      }),
    });

    assert.deepEqual(
      items.map((item) => item.label),
      ["Media center", "Tool", "loose.example.test"],
    );
    assert.equal(items[0]?.iconSrc, customIcon);
    assert.equal(items[0]?.href, "https://media.example.test:8443/");
    assert.ok(items.every((item) => item.kind === "host"));
  });

  it("honors domain labels and disabled portal icons", () => {
    const items = buildConsoleApplicationItems({
      accessEntryPort: "443",
      location,
      config: config({
        gateway_portal: {
          enabled: true,
          display_style: "domain",
          show_app_icon: false,
          show_wol: true,
          icon_drag_mode: "corners",
          version: "v1",
        },
        host_mappings: [
          hostMapping("app.example.test", {
            title_override: "Ignored title",
            favicon_override: "data:image/png;base64,aWNvbg==",
          }),
        ],
      }),
    });

    assert.equal(items[0]?.label, "app.example.test");
    assert.equal(items[0]?.iconSrc, "");
    assert.equal(items[0]?.showIcon, false);
    assert.equal(items[0]?.href, "https://app.example.test/");
  });

  it("keeps static file and directory mappings in the portal", () => {
    const items = buildConsoleApplicationItems({
      accessEntryPort: "443",
      location,
      config: config({
        host_mappings: [
          hostMapping("docs.example.test", {
            target_type: "directory",
            target: "",
            static_serve: {
              path: "/srv/docs",
              index_files: ["index.html", "index.htm"],
              directory_listing: { enabled: true, render_readme: true },
            },
            title_override: "Documentation",
          }),
          hostMapping("manual.example.test", {
            target_type: "file",
            target: "",
            static_serve: {
              path: "/srv/manual.pdf",
              index_files: [],
              directory_listing: { enabled: false, render_readme: false },
            },
          }),
        ],
      }),
    });

    assert.deepEqual(
      items.map(({ label, href, kind }) => ({ label, href, kind })),
      [
        {
          label: "Documentation",
          href: "https://docs.example.test/",
          kind: "host",
        },
        {
          label: "manual.example.test",
          href: "https://manual.example.test/",
          kind: "host",
        },
      ],
    );
  });

  it("falls back to same-gateway path mappings when no Host app is eligible", () => {
    const items = buildConsoleApplicationItems({
      accessEntryPort: "7999",
      location: { ...location, protocol: "http:" },
      config: config({
        ssl: { enabled: false },
        host_mappings: [
          hostMapping("auth.example.test", { service_role: "auth" }),
          hostMapping("disabled.example.test", { disabled: true }),
        ],
        proxy_mappings: [
          {
            path: "/photos",
            target: "http://photos:3000",
            rewrite_html: false,
            use_auth: true,
            use_root_mode: false,
            strip_path: false,
          },
          {
            path: "tools/",
            target: "http://tools:3000",
            rewrite_html: false,
            use_auth: true,
            use_root_mode: false,
            strip_path: false,
          },
        ],
      }),
    });

    assert.deepEqual(
      items.map(({ label, href, kind, showIcon }) => ({
        label,
        href,
        kind,
        showIcon,
      })),
      [
        {
          label: "/photos",
          href: "http://nas.example.test:7999/photos/",
          kind: "path",
          showIcon: true,
        },
        {
          label: "tools/",
          href: "http://nas.example.test:7999/tools/",
          kind: "path",
          showIcon: true,
        },
      ],
    );
  });

  it("uses path mappings when HostRules are inactive", () => {
    const items = buildConsoleApplicationItems({
      accessEntryPort: "7999",
      location,
      config: config({
        run_type: 1,
        reverse_proxy_submode: "path",
        host_mappings: [hostMapping("stale.example.test")],
        proxy_mappings: [
          {
            path: "/active",
            target: "http://active:3000",
            rewrite_html: false,
            use_auth: true,
            use_root_mode: false,
            strip_path: false,
          },
        ],
      }),
    });

    assert.deepEqual(
      items.map(({ label, kind }) => ({ label, kind })),
      [{ label: "/active", kind: "path" }],
    );
  });

  it("falls back to paths when every active Host app has an unsafe host", () => {
    const items = buildConsoleApplicationItems({
      accessEntryPort: "7999",
      location,
      config: config({
        host_mappings: [hostMapping("trusted.example.test@evil.example.test")],
        proxy_mappings: [
          {
            path: "/safe",
            target: "http://safe:3000",
            rewrite_html: false,
            use_auth: true,
            use_root_mode: false,
            strip_path: false,
          },
        ],
      }),
    });

    assert.equal(items.length, 1);
    assert.equal(items[0]?.label, "/safe");
    assert.equal(items[0]?.kind, "path");
  });

  it("tolerates legacy Host metadata fields that are absent", () => {
    const legacyMapping = hostMapping("legacy.example.test");
    const legacyRecord = legacyMapping as unknown as Record<string, unknown>;
    delete legacyRecord.title;
    delete legacyRecord.title_override;
    delete legacyRecord.favicon;
    delete legacyRecord.favicon_override;

    const items = buildConsoleApplicationItems({
      accessEntryPort: "7999",
      location,
      config: config({ host_mappings: [legacyMapping] }),
    });

    assert.equal(items[0]?.label, "legacy.example.test");
    assert.equal(items[0]?.iconSrc, "");
  });

  it("uses the console protocol and omits only its matching default port", () => {
    assert.equal(
      buildConsoleHostApplicationHref(
        "app.example.test",
        location,
        config({ ssl: { enabled: false } }),
        "443",
      ),
      "https://app.example.test/",
    );
    assert.equal(
      buildConsoleHostApplicationHref(
        "app.example.test",
        { ...location, protocol: "http:" },
        config({ ssl: { enabled: true } }),
        "80",
      ),
      "http://app.example.test/",
    );
    assert.equal(
      buildConsolePathApplicationHref(
        "/app",
        location,
        config({ ssl: { enabled: true } }),
        "invalid",
      ),
      "https://nas.example.test:7999/app/",
    );
    assert.equal(
      buildConsoleHostApplicationHref(
        "app.example.test",
        location,
        config({ ssl: { enabled: true } }),
        "443oops",
      ),
      "https://app.example.test:7999/",
    );
  });

  it("prefers the scheme-specific public port over the origin gateway port", () => {
    assert.equal(
      buildConsoleHostApplicationHref(
        "app.example.test",
        location,
        config({
          ssl: { enabled: true },
          subdomain_mode: {
            ...config().subdomain_mode,
            public_https_port: 8443,
            public_http_port: 8080,
          },
        }),
        "7999",
      ),
      "https://app.example.test:8443/",
    );
    assert.equal(
      buildConsoleHostApplicationHref(
        "app.example.test",
        location,
        config({
          ssl: { enabled: true },
          subdomain_mode: {
            ...config().subdomain_mode,
            public_auth_base_url: "https://auth.example.test:9443",
            public_https_port: 8443,
          },
        }),
        "7999",
      ),
      "https://app.example.test:9443/",
    );
  });

  it("omits origin ports behind standard-port edge ingress", () => {
    const edgeConfig = config({
      ssl: { enabled: true },
      subdomain_mode: {
        ...config().subdomain_mode,
        edge_client_ip_enabled: true,
        aliyun_esa_enabled: true,
        public_auth_base_url: "https://auth.example.test:7999",
        public_https_port: 7999,
      },
    });
    assert.equal(
      buildConsoleHostApplicationHref(
        "app.example.test",
        location,
        edgeConfig,
        "7999",
      ),
      "https://app.example.test/",
    );

    const cloudflaredConfig = config({
      run_type: 1,
      reverse_proxy_submode: "subdomain",
      default_tunnel: "cloudflared",
      ssl: { enabled: true },
      subdomain_mode: {
        ...config().subdomain_mode,
        public_https_port: 7999,
      },
    });
    assert.equal(
      buildConsoleHostApplicationHref(
        "app.example.test",
        location,
        cloudflaredConfig,
        "7999",
      ),
      "https://app.example.test/",
    );
  });

  it("uses the FRP remote entry instead of a reverse-mode origin port", () => {
    assert.equal(
      buildConsoleHostApplicationHref(
        "app.example.test",
        location,
        config({
          run_type: 1,
          reverse_proxy_submode: "subdomain",
          default_tunnel: "frp",
          ssl: { enabled: true },
          subdomain_mode: {
            ...config().subdomain_mode,
            public_https_port: 8443,
          },
        }),
        "24443",
      ),
      "https://app.example.test:24443/",
    );
  });
});
