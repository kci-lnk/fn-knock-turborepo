/// <reference types="node" />

import { describe, it } from "node:test";
import assert from "node:assert/strict";

import {
  createDefaultMapping,
  normalizeMappingForm,
} from "../src/views/subdomain-proxy/model";
import { toHostMappingUpdatePayload } from "../src/lib/api/host-mapping-payload";

describe("subdomain WAF mapping state", () => {
  it("defaults new mappings to WAF enabled", () => {
    assert.equal(createDefaultMapping().waf_enabled, true);
  });

  it("preserves a business-subdomain exception", () => {
    const mapping = createDefaultMapping();
    mapping.target = "http://127.0.0.1:8080";
    mapping.waf_enabled = false;

    const normalized = normalizeMappingForm(mapping, {
      hasFreshFaviconMetadata: false,
      hasFreshTitleMetadata: false,
      host: "app.example.com",
      isAuthServiceTarget: () => false,
      isWebSocketTarget: () => false,
    });

    assert.equal(normalized.waf_enabled, false);
  });

  it("forces authentication-service mappings to inherit global WAF", () => {
    const mapping = createDefaultMapping();
    mapping.target = "http://127.0.0.1:7997";
    mapping.waf_enabled = false;

    const normalized = normalizeMappingForm(mapping, {
      hasFreshFaviconMetadata: false,
      hasFreshTitleMetadata: false,
      host: "auth.example.com",
      isAuthServiceTarget: () => true,
      isWebSocketTarget: () => false,
    });

    assert.equal(normalized.waf_enabled, true);
  });

  it("includes the WAF declaration in update payloads", () => {
    const mapping = createDefaultMapping();
    mapping.waf_enabled = false;

    assert.equal(toHostMappingUpdatePayload(mapping).waf_enabled, false);
  });
});
