import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { isSynologyCgiApiPath } from "../src/lib/api/synology-cgi";

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
