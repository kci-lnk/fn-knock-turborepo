/// <reference types="node" />

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import {
  canonicalizeRedirectTarget,
  guardAuthAutoRedirect,
  inspectAuthRedirect,
  resetAuthAutoRedirectGuard,
  type RedirectGuardStorage,
} from "../src/lib/auth-redirect-guard";

class MemoryStorage implements RedirectGuardStorage {
  private readonly values = new Map<string, string>();

  getItem(key: string) {
    return this.values.get(key) ?? null;
  }

  setItem(key: string, value: string) {
    this.values.set(key, value);
  }

  removeItem(key: string) {
    this.values.delete(key);
  }
}

const currentLoginUrl =
  "https://auth.example.com:7999/login?redirect_uri=https%3A%2F%2Fapp.example.com%3A7999%2F";

describe("auth redirect guard", () => {
  it("canonicalizes legacy hash login routes and removes cache noise", () => {
    const historyTarget =
      "https://auth.example.com:7999/login?redirect_uri=https%3A%2F%2Fapp.example.com%3A7999%2F&_ts=100";
    const hashTarget =
      "https://auth.example.com:7999/#/login?_ts=200&redirect_uri=https%3A%2F%2Fapp.example.com%3A7999%2F";

    assert.equal(
      canonicalizeRedirectTarget(historyTarget, currentLoginUrl),
      canonicalizeRedirectTarget(hashTarget, currentLoginUrl),
    );
  });

  it("keeps redirect_uri in the canonical target identity", () => {
    const first = canonicalizeRedirectTarget(
      "https://target.example.com/callback?redirect_uri=https%3A%2F%2Fone.example.com%2F&_ts=1",
      currentLoginUrl,
    );
    const second = canonicalizeRedirectTarget(
      "https://target.example.com/callback?_ts=2&redirect_uri=https%3A%2F%2Ftwo.example.com%2F",
      currentLoginUrl,
    );

    assert.notEqual(first, second);
  });

  it("blocks redirects back to the current login route across router formats", () => {
    assert.deepEqual(
      inspectAuthRedirect(
        "https://auth.example.com:7999/#/login?_ts=2",
        currentLoginUrl,
      ),
      { allowed: false, reason: "self_redirect" },
    );
    assert.deepEqual(
      inspectAuthRedirect(
        "https://auth.example.com:7999/auth/#/login",
        "https://auth.example.com:7999/auth/login?redirect_uri=%2F",
      ),
      { allowed: false, reason: "self_redirect" },
    );
  });

  it("fails closed for malformed and non-http redirect targets", () => {
    assert.deepEqual(inspectAuthRedirect("http://[", currentLoginUrl), {
      allowed: false,
      reason: "invalid_redirect",
    });
    assert.deepEqual(
      inspectAuthRedirect("javascript:alert(1)", currentLoginUrl),
      {
        allowed: false,
        reason: "invalid_redirect",
      },
    );
    for (const redirectTo of [
      "//evil.example/path",
      "///evil.example/path",
      "/\\evil.example/path",
      "\\\\evil.example/path",
      "https:\\evil.example/path",
      "/\t/evil.example/path",
      "/\n/evil.example/path",
      "/\r/evil.example/path",
    ]) {
      assert.deepEqual(inspectAuthRedirect(redirectTo, currentLoginUrl), {
        allowed: false,
        reason: "invalid_redirect",
      });
      assert.equal(
        canonicalizeRedirectTarget(redirectTo, currentLoginUrl),
        null,
      );
    }
  });

  it("preserves business hash routes when identifying redirect targets", () => {
    const first = canonicalizeRedirectTarget(
      "https://portal.example.com/app1/#/home",
      currentLoginUrl,
    );
    const second = canonicalizeRedirectTarget(
      "https://portal.example.com/app2/#/home",
      currentLoginUrl,
    );

    assert.notEqual(first, second);
    assert.equal(first, "https://portal.example.com/app1/#/home");
    assert.equal(second, "https://portal.example.com/app2/#/home");
    assert.equal(
      canonicalizeRedirectTarget(
        "https://auth.example.com:7999/#/business-home",
        currentLoginUrl,
      ),
      "https://auth.example.com:7999/#/business-home",
    );
  });

  it("blocks a repeated automatic redirect to the same target in one tab", () => {
    const storage = new MemoryStorage();
    const first = guardAuthAutoRedirect({
      redirectTo: "https://app.example.com:7999/?_ts=100",
      currentUrl: currentLoginUrl,
      storage,
      now: 1_000,
    });
    const repeated = guardAuthAutoRedirect({
      redirectTo: "https://app.example.com:7999/?_ts=200",
      currentUrl: currentLoginUrl,
      storage,
      now: 2_000,
    });

    assert.equal(first.allowed, true);
    assert.deepEqual(repeated, {
      allowed: false,
      reason: "repeat_redirect",
    });
  });

  it("allows a changed semantic target and retries after the guard window", () => {
    const storage = new MemoryStorage();
    const first = guardAuthAutoRedirect({
      redirectTo:
        "https://app.example.com/callback?redirect_uri=https%3A%2F%2Fone.example.com%2F",
      currentUrl: currentLoginUrl,
      storage,
      now: 1_000,
      windowMs: 5_000,
    });
    const changed = guardAuthAutoRedirect({
      redirectTo:
        "https://app.example.com/callback?redirect_uri=https%3A%2F%2Ftwo.example.com%2F",
      currentUrl: currentLoginUrl,
      storage,
      now: 2_000,
      windowMs: 5_000,
    });
    const afterWindow = guardAuthAutoRedirect({
      redirectTo:
        "https://app.example.com/callback?redirect_uri=https%3A%2F%2Ftwo.example.com%2F",
      currentUrl: currentLoginUrl,
      storage,
      now: 8_000,
      windowMs: 5_000,
    });

    assert.equal(first.allowed, true);
    assert.equal(changed.allowed, true);
    assert.equal(afterWindow.allowed, true);
  });

  it("allows and records one fresh redirect after an explicit-login reset", () => {
    const storage = new MemoryStorage();
    const redirectTo = "https://app.example.com/";

    assert.equal(
      guardAuthAutoRedirect({
        redirectTo,
        currentUrl: currentLoginUrl,
        storage,
        now: 1_000,
      }).allowed,
      true,
    );
    assert.deepEqual(
      guardAuthAutoRedirect({
        redirectTo,
        currentUrl: currentLoginUrl,
        storage,
        now: 2_000,
      }),
      { allowed: false, reason: "repeat_redirect" },
    );

    resetAuthAutoRedirectGuard(storage);

    assert.equal(
      guardAuthAutoRedirect({
        redirectTo,
        currentUrl: currentLoginUrl,
        storage,
        now: 3_000,
      }).allowed,
      true,
    );
    assert.deepEqual(
      guardAuthAutoRedirect({
        redirectTo,
        currentUrl: currentLoginUrl,
        storage,
        now: 4_000,
      }),
      { allowed: false, reason: "repeat_redirect" },
    );
  });

  it("blocks changing-target loops after the short-window redirect budget", () => {
    const storage = new MemoryStorage();
    for (let index = 1; index <= 3; index += 1) {
      assert.equal(
        guardAuthAutoRedirect({
          redirectTo: `https://app.example.com/?nonce=${index}`,
          currentUrl: currentLoginUrl,
          storage,
          now: index * 1_000,
        }).allowed,
        true,
      );
    }

    assert.deepEqual(
      guardAuthAutoRedirect({
        redirectTo: "https://app.example.com/?nonce=4",
        currentUrl: currentLoginUrl,
        storage,
        now: 4_000,
      }),
      { allowed: false, reason: "repeat_redirect" },
    );
  });
});
