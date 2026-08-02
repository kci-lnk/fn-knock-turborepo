/// <reference types="node" />

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { toHostMappingUpdatePayload } from "../src/lib/api/host-mapping-payload";
import { createDefaultMapping } from "../src/views/subdomain-proxy/model";

describe("subdomain rename identity", () => {
  it("sends the previous host only for a genuine rename", () => {
    const mapping = {
      ...createDefaultMapping(),
      host: "new.example.com",
      target: "http://127.0.0.1:9090",
    };

    assert.equal(
      toHostMappingUpdatePayload(mapping, {
        previousHost: " old.example.com ",
      }).previous_host,
      "old.example.com",
    );
    assert.equal(
      toHostMappingUpdatePayload(mapping, {
        previousHost: "new.example.com",
      }).previous_host,
      undefined,
    );
    assert.equal(
      toHostMappingUpdatePayload(mapping).previous_host,
      undefined,
    );
  });
});
