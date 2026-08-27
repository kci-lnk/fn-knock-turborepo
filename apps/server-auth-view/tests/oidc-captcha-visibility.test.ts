import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";

const loginViewSource = readFileSync(
  new URL("../src/views/Login.vue", import.meta.url),
  "utf8",
);

describe("OIDC login visibility", () => {
  it("shows OIDC before verification and retains it afterward only for Turnstile", () => {
    const oidcComponent = loginViewSource.match(
      /<OidcProviderButtons[\s\S]*?\/>/u,
    )?.[0];

    assert.ok(oidcComponent, "Login view must render OidcProviderButtons");
    const visibilityCondition = oidcComponent
      .match(/v-if="([\s\S]*?)"/u)?.[1]
      ?.replace(/\s+/gu, " ")
      .trim();
    const dividerCondition = oidcComponent
      .match(/:show-divider="([\s\S]*?)"/u)?.[1]
      ?.replace(/\s+/gu, " ")
      .trim();

    assert.equal(
      visibilityCondition,
      "oidcProviders.length > 0 && (!isCaptchaVerified || activeCaptchaProvider === 'turnstile')",
    );
    assert.equal(
      dividerCondition,
      "!isCaptchaVerified && isPasskeySupported && isPasskeyAvailable",
    );
  });
});
