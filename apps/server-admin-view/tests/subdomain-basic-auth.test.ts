/// <reference types="node" />

import { describe, it } from "node:test";
import assert from "node:assert/strict";

import { toHostMappingUpdatePayload } from "../src/lib/api/host-mapping-payload";
import { createDefaultMapping } from "../src/views/subdomain-proxy/model";
import type { HostMapping } from "../src/types";

describe("subdomain Basic Auth payload", () => {
  it("treats a missing legacy Basic Auth object as disabled", () => {
    const legacyMapping = createDefaultMapping();
    delete (legacyMapping as Partial<HostMapping>).basic_auth;

    assert.deepEqual(toHostMappingUpdatePayload(legacyMapping).basic_auth, {
      enabled: false,
      username: "",
      password: "",
    });
  });

  it("normalizes incomplete enabled Basic Auth credentials", () => {
    const legacyMapping = createDefaultMapping();
    legacyMapping.basic_auth = {
      enabled: true,
      username: "  admin  ",
    } as HostMapping["basic_auth"];

    assert.deepEqual(toHostMappingUpdatePayload(legacyMapping).basic_auth, {
      enabled: false,
      username: "",
      password: "",
    });
  });

  it("preserves valid Basic Auth credentials", () => {
    const mapping = createDefaultMapping();
    mapping.basic_auth = {
      enabled: true,
      username: "  admin  ",
      password: "secret",
    };

    assert.deepEqual(toHostMappingUpdatePayload(mapping).basic_auth, {
      enabled: true,
      username: "admin",
      password: "secret",
    });
  });
});
