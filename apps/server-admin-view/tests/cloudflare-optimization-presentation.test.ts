import assert from "node:assert/strict";
import { describe, it } from "node:test";

import {
  formatOptimizationDate,
  formatOptimizationNumber,
  optimizationDomainMessageLabel,
  optimizationSourceWarningLabel,
  requiresCloudflareSaasSetup,
} from "../src/views/tunnel/cloudflare/cloudflareOptimizationPresentation";

const translate = ((key: string, values?: Record<string, unknown>) =>
  values?.detail ? `${key}:${String(values.detail)}` : key) as Parameters<
  typeof optimizationDomainMessageLabel
>[1];

describe("Cloudflare optimization presentation", () => {
  it("keeps numeric and locale-aware date fallbacks deterministic", () => {
    assert.equal(formatOptimizationNumber(Number.NaN), "-");
    assert.equal(formatOptimizationNumber(12.345, 2), "12.35");
    assert.equal(formatOptimizationDate(undefined, "en-US"), "-");
    assert.equal(formatOptimizationDate("not-a-date", "en-US"), "not-a-date");
    assert.notEqual(
      formatOptimizationDate("2026-01-02T03:04:05.000Z", "en-US"),
      "2026-01-02T03:04:05.000Z",
    );
  });

  it("distinguishes SaaS entitlement errors from ordinary readiness", () => {
    assert.equal(
      requiresCloudflareSaasSetup("cloudflare-saas-required", null),
      true,
    );
    assert.equal(
      requiresCloudflareSaasSetup(
        null,
        "No quota has been allocated for this account. (1404)",
      ),
      true,
    );
    assert.equal(
      requiresCloudflareSaasSetup(
        "cloudflare-saas-required",
        "No active business or capability hostname is ready",
      ),
      false,
    );
  });

  it("localizes structured and legacy domain diagnostics", () => {
    const structured = {
      hostname: "app.example.com",
      messageCode: "preferredEdgeProbeFailed",
      messageDetail: "HTTP 530",
    } as Parameters<typeof optimizationDomainMessageLabel>[0];
    assert.equal(
      optimizationDomainMessageLabel(structured, translate),
      "admin.cloudflareTunnel.optimization.domainMessages.preferredEdgeProbeFailed:HTTP 530",
    );

    const legacy = {
      hostname: "app.example.com",
      message: "Custom Hostname quota is unavailable: API unavailable",
    } as Parameters<typeof optimizationDomainMessageLabel>[0];
    assert.equal(
      optimizationDomainMessageLabel(legacy, translate),
      "admin.cloudflareTunnel.optimization.domainMessages.customHostnameQuotaUnavailable:API unavailable",
    );
  });

  it("preserves candidate-source warning details", () => {
    assert.equal(
      optimizationSourceWarningLabel(
        "edge.example.com (custom) did not resolve to a verified Cloudflare IPv4 address",
        translate,
      ),
      "admin.cloudflareTunnel.optimization.sources.unverifiedAddress",
    );
    assert.equal(
      optimizationSourceWarningLabel("edge.example.com: DNS timeout", translate),
      "admin.cloudflareTunnel.optimization.sources.resolveFailed:DNS timeout",
    );
  });
});
