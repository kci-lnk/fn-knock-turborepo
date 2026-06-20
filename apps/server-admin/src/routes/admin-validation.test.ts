import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  cleanHostLocationPath,
  normalizeHostBasicAuth,
  normalizeHostMappingLocationsForRoute,
  validateProxyMappings,
  validateStreamMappings,
  type AdminRouteTranslator,
} from "./admin/validation";

const t: AdminRouteTranslator = (key, params) =>
  params ? `${key}:${JSON.stringify(params)}` : key;

describe("admin route validation helpers", () => {
  it("normalizes nested host location paths", () => {
    assert.equal(cleanHostLocationPath("/foo/./bar/../baz//"), "/foo/baz");
  });

  it("normalizes response locations and drops unsafe headers", () => {
    const locations = normalizeHostMappingLocationsForRoute([
      {
        path: "/foo/../status",
        action: "response",
        response: {
          status: 204,
          headers: {
            "X-Trace": "enabled",
            "Content-Length": "999",
            "Bad Header": "ignored",
          },
          body: "ok",
        },
      },
    ]);
    assert.equal(locations.length, 1);
    const location = locations[0]!;

    assert.equal(location.path, "/status");
    assert.equal(location.action, "response");
    assert.deepEqual(location.response.headers, { "X-Trace": "enabled" });
    assert.equal(location.response.body, "ok");
    assert.equal(location.response.status, 204);
  });

  it("normalizes invalid basic auth to disabled", () => {
    assert.deepEqual(
      normalizeHostBasicAuth({
        enabled: true,
        username: "bad:name",
        password: "secret",
      }),
      { enabled: false, username: "", password: "" },
    );
  });

  it("validates proxy mapping targets", () => {
    assert.equal(
      validateProxyMappings(
        [
          {
            path: "/app",
            target: "http://127.0.0.1:8080",
            rewrite_html: true,
            use_auth: true,
            use_root_mode: false,
            strip_path: true,
          },
        ],
        t,
      ).valid,
      true,
    );

    const invalid = validateProxyMappings(
      [
        {
          path: "/app",
          target: "127.0.0.1:8080",
          rewrite_html: true,
          use_auth: true,
          use_root_mode: false,
          strip_path: true,
        },
      ],
      t,
    );
    assert.equal(invalid.valid, false);
    assert.match(invalid.message, /proxyTargetUrlRequired/);
  });

  it("validates stream port/protocol uniqueness and targets", () => {
    assert.equal(
      validateStreamMappings(
        [
          {
            protocol: "tcp",
            listen_port: 8443,
            target: "127.0.0.1:443",
            use_auth: true,
          },
          {
            protocol: "udp",
            listen_port: 8443,
            target: "127.0.0.1:443",
            use_auth: true,
          },
        ],
        t,
      ).valid,
      true,
    );

    const duplicate = validateStreamMappings(
      [
        {
          protocol: "tcp",
          listen_port: 8443,
          target: "127.0.0.1:443",
          use_auth: true,
        },
        {
          protocol: "tcp",
          listen_port: 8443,
          target: "127.0.0.1:444",
          use_auth: true,
        },
      ],
      t,
    );
    assert.equal(duplicate.valid, false);
    assert.match(duplicate.message, /duplicatePort/);
  });
});
