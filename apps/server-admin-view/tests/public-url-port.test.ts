import assert from "node:assert/strict";
import test from "node:test";
import {
  parseExplicitPublicUrlPort,
  resolveExplicitPublicAccessEntryPort,
} from "../src/lib/reverse-proxy-submode";
import {
  normalizePublicPort,
  parsePublicAuthBaseUrlPort,
} from "../src/views/subdomain-proxy/model";

for (const [raw, scheme, expected] of [
  ["https://auth.example.com:443/path?port=7999", "https", 443],
  ["http://auth.example.com:80/", "http", 80],
  ["https://[::1]:443/", "https", 443],
  ["https://[::1]/", "https", null],
  ["https://auth.example.com:8443/", "https", 8443],
  ["https://auth.example.com/", "https", null],
  ["https://user:443@auth.example.com/", "https", null],
  ["https://auth.example.com:0/", "https", null],
  ["https://auth.example.com:65536/", "https", null],
  ["https://auth.example.com:443/", "http", null],
  ["ftp://auth.example.com:21/", undefined, null],
] as const) {
  test(`public URL parser handles ${raw} for ${scheme}`, () => {
    assert.equal(parseExplicitPublicUrlPort(raw, scheme), expected);
    assert.equal(parsePublicAuthBaseUrlPort(raw, scheme), expected ?? 0);
  });
}

test("gateway host settings preserve an explicit default port", () => {
  const config = {
    subdomain_mode: {
      public_auth_base_url: "https://auth.example.com:443",
      public_https_port: 7999,
    },
  } as Parameters<typeof resolveExplicitPublicAccessEntryPort>[0];
  assert.equal(resolveExplicitPublicAccessEntryPort(config), 443);
});

test("out-of-range ports cannot become valid-looking form previews", () => {
  for (const port of [65536, -1, Infinity, NaN])
    assert.equal(normalizePublicPort(port), 0);
  assert.equal(normalizePublicPort(65535), 65535);
});

test("gateway host settings choose ports for the public protocol", () => {
  const config = {
    subdomain_mode: {
      public_auth_base_url: "http://auth.example.com:9080",
      public_http_port: 8080,
      public_https_port: 8443,
    },
  } as Parameters<typeof resolveExplicitPublicAccessEntryPort>[0];
  assert.equal(resolveExplicitPublicAccessEntryPort(config, "https"), 8443);
  assert.equal(resolveExplicitPublicAccessEntryPort(config, "http"), 9080);
});
