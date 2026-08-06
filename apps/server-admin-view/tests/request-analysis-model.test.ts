import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import {
  REQUEST_ANALYTICS_RANGE_OPTIONS,
  analyticsDimensionLabel,
  analyticsRegionLabel,
  analyticsRangeDays,
  analyticsTimestampOffsetMinutes,
  formatAnalyticsBytes,
  formatAnalyticsDuration,
  isValidAnalyticsRange,
  mapAnalyticsBuckets,
  resolveAnalyticsRange,
  subtractCalendarDays,
  todayDateString,
} from "../src/views/request-analysis/model";

test("request analytics presets use inclusive calendar ranges", () => {
  const today = todayDateString();
  const lastSeven = resolveAnalyticsRange("7d");
  const lastThirty = resolveAnalyticsRange("30d");
  assert.equal(lastSeven.to, today);
  assert.equal(lastSeven.from, subtractCalendarDays(today, 6));
  assert.equal(analyticsRangeDays(lastSeven.from, lastSeven.to), 7);
  assert.equal(analyticsRangeDays(lastThirty.from, lastThirty.to), 30);
  assert.deepEqual(resolveAnalyticsRange("7d", "2026-01-02"), {
    from: "2025-12-27",
    to: "2026-01-02",
  });
});

test("request analytics only exposes fixed range presets", () => {
  assert.deepEqual(
    REQUEST_ANALYTICS_RANGE_OPTIONS.map((option) => option.key),
    ["today", "7d", "30d"],
  );
});

test("request analytics rejects reversed, future, and overlong ranges", () => {
  const today = todayDateString();
  const tomorrow = subtractCalendarDays(today, -1);
  assert.equal(isValidAnalyticsRange(today, today), true);
  assert.equal(
    isValidAnalyticsRange(subtractCalendarDays(today, 29), today),
    true,
  );
  assert.equal(
    isValidAnalyticsRange(subtractCalendarDays(today, 30), today),
    false,
  );
  assert.equal(
    isValidAnalyticsRange(today, subtractCalendarDays(today, 1)),
    false,
  );
  assert.equal(isValidAnalyticsRange(today, tomorrow), false);
  assert.equal(isValidAnalyticsRange(tomorrow, tomorrow, tomorrow), true);
  assert.equal(isValidAnalyticsRange("2026-02-31", today), false);
  assert.equal(analyticsRangeDays("", today), 0);
});

test("request analytics formatters and bucket labels remain presentation-only", () => {
  assert.equal(formatAnalyticsDuration(1500, "en"), "1.5 s");
  assert.equal(formatAnalyticsBytes(1024, "en"), "1 KB");
  assert.deepEqual(
    mapAnalyticsBuckets([{ key: "GET", count: 2, share: 0.5 }], (key) =>
      key.toLowerCase(),
    ),
    [{ key: "GET", label: "get", count: 2, share: 0.5 }],
  );
});

test("request analytics preserves gateway offsets for chart labels", () => {
  assert.equal(
    analyticsTimestampOffsetMinutes("2026-08-06T00:00:00+08:00"),
    480,
  );
  assert.equal(
    analyticsTimestampOffsetMinutes("2026-01-06T00:00:00-05:30"),
    -330,
  );
  assert.equal(analyticsTimestampOffsetMinutes("2026-08-06T00:00:00Z"), 0);
  assert.equal(analyticsTimestampOffsetMinutes("not-a-time"), 0);
});

test("request analytics localizes security and region dimension values", () => {
  const translated = (key: string) => `translated:${key}`;
  assert.equal(
    analyticsDimensionLabel("waf_blocked", translated),
    "translated:admin.gatewayRequestLogs.authDecisions.wafBlocked",
  );
  assert.equal(
    analyticsDimensionLabel("robots_txt_served", translated),
    "translated:admin.gatewayRequestLogs.authDecisions.robotsTxtServed",
  );
  assert.equal(
    analyticsRegionLabel(
      {
        key: "CN|广东省|深圳市",
        count: 2,
        share: 0.5,
        country_code: "CN",
        province: "广东省",
        city: "深圳市",
      },
      "zh-CN",
      translated,
    ),
    "中国 · 广东省 · 深圳市",
  );
});

test("legacy request log route redirects to the logs tab", () => {
  const routerSource = readFileSync(
    new URL("../src/router/index.ts", import.meta.url),
    "utf8",
  );
  assert.match(routerSource, /path: "request-analysis"/u);
  assert.match(routerSource, /path: "request-logs"[\s\S]*tab: "logs"/u);
});

