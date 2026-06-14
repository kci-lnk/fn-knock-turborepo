import assert from "node:assert/strict";
import test from "node:test";
import {
  translate,
  type LocaleCode,
  type MessageParams,
} from "../../../../packages/i18n/src";
import {
  buildSubdomainCertificateCoverage,
  buildSubdomainCertificateInventoryCoverage,
} from "./subdomain-mode";
import type { AppConfig } from "./redis";

const translateWithLocale =
  (locale: LocaleCode) => (key: string, params?: MessageParams) =>
    translate(locale, key, params);

const config = {
  subdomain_mode: {
    root_domain: "example.com",
    auth_host: "auth.example.com",
  },
  host_mappings: [{ host: "app.example.com", target: "http://127.0.0.1:3000" }],
} as Pick<AppConfig, "subdomain_mode" | "host_mappings">;

test("subdomain certificate coverage summary uses provided translator", () => {
  const coverage = buildSubdomainCertificateCoverage({
    config,
    certificateDomains: ["example.com", "*.example.com"],
    t: translateWithLocale("en"),
  });

  assert.equal(coverage.status, "ready");
  assert.equal(
    coverage.summary,
    "The deployed certificate covers the auth service and all configured Host mappings.",
  );
});

test("subdomain certificate inventory summary uses provided translator", () => {
  const coverage = buildSubdomainCertificateInventoryCoverage({
    config,
    certificates: [
      {
        id: "active-cert",
        certificateDomains: ["example.com", "*.example.com"],
      },
    ],
    activeCertificateId: "active-cert",
    t: translateWithLocale("en"),
  });

  assert.equal(coverage.status, "ready");
  assert.equal(
    coverage.summary,
    "The active certificate fully covers the domains required by subdomain mode.",
  );
});
