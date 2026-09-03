import type { Component } from "vue";
import type { SidebarNavItemId } from "@/types";

export const DEFAULT_SIDEBAR_MENU_ORDER: readonly SidebarNavItemId[] = [
  "dashboard",
  "route_mapping",
  "tunnel",
  "protocol_mapping",
  "sessions",
  "ssl_certificate",
  "ddns",
  "auth",
  "ssh_security",
  "events",
  "gateway_request_logs",
  "waf_logs",
  "web_terminal",
  "wol",
  "system_settings",
];

const SIDEBAR_MENU_ID_SET = new Set<string>(DEFAULT_SIDEBAR_MENU_ORDER);

export interface SidebarNavItem {
  id: SidebarNavItemId;
  name: string;
  path: string;
  icon: Component;
}

export const isSidebarNavItemId = (value: unknown): value is SidebarNavItemId =>
  typeof value === "string" && SIDEBAR_MENU_ID_SET.has(value);

export const normalizeSidebarMenuOrder = (
  value: readonly unknown[] | null | undefined,
): SidebarNavItemId[] => {
  if (!Array.isArray(value)) return [...DEFAULT_SIDEBAR_MENU_ORDER];

  const seen = new Set<SidebarNavItemId>();
  const normalized: SidebarNavItemId[] = [];
  for (const item of value) {
    if (!isSidebarNavItemId(item) || seen.has(item)) continue;
    seen.add(item);
    normalized.push(item);
  }
  for (const item of DEFAULT_SIDEBAR_MENU_ORDER) {
    if (!seen.has(item)) normalized.push(item);
  }
  if (!seen.has("wol")) {
    const wolIndex = normalized.indexOf("wol");
    if (wolIndex >= 0) normalized.splice(wolIndex, 1);
    const settingsIndex = normalized.indexOf("system_settings");
    normalized.splice(
      settingsIndex >= 0 ? settingsIndex : normalized.length,
      0,
      "wol",
    );
  }
  return normalized;
};

export const orderSidebarNavItems = <T extends Pick<SidebarNavItem, "id">>(
  items: readonly T[],
  order: readonly unknown[] | null | undefined,
): T[] => {
  const normalized = normalizeSidebarMenuOrder(order);
  const rank = new Map(normalized.map((id, index) => [id, index]));
  return [...items].sort(
    (left, right) =>
      (rank.get(left.id) ?? Number.MAX_SAFE_INTEGER) -
      (rank.get(right.id) ?? Number.MAX_SAFE_INTEGER),
  );
};

export const mergeVisibleSidebarMenuOrder = ({
  fullOrder,
  nextVisibleOrder,
}: {
  fullOrder: readonly unknown[] | null | undefined;
  nextVisibleOrder: readonly SidebarNavItemId[];
}): SidebarNavItemId[] => {
  const normalizedFullOrder = normalizeSidebarMenuOrder(fullOrder);
  const visibleIds = new Set(nextVisibleOrder);
  let visibleIndex = 0;

  return normalizedFullOrder.map((id) =>
    visibleIds.has(id) ? (nextVisibleOrder[visibleIndex++] ?? id) : id,
  );
};

export const hasSameSidebarMenuOrder = (
  left: readonly SidebarNavItemId[],
  right: readonly SidebarNavItemId[],
): boolean =>
  left.length === right.length &&
  left.every((item, index) => item === right[index]);