test("request analytics tabs expose real panels while retaining component state", () => {
  const pageSource = readFileSync(
    new URL("../src/views/RequestAnalysis.vue", import.meta.url),
    "utf8",
  );
  const cardSource = readFileSync(
    new URL(
      "../src/views/request-analysis/AnalyticsBreakdownCard.vue",
      import.meta.url,
    ),
    "utf8",
  );
  const analyticsSource = readFileSync(
    new URL(
      "../src/views/request-analysis/RequestAnalyticsTab.vue",
      import.meta.url,
    ),
    "utf8",
  );
  const logsSource = readFileSync(
    new URL("../src/views/GatewayRequestLogs.vue", import.meta.url),
    "utf8",
  );
  const logsTableSource = readFileSync(
    new URL(
      "../src/views/gateway-request-logs/GatewayRequestLogsTable.vue",
      import.meta.url,
    ),
    "utf8",
  );

  assert.match(pageSource, /defaultTab: "logs"/u);
  assert.match(pageSource, /allowedTabs: \["logs", "analytics"\]/u);
  assert.match(
    pageSource,
    /<TabsTrigger value="logs">[\s\S]*<TabsTrigger value="analytics">/u,
  );
  assert.match(
    pageSource,
    /<TabsContent value="logs"[\s\S]*<TabsContent value="analytics"/u,
  );
  assert.match(pageSource, /<KeepAlive>/u);
  assert.doesNotMatch(pageSource, /DocsLinkButton/u);
  assert.match(pageSource, /request-analysis-analytics-actions/u);
  assert.match(pageSource, /request-analysis-logs-actions/u);
  assert.match(pageSource, /v-show="currentTab === 'analytics'"/u);
  assert.match(pageSource, /v-show="currentTab === 'logs'"/u);
  assert.match(cardSource, /<TabsContent/u);
  assert.match(cardSource, /analytics-breakdown-tab/u);
  assert.match(
    cardSource,
    /\{\{ formatAnalyticsPercent\(item\.share, String\(locale\)\) \}\}/u,
  );
  assert.match(analyticsSource, /<TabsList/u);
  assert.match(analyticsSource, /<TabsTrigger/u);
  assert.match(analyticsSource, /grid-cols-3/u);
  assert.doesNotMatch(analyticsSource, /rangeKey === 'custom'/u);
  assert.match(analyticsSource, /refreshGeo/u);
  assert.match(
    analyticsSource,
    /<Teleport defer to="#request-analysis-analytics-actions">/u,
  );
  assert.match(logsSource, /#request-analysis-logs-actions/u);
  assert.doesNotMatch(analyticsSource, /rangeText/u);
  assert.doesNotMatch(analyticsSource, /requestAnalysis\.rangeTimezone/u);
  assert.match(analyticsSource, /analyticsRegionLabel/u);
  assert.doesNotMatch(analyticsSource, /metric\.hint/u);
  assert.doesNotMatch(analyticsSource, /metric\.tone/u);
  assert.doesNotMatch(analyticsSource, /averageDuration/u);
  assert.match(cardSource, /bg-foreground\/\[0\.08\]/u);
  assert.match(pageSource, /grid-cols-2 sm:flex sm:w-auto/u);
  assert.match(analyticsSource, /grid-cols-\[minmax\(0,1fr\)_auto\]/u);
  assert.match(analyticsSource, /col-span-2 lg:col-span-1/u);
  assert.match(logsSource, /grid-cols-2 items-center/u);
  assert.match(logsTableSource, /class="divide-y md:hidden"/u);
  assert.match(logsTableSource, /hidden min-w-\[1060px\] md:table/u);
});

test("cached request-log pagination stops floating while its tab is inactive", () => {
  const dockSource = readFileSync(
    new URL(
      "../../../packages/admin-shared/src/components/common/FloatingActionDock.vue",
      import.meta.url,
    ),
    "utf8",
  );

  assert.match(dockSource, /onActivated/u);
  assert.match(dockSource, /onDeactivated/u);
  assert.match(
    dockSource,
    /isLifecycleActive\.value\s*&&\s*\(props\.active/u,
  );
  assert.match(
    dockSource,
    /onDeactivated\(\(\) => \{[\s\S]*isLifecycleActive\.value = false;[\s\S]*disconnectIntersectionObserver\(\);[\s\S]*hasFloatingFocus\.value = false;/u,
  );
});
