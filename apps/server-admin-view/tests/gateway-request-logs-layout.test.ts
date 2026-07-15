import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const tableSource = readFileSync(
  new URL(
    "../src/views/gateway-request-logs/GatewayRequestLogsTable.vue",
    import.meta.url,
  ),
  "utf8",
);

test("request log route column cannot expand beyond its fixed maximum", () => {
  const widthClass = "w-[220px] min-w-[160px] max-w-[220px]";
  assert.equal(tableSource.split(widthClass).length - 1, 2);
  assert.match(tableSource, /class="w-full max-w-\[204px\] overflow-hidden"/);
  assert.match(
    tableSource,
    /class="truncate text-sm text-foreground"\s+:title="routeTypeLabel\(entry\.route_type\)"/,
  );
  assert.match(
    tableSource,
    /class="truncate text-\[11px\] text-muted-foreground"\s+:title="entry\.route_key \|\| '-'"/,
  );
});
