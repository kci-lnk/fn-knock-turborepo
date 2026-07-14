/// <reference types="node" />

import { describe, it } from "node:test";
import assert from "node:assert/strict";

import type { HostMapping } from "../src/types";
import {
  createDefaultMapping,
  getMappingSecurityIndicatorState,
} from "../src/views/subdomain-proxy/model";

const getState = (
  mapping: HostMapping,
  overrides: Partial<{
    globalVisibilityEnabled: boolean;
    globalWafEnabled: boolean;
    isAuthService: boolean;
  }> = {},
) =>
  getMappingSecurityIndicatorState({
    globalVisibilityEnabled: true,
    globalWafEnabled: true,
    isAuthService: false,
    mapping,
    ...overrides,
  });

describe("subdomain mapping security indicators", () => {
  it("shows WAF only when it is globally active and not explicitly disabled", () => {
    const mapping = createDefaultMapping();

    assert.equal(getState(mapping).waf, true);
    assert.equal(getState(mapping, { globalWafEnabled: false }).waf, false);

    mapping.waf_enabled = false;
    assert.equal(getState(mapping).waf, false);

    const legacyMapping = createDefaultMapping();
    delete (legacyMapping as Partial<HostMapping>).waf_enabled;
    assert.equal(getState(legacyMapping).waf, true);
  });

  it("distinguishes inherited, custom, disabled and globally inactive visibility", () => {
    const mapping = createDefaultMapping();

    assert.equal(getState(mapping).visibility, "inherit");
    assert.equal(
      getState(mapping, { globalVisibilityEnabled: false }).visibility,
      null,
    );

    mapping.visibility = {
      mode: "custom",
      selections: [
        {
          province: "上海市",
          city: null,
          label: "上海市",
          value: "上海市",
          query_city: null,
          is_province_wide: true,
          is_municipality: true,
        },
      ],
      custom_cidrs: ["203.0.113.0/24", "2001:db8::/32"],
      cidrs: ["203.0.113.0/24", "2001:db8::/32"],
    };
    assert.deepEqual(getState(mapping), {
      customCidrCount: 2,
      regionCount: 1,
      visibility: "custom",
      waf: true,
    });

    mapping.visibility.mode = "disabled";
    assert.equal(getState(mapping).visibility, null);
  });

  it("excludes authentication services and disabled mappings", () => {
    const mapping = createDefaultMapping();

    assert.deepEqual(getState(mapping, { isAuthService: true }), {
      customCidrCount: 0,
      regionCount: 0,
      visibility: null,
      waf: false,
    });

    mapping.disabled = true;
    assert.deepEqual(getState(mapping), {
      customCidrCount: 0,
      regionCount: 0,
      visibility: null,
      waf: false,
    });
  });
});
