import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";

const contract = JSON.parse(
  readFileSync(
    new URL("../../../packages/api-contract/openapi.json", import.meta.url),
    "utf8",
  ),
) as {
  paths: Record<
    string,
    Record<
      string,
      {
        "x-fn-knock-contract-source"?: string;
        requestBody?: {
          required?: boolean;
          content?: Record<string, { schema?: { $ref?: string } }>;
        };
      }
    >
  >;
};

describe("LDAP administration API contract", () => {
  it("uses typed runtime routes for every provider and binding operation", () => {
    for (const [method, path] of [
      ["get", "/api/admin/auth/ldap/catalog"],
      ["get", "/api/admin/auth/ldap/providers"],
      ["post", "/api/admin/auth/ldap/providers"],
      ["patch", "/api/admin/auth/ldap/providers/{id}"],
      ["delete", "/api/admin/auth/ldap/providers/{id}"],
      ["post", "/api/admin/auth/ldap/providers/{id}/test"],
      ["get", "/api/admin/auth/ldap/totp/{totp_id}/bindings"],
      ["delete", "/api/admin/auth/ldap/bindings/{id}"],
      ["post", "/api/admin/auth/ldap/invitations"],
    ] as const) {
      assert.equal(
        contract.paths[path]?.[method]?.["x-fn-knock-contract-source"],
        "utoipa",
        `${method.toUpperCase()} ${path}`,
      );
    }
  });

  it("keeps LDAP connection-test credentials optional", () => {
    const request = contract.paths[
      "/api/admin/auth/ldap/providers/{id}/test"
    ]?.post?.requestBody;
    assert.equal(request?.required, false);
    assert.equal(
      request?.content?.["application/json"]?.schema?.$ref,
      "#/components/schemas/LdapProviderTestBodyData",
    );
  });
});
