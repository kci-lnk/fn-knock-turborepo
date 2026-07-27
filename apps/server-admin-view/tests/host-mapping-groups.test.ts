import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { computed } from "vue";
import type { HostMapping, HostMappingGroup } from "../src/types";
import { createDefaultMapping } from "../src/views/subdomain-proxy/model";
import {
  applyHostMappingGroupSections,
  buildHostMappingDragRenderKey,
  buildHostMappingGroupSections,
  createHostMappingGroupId,
  isHostMappingGroupNameLengthValid,
  moveHostMappingsToGroup,
  normalizeHostMappingGroupNameKey,
  resolveHostMappingGroupSaveFeedback,
} from "../src/views/subdomain-proxy/host-mapping-groups";
import { useSubdomainMappingGroups } from "../src/views/subdomain-proxy/useSubdomainMappingGroups";

const groups: HostMappingGroup[] = [
  { id: "11111111-1111-4111-8111-111111111111", name: "Media" },
  { id: "22222222-2222-4222-8222-222222222222", name: "Tools" },
];

const mapping = (host: string, groupId: string | null): HostMapping => ({
  ...createDefaultMapping(),
  host,
  target: `http://${host}`,
  group_id: groupId,
});

test("creates an RFC 4122 UUID when randomUUID is unavailable", () => {
  const id = createHostMappingGroupId({
    getRandomValues: (bytes) => {
      bytes.fill(0);
      return bytes;
    },
  });

  assert.equal(id, "00000000-0000-4000-8000-000000000000");
});

test("validates group names by Unicode code points without locale-dependent casing", () => {
  assert.equal(isHostMappingGroupNameLengthValid("😀".repeat(40)), true);
  assert.equal(isHostMappingGroupNameLengthValid("😀".repeat(41)), false);
  assert.equal(isHostMappingGroupNameLengthValid("   "), false);
  assert.equal(normalizeHostMappingGroupNameKey("  MEDIA  "), "media");
});

test("projects mappings in group order with ungrouped last", () => {
  const sections = buildHostMappingGroupSections(
    [
      mapping("tool.example.test", groups[1].id),
      mapping("loose.example.test", null),
      mapping("media.example.test", groups[0].id),
    ],
    groups,
    "Ungrouped",
  );

  assert.deepEqual(
    sections.map((section) => [
      section.name,
      section.mappings.map((item) => item.host),
    ]),
    [
      ["Media", ["media.example.test"]],
      ["Tools", ["tool.example.test"]],
      ["Ungrouped", ["loose.example.test"]],
    ],
  );
});

test("keeps the flat projection when no groups exist", () => {
  const mappings = [mapping("one.example.test", null)];
  const sections = buildHostMappingGroupSections(mappings, [], "Ungrouped");
  assert.equal(sections.length, 1);
  assert.equal(sections[0]?.name, "");
  assert.deepEqual(sections[0]?.mappings, mappings);
});

test("changes the draggable render identity when membership or order changes", () => {
  const first = mapping("one.example.test", groups[0].id);
  const second = mapping("two.example.test", groups[0].id);

  assert.equal(
    buildHostMappingDragRenderKey([first, second]),
    '["one.example.test","two.example.test"]',
  );
  assert.notEqual(
    buildHostMappingDragRenderKey([first, second]),
    buildHostMappingDragRenderKey([second, first]),
  );
  assert.notEqual(
    buildHostMappingDragRenderKey([first, second]),
    buildHostMappingDragRenderKey([first]),
  );
});

test("applies a cross-group drag while preserving the auth mapping slot", () => {
  const auth = {
    ...mapping("auth.example.test", null),
    target: "http://127.0.0.1:7997",
    service_role: "auth" as const,
  };
  const media = mapping("media.example.test", groups[0].id);
  const tool = mapping("tool.example.test", groups[1].id);
  const sections = buildHostMappingGroupSections(
    [media, tool],
    groups,
    "Ungrouped",
  );
  sections[0]!.mappings = [tool, media];
  sections[1]!.mappings = [];

  const next = applyHostMappingGroupSections(
    [auth, media, tool],
    sections,
    (target) => target.endsWith(":7997"),
  );
  assert.equal(next[0]?.host, auth.host);
  assert.equal(next[1]?.host, tool.host);
  assert.equal(next[1]?.group_id, groups[0].id);
  assert.equal(next[2]?.host, media.host);
});

