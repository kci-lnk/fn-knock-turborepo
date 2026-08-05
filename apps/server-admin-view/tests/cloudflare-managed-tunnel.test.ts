import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";

const readSource = (path: string) =>
  readFileSync(new URL(path, import.meta.url), "utf8");

describe("managed Cloudflare Tunnel", () => {
  it("exposes the credential, reconcile, scan, apply, and fallback contracts", () => {
    const source = readSource("../src/lib/api/tunnel.ts");
    for (const route of [
      "/cloudflared/cloudflare/credential",
      "/cloudflared/cloudflare/state",
      "/cloudflared/reconcile/preview",
      "/cloudflared/reconcile/apply",
      "/cloudflared/optimization/scans",
      "/cloudflared/optimization/settings",
      "/cloudflared/optimization/apply",
      "/cloudflared/optimization/fallback",
    ]) {
      assert.match(
        source,
        new RegExp(route.replaceAll("/", String.raw`\/`), "u"),
      );
    }
  });

  it("keeps Tunnel and API tokens write-only in the frontend contract", () => {
    const apiSource = readSource("../src/lib/api/tunnel.ts");
    const configContract = apiSource.slice(
      apiSource.indexOf("export type CloudflaredConfig"),
      apiSource.indexOf("export type CloudflareTunnelSummary"),
    );
    assert.match(configContract, /apiTokenConfigured: boolean/u);
    assert.match(configContract, /tunnelTokenConfigured: boolean/u);
    assert.doesNotMatch(configContract, /\btoken:\s*string/u);

    const controller = readSource(
      "../src/views/tunnel/cloudflare/useCloudflareTunnelController.ts",
    );
    assert.match(controller, /token\.value = ""/u);
    assert.doesNotMatch(controller, /token\.value\s*=\s*config\.token/u);
  });

  it("keeps automation, takeover preview, capability probe, and fallback visible", () => {
    const page = readSource("../src/views/tunnel/CloudflareTunnel.vue");
    assert.match(page, /CloudflareApiConnectionCard/u);
    assert.match(page, /CloudflareManagedTunnelCard/u);
    assert.match(page, /CloudflareOptimizationCard/u);
    assert.match(page, /CloudflareManualConfigCard/u);
    assert.ok(
      page.indexOf("admin.cloudflareTunnel.runtimeStatus") <
        page.indexOf("<CloudflareApiConnectionCard"),
    );
    assert.match(page, /:configured="false"/u);

    const managed = readSource(
      "../src/views/tunnel/cloudflare/CloudflareManagedTunnelCard.vue",
    );
    assert.match(managed, /ConfigCollapsibleCard/u);
    assert.match(managed, /reconcilePlan\.operations/u);
    assert.match(managed, /toggleTakeover/u);

    const optimization = readSource(
      "../src/views/tunnel/cloudflare/CloudflareOptimizationCard.vue",
    );
    assert.match(optimization, /ConfigCollapsibleCard/u);
    assert.match(optimization, /allCandidates/u);
    assert.match(optimization, /capabilityProbe/u);
    assert.match(optimization, /fallbackOptimization/u);
    assert.match(optimization, /recommendedIp/u);
    assert.match(optimization, /candidateSources/u);
    assert.match(optimization, /businessColo \|\| candidate\.colo/u);
    assert.match(optimization, /optimizationCustomHostnames/u);

    const connection = readSource(
      "../src/views/tunnel/cloudflare/CloudflareApiConnectionCard.vue",
    );
    assert.match(connection, /ConfigCollapsibleCard/u);
    assert.doesNotMatch(connection, /permissionsTitle|CheckCircle2/u);

    const manual = readSource(
      "../src/views/tunnel/cloudflare/CloudflareManualConfigCard.vue",
    );
    assert.match(manual, /:configured="true"/u);

    for (const source of [page, connection, managed, optimization, manual]) {
      assert.match(source, /#actions="\{ collapse \}"/u);
      assert.match(source, /@click="collapse"/u);
    }
  });

  it("keeps third-party candidate hostnames DNS-only and provenance visible", () => {
    const backend = readSource(
      "../../server-admin-rs/src/tunnels/cloudflared/optimization.rs",
    );
    assert.match(backend, /cloudflare-dns\.com\/dns-query/u);
    assert.match(backend, /dns\.google\/resolve/u);
    assert.match(backend, /candidate_ip_is_cloudflare/u);
    assert.match(backend, /source_hostnames/u);
    assert.match(backend, /business_validated/u);
    assert.doesNotMatch(backend, /content:\s*source\.hostname/u);
    assert.doesNotMatch(backend, /www\.fbi\.gov/u);

    for (const locale of ["zh-CN", "zh-Hant", "en", "ja-JP", "ko-KR"]) {
      const messages = readSource(
        `../../../packages/i18n/src/messages/admin/${locale}.ts`,
      );
      assert.doesNotMatch(messages, /us-fbi/u);
    }
  });

  it("gates scans and candidate publishing on the applied managed state", () => {
    const controller = readSource(
      "../src/views/tunnel/cloudflare/useCloudflareTunnelController.ts",
    );
    assert.match(controller, /optimization\.value\?\.enabled === true/u);
    assert.match(controller, /if \(!optimizationApplied\.value\)/u);

    const optimization = readSource(
      "../src/views/tunnel/cloudflare/CloudflareOptimizationCard.vue",
    );
    assert.match(optimization, /!optimizationApplied/u);
    assert.match(optimization, /reconcileRequiredDescription/u);
  });
});
