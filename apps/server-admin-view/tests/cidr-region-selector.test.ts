/// <reference types="node" />

import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import { extname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, it } from "node:test";
import { nextTick, ref } from "vue";

import { createCidrRegionSelectorState } from "../src/components/cidr-region-selector-state";
import { getCidrRegionSelectionKey } from "../src/types/cidr";
import type { GatewayVisibilitySelection } from "../src/types";

const srcRoot = fileURLToPath(new URL("../src", import.meta.url));
const selectorPath = "components/CidrRegionSelector.vue";
const selectorStatePath = "components/cidr-region-selector-state.ts";
const selectorConsumers = [
  "views/IPWhitelist.vue",
  "views/SSHSecurity.vue",
  "views/subdomain-proxy/SubdomainMappingDialog.vue",
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

const province = {
  label: "浙江省",
  value: "浙江省",
  cityCount: 1,
  isMunicipality: false,
};
const city = {
  label: "杭州市",
  value: "浙江省::杭州市",
  queryCity: "杭州市",
  isProvinceWide: false,
  isMunicipality: false,
  ipv4Count: 1,
  ipv6Count: 1,
};
const secondCity = {
  ...city,
  label: "宁波市",
  value: "浙江省::宁波市",
  queryCity: "宁波市",
};
const provinceWide = {
  ...city,
  label: "浙江全省",
  value: "__province_all__",
  queryCity: null,
  isProvinceWide: true,
};
const selectionFromOption = (
  option: typeof city | typeof provinceWide,
): GatewayVisibilitySelection => ({
  province: province.value,
  city: option.isProvinceWide ? null : option.label,
  label: option.label,
  value: option.value,
  query_city: option.queryCity,
  is_province_wide: option.isProvinceWide,
  is_municipality: option.isMunicipality,
});
const flushPromises = () =>
  new Promise<void>((resolve) => setImmediate(resolve));

describe("CIDR region selector", () => {
  it("builds one stable key for city and province-wide selections", () => {
    assert.equal(
      getCidrRegionSelectionKey({
        province: "浙江省",
        query_city: "杭州市",
      }),
      "浙江省::杭州市",
    );
    assert.equal(
      getCidrRegionSelectionKey({ province: "浙江省", query_city: null }),
      "浙江省::",
    );
  });

  it("loads a new province empty and saves multiple cities at once", async () => {
    const selections = ref<GatewayVisibilitySelection[]>([]);
    const disabled = ref(false);
    const state = createCidrRegionSelectorState({
      disabled,
      formatLoadError: String,
      loadCities: async () => ({
        defaultValue: provinceWide.value,
        options: [provinceWide, city, secondCity],
      }),
      loadProvinces: async () => ({ options: [province] }),
      onLoadError: () => assert.fail("loading should succeed"),
      selections,
    });

    await state.loadProvinces();
    state.openDialog();
    await nextTick();
    await flushPromises();

    assert.deepEqual(state.draft.cityValues, []);
    assert.equal(state.canSaveSelections.value, false);

    state.toggleCity("浙江省::杭州市", true);
    state.toggleCity("浙江省::宁波市", true);
    assert.equal(state.selectedCityCount.value, 2);
    assert.equal(state.canSaveSelections.value, true);
    state.handleDialogOpenChange(false);
    assert.deepEqual(selections.value, []);

    state.openDialog();
    await flushPromises();
    state.toggleCity("浙江省::杭州市", true);
    state.toggleCity("浙江省::宁波市", true);
    state.saveProvinceSelections();
    assert.deepEqual(selections.value.map(getCidrRegionSelectionKey), [
      "浙江省::杭州市",
      "浙江省::宁波市",
    ]);
    assert.equal(state.isDialogOpen.value, false);

    state.removeRegion(selections.value[0]!);
    assert.deepEqual(selections.value.map(getCidrRegionSelectionKey), [
      "浙江省::宁波市",
    ]);
    state.dispose();
  });

  it("edits one province as a group without changing other provinces", async () => {
    const otherProvinceSelection: GatewayVisibilitySelection = {
      ...selectionFromOption(city),
      province: "江苏省",
      city: "南京市",
      label: "南京市",
      value: "江苏省::南京市",
      query_city: "南京市",
    };
    const selections = ref<GatewayVisibilitySelection[]>([
      otherProvinceSelection,
      selectionFromOption(city),
    ]);
    const state = createCidrRegionSelectorState({
      disabled: ref(false),
      formatLoadError: String,
      loadCities: async () => ({ options: [provinceWide, city, secondCity] }),
      loadProvinces: async () => ({ options: [province] }),
      onLoadError: () => assert.fail("loading should succeed"),
      selections,
    });

    state.selectProvince(province.value);
    await flushPromises();
    assert.deepEqual(state.draft.cityValues, ["浙江省::杭州市"]);

    state.toggleCity("浙江省::杭州市", false);
    state.toggleCity("浙江省::宁波市", true);
    state.saveProvinceSelections();
    assert.deepEqual(selections.value.map(getCidrRegionSelectionKey), [
      "江苏省::南京市",
      "浙江省::宁波市",
    ]);
    state.dispose();
  });

  it("keeps province-wide and city selections mutually exclusive", async () => {
    const selections = ref<GatewayVisibilitySelection[]>([]);
    const state = createCidrRegionSelectorState({
      disabled: ref(false),
      formatLoadError: String,
      loadCities: async () => ({ options: [provinceWide, city, secondCity] }),
      loadProvinces: async () => ({ options: [province] }),
      onLoadError: () => assert.fail("loading should succeed"),
      selections,
    });

    state.selectProvince(province.value);
    await flushPromises();
    state.toggleCity("浙江省::杭州市", true);
    state.toggleCity("浙江省::宁波市", true);
    state.toggleCity("浙江省::", true);
    assert.deepEqual(state.draft.cityValues, ["浙江省::"]);

    state.toggleCity("浙江省::杭州市", true);
    assert.deepEqual(state.draft.cityValues, ["浙江省::杭州市"]);
    state.dispose();
  });

  it("normalizes legacy province-wide and city conflicts on load", async () => {
    const selections = ref<GatewayVisibilitySelection[]>([
      selectionFromOption(city),
      selectionFromOption(provinceWide),
      selectionFromOption(secondCity),
    ]);
    const state = createCidrRegionSelectorState({
      disabled: ref(false),
      formatLoadError: String,
      loadCities: async () => ({ options: [provinceWide, city, secondCity] }),
      loadProvinces: async () => ({ options: [province] }),
      onLoadError: () => assert.fail("loading should succeed"),
      selections,
    });

    state.selectProvince(province.value);
    await flushPromises();
    assert.deepEqual(state.draft.cityValues, ["浙江省::"]);
    assert.equal(state.canSaveSelections.value, true);

    state.saveProvinceSelections();
    assert.deepEqual(selections.value.map(getCidrRegionSelectionKey), [
      "浙江省::",
    ]);
    state.dispose();
  });

  it("can clear an existing province and preserves unavailable selections until unchecked", async () => {
    const unavailableSelection: GatewayVisibilitySelection = {
      ...selectionFromOption(city),
      city: "旧城市",
      label: "旧城市",
      value: "浙江省::旧城市",
      query_city: "旧城市",
    };
    const selections = ref<GatewayVisibilitySelection[]>([
      selectionFromOption(city),
      unavailableSelection,
    ]);
    const state = createCidrRegionSelectorState({
      disabled: ref(false),
      formatLoadError: String,
      loadCities: async () => ({ options: [city] }),
      loadProvinces: async () => ({ options: [province] }),
      onLoadError: () => assert.fail("loading should succeed"),
      selections,
    });

    state.selectProvince(province.value);
    await flushPromises();
    assert.deepEqual(
      state.cityChoices.value.map((choice) => [choice.key, choice.unavailable]),
      [
        ["浙江省::杭州市", false],
        ["浙江省::旧城市", true],
      ],
    );

    state.toggleCity("浙江省::杭州市", false);
    state.saveProvinceSelections();
    assert.deepEqual(selections.value.map(getCidrRegionSelectionKey), [
      "浙江省::旧城市",
    ]);

    state.selectProvince(province.value);
    await flushPromises();
    state.toggleCity("浙江省::旧城市", false);
    assert.equal(state.canSaveSelections.value, true);
    state.saveProvinceSelections();
    assert.deepEqual(selections.value, []);
    state.dispose();
  });

  it("ignores stale city responses", async () => {
    let resolveFirst!: (value: { options: [typeof city] }) => void;
    let resolveSecond!: (value: { options: [typeof city] }) => void;
    const firstCity = { ...city, label: "旧城市", value: "old" };
    const secondCity = { ...city, label: "新城市", value: "new" };
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
      loadCities: (name) => (name === "first" ? firstResponse : secondResponse),
      loadProvinces: async () => ({ options: [province] }),
      onLoadError: () => assert.fail("loading should succeed"),
      selections: ref([]),
    });

    const firstRequest = state.loadCityOptions("first");
    const secondRequest = state.loadCityOptions("second");
    resolveSecond({ options: [secondCity] });
    await secondRequest;
    resolveFirst({ options: [firstCity] });
    await firstRequest;

    assert.deepEqual(state.cityOptions.value, [secondCity]);
    assert.deepEqual(state.draft.cityValues, []);
    assert.equal(state.cityOptionsLoading.value, false);
    state.dispose();
  });

  it("recovers from province failures and closes when disabled", async () => {
    const disabled = ref(false);
    const errors: string[] = [];
    let attempts = 0;
    const state = createCidrRegionSelectorState({
      disabled,
      formatLoadError: (error) => (error as Error).message,
      loadCities: async () => ({ defaultValue: city.value, options: [city] }),
      loadProvinces: async () => {
        attempts += 1;
        if (attempts === 1) throw new Error("province failure");
        return { options: [province] };
      },
      onLoadError: (description) => errors.push(description),
      selections: ref([]),
    });

    await state.loadProvinces();
    assert.equal(state.provincesLoadError.value, "province failure");
    assert.deepEqual(errors, ["province failure"]);

    await state.loadProvinces();
    assert.equal(state.provincesLoadError.value, "");
    assert.deepEqual(state.provinces.value, [province]);

    state.openDialog();
    assert.equal(state.isDialogOpen.value, true);
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
        assert.equal(
          source.match(/<CidrRegionSelector\b/gu)?.length,
          1,
          `${path} must render exactly one shared CIDR region selector`,
        );
        assert.doesNotMatch(
          source,
          /\bCidrAPI\b|\bregionDraft\b|\bcityOptionsLoading\b/u,
        );
        assert.doesNotMatch(
          source,
          /<Select[\s\S]{0,600}v-model="[^"]*province/u,
          `${path} must not implement its own province selector`,
        );
      }),
    );
  });

  it("keeps province/city loading logic inside the shared component", async () => {
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
          /["']\/cidr\/(?:provinces|cities|selector)["']/u.test(source)
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

    assert.deepEqual(
      violations.sort(),
      [],
      "CIDR region loading and selection logic must not be reimplemented",
    );
  });
});
