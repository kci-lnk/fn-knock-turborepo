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

    const managed = readSource(
      "../src/views/tunnel/cloudflare/CloudflareManagedTunnelCard.vue",
    );
    assert.match(managed, /reconcilePlan\.operations/u);
    assert.match(managed, /toggleTakeover/u);

    const optimization = readSource(
      "../src/views/tunnel/cloudflare/CloudflareOptimizationCard.vue",
    );
    assert.match(optimization, /capabilityProbe/u);
    assert.match(optimization, /fallbackOptimization/u);
    assert.match(optimization, /recommendedIp/u);
  });

  it("gates scans and candidate publishing on the applied managed state", () => {
    const controller = readSource(
      "../src/views/tunnel/cloudflare/useCloudflareTunnelController.ts",
    );
    assert.match(
      controller,
      /optimization\.value\?\.enabled === true/u,
    );
    assert.match(controller, /if \(!optimizationApplied\.value\)/u);

    const optimization = readSource(
      "../src/views/tunnel/cloudflare/CloudflareOptimizationCard.vue",
    );
    assert.match(optimization, /!optimizationApplied/u);
    assert.match(optimization, /reconcileRequiredDescription/u);
  });
});
