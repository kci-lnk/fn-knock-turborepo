/// <reference types="node" />

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { isDockerAdminAuthRequiredResponse } from "../src/lib/docker-admin-auth-response";

describe("Docker admin authentication response marker", () => {
  it("accepts only marked 401 responses", () => {
    assert.equal(
      isDockerAdminAuthRequiredResponse({
        response: {
          status: 401,
          headers: { "x-fn-knock-admin-auth": "required" },
        },
      }),
      true,
    );
    assert.equal(
      isDockerAdminAuthRequiredResponse({
        response: { status: 401, headers: {} },
      }),
      false,
    );
    assert.equal(
      isDockerAdminAuthRequiredResponse({
        response: {
          status: 403,
          headers: { "x-fn-knock-admin-auth": "required" },
        },
      }),
      false,
    );
  });

  it("supports AxiosHeaders-style access and case-insensitive values", () => {
    const headers = new Map<string, string>([
      ["x-fn-knock-admin-auth", "required"],
    ]);
    assert.equal(
      isDockerAdminAuthRequiredResponse({
        response: { status: 401, headers },
      }),
      true,
    );
    assert.equal(
      isDockerAdminAuthRequiredResponse({
        response: {
          status: 401,
          headers: { "X-Fn-Knock-Admin-Auth": "required" },
        },
      }),
      true,
    );
  });
});