test("batch move only updates selected mappings", () => {
  const first = mapping("one.example.test", null);
  const second = mapping("two.example.test", groups[0].id);
  const next = moveHostMappingsToGroup(
    [first, second],
    new Set([first.host]),
    groups[1].id,
  );
  assert.equal(next[0]?.group_id, groups[1].id);
  assert.equal(next[1]?.group_id, groups[0].id);
});

test("selects a precise success message for a single group change", () => {
  assert.equal(
    resolveHostMappingGroupSaveFeedback(groups, [
      ...groups,
      { id: "33333333-3333-4333-8333-333333333333", name: "New" },
    ]),
    "created",
  );
  assert.equal(
    resolveHostMappingGroupSaveFeedback(groups, [
      { ...groups[0]!, name: "Streaming" },
      groups[1]!,
    ]),
    "renamed",
  );
  assert.equal(
    resolveHostMappingGroupSaveFeedback(groups, [groups[0]!]),
    "deleted",
  );
  assert.equal(
    resolveHostMappingGroupSaveFeedback(groups, [groups[1]!, groups[0]!]),
    "reordered",
  );
});

test("uses the generic saved message when group changes are combined", () => {
  assert.equal(
    resolveHostMappingGroupSaveFeedback(groups, [
      { ...groups[1]!, name: "Utilities" },
      groups[0]!,
    ]),
    "saved",
  );
  assert.equal(resolveHostMappingGroupSaveFeedback(groups, groups), "saved");
});

test("reports a failed group catalog save without closing its editor", async () => {
  let completed: boolean | undefined;
  const originalMappings = [mapping("media.example.test", groups[0].id)];
  const controller = useSubdomainMappingGroups({
    allMappings: computed(() => originalMappings),
    groupedView: computed(() => true),
    groups: computed(() => groups),
    isAuthServiceTarget: () => false,
    runSaveMappings: async (action) => {
      try {
        return await action();
      } catch {
        return undefined;
      }
    },
    saveCatalog: async () => {
      throw new Error("conflict");
    },
    translate: (key) => key,
  });

  const saved = await controller.saveMappingGroups(groups, (value) => {
    completed = value;
  });

  assert.equal(saved, false);
  assert.equal(completed, false);
  assert.equal(originalMappings[0]?.group_id, groups[0].id);
});

