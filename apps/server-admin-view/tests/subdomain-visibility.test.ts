/// <reference types="node" />

import { describe, it } from "node:test";
import assert from "node:assert/strict";

import { toHostMappingUpdatePayload } from "../src/lib/api/host-mapping-payload";
import {
  createDefaultMapping,
  normalizeMappingForm,
  normalizeMappingVisibility,
} from "../src/views/subdomain-proxy/model";
import {
  getMappingVisibilityValidationIssue,
  shouldBlockMappingSaveForVisibility,
  shouldReturnToVisibilityAfterSaveError,
} from "../src/views/subdomain-proxy/useMappingVisibility";

describe("subdomain visibility", () => {
  it("defaults new and legacy mappings to global inheritance", () => {
    assert.equal(createDefaultMapping().visibility.mode, "inherit");
    assert.deepEqual(normalizeMappingVisibility(undefined), {
      mode: "inherit",
      selections: [],
      custom_cidrs: [],
      cidrs: [],
    });
  });

  it("retains the custom draft while inheritance is selected", () => {
    assert.deepEqual(
      normalizeMappingVisibility({
        mode: "inherit",
        selections: [
          {
            province: "浙江省",
            city: "杭州市",
            label: "杭州市",
            value: "浙江省::杭州市",
            query_city: "杭州市",
            is_province_wide: false,
            is_municipality: false,
          },
        ],
        custom_cidrs: ["203.0.113.0/24"],
        cidrs: ["203.0.113.0/24"],
      }),
      {
        mode: "inherit",
        selections: [
          {
            province: "浙江省",
            city: "杭州市",
            label: "杭州市",
            value: "浙江省::杭州市",
            query_city: "杭州市",
            is_province_wide: false,
            is_municipality: false,
          },
        ],
        custom_cidrs: ["203.0.113.0/24"],
        cidrs: ["203.0.113.0/24"],
      },
    );
  });

  it("normalizes and submits disabled visibility without validating its saved draft", () => {
    const mapping = createDefaultMapping();
    mapping.visibility = {
      mode: "disabled",
      selections: [],
      custom_cidrs: ["203.0.113.0/24"],
      cidrs: ["203.0.113.0/24"],
    };

    assert.equal(
      normalizeMappingVisibility(mapping.visibility).mode,
      "disabled",
    );
    assert.equal(
      toHostMappingUpdatePayload(mapping).visibility.mode,
      "disabled",
    );
    assert.equal(
      getMappingVisibilityValidationIssue({
        available: true,
        mode: "disabled",
        selectionsCount: 0,
        customCidrsText: "not-a-cidr",
      }),
      null,
    );
  });

  it("forces authentication-service mappings to inherit globally", () => {
    const mapping = createDefaultMapping();
    mapping.target = "http://127.0.0.1:7997";
    mapping.visibility = {
      mode: "custom",
      selections: [
        {
          province: "浙江省",
          city: "杭州市",
          label: "杭州市",
          value: "浙江省::杭州市",
          query_city: "杭州市",
          is_province_wide: false,
          is_municipality: false,
        },
      ],
      custom_cidrs: ["203.0.113.0/24"],
      cidrs: ["203.0.113.0/24"],
    };

    const normalized = normalizeMappingForm(mapping, {
      hasFreshMetadata: false,
      host: "auth.example.com",
      isAuthServiceTarget: () => true,
      isWebSocketTarget: () => false,
    });

    assert.deepEqual(normalized.visibility, {
      mode: "inherit",
      selections: [],
      custom_cidrs: [],
      cidrs: [],
    });
  });

  it("does not submit the server-derived CIDRs", () => {
    const mapping = createDefaultMapping();
    mapping.visibility = {
      mode: "custom",
      selections: [
        {
          province: "浙江省",
          city: "杭州市",
          label: "杭州市",
          value: "浙江省::杭州市",
          query_city: "杭州市",
          is_province_wide: false,
          is_municipality: false,
        },
      ],
      custom_cidrs: ["203.0.113.0/24"],
      cidrs: ["198.51.100.0/24"],
    };

    const payload = toHostMappingUpdatePayload(mapping);
    assert.deepEqual(payload.visibility.custom_cidrs, ["203.0.113.0/24"]);
    assert.deepEqual(payload.visibility.selections, [
      { province: "浙江省", query_city: "杭州市" },
    ]);
    assert.equal("cidrs" in payload.visibility, false);
  });

  it("rejects empty custom rules and malformed CIDRs", () => {
    assert.deepEqual(
      getMappingVisibilityValidationIssue({
        available: true,
        mode: "custom",
        selectionsCount: 0,
        customCidrsText: "",
      }),
      { kind: "empty" },
    );
    assert.deepEqual(
      getMappingVisibilityValidationIssue({
        available: true,
        mode: "custom",
        selectionsCount: 0,
        customCidrsText: "not-a-cidr",
      }),
      { kind: "invalid_cidrs", invalid: ["not-a-cidr"] },
    );
  });

  it("blocks business mapping saves when global visibility cannot be loaded", () => {
    assert.equal(
      shouldBlockMappingSaveForVisibility({
        isAuthService: false,
        isLoading: false,
        loadError: "network unavailable",
        validationMessage: "",
      }),
      true,
    );
    assert.equal(
      shouldBlockMappingSaveForVisibility({
        isAuthService: true,
        isLoading: false,
        loadError: "network unavailable",
        validationMessage: "",
      }),
      false,
    );
  });

  it("returns to visibility settings after custom or disabled save failures", () => {
    assert.equal(shouldReturnToVisibilityAfterSaveError("inherit"), false);
    assert.equal(shouldReturnToVisibilityAfterSaveError("custom"), true);
    assert.equal(shouldReturnToVisibilityAfterSaveError("disabled"), true);
  });
});
