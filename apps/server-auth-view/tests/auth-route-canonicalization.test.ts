import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { canonicalAuthHistoryTarget } from "../src/lib/auth-route-canonicalization";

describe("auth route canonicalization", () => {
  it("migrates known legacy auth hash routes and retains both query sources", () => {
    assert.equal(
      canonicalAuthHistoryTarget({
        pathname: "/",
        search: "?_ts=100",
        hash: "#/login?redirect_uri=https%3A%2F%2Fapp.example.com%2F",
      }),
      "/login?_ts=100&redirect_uri=https%3A%2F%2Fapp.example.com%2F",
    );
    assert.equal(
      canonicalAuthHistoryTarget({
        pathname: "/auth/",
        search: "",
        hash: "#/oidc/bind?invite=one",
      }),
      "/auth/oidc/bind?invite=one",
    );
  });

  it("does not reinterpret admin or business hash routes as auth paths", () => {
    for (const hash of ["#/whitelist", "#/whitelist?", "#/dashboard"]) {
      assert.equal(
        canonicalAuthHistoryTarget({
          pathname: "/",
          search: "",
          hash,
        }),
        null,
        `must preserve ${hash}`,
      );
    }
  });

  it("still repairs duplicated mounted auth prefixes", () => {
    assert.equal(
      canonicalAuthHistoryTarget({
        pathname: "/__auth__/__auth__/login",
        search: "?redirect_uri=%2F",
        hash: "",
      }),
      "/__auth__/login?redirect_uri=%2F",
    );
  });
});
