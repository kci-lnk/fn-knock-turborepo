import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";
import { fileURLToPath } from "node:url";

const readSource = (relativePath: string) =>
  readFileSync(fileURLToPath(new URL(relativePath, import.meta.url)), "utf8");

describe("unified external login management", () => {
  it("keeps the legacy OIDC route and redirects to the unified page", () => {
    const router = readSource("../src/router/index.ts");
    assert.match(router, /path: "auth\/external-providers"/);
    assert.match(router, /path: "auth\/oidc-providers"/);
    assert.match(router, /redirect: "\/auth\/external-providers"/);
  });

  it("loads and renders both OIDC and LDAP provider settings", () => {
    const configApi = readSource("../src/lib/api/config.ts");
    const page = readSource("../src/views/OIDCProviderSettings.vue");
    const ldapCard = readSource(
      "../src/views/oidc-provider-settings/LDAPProviderSettingsCard.vue",
    );
    assert.match(page, /LDAPProviderSettingsCard/);
    assert.match(ldapCard, /ConfigAPI\.getLdapProviders/);
    assert.match(ldapCard, /ConfigAPI\.testLdapProvider/);
    assert.match(ldapCard, /testCredentialsDescription/);
    assert.match(ldapCard, /password:\s*testPassword\.value/);
    assert.match(
      configApi,
      /\/auth\/ldap\/providers\/\$\{encodeURIComponent\(id\)\}\/test`[\s\S]*credentials \?\? \{\}/,
    );
  });

  it("merges bindings and dispatches invitation/deletion by protocol", () => {
    const inviteDialog = readSource(
      "../src/views/passkey-settings/OidcInviteDialog.vue",
    );
    const workflow = readSource(
      "../src/views/passkey-settings/useOidcBindingWorkflow.ts",
    );
    assert.match(workflow, /ConfigAPI\.getOIDCBindings/);
    assert.match(workflow, /ConfigAPI\.getLdapBindings/);
    assert.match(workflow, /provider\.kind === "ldap"/);
    assert.match(workflow, /ConfigAPI\.createLdapInvite/);
    assert.match(workflow, /binding\?\.protocol === "ldap"/);
    assert.match(workflow, /ConfigAPI\.deleteLdapBinding/);
    assert.match(inviteDialog, /\{\{ provider\.name \}\}/);
    assert.doesNotMatch(inviteDialog, /provider\.kind\.toUpperCase/);
  });
});
