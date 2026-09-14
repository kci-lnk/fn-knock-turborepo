import assert from "node:assert/strict";
import test from "node:test";
import { computed, ref } from "vue";
import { createPinia, setActivePinia } from "pinia";
import { ConfigAPI } from "../src/lib/api/config";
import { useConfigStore } from "../src/store/config";
import type { AppConfig } from "../src/types";
import { createDefaultMapping } from "../src/views/subdomain-proxy/model";
import { useSubdomainBatchEdit } from "../src/views/subdomain-proxy/useSubdomainBatchEdit";

test("a batch queued behind another catalog save cannot overwrite that save", async (t) => {
  setActivePinia(createPinia());
  const store = useConfigStore();
  const mapping = {
    ...createDefaultMapping(),
    host: "app.test",
    target: "http://backend",
    title: "App",
  };
  store.config = {
    host_mappings: [mapping],
    host_mapping_groups: [],
    host_mapping_grouped_view: false,
  } as unknown as AppConfig;
  let release!: () => void;
  const pending = new Promise<void>((resolve) => {
    release = resolve;
  });
  let requests = 0;
  t.mock.method(ConfigAPI, "updateHostMappingCatalog", async (mappings) => {
    requests++;
    if (requests === 1) await pending;
    return {
      mappings,
      groups: [],
      groupedView: false,
      revision: String(requests),
      hostMappingsRevision: String(requests),
    };
  });
  const editor = useSubdomainBatchEdit({
    allMappings: computed(() => store.config!.host_mappings),
    isSavingMappings: ref(false),
    isWindows: () => false,
    isAuthServiceTarget: () => false,
    reloadMappings: () => store.loadConfig({ force: true }),
    saveHostMappings: store.saveHostMappings,
    readGatewayHosts: async () => [],
    writeGatewayHosts: async () => {},
    translate: (key) => key,
    onSaved: () => {},
  });
  editor.openDialog([mapping.host], () => {});
  editor.rows.value[0]!.title = "Draft";
  const other = store.saveHostMappings([{ ...mapping, disabled: true }]);
  const batch = editor.save();
  release();
  await Promise.all([other, batch]);
  assert.equal(requests, 1);
  assert.equal(store.config!.host_mappings[0]!.disabled, true);
  assert.equal(editor.rows.value[0]!.title, "Draft");
  assert.match(editor.error.value, /conflict/);
});

test("remote catalog conflict refreshes the revision without discarding the draft", async (t) => {
  setActivePinia(createPinia());
  const store = useConfigStore();
  const mapping = {
    ...createDefaultMapping(),
    host: "app.test",
    target: "http://backend",
    title: "App",
  };
  store.config = {
    host_mappings: [mapping],
    host_mapping_groups: [],
    host_mapping_grouped_view: false,
  } as unknown as AppConfig;
  let requests = 0,
    refreshes = 0;
  t.mock.method(
    ConfigAPI,
    "updateHostMappingCatalog",
    async (mappings, _groups, _grouped, revision) => {
      requests++;
      if (requests === 1)
        throw Object.assign(new Error("Catalog conflict"), {
          response: { status: 409 },
        });
      assert.equal(revision, "latest");
      return {
        mappings,
        groups: [],
        groupedView: false,
        revision: "saved",
        hostMappingsRevision: "saved",
      };
    },
  );
  t.mock.method(ConfigAPI, "getConfig", async () => {
    refreshes++;
    return {
      config: {
        ...store.config!,
        host_mappings: [{ ...mapping, disabled: true }],
      },
      hostMappingCatalogRevision: "latest",
      hostMappingsRevision: "latest",
    };
  });
  const editor = useSubdomainBatchEdit({
    allMappings: computed(() => store.config!.host_mappings),
    isSavingMappings: ref(false),
    isWindows: () => false,
    isAuthServiceTarget: () => false,
    saveHostMappings: store.saveHostMappings,
    reloadMappings: () => store.loadConfig({ force: true }),
    readGatewayHosts: async () => [],
    writeGatewayHosts: async () => {},
    translate: (key) => key,
    onSaved: () => {},
  });
  editor.openDialog([mapping.host], () => {});
  editor.rows.value[0]!.title = "Draft";
  await editor.save();
  assert.equal(refreshes, 1);
  assert.equal(editor.rows.value[0]!.title, "Draft");
  assert.equal(store.config!.host_mappings[0]!.disabled, true);
  await editor.save();
  assert.equal(requests, 1); // Do not overwrite the newly discovered change.
  editor.discard();
  editor.openDialog([mapping.host], () => {});
  editor.rows.value[0]!.title = "Rebased";
  await editor.save();
  assert.equal(store.config!.host_mappings[0]!.title_override, "Rebased");
  assert.equal(store.config!.host_mappings[0]!.disabled, true);
});
