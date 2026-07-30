import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import {
  buildGatewayUnmatchedRoutePatch,
  isDefaultDomainAvailableForBehavior,
  normalizeGatewayUnmatchedRouteBehavior,
  normalizeGatewayUpstreamErrorDetail,
} from "../src/lib/gatewayUnmatchedRoute";

const readSource = (path: string) =>
  readFileSync(new URL(path, import.meta.url), "utf8");

test("gateway unmatched-route behavior normalizes legacy and invalid values", () => {
  assert.equal(normalizeGatewayUnmatchedRouteBehavior(), "error_page");
  assert.equal(normalizeGatewayUnmatchedRouteBehavior("invalid"), "error_page");
  assert.equal(
    normalizeGatewayUnmatchedRouteBehavior("reset_connection"),
    "reset_connection",
  );
});

test("gateway unmatched-route selection builds the unified-save patch", () => {
  assert.deepEqual(
    buildGatewayUnmatchedRoutePatch("reset_connection", "more"),
    {
      unmatched_route: {
        behavior: "reset_connection",
        upstream_error_detail: "more",
      },
    },
  );
});

test("gateway upstream error detail defaults to less", () => {
  assert.equal(normalizeGatewayUpstreamErrorDetail(), "less");
  assert.equal(normalizeGatewayUpstreamErrorDetail("invalid"), "less");
  assert.equal(normalizeGatewayUpstreamErrorDetail("more"), "more");
  assert.equal(
    normalizeGatewayUpstreamErrorDetail("reset_connection"),
    "reset_connection",
  );
});

test("gateway upstream error setting offers connection blocking", () => {
  const source = readSource(
    "../src/views/system-settings/GatewayUpstreamErrorSettingRow.vue",
  );

  assert.match(source, /selectDetail\('reset_connection'\)/u);
  assert.match(source, /admin\.gatewaySettings\.upstreamErrorDetailReset/u);
  assert.match(source, /grid w-full gap-1/u);
  assert.match(source, /sm:inline-flex sm:w-fit/u);
});

test("default-domain availability follows the behavior", () => {
  assert.equal(isDefaultDomainAvailableForBehavior(), true);
  assert.equal(isDefaultDomainAvailableForBehavior("error_page"), true);
  assert.equal(isDefaultDomainAvailableForBehavior("reset_connection"), false);
});

test("gateway settings keeps unmatched-route selection inline and in unified save", () => {
  const viewSource = readSource(
    "../src/views/system-settings/GatewaySettings.vue",
  );
  const controllerSource = readSource(
    "../src/views/system-settings/useGatewaySettingsController.ts",
  );
  const routerSource = readSource("../src/router/index.ts");

  assert.match(viewSource, /<GatewayUnmatchedRouteSettingRow/u);
  assert.match(viewSource, /v-model="form\.unmatched_route\.behavior"/u);
  assert.match(viewSource, /<GatewayUpstreamErrorSettingRow/u);
  assert.match(
    viewSource,
    /v-model="form\.unmatched_route\.upstream_error_detail"/u,
  );
  assert.match(
    controllerSource,
    /\.\.\.buildGatewayUnmatchedRoutePatch\(\s*form\.unmatched_route\.behavior,\s*form\.unmatched_route\.upstream_error_detail,\s*\)/u,
  );
  assert.doesNotMatch(routerSource, /gateway-unmatched-route/u);
});

test("only the default-domain indicator receives the unavailable visual state", () => {
  const source = readSource(
    "../src/views/subdomain-proxy/SubdomainMappingStatusIndicators.vue",
  );
  const scheduledOpenBlock = source.match(
    /<TooltipProvider v-else-if="availabilityState === 'scheduled_open'">[\s\S]*?<\/TooltipProvider>/u,
  )?.[0];
  const defaultDomainBlock = source.match(
    /<TooltipProvider v-if="mapping\.is_default">[\s\S]*?<\/TooltipProvider>/u,
  )?.[0];

  assert.ok(scheduledOpenBlock);
  assert.doesNotMatch(
    scheduledOpenBlock,
    /isDefaultDomainAvailable|text-amber/u,
  );
  assert.ok(defaultDomainBlock);
  assert.match(defaultDomainBlock, /isDefaultDomainAvailable/u);
  assert.match(defaultDomainBlock, /text-amber/u);
});
