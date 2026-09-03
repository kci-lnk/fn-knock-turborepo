/// <reference types="node" />

import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const readSource = (path: string) =>
  readFileSync(new URL(path, import.meta.url), "utf8");

describe("mapping management navigation", () => {
  it("keeps subdomain first and conditionally exposes protocol mappings", () => {
    const page = readSource("../src/views/MappingManagement.vue");
    const subdomain = page.indexOf('value="subdomain"');
    const protocol = page.indexOf('value="protocol"');

    assert.ok(subdomain >= 0);
    assert.ok(protocol > subdomain);
    assert.match(page, /defaultTab: "subdomain"/u);
    assert.match(page, /isProtocolMappingVisible\(configStore\.config\)/u);
    assert.match(
      page,
      /<TabsTrigger v-if="showProtocolTab" value="protocol">/u,
    );
  });

  it("places tabs beside the title on desktop and after the description on mobile", () => {
    const page = readSource("../src/views/MappingManagement.vue");

    assert.match(
      page,
      /sm:grid-cols-\[auto_minmax\(0,1fr\)\][\s\S]*sm:col-start-2 sm:row-start-1/u,
    );
    assert.match(page, /class="order-2 text-sm text-muted-foreground/u);
    assert.match(page, /class="order-3 min-w-0 overflow-x-auto/u);
  });

  it("redirects legacy list routes while preserving their queries", () => {
    const router = readSource("../src/router/index.ts");

    assert.match(router, /path: "subdomains",\s*redirect: \(to\) =>/u);
    assert.match(router, /query: \{ \.\.\.to\.query, tab: "subdomain" \}/u);
    assert.match(router, /path: "streams",\s*redirect: \(to\) =>/u);
    assert.match(router, /query: \{ \.\.\.to\.query, tab: "protocol" \}/u);
  });

  it("uses one sidebar item and keeps legacy detail routes active", () => {
    const navigation = readSource("../src/views/layout/useLayoutNavigation.ts");

    assert.match(navigation, /name: t\("admin\.nav\.mappingManagement"\)/u);
    assert.match(navigation, /path: "\/mappings"/u);
    assert.match(navigation, /activePath\.startsWith\("\/subdomains\/"\)/u);
    assert.match(navigation, /activePath\.startsWith\("\/streams\/"\)/u);
    assert.doesNotMatch(navigation, /id: "protocol_mapping"/u);
  });

  it("guards protocol details and labels detail breadcrumbs with the unified parent", () => {
    const router = readSource("../src/router/index.ts");
    const detailPages = [
      "../src/views/PanelSync.vue",
      "../src/views/DeepMonitor.vue",
      "../src/views/subdomain-proxy/SubdomainAdvancedAuth.vue",
      "../src/views/system-settings/GatewayLocationsSettings.vue",
      "../src/views/stream-mappings/StreamBypassPolicy.vue",
    ];

    assert.match(router, /!to\.path\.startsWith\("\/streams\/"\)/u);
    assert.match(
      router,
      /to\.path\.startsWith\("\/streams\/"\) && !isSubdomainRoutingMode/u,
    );
    assert.match(router, /!isProtocolMappingVisible\(configStore\.config\)/u);
    for (const pagePath of detailPages) {
      assert.match(
        readSource(pagePath),
        /t\("admin\.nav\.mappingManagement"\)/u,
      );
    }
  });
});
