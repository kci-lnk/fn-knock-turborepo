import assert from "node:assert/strict";
import { describe, it } from "node:test";

import {
  formatOptimizationDate,
  formatOptimizationNumber,
  optimizationCandidateSourceLabel,
  optimizationDomainMessageLabel,
  optimizationPreferredIpErrorLabel,
  optimizationResolverProviderLabel,
  optimizationResolverPathLabel,
  optimizationResolverStatusLabel,
  optimizationScanErrorPresentation,
  optimizationSourceWarningLabel,
  requiresCloudflareSaasSetup,
} from "../src/views/tunnel/cloudflare/cloudflareOptimizationPresentation";

const translate = ((key: string, values?: Record<string, unknown>) =>
  values?.detail
    ? `${key}:${String(values.detail)}`
    : values?.providers
      ? `${key}:${String(values.providers)}`
      : key) as Parameters<typeof optimizationDomainMessageLabel>[1];

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

  it("keeps scan error severity and localization in one presentation model", () => {
    assert.deepEqual(
      optimizationScanErrorPresentation(
        "cloudflare-saas-validation-pending",
        "pending",
        translate,
      ),
      {
        message:
          "admin.cloudflareTunnel.optimization.cloudflareSaasValidationPendingDescription",
        neutral: true,
        title:
          "admin.cloudflareTunnel.optimization.cloudflareSaasValidationPendingTitle",
      },
    );
    assert.deepEqual(
      optimizationScanErrorPresentation(null, "unexpected failure", translate),
      { message: "unexpected failure", neutral: false, title: "" },
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
      optimizationCandidateSourceLabel(
        {
          sourceHostnames: ["www.example.org"],
          sourceTypes: ["official-range", "preferred-ip"],
        },
        translate,
      ),
      "admin.cloudflareTunnel.optimization.sources.preferredIpShort",
    );
    assert.equal(
      optimizationSourceWarningLabel(
        "edge.example.com (custom) did not resolve to a verified Cloudflare IPv4 address",
        translate,
      ),
      "admin.cloudflareTunnel.optimization.sources.unverifiedAddress",
    );
    assert.equal(
      optimizationSourceWarningLabel(
        "edge.example.com: DNS timeout",
        translate,
      ),
      "admin.cloudflareTunnel.optimization.sources.resolveFailed:DNS timeout",
    );
    assert.equal(
      optimizationPreferredIpErrorLabel(
        "Preferred IP must be a valid IPv4 address",
        translate,
      ),
      "admin.cloudflareTunnel.optimization.preferredIpInvalid",
    );
  });

  it("localizes resolver provider and health diagnostics", () => {
    assert.equal(
      optimizationResolverProviderLabel("dnspod", translate),
      "admin.cloudflareTunnel.optimization.sources.resolvers.dnspod",
    );
    assert.equal(
      optimizationResolverStatusLabel("degraded", translate),
      "admin.cloudflareTunnel.optimization.sources.resolverStatuses.degraded",
    );
    assert.equal(
      optimizationResolverPathLabel("multi-doh", ["DNSPod"], translate),
      "admin.cloudflareTunnel.optimization.sources.resolverPathAvailable:DNSPod",
    );
    assert.equal(
      optimizationResolverPathLabel("official-ranges", [], translate),
      "admin.cloudflareTunnel.optimization.sources.resolverPathOfficialRanges",
    );
    assert.equal(
      optimizationResolverPathLabel("current-candidate", [], translate),
      "admin.cloudflareTunnel.optimization.sources.resolverPathCurrentCandidate",
    );
    assert.equal(
      optimizationResolverPathLabel("preferred-ip", [], translate),
      "admin.cloudflareTunnel.optimization.sources.resolverPathPreferredIp",
    );
    assert.equal(
      optimizationResolverPathLabel("unavailable", [], translate),
      "admin.cloudflareTunnel.optimization.sources.resolverPathUnavailable",
    );
  });
});
