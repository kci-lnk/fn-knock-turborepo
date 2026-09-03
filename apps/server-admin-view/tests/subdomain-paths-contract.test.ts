/// <reference types="node" />

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";
import { toHostMappingUpdatePayload } from "../src/lib/api/host-mapping-payload";
import { createDefaultMapping } from "../src/views/subdomain-proxy/model";
import { createDefaultLocation } from "../src/views/system-settings/gateway-locations/gatewayLocationModel";

const source = (path: string) =>
  readFileSync(new URL(path, import.meta.url), "utf8");

describe("subdomain path rules", () => {
  it("uses only the host-scoped subdomain route", () => {
    const router = source("../src/router/index.ts");
    const navigation = source(
      "../src/views/subdomain-proxy/useSubdomainNavigation.ts",
    );

    assert.match(router, /path: "subdomains\/:host\/paths"/u);
    assert.doesNotMatch(router, /path: "system\/gateway-locations"/u);
    assert.match(navigation, /openSubdomainPage\(host, "paths"\)/u);
  });

  it("places the breadcrumb under subdomain mappings", () => {
    const page = source(
      "../src/views/system-settings/GatewayLocationsSettings.vue",
    );

    assert.match(page, /href="#\/subdomains"/u);
    assert.doesNotMatch(page, /href="#\/system\?tab=gateway"/u);
  });

  it("removes the path editor entry from gateway settings", () => {
    const settings = source("../src/views/system-settings/GatewaySettings.vue");
    const controller = source(
      "../src/views/system-settings/useGatewaySettingsController.ts",
    );

    assert.doesNotMatch(settings, /gatewaySettings\.locations/u);
    assert.doesNotMatch(controller, /openLocationsEditor/u);
  });

  it("exposes inherited and public authentication in the editor and table", () => {
    const accessSection = source(
      "../src/views/system-settings/gateway-locations/GatewayLocationAccessSection.vue",
    );
    const table = source(
      "../src/views/system-settings/gateway-locations/GatewayLocationRulesTable.vue",
    );

    assert.match(accessSection, /v-model="form\.auth_mode"/u);
    assert.match(accessSection, /value="inherit"/u);
    assert.match(accessSection, /value="public"/u);
    assert.match(table, /formatAuthMode\(location\)/u);
  });

  it("presents match mode before a path label that follows its semantics", () => {
    const matchSection = source(
      "../src/views/system-settings/gateway-locations/GatewayLocationMatchSection.vue",
    );

    assert.ok(
      matchSection.indexOf('id="location-match"') <
        matchSection.indexOf('id="location-path"'),
    );
    assert.match(matchSection, /form\.match === "exact"/u);
    assert.match(matchSection, /gatewayLocationsSettings\.exactPath/u);
    assert.match(matchSection, /gatewayLocationsSettings\.pathPrefix/u);
  });

  it("keeps the HTML rewrite explanation beside the proxy control", () => {
    const proxyFields = source(
      "../src/views/system-settings/gateway-locations/GatewayLocationProxyFields.vue",
    );

    assert.match(proxyFields, /CircleAlert/u);
    assert.match(proxyFields, /TooltipTrigger/u);
    assert.match(proxyFields, /rewriteHtmlPathHelpAria/u);
    assert.match(proxyFields, /rewriteHtmlPathHelp/u);
    assert.match(proxyFields, /v-if="!isWebSocketTarget"/u);
  });

  it("keeps the long form scrollable between a fixed header and footer", () => {
    const dialog = source(
      "../src/views/system-settings/gateway-locations/GatewayLocationRuleDialog.vue",
    );
    const matchSection = source(
      "../src/views/system-settings/gateway-locations/GatewayLocationMatchSection.vue",
    );

    assert.match(dialog, /flex max-h-\[calc\(100dvh-1rem\)\]/u);
    assert.match(dialog, /flex-1 space-y-4 overflow-y-auto/u);
    assert.ok(
      dialog.indexOf("overflow-y-auto") < dialog.indexOf("<DialogFooter"),
    );
    assert.match(matchSection, /sm:grid-cols-\[13rem_minmax\(0,1fr\)\]/u);
  });

  it("serializes public authentication and normalizes legacy rules", () => {
    const mapping = createDefaultMapping();
    const legacy = createDefaultLocation();
    delete (legacy as Partial<typeof legacy>).auth_mode;
    mapping.locations = [
      legacy,
      { ...createDefaultLocation(), path: "/public", auth_mode: "public" },
    ];

    const locations = toHostMappingUpdatePayload(mapping).locations;
    assert.equal(locations[0]?.auth_mode, "inherit");
    assert.equal(locations[1]?.auth_mode, "public");
  });
});
