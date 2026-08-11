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
          content?: Record<string, { schema?: { $ref?: string } }>;
        };
      }
    >
  >;
};

describe("OIDC administration API contract", () => {
  it("uses typed runtime routes for every provider and binding operation", () => {
    for (const [method, path] of [
      ["get", "/api/admin/auth/oidc/catalog"],
      ["get", "/api/admin/auth/oidc/providers"],
      ["post", "/api/admin/auth/oidc/providers"],
      ["patch", "/api/admin/auth/oidc/providers/{id}"],
      ["delete", "/api/admin/auth/oidc/providers/{id}"],
      ["post", "/api/admin/auth/oidc/providers/{id}/test"],
      ["get", "/api/admin/auth/oidc/totp/{totp_id}/bindings"],
      ["delete", "/api/admin/auth/oidc/bindings/{id}"],
      ["post", "/api/admin/auth/oidc/invitations"],
    ] as const) {
      assert.equal(
        contract.paths[path]?.[method]?.["x-fn-knock-contract-source"],
        "utoipa",
        `${method.toUpperCase()} ${path}`,
      );
    }
  });

  it("preserves the distinct write schemas for providers and invitations", () => {
    const schemaFor = (path: string, method: string) =>
      contract.paths[path]?.[method]?.requestBody?.content?.[
        "application/json"
      ]?.schema?.$ref;

    assert.equal(
      schemaFor("/api/admin/auth/oidc/providers", "post"),
      "#/components/schemas/OidcProviderCreateData",
    );
    assert.equal(
      schemaFor("/api/admin/auth/oidc/providers/{id}", "patch"),
      "#/components/schemas/OidcProviderUpdateData",
    );
    assert.equal(
      schemaFor("/api/admin/auth/oidc/invitations", "post"),
      "#/components/schemas/ExternalAuthInvitationBodyData",
    );
  });
});
