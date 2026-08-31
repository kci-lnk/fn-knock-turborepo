import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const readSource = (path: string) =>
  readFileSync(new URL(path, import.meta.url), "utf8");

const coordinatorSource = readSource(
  "../src/views/subdomain-proxy/SubdomainMappingsTable.vue",
);
const desktopSource = readSource(
  "../src/views/subdomain-proxy/SubdomainMappingsDesktopTable.vue",
);
const mobileListSource = readSource(
  "../src/views/subdomain-proxy/SubdomainMappingsMobileList.vue",
);
const mobileRowSource = readSource(
  "../src/views/subdomain-proxy/SubdomainMappingMobileRow.vue",
);
const mobileGroupSource = readSource(
  "../src/views/subdomain-proxy/SubdomainMappingMobileGroupHeader.vue",
);

test("subdomain mappings isolate mobile cards from the desktop table at md", () => {
  assert.match(coordinatorSource, /SubdomainMappingsMobileList/u);
  assert.match(coordinatorSource, /SubdomainMappingsDesktopTable/u);
  assert.match(
    coordinatorSource,
    /useMediaQueryMatch\("\(min-width: 768px\)"\)/u,
  );
  assert.match(coordinatorSource, /v-if="!isDesktopViewport"/u);
  assert.match(desktopSource, /<Table\b/u);
  assert.match(desktopSource, /class="hidden[^\n"]*md:block"/u);
  assert.doesNotMatch(mobileListSource, /<Table\b|overflow-x-auto/u);
  assert.match(mobileListSource, /class="[^\n"]*md:hidden"/u);
  assert.match(mobileRowSource, /<article\b/u);
});

test("mobile mapping cards keep summary data and the shared interaction surfaces", () => {
  for (const component of [
    "SubdomainMappingTitleCell",
    "SubdomainMappingTargetCell",
    "HostTrafficActivity",
    "SubdomainMappingStatusIndicators",
    "SubdomainMappingRowActions",
  ]) {
    assert.match(mobileRowSource, new RegExp(`<${component}\\b`, "u"));
  }
  assert.match(mobileRowSource, /:as-cell="false"/u);
  assert.match(mobileRowSource, /\bcompact\b/u);
  assert.doesNotMatch(mobileRowSource, /<dl\b|rounded-md bg-muted\/25/u);
  assert.match(
    mobileRowSource,
    /justify-between[^"]*border-t[^"]*pt-2/u,
  );
  assert.match(mobileRowSource, /@click="actions\.copyHost\(mapping\)"/u);
  assert.match(mobileRowSource, /selectionMode && selectable/u);
  assert.match(
    mobileRowSource,
    /:aria-label="`\$\{model\.getMappingTitleForDisplay\(mapping\)\} · \$\{model\.formatHost\(mapping\.host\)\}`"/u,
  );
  assert.match(
    mobileRowSource,
    /dragSortAria[\s\S]*model\.formatHost\(mapping\.host\)/u,
  );
  assert.match(mobileRowSource, /<Globe2[\s\S]*?\bv-else\b/u);
  assert.match(mobileRowSource, /:trigger-aria-label=/u);
  assert.match(
    mobileListSource,
    /:selectable="!model\.isAuthServiceTarget\(mapping\.target\)"/u,
  );
});

test("mobile mapping groups preserve selection, collapse, and touch drag sorting", () => {
  assert.match(mobileListSource, /<VueDraggable\b/u);
  assert.match(mobileListSource, /handle="\.mapping-drag-handle"/u);
  assert.match(mobileListSource, /draggable="\.mapping-mobile-row"/u);
  assert.match(mobileListSource, /buildHostMappingDragRenderKey/u);
  assert.match(mobileListSource, /@update:model-value="updateMappings/u);
  assert.match(mobileListSource, /@end="handleSortEnd"/u);
  assert.match(mobileListSource, /setAllVisibleSelected/u);
  assert.match(mobileListSource, /setSectionSelected/u);
  assert.match(mobileListSource, /toggleSectionCollapsed/u);
  assert.match(mobileGroupSource, /selectGroupMappings/u);
  assert.match(mobileGroupSource, /actions\.openCreate\(section\.groupId\)/u);
  assert.match(mobileGroupSource, /actions\.manageGroups/u);
  assert.match(mobileGroupSource, /moreActions[\s\S]*section\.name/u);
});
