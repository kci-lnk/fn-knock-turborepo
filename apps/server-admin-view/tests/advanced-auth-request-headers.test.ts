/// <reference types="node" />

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";

import { advancedAuthRequestHeaderNames } from "../src/views/subdomain-proxy/advanced-auth-request-headers";

const protectedHeaderNames = [
  "authorization",
  "cookie",
  "host",
  "connection",
  "forwarded",
  "proxy-authorization",
  "transfer-encoding",
  "upgrade",
  "x-forwarded-for",
  "x-real-ip",
  "cf-connecting-ip",
];

describe("advanced authentication request header suggestions", () => {
  it("does not contain case-insensitive duplicates", () => {
    const normalized = advancedAuthRequestHeaderNames.map((header) =>
      header.toLowerCase(),
    );

    assert.equal(new Set(normalized).size, normalized.length);
  });

  it("does not suggest protected request headers", () => {
    const normalized = new Set(
      advancedAuthRequestHeaderNames.map((header) => header.toLowerCase()),
    );

    for (const header of protectedHeaderNames) {
      assert.equal(
        normalized.has(header),
        false,
        `${header} must not be suggested`,
      );
    }
    assert.equal(
      [...normalized].some((header) => header.startsWith("x-forwarded-")),
      false,
    );
  });

  it("uses the control-plane canonical header casing", () => {
    const canonicalize = (header: string) =>
      header
        .trim()
        .split("-")
        .map(
          (segment) =>
            `${segment.charAt(0).toUpperCase()}${segment.slice(1).toLowerCase()}`,
        )
        .join("-");

    for (const header of advancedAuthRequestHeaderNames) {
      assert.equal(header, canonicalize(header));
    }
  });

  it("leaves native input and IME composition events under Reka ownership", () => {
    const source = readFileSync(
      new URL(
        "../src/views/subdomain-proxy/AdvancedAuthHeaderNameField.vue",
        import.meta.url,
      ),
      "utf8",
    );

    assert.match(source, /<AutocompleteInput\s/u);
    assert.doesNotMatch(source, /<AutocompleteInput\s+as-child/u);
    assert.doesNotMatch(source, /components\/ui\/input/u);
  });
});
