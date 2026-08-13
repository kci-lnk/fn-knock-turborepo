import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { toHostMappingUpdatePayload } from "../src/lib/api/host-mapping-payload";
import {
  createDefaultMapping,
  normalizeMappingForm,
} from "../src/views/subdomain-proxy/model";

describe("subdomain target path mode", () => {
  it("defaults new and legacy mappings to backward-compatible entry mode", () => {
    const mapping = createDefaultMapping();

    assert.equal(mapping.target_path_mode, "entry");
    assert.equal(toHostMappingUpdatePayload(mapping).target_path_mode, "entry");
  });

  it("preserves explicit prefix mode in normalized forms and API payloads", () => {
    const mapping = createDefaultMapping();
    mapping.target = "http://127.0.0.1:8080/webdav";
    mapping.target_path_mode = "prefix";

    const normalized = normalizeMappingForm(mapping, {
      hasFreshFaviconMetadata: false,
      hasFreshTitleMetadata: false,
      host: "dav.example.com",
      isAuthServiceTarget: () => false,
      isWebSocketTarget: () => false,
    });

    assert.equal(normalized.target_path_mode, "prefix");
    assert.equal(
      toHostMappingUpdatePayload(normalized).target_path_mode,
      "prefix",
    );
  });

  it("forces the internal authentication mapping to entry mode", () => {
    const mapping = createDefaultMapping();
    mapping.target = "http://127.0.0.1:7997/auth";
    mapping.use_auth = false;
    mapping.target_path_mode = "prefix";

    assert.equal(
      normalizeMappingForm(mapping, {
        hasFreshFaviconMetadata: false,
        hasFreshTitleMetadata: false,
        host: "auth.example.com",
        isAuthServiceTarget: () => true,
        isWebSocketTarget: () => false,
      }).target_path_mode,
      "entry",
    );
  });
});