test("provides success toasts for every persisted group operation", () => {
  const controller = readFileSync(
    new URL(
      "../src/views/subdomain-proxy/useSubdomainMappingGroups.ts",
      import.meta.url,
    ),
    "utf8",
  );

  for (const key of [
    "groupCreated",
    "groupRenamed",
    "groupDeleted",
    "groupOrderUpdated",
    "groupsSaved",
    "mappingsMoved",
    "groupedMappingOrderUpdated",
    "groupedViewEnabled",
    "groupedViewDisabled",
  ]) {
    assert.match(
      controller,
      new RegExp(`admin\\.subdomainProxy\\.${key}`, "u"),
    );
  }
  assert.equal((controller.match(/toast\.success\(/gu) ?? []).length, 4);
});

test("keeps mobile actions on one row and group headings visible while scrolling", () => {
  const card = readFileSync(
    new URL(
      "../src/views/subdomain-proxy/SubdomainMappingsCard.vue",
      import.meta.url,
    ),
    "utf8",
  );
  const styles = readFileSync(
    new URL("../src/assets/index.css", import.meta.url),
    "utf8",
  );

  assert.match(
    card,
    /grid w-full grid-cols-\[auto_auto_minmax\(0,1fr\)\] items-center gap-2 sm:flex/u,
  );
  assert.match(
    card,
    /class="hidden sm:inline-flex"[\s\S]*?admin\.subdomainProxy\.manageGroups/u,
  );
  assert.match(
    card,
    /v-if="isGroupedViewActive"[\s\S]*?data-testid="mobile-manage-groups-menu-item"[\s\S]*?class="sm:hidden"[\s\S]*?@select="isGroupManagerOpen = true"[\s\S]*?admin\.subdomainProxy\.manageGroups/u,
  );
  assert.match(card, /class="col-span-3 w-full sm:w-auto"/u);
  assert.match(card, /mapping-group-header-row/u);
  assert.match(card, /class="mapping-group-header-layout"/u);
  assert.match(card, /mapping-group-header-sticky/u);
  assert.match(card, /class="mapping-group-header-actions"/u);
  assert.match(
    card,
    /class="mapping-group-header-sticky[\s\S]*?<\/div>\s*<div class="mapping-group-header-actions">[\s\S]*?<MoreHorizontal/u,
  );
  assert.match(
    styles,
    /\.mapping-table-scroll \.mapping-group-header-sticky \{[\s\S]*?position: sticky;[\s\S]*?left: 0;[\s\S]*?width: min\(24rem, calc\(100vw - 3rem\)\);/u,
  );
  assert.match(
    styles,
    /\.mapping-table-scroll \.mapping-group-header-row \{[\s\S]*?--mapping-group-header-background:[\s\S]*?background-color: var\(--mapping-group-header-background\);/u,
  );
  assert.match(
    styles,
    /\.mapping-table-scroll \.mapping-group-header-sticky \{[\s\S]*?background-color: var\(--mapping-group-header-background\);/u,
  );
  assert.match(
    styles,
    /\.mapping-table-scroll \.mapping-group-header-actions \{[\s\S]*?width: 8rem;[\s\S]*?margin-left: auto;[\s\S]*?justify-content: flex-end;/u,
  );
});

test("animates grouped rows and the disclosure chevron", () => {
  const card = readFileSync(
    new URL(
      "../src/views/subdomain-proxy/SubdomainMappingsCard.vue",
      import.meta.url,
    ),
    "utf8",
  );
  const groupRows = readFileSync(
    new URL(
      "../src/views/subdomain-proxy/SubdomainMappingGroupRows.vue",
      import.meta.url,
    ),
    "utf8",
  );
  const styles = readFileSync(
    new URL("../src/assets/index.css", import.meta.url),
    "utf8",
  );

  assert.match(groupRows, /isBodyVisuallyCollapsed/u);
  assert.match(groupRows, /@transitionend="handleBodyTransitionEnd"/u);
  assert.match(card, /transition-transform duration-200/u);
  assert.match(card, /'rotate-90': !isSectionCollapsed\(section\)/u);
  assert.match(
    styles,
    /\.mapping-group-collapse-body \{[\s\S]*?clip-path 220ms/u,
  );
  assert.match(
    styles,
    /\.mapping-group-collapse-body--collapsed \{[\s\S]*?clip-path: inset\(0 0 100% 0\)/u,
  );
  assert.match(
    styles,
    /@media \(prefers-reduced-motion: reduce\)[\s\S]*?mapping-group-collapse-body/u,
  );
});

test("reconciles draggable table rows after a cross-group move", () => {
  const card = readFileSync(
    new URL(
      "../src/views/subdomain-proxy/SubdomainMappingsCard.vue",
      import.meta.url,
    ),
    "utf8",
  );
  const groupRows = readFileSync(
    new URL(
      "../src/views/subdomain-proxy/SubdomainMappingGroupRows.vue",
      import.meta.url,
    ),
    "utf8",
  );

  assert.match(groupRows, /:key="draggableRenderKey"/u);
  assert.match(
    groupRows,
    /buildHostMappingDragRenderKey\(props\.mappings\)/u,
  );
  assert.match(
    card,
    /<TableRow[\s\S]*?:key="mapping\.host"[\s\S]*?:data-host-mapping="mapping\.host"[\s\S]*?class="mapping-row"/u,
  );
  assert.match(
    card,
    /\(isSaving, wasSaving\) => \{[\s\S]*?if \(wasSaving && !isSaving\) syncGroupSections\(\);/u,
  );
  assert.doesNotMatch(
    card,
    /props\.searchQuery,\s*props\.isSavingMappings,\s*showGroupedView\.value/u,
  );
});
