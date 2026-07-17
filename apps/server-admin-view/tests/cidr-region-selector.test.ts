/// <reference types="node" />

import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import { extname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, it } from "node:test";
import { nextTick, ref } from "vue";

import { createCidrRegionSelectorState } from "../src/components/cidr-region-selector-state";
import {
  getCidrRegionSelectionKey,
  getCidrRegionSelectionLabel,
} from "../src/types/cidr";
import type {
  CidrCapabilitiesPayload,
  GatewayVisibilitySelection,
} from "../src/types";

const srcRoot = fileURLToPath(new URL("../src", import.meta.url));
const selectorPath = "components/CidrRegionSelector.vue";
const selectorStatePath = "components/cidr-region-selector-state.ts";
const selectorConsumers = [
  "views/ip-whitelist/WhitelistAddDialog.vue",
  "views/SSHSecurity.vue",
  "views/subdomain-proxy/SubdomainMappingVisibilityPanel.vue",
  "views/system-settings/GatewayVisibilitySettings.vue",
  "views/system-settings/ScannerFirewallSettings.vue",
];

const listSourceFiles = async (directory: string): Promise<string[]> => {
  const entries = await readdir(directory, { withFileTypes: true });
  const files = await Promise.all(
    entries.map(async (entry) => {
      const path = join(directory, entry.name);
      return entry.isDirectory() ? listSourceFiles(path) : [path];
    }),
  );
  return files.flat().filter((path) => [".ts", ".vue"].includes(extname(path)));
};

const capabilities = (supported: boolean): CidrCapabilitiesPayload => ({
  source: "custom",
  operatorFiltering: {
    supported,
    operators: ["电信", "联通", "移动"],
    minimumContainerVersion: "0.1.3",
  },
});
const province = {
  label: "江苏省",
  value: "江苏省",
  cityCount: 2,
  isMunicipality: false,
};
const city = {
  label: "南京市",
  value: "南京市",
  queryCity: "南京市",
  isProvinceWide: false,
  isMunicipality: false,
  ipv4Count: 1,
  ipv6Count: 1,
};
const secondCity = {
  ...city,
  label: "苏州市",
  value: "苏州市",
  queryCity: "苏州市",
};
const provinceWide = {
  ...city,
  label: "江苏全省",
  value: "__province_all__",
  queryCity: null,
  isProvinceWide: true,
};
const selectionFromOption = (
  option: typeof city | typeof provinceWide,
  operator: GatewayVisibilitySelection["operator"] = null,
): GatewayVisibilitySelection => ({
  province: province.value,
  city: option.isProvinceWide ? null : option.label,
  label: operator ? `${option.label} · ${operator}` : option.label,
  value: option.value,
  query_city: option.queryCity,
  operator,
  is_province_wide: option.isProvinceWide,
  is_municipality: option.isMunicipality,
});
const flushPromises = () =>
  new Promise<void>((resolve) => setImmediate(resolve));

const createState = (
  selections = ref<GatewayVisibilitySelection[]>([]),
  operatorSupport = true,
) =>
  createCidrRegionSelectorState({
    disabled: ref(false),
    formatLoadError: String,
    loadCapabilities: async () => capabilities(operatorSupport),
    loadCities: async () => ({ options: [provinceWide, city, secondCity] }),
    loadProvinces: async () => ({ options: [province] }),
    onLoadError: () => assert.fail("loading should succeed"),
    selections,
  });

