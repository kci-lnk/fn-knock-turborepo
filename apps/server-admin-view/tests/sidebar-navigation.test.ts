/// <reference types="node" />

import { describe, it } from "node:test";
import assert from "node:assert/strict";

import type { SidebarNavItemId } from "../src/types";
import {
  DEFAULT_SIDEBAR_MENU_ORDER,
  hasSameSidebarMenuOrder,
  mergeVisibleSidebarMenuOrder,
  normalizeSidebarMenuOrder,
  orderSidebarNavItems,
} from "../src/views/layout/sidebarNavigation";

const item = (id: SidebarNavItemId) => ({ id });

describe("sidebar navigation order", () => {
  it("keeps deep monitoring under subdomain mappings instead of the sidebar", () => {
    assert.equal(
      (DEFAULT_SIDEBAR_MENU_ORDER as readonly string[]).includes(
        "deep_monitor",
      ),
      false,
    );
  });

  it("keeps the existing default order when no preference is saved", () => {
    assert.deepEqual(
      normalizeSidebarMenuOrder(undefined),
      DEFAULT_SIDEBAR_MENU_ORDER,
    );

    const reverseModeVisible: SidebarNavItemId[] = [
      "dashboard",
      "route_mapping",
      "tunnel",
      "sessions",
      "ip_whitelist",
      "ssl_certificate",
      "ddns",
      "auth",
      "events",
      "wol",
      "system_settings",
    ];
    assert.deepEqual(
      orderSidebarNavItems(
        reverseModeVisible.map(item).reverse(),
        undefined,
      ).map(({ id }) => id),
      reverseModeVisible,
    );

    const directModeVisible: SidebarNavItemId[] = [
      "ip_whitelist",
      "ssl_certificate",
      "ddns",
      "auth",
      "events",
      "wol",
      "system_settings",
    ];
    assert.deepEqual(
      orderSidebarNavItems(
        directModeVisible.map(item).reverse(),
        undefined,
      ).map(({ id }) => id),
      directModeVisible,
    );
  });

  it("applies a custom order to the current visible items", () => {
    const customOrder = normalizeSidebarMenuOrder([
      "system_settings",
      "events",
      "dashboard",
    ]);
    const visible = ["dashboard", "events", "system_settings"] as const;

    assert.deepEqual(
      orderSidebarNavItems(visible.map(item), customOrder).map(({ id }) => id),
      ["system_settings", "events", "dashboard"],
    );
  });

  it("inserts a newly available WOL entry immediately above system settings", () => {
    const legacyOrder = DEFAULT_SIDEBAR_MENU_ORDER.filter((id) => id !== "wol");
    const normalized = normalizeSidebarMenuOrder(legacyOrder);
    const settingsIndex = normalized.indexOf("system_settings");

    assert.equal(normalized[settingsIndex - 1], "wol");
  });

  it("keeps hidden menu slots while merging a visible drag order", () => {
    const visibleOrder = DEFAULT_SIDEBAR_MENU_ORDER.filter(
      (id) => id !== "tunnel" && id !== "protocol_mapping",
    );
    const nextVisibleOrder = [
      "sessions",
      ...visibleOrder.filter((id) => id !== "sessions"),
    ];
    const merged = mergeVisibleSidebarMenuOrder({
      fullOrder: DEFAULT_SIDEBAR_MENU_ORDER,
      nextVisibleOrder,
    });

    assert.equal(merged[2], "tunnel");
    assert.equal(merged[3], "protocol_mapping");
    assert.equal(merged[0], "sessions");
    assert.deepEqual(
      orderSidebarNavItems(DEFAULT_SIDEBAR_MENU_ORDER.map(item), merged).map(
        ({ id }) => id,
      ),
      merged,
    );
  });

  it("filters duplicates and unknown values, then fills missing ids", () => {
    const normalized = normalizeSidebarMenuOrder([
      "events",
      "events",
      "unknown",
      42,
      "dashboard",
    ]);

    assert.deepEqual(normalized.slice(0, 2), ["events", "dashboard"]);
    assert.equal(normalized.length, DEFAULT_SIDEBAR_MENU_ORDER.length);
    assert.equal(normalized.filter((id) => id === "events").length, 1);
    assert.equal(
      hasSameSidebarMenuOrder(
        normalizeSidebarMenuOrder(DEFAULT_SIDEBAR_MENU_ORDER),
        [...DEFAULT_SIDEBAR_MENU_ORDER],
      ),
      true,
    );
  });
});
