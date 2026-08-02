import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";
import { fileURLToPath } from "node:url";

const readSource = (relativePath: string) =>
  readFileSync(fileURLToPath(new URL(relativePath, import.meta.url)), "utf8");

describe("LDAP login UI contract", () => {
  it("submits the selected provider and directory credentials", () => {
    const source = readSource("../src/composables/useCredentialLogin.ts");
    assert.match(source, /method === "ldap"/);
    assert.match(source, /provider_id:\s*method === "ldap"/);
    assert.match(source, /ldapProviderRequired/);
  });

  it("auto-selects a sole provider and keeps LDAP in TOTP mode", () => {
    const bootstrap = readSource("../src/composables/useLoginBootstrap.ts");
    const login = readSource("../src/views/Login.vue");
    assert.match(
      bootstrap,
      /ldapProviderId\.value = ldapProviders\.value\[0\]\?\.id/,
    );
    assert.match(login, /loginMode === 'totp'/);
    assert.match(login, /LdapLoginControls/);
  });

  it("routes invitations through captcha-protected binding", () => {
    const router = readSource("../src/router/index.ts");
    const binding = readSource("../src/views/LdapBind.vue");
    const zhCn = readSource(
      "../../../packages/i18n/src/messages/auth/zh-CN.ts",
    );
    assert.match(router, /path: ["']\/ldap\/bind["']/);
    assert.match(binding, /CaptchaAPI\.getConfig/);
    assert.match(binding, /apiClient\.post\("\/ldap\/bind"/);
    assert.match(binding, /captcha:\s*captchaSubmission\.value/);
    assert.match(binding, /useLoginCooldown/);
    assert.match(binding, /isLoginCoolingDown/);
    assert.match(binding, /redirect_uri:\s*redirectUri/);
    assert.match(zhCn, /title: "绑定 LDAP 账号"/);
    assert.match(zhCn, /ldapUsername: "LDAP 用户名"/);
    assert.doesNotMatch(zhCn, /绑定目录账号/);
  });
});
