/// <reference types="node" />

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { extractErrorMessage } from "../src/errors/extractErrorMessage";

describe("extractErrorMessage", () => {
  it("prefers and normalizes an API response message", () => {
    assert.equal(
      extractErrorMessage({
        message: "request failed",
        response: { data: { message: "  validation failed  " } },
      }),
      "validation failed",
    );
  });

  it("supports plain-text API response bodies", () => {
    assert.equal(
      extractErrorMessage({ response: { data: "  service unavailable  " } }),
      "service unavailable",
    );
  });

  it("falls back to the direct error message when the response is empty", () => {
    assert.equal(
      extractErrorMessage({
        message: "  network error  ",
        response: { data: { message: "   " } },
      }),
      "network error",
    );
  });

  it("uses the supplied or default fallback for unknown errors", () => {
    assert.equal(extractErrorMessage(null, "Try again"), "Try again");
    assert.equal(
      extractErrorMessage({ response: { data: {} } }),
      "Operation failed",
    );
  });
});
