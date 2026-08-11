import assert from "node:assert/strict";
import { describe, it } from "node:test";
import {
  isFnKnockCgiApiPath,
  isSynologyCgiApiPath,
} from "../src/lib/api/synology-cgi";

describe("Synology CGI method override detection", () => {
  it("enables method overrides for the Synology package CGI", () => {
    assert.equal(
      isSynologyCgiApiPath(
        "/webman/3rdparty/fn-knock-synology/index.cgi/api/admin",
      ),
      true,
    );
  });

  it("does not change fnOS FPK requests", () => {
    assert.equal(
      isSynologyCgiApiPath("/cgi/ThirdParty/fn-knock/index.cgi/api/admin"),
      false,
    );
  });

  it("detects every fn-knock CGI transport for browser-origin proof", () => {
    assert.equal(
      isFnKnockCgiApiPath("/cgi/ThirdParty/fn-knock/index.cgi/api/admin"),
      true,
    );
    assert.equal(
      isFnKnockCgiApiPath("/cgi/ThirdParty/fn-knock-lite/index.cgi/api/admin"),
      true,
    );
    assert.equal(
      isFnKnockCgiApiPath(
        "/webman/3rdparty/fn-knock-synology/index.cgi/api/admin",
      ),
      true,
    );
    assert.equal(isFnKnockCgiApiPath("/api/admin"), false);
  });

  it("does not change ordinary or similarly named application paths", () => {
    assert.equal(isSynologyCgiApiPath("/api/admin"), false);
    assert.equal(
      isSynologyCgiApiPath(
        "/webman/3rdparty/my-fn-knock-synology/index.cgi/api/admin",
      ),
      false,
    );
  });
});
