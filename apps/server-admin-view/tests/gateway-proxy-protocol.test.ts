import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";

const read = (path: string) =>
  readFileSync(new URL(path, import.meta.url), "utf8");

describe("gateway PROXY Protocol settings", () => {
  it("keeps both operations and managed runtime fields in the typed contract", () => {
    const openapi = JSON.parse(
      read("../../../packages/api-contract/openapi.json"),
    );
    const path = openapi.paths["/api/admin/config/gateway/proxy-protocol"];
    assert.ok(path.get);
    assert.ok(path.post);
    const schema = openapi.components.schemas.GatewayProxyProtocolData;
    assert.deepEqual(schema.required.sort(), [
      "effective_enabled",
      "enabled",
      "managed_frp_enabled",
      "trusted_sources",
    ]);
  });

  it("uses the dedicated API from both the summary and editor flows", () => {
    const api = read("../src/lib/api/config-proxy-api.ts");
    const controller = read(
      "../src/views/system-settings/useGatewaySettingsController.ts",
    );
    const editor = read(
      "../src/views/system-settings/GatewayProxyProtocolSettings.vue",
    );
    assert.match(api, /getGatewayProxyProtocol/u);
    assert.match(api, /updateGatewayProxyProtocol/u);
    assert.match(controller, /proxyProtocolSummary/u);
    assert.match(editor, /managed_frp_enabled/u);
    assert.match(editor, /0\.0\.0\.0\/0/u);
    assert.match(editor, /::\/0/u);
  });
});