describe("CIDR region selector", () => {
  it("builds stable keys from province, city, and operator", () => {
    assert.equal(
      getCidrRegionSelectionKey({ province: "浙江省", query_city: "杭州市" }),
      "浙江省::杭州市::",
    );
    assert.equal(
      getCidrRegionSelectionKey({
        province: "浙江省",
        query_city: "杭州市",
        operator: "移动",
      }),
      "浙江省::杭州市::移动",
    );
    assert.equal(
      getCidrRegionSelectionLabel({
        province: "浙江省",
        query_city: "杭州市",
        operator: "移动",
      }),
      "杭州市 · 移动",
    );
    assert.equal(
      getCidrRegionSelectionLabel(
        {
          province: "浙江省",
          query_city: "杭州市",
          operator: "移动",
        },
        { includeProvince: true },
      ),
      "浙江省 / 杭州市 · 移动",
    );
  });

  it("saves multiple cities in the all-operator layer", async () => {
    const selections = ref<GatewayVisibilitySelection[]>([]);
    const state = createState(selections);
    await state.loadProvinces();
    state.openDialog();
    await flushPromises();

    state.toggleCity("江苏省::南京市::", true);
    state.toggleCity("江苏省::苏州市::", true);
    assert.equal(state.selectedCityCount.value, 2);
    state.saveProvinceSelections();
    assert.deepEqual(selections.value.map(getCidrRegionSelectionKey), [
      "江苏省::南京市::",
      "江苏省::苏州市::",
    ]);
    state.dispose();
  });

  it("supports multiple carriers and normalizes all-carrier overlap", async () => {
    const selections = ref<GatewayVisibilitySelection[]>([]);
    const state = createState(selections);
    await state.loadCapabilities();
    state.selectProvince(province.value);
    await flushPromises();

    state.selectOperator("移动");
    state.toggleCity("江苏省::南京市::移动", true);
    state.selectOperator("电信");
    state.toggleCity("江苏省::南京市::电信", true);
    assert.deepEqual(state.draft.selections.map(getCidrRegionSelectionKey), [
      "江苏省::南京市::移动",
      "江苏省::南京市::电信",
    ]);

    state.selectOperator(null);
    state.toggleCity("江苏省::南京市::", true);
    assert.deepEqual(state.draft.selections.map(getCidrRegionSelectionKey), [
      "江苏省::南京市::",
    ]);

    state.selectOperator("移动");
    state.toggleCity("江苏省::南京市::移动", true);
    assert.deepEqual(state.draft.selections.map(getCidrRegionSelectionKey), [
      "江苏省::南京市::移动",
    ]);
    state.dispose();
  });

  it("keeps province-wide and city choices exclusive within each carrier", async () => {
    const state = createState();
    await state.loadCapabilities();
    state.selectProvince(province.value);
    await flushPromises();
    state.selectOperator("移动");
    state.toggleCity("江苏省::南京市::移动", true);
    state.toggleCity("江苏省::::移动", true);
    assert.deepEqual(state.draft.selections.map(getCidrRegionSelectionKey), [
      "江苏省::::移动",
    ]);

    state.selectOperator("电信");
    state.toggleCity("江苏省::南京市::电信", true);
    assert.deepEqual(state.draft.selections.map(getCidrRegionSelectionKey), [
      "江苏省::::移动",
      "江苏省::南京市::电信",
    ]);
    state.dispose();
  });

  it("degrades to regular region selection for legacy containers", async () => {
    const existing = selectionFromOption(city, "移动");
    const selections = ref([existing]);
    const state = createState(selections, false);
    await state.loadCapabilities();
    assert.equal(state.operatorFilteringSupported.value, false);
    state.selectOperator("移动");
    assert.equal(state.draft.operator, null);
    state.removeRegion(existing);
    assert.deepEqual(selections.value, []);
    state.dispose();
  });

  it("ignores stale city responses", async () => {
    let resolveFirst!: (value: { options: [typeof city] }) => void;
    let resolveSecond!: (value: { options: [typeof city] }) => void;
    const firstResponse = new Promise<{ options: [typeof city] }>((resolve) => {
      resolveFirst = resolve;
    });
    const secondResponse = new Promise<{ options: [typeof city] }>(
      (resolve) => {
        resolveSecond = resolve;
      },
    );
    const state = createCidrRegionSelectorState({
      disabled: ref(false),
      formatLoadError: String,
      loadCapabilities: async () => capabilities(true),
      loadCities: (name) => (name === "first" ? firstResponse : secondResponse),
      loadProvinces: async () => ({ options: [province] }),
      onLoadError: () => assert.fail("loading should succeed"),
      selections: ref([]),
    });

    const firstRequest = state.loadCityOptions("first");
    const secondRequest = state.loadCityOptions("second");
    resolveSecond({ options: [{ ...city, label: "新城市" }] });
    await secondRequest;
    resolveFirst({ options: [{ ...city, label: "旧城市" }] });
    await firstRequest;
    assert.equal(state.cityOptions.value[0]?.label, "新城市");
    state.dispose();
  });

  it("closes and clears the draft when disabled", async () => {
    const disabled = ref(false);
    const state = createCidrRegionSelectorState({
      disabled,
      formatLoadError: String,
      loadCapabilities: async () => capabilities(true),
      loadCities: async () => ({ options: [city] }),
      loadProvinces: async () => ({ options: [province] }),
      onLoadError: () => assert.fail("loading should succeed"),
      selections: ref([]),
    });
    await state.loadProvinces();
    state.openDialog();
    disabled.value = true;
    await nextTick();
    assert.equal(state.isDialogOpen.value, false);
    assert.equal(state.draft.province, "");
    state.dispose();
  });

  it("keeps all five business entry points on the shared component", async () => {
    await Promise.all(
      selectorConsumers.map(async (path) => {
        const source = await readFile(join(srcRoot, path), "utf8");
        assert.equal(source.match(/<CidrRegionSelector\b/gu)?.length, 1);
        assert.doesNotMatch(
          source,
          /\bCidrAPI\b|\bregionDraft\b|\bcityOptionsLoading\b/u,
        );
      }),
    );
  });

  it("keeps CIDR loading and selection logic encapsulated", async () => {
    const sourceFiles = await listSourceFiles(srcRoot);
    const violations: string[] = [];
    await Promise.all(
      sourceFiles.map(async (path) => {
        const source = await readFile(path, "utf8");
        const sourcePath = relative(srcRoot, path);
        const isSelectorImplementation =
          sourcePath === selectorPath || sourcePath === selectorStatePath;
        const isSelectorDomainSource =
          isSelectorImplementation || sourcePath === "types/cidr.ts";
        if (!isSelectorImplementation && /\bCidrAPI\s*\./u.test(source)) {
          violations.push(sourcePath);
        }
        if (
          sourcePath !== "lib/api/gateway.ts" &&
          /["']\/cidr\/(?:capabilities|provinces|cities|selector)["']/u.test(
            source,
          )
        ) {
          violations.push(sourcePath);
        }
        if (
          !isSelectorDomainSource &&
          /\b(?:regionDraft|cityOptionsLoading|CidrCityOption|CidrProvinceOption)\b/u.test(
            source,
          )
        ) {
          violations.push(sourcePath);
        }
      }),
    );
    assert.deepEqual(violations.sort(), []);
  });
});
