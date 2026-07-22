/// <reference types="node" />

import { describe, it } from "node:test";
import assert from "node:assert/strict";

import { toHostMappingUpdatePayload } from "../src/lib/api/host-mapping-payload";
import {
  getMappingFaviconSource,
  getMappingFaviconSrc,
  createDefaultMapping,
  normalizeMappingForm,
} from "../src/views/subdomain-proxy/model";
import {
  getMappingIconFileValidationIssue,
  MappingIconProcessingError,
  MAPPING_ICON_FILE_ACCEPT,
  MAX_MAPPING_ICON_SOURCE_BYTES,
  prepareMappingIconSvgSource,
} from "../src/views/subdomain-proxy/mapping-icon";

const AUTO_ICON = "data:image/png;base64,YXV0bw==";
const CUSTOM_ICON = "data:image/webp;base64,Y3VzdG9t";

describe("subdomain custom icon", () => {
  it("prefers the custom icon and reports its source", () => {
    const mapping = createDefaultMapping();
    mapping.favicon = AUTO_ICON;
    mapping.favicon_override = CUSTOM_ICON;

    assert.equal(getMappingFaviconSrc(mapping), CUSTOM_ICON);
    assert.equal(getMappingFaviconSource(mapping), "custom");

    mapping.favicon_override = "";
    assert.equal(getMappingFaviconSrc(mapping), AUTO_ICON);
    assert.equal(getMappingFaviconSource(mapping), "auto");

    mapping.favicon = "";
    assert.equal(getMappingFaviconSource(mapping), "missing");
  });

  it("persists overrides while only submitting refreshed metadata explicitly", () => {
    const mapping = createDefaultMapping();
    mapping.favicon = AUTO_ICON;
    mapping.favicon_override = CUSTOM_ICON;
    mapping.title = "Collected title";

    const ordinary = toHostMappingUpdatePayload(mapping);
    assert.equal(ordinary.favicon_override, CUSTOM_ICON);
    assert.equal("favicon" in ordinary, false);
    assert.equal("title" in ordinary, false);

    const refreshed = toHostMappingUpdatePayload(mapping, {
      includeFavicon: true,
      includeTitle: true,
    });
    assert.equal(refreshed.favicon, AUTO_ICON);
    assert.equal(refreshed.title, "Collected title");
  });

  it("keeps the custom icon when a target change invalidates automatic metadata", () => {
    const mapping = createDefaultMapping();
    mapping.target = "http://127.0.0.1:8080";
    mapping.favicon = AUTO_ICON;
    mapping.favicon_override = CUSTOM_ICON;

    const normalized = normalizeMappingForm(mapping, {
      hasFreshFaviconMetadata: false,
      hasFreshTitleMetadata: false,
      host: "app.example.com",
      isAuthServiceTarget: () => false,
      isWebSocketTarget: () => false,
    });

    assert.equal(normalized.favicon, "");
    assert.equal(normalized.favicon_override, CUSTOM_ICON);
  });

  it("validates supported source formats and the source size limit", () => {
    assert.equal(
      getMappingIconFileValidationIssue({
        name: "icon.png",
        size: 1024,
        type: "image/png",
      }),
      null,
    );
    for (const file of [
      { name: "icon.svg", size: 1024, type: "image/svg+xml" },
      { name: "icon.avif", size: 1024, type: "image/avif" },
      { name: "icon.webp", size: 1024, type: "image/x-webp" },
      { name: "icon.webp", size: 1024, type: "application/x-unknown" },
    ]) {
      assert.equal(getMappingIconFileValidationIssue(file), null);
    }
    assert.match(MAPPING_ICON_FILE_ACCEPT, /\.svg/);
    assert.match(MAPPING_ICON_FILE_ACCEPT, /\.avif/);
    assert.equal(
      getMappingIconFileValidationIssue({
        name: "icon.webp",
        size: MAX_MAPPING_ICON_SOURCE_BYTES + 1,
        type: "image/webp",
      }),
      "source_too_large",
    );
  });

  it("strips a standard external SVG doctype before parsing", () => {
    const source = `<?xml version="1.0" standalone="no"?><!DOCTYPE svg PUBLIC "-//W3C//DTD SVG 1.1//EN" "http://www.w3.org/Graphics/SVG/1.1/DTD/svg11.dtd"><svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M0 0h24v24H0z"/></svg>`;

    const prepared = prepareMappingIconSvgSource(source);

    assert.doesNotMatch(prepared, /<!doctype/i);
    assert.match(prepared, /^<\?xml[^>]*\?><svg/);
    assert.match(prepared, /<path d="M0 0h24v24H0z"\/>/);
  });

  it("rejects entity declarations and unsafe SVG doctypes", () => {
    const unsafeSources = [
      `<!DOCTYPE svg [<!ENTITY payload SYSTEM "file:///etc/passwd">]><svg xmlns="http://www.w3.org/2000/svg"><text>&payload;</text></svg>`,
      `<!DOCTYPE svg [<!ELEMENT svg ANY>]><svg xmlns="http://www.w3.org/2000/svg"/>`,
      `<!DOCTYPE html SYSTEM "https://example.com/example.dtd"><svg xmlns="http://www.w3.org/2000/svg"/>`,
      `<svg xmlns="http://www.w3.org/2000/svg"/><!DOCTYPE svg SYSTEM "https://example.com/example.dtd">`,
      `<!DOCTYPE svg><!DOCTYPE svg><svg xmlns="http://www.w3.org/2000/svg"/>`,
    ];

    for (const source of unsafeSources) {
      assert.throws(
        () => prepareMappingIconSvgSource(source),
        (error) =>
          error instanceof MappingIconProcessingError &&
          error.kind === "decode_failed",
      );
    }
  });
});
