import assert from "node:assert/strict";
import test from "node:test";

import {
  formatSessionCredentialLoginDetail,
  getSessionCredentialDisplayName,
} from "../src/views/session-management/sessionCredentialPresentation";

const messages: Record<string, string> = {
  "admin.sessions.credentialMethods.totp": "TOTP",
  "admin.sessions.credentialMethods.passkey": "Passkey",
  "admin.sessions.credentialMethods.password": "密码",
  "admin.sessions.credentialMethods.oidc": "OIDC",
  "admin.sessions.credentialMethods.ldap": "LDAP",
};

const translate = (key: string, params?: Record<string, string>) => {
  if (key === "admin.sessions.credentialDisplay.methodWithCredential") {
    return `${params?.method}：${params?.name}`;
  }
  if (key === "admin.sessions.credentialDisplay.relation") {
    return `${params?.parent} / ${params?.child}`;
  }
  return messages[key] || key;
};

test("linked TOTP name is the primary session credential name", () => {
  const session = {
    method: "PASSKEY" as const,
    credentialName: "macOS",
    linkedTotpName: "admin mac",
  };

  assert.equal(getSessionCredentialDisplayName(session), "admin mac");
  assert.equal(
    formatSessionCredentialLoginDetail(session, translate),
    "TOTP：admin mac / Passkey：macOS",
  );
});

test("direct TOTP sessions show one method and credential pair", () => {
  const session = {
    method: "TOTP" as const,
    credentialName: "admin mac",
  };

  assert.equal(getSessionCredentialDisplayName(session), "admin mac");
  assert.equal(
    formatSessionCredentialLoginDetail(session, translate),
    "TOTP：admin mac",
  );
});

test("non-TOTP methods preserve their complete linked credential relation", () => {
  for (const [method, label, child] of [
    ["PASSWORD", "密码", "admin"],
    ["OIDC", "OIDC", "张三"],
    ["LDAP", "LDAP", "alice"],
  ] as const) {
    assert.equal(
      formatSessionCredentialLoginDetail(
        {
          method,
          credentialName: child,
          linkedTotpName: "主凭证",
        },
        translate,
      ),
      `TOTP：主凭证 / ${label}：${child}`,
    );
  }
});

test("legacy and blank session names fall back without empty labels", () => {
  assert.equal(
    getSessionCredentialDisplayName({
      method: "PASSKEY",
      credentialName: " macOS ",
      linkedTotpName: "   ",
    }),
    "macOS",
  );
  assert.equal(
    formatSessionCredentialLoginDetail(
      {
        method: "PASSKEY",
        credentialName: " macOS ",
        linkedTotpName: "   ",
      },
      translate,
    ),
    "Passkey：macOS",
  );
  assert.equal(
    getSessionCredentialDisplayName({
      method: "TOTP",
      credentialName: " ",
      linkedTotpName: "",
    }),
    "-",
  );
  assert.equal(
    formatSessionCredentialLoginDetail(
      { method: "TOTP", credentialName: "", linkedTotpName: "" },
      translate,
    ),
    "TOTP",
  );
});
