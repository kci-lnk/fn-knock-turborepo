import assert from "node:assert/strict";
import test from "node:test";
import { computed } from "vue";
import { createPinia, setActivePinia } from "pinia";
import { useAsyncAction } from "../../../packages/admin-shared/src/composables/useAsyncAction";
import { ConfigAPI } from "../src/lib/api/config";
import { useConfigStore } from "../src/store/config";
import type { AppConfig } from "../src/types";
import { createDefaultMapping } from "../src/views/subdomain-proxy/model";
import { useSubdomainBatchActions } from "../src/views/subdomain-proxy/useSubdomainBatchActions";

for (const mutation of ["require-auth", "public-access", "schedule"] as const) {
  test(`${mutation} cannot overwrite a catalog save it was queued behind`, async (t) => {
    setActivePinia(createPinia());
    const store = useConfigStore();
    const mapping = {
      ...createDefaultMapping(),
      host: "app.test",
      target: "http://backend",
      use_auth: mutation !== "require-auth",
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
    let completed = 0;
    const errors: unknown[] = [];
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
    const asyncAction = useAsyncAction({
      onError: (error) => errors.push(error),
    });
    const actions = useSubdomainBatchActions({
      allMappings: computed(() => store.config!.host_mappings),
      isAuthServiceTarget: () => false,
      isSavingMappings: asyncAction.isPending,
      runSaveMappings: asyncAction.run,
      saveHostMappings: (mappings, beforeSave) =>
        store.saveHostMappings(
          mappings,
          undefined,
          undefined,
          undefined,
          beforeSave,
        ),
      translate: (key) => key,
    });
    if (mutation === "schedule")
      actions.openBatchAvailability([mapping.host], () => completed++);
    else actions.openBatchMutation([mapping.host], mutation, () => completed++);
    const other = store.saveHostMappings([{ ...mapping, disabled: true }]);
    const save = () =>
      mutation === "schedule"
        ? actions.saveBatchAvailability()
        : actions.confirmBatchMutation();
    const batch = save();
    release();
    await Promise.all([other, batch]);
    assert.equal(
      requests,
      1,
      "the stale batch must be rejected before sending another catalog write",
    );
    assert.equal(store.config!.host_mappings[0]!.disabled, true);
    assert.equal(store.config!.host_mappings[0]!.use_auth, mapping.use_auth);
    assert.equal(completed, 0);
    assert.match(String(errors[0]), /batchConfigurationChanged/);
    assert.equal(actions.batchSelectedCount.value, 1);
    await save();
    assert.equal(requests, 2);
    assert.equal(store.config!.host_mappings[0]!.disabled, true);
    if (mutation !== "schedule")
      assert.equal(
        store.config!.host_mappings[0]!.use_auth,
        mutation === "require-auth",
      );
    assert.equal(completed, 1);
  });
}
