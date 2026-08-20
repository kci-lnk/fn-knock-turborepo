import assert from "node:assert/strict";
import test from "node:test";

import {
  ROUTE_TYPE_TRANSLATION_KEYS,
  routeTypeLabel,
} from "../src/lib/routeType";

const gatewayRouteTypes = [
  "auth_proxy",
  "certificate_deploy",
  "crawler_blocker",
  "default_host_redirect",
  "favicon",
  "fn_connect",
  "general_blacklist",
  "host_location",
  "host_rule",
  "host_unavailable",
  "not_found",
  "path_rule",
  "preflight",
  "protocol_misdirected",
  "select",
  "slash_redirect",
  "stream_rule",
  "toolbar_asset",
  "unmatched_route_blocked",
  "visibility",
  "wol",
] as const;

test("every gateway route type resolves through i18n", () => {
  assert.deepEqual(
    Object.keys(ROUTE_TYPE_TRANSLATION_KEYS).sort(),
    [...gatewayRouteTypes].sort(),
  );

  for (const routeType of gatewayRouteTypes) {
    const label = routeTypeLabel(routeType, (key) => `translated:${key}`);
    assert.match(label, /^translated:admin\.wafLogs\.routeTypes\./);
    assert.notEqual(label, routeType);
  }
});

test("unknown and empty route types retain safe fallbacks", () => {
  assert.equal(
    routeTypeLabel("future_route", (key) => key),
    "future_route",
  );
  assert.equal(
    routeTypeLabel(undefined, (key) => key),
    "-",
  );
});
