/// <reference types="node" />

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { resolveAppRelativePathFromUrl } from "../src/lib/api/client";

describe("admin API base path", () => {
  it("uses browser URL resolution for an index document", () => {
    assert.equal(
      resolveAppRelativePathFromUrl(
        "./api/admin",
        "https://admin.example.com/index.html#/wol",
      ),
      "/api/admin",
    );
  });

  it("preserves a directory-style CGI application prefix", () => {
    assert.equal(
      resolveAppRelativePathFromUrl(
        "./api/admin",
        "https://nas.example.com/cgi/ThirdParty/fn-knock/index.cgi/?launch=1#/wol",
      ),
      "/cgi/ThirdParty/fn-knock/index.cgi/api/admin",
    );
  });
});
