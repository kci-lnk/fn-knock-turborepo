import assert from "node:assert/strict";
import test from "node:test";

import {
  ROUTE_TYPE_TRANSLATION_KEYS,
  routeTypeLabel,
} from "../src/lib/routeType";
import { readMessagePath } from "../../../packages/i18n/src/core";
import { enAdmin } from "../../../packages/i18n/src/messages/admin/en";
import { jaJPAdmin } from "../../../packages/i18n/src/messages/admin/ja-JP";
import { koKRAdmin } from "../../../packages/i18n/src/messages/admin/ko-KR";
import { zhCNAdmin } from "../../../packages/i18n/src/messages/admin/zh-CN";
import { zhHantAdmin } from "../../../packages/i18n/src/messages/admin/zh-Hant";

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
  "static_directory",
  "static_file",
  "stream_rule",
  "toolbar_asset",
  "toolbar_data",
  "unmatched_route_blocked",
  "visibility",
  "wol",
] as const;

const adminMessagesByLocale = {
  en: enAdmin,
  "ja-JP": jaJPAdmin,
  "ko-KR": koKRAdmin,
  "zh-CN": zhCNAdmin,
  "zh-Hant": zhHantAdmin,
} as const;

test("every gateway route type resolves through i18n", () => {
  assert.deepEqual(
    Object.keys(ROUTE_TYPE_TRANSLATION_KEYS).sort(),
    [...gatewayRouteTypes].sort(),
  );

  for (const [locale, adminMessages] of Object.entries(adminMessagesByLocale)) {
    for (const routeType of gatewayRouteTypes) {
      const translationKey = ROUTE_TYPE_TRANSLATION_KEYS[routeType];
      const label = routeTypeLabel(routeType, (key) => {
        const message = readMessagePath({ admin: adminMessages }, key);
        assert.equal(
          typeof message,
          "string",
          `${locale} is missing ${translationKey}`,
        );
        return message as string;
      });

      assert.ok(label.trim(), `${locale} has an empty ${translationKey}`);
      assert.notEqual(label, routeType);
      assert.notEqual(label, translationKey);
    }
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
