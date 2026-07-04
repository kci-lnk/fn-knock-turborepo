import assert from "node:assert/strict";
import test from "node:test";
import { analyzeService } from "./scanner/analyzers";
import { isDiscoverableServiceResult } from "./scanner";

test("HTTP redirect results are discoverable services", () => {
  assert.equal(
    isDiscoverableServiceResult({
      open: true,
      httpStatus: 302,
    }),
    true,
  );
});

test("plain HTTP requests sent to HTTPS ports are not discoverable services", () => {
  assert.equal(
    isDiscoverableServiceResult({
      open: true,
      httpStatus: 400,
      body: "<html><title>400 The plain HTTP request was sent to HTTPS port</title></html>",
    }),
    false,
  );
});

test("unknown HTTP services get a usable fallback mapping rule", async () => {
  const rule = await analyzeService({
    host: "192.168.31.1",
    port: 80,
    open: true,
    httpStatus: 302,
    headers: {
      location: "https://192.168.31.1/",
    },
  });

  assert.equal(rule.name, "http-80");
  assert.equal(rule.label, "HTTP 80");
  assert.equal(rule.rule.path, "/app-80");
});

test("generic HTTP service identities stay unique per port even with the same title", async () => {
  const firstRule = await analyzeService({
    host: "192.168.31.1",
    port: 8080,
    open: true,
    httpStatus: 200,
    body: "<html><title>Login</title></html>",
  });
  const secondRule = await analyzeService({
    host: "192.168.31.1",
    port: 9090,
    open: true,
    httpStatus: 200,
    body: "<html><title>Login</title></html>",
  });

  assert.equal(firstRule.name, "http-8080");
  assert.equal(secondRule.name, "http-9090");
  assert.equal(firstRule.label, "Login");
  assert.equal(secondRule.label, "Login");
});
