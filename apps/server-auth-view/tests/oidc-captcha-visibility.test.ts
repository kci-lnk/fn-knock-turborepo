import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";

const loginViewSource = readFileSync(
  new URL("../src/views/Login.vue", import.meta.url),
  "utf8",
);

describe("OIDC login visibility", () => {
  it("keeps configured OIDC providers visible after captcha verification", () => {
    const oidcComponent = loginViewSource.match(
      /<OidcProviderButtons[\s\S]*?\/>/u,
    )?.[0];

    assert.ok(oidcComponent, "Login view must render OidcProviderButtons");
    assert.match(oidcComponent, /v-if="oidcProviders\.length > 0"/u);
    assert.doesNotMatch(oidcComponent, /isCaptchaVerified/u);
  });
});
