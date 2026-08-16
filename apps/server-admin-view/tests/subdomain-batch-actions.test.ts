import assert from "node:assert/strict";
import test from "node:test";
import { computed, ref } from "vue";
import type { HostMapping } from "../src/types";
import { createDefaultMapping } from "../src/views/subdomain-proxy/model";
import { useSubdomainBatchActions } from "../src/views/subdomain-proxy/useSubdomainBatchActions";

const mapping = (host: string, target = `http://${host}`): HostMapping => ({
  ...createDefaultMapping(),
  host,
  target,
});

const createActions = (mappings: HostMapping[]) => {
  const saved = ref<HostMapping[] | null>(null);
  const actions = useSubdomainBatchActions({
    allMappings: computed(() => mappings),
    isAuthServiceTarget: (target) => target === "http://auth:7997",
    isSavingMappings: ref(false),
    runSaveMappings: async (action) => action(),
    saveHostMappings: async (next) => {
      saved.value = next;
    },
    translate: (key, params) => `${key}:${params?.count ?? ""}`,
  });
  return { actions, saved };
};

test("batch disable changes only selected regular mappings and clears selection after saving", async () => {
  const first = mapping("one.example.test");
  const second = mapping("two.example.test");
  const auth = mapping("auth.example.test", "http://auth:7997");
  const { actions, saved } = createActions([first, second, auth]);
  let completed = false;

  actions.openBatchMutation(
    [first.host, auth.host],
    "disable",
    () => (completed = true),
  );
  await actions.confirmBatchMutation();

  assert.equal(completed, true);
  assert.equal(saved.value?.find((item) => item.host === first.host)?.disabled, true);
  assert.equal(saved.value?.find((item) => item.host === second.host)?.disabled, false);
  assert.equal(saved.value?.find((item) => item.host === auth.host)?.disabled, false);
  assert.equal(actions.isBatchMutationOpen.value, false);
});

test("batch availability applies one daily window and can clear it", async () => {
  const first = mapping("one.example.test");
  const second = mapping("two.example.test");
  const { actions, saved } = createActions([first, second]);

  actions.openBatchAvailability([first.host, second.host], () => undefined);
  actions.availabilityFormStartTime.value = "22:00";
  actions.availabilityFormEndTime.value = "06:00";
  await actions.saveBatchAvailability();
  assert.deepEqual(saved.value?.map((item) => item.availability), [
    { enabled: true, start_time: "22:00", end_time: "06:00" },
    { enabled: true, start_time: "22:00", end_time: "06:00" },
  ]);

  actions.openBatchAvailability([first.host, second.host], () => undefined);
  actions.availabilityFormEnabled.value = false;
  await actions.saveBatchAvailability();
  assert.deepEqual(saved.value?.map((item) => item.availability), [null, null]);
});

test("batch availability rejects equal start and end times without saving", async () => {
  const first = mapping("one.example.test");
  const { actions, saved } = createActions([first]);

  actions.openBatchAvailability([first.host], () => undefined);
  actions.availabilityFormStartTime.value = "09:00";
  actions.availabilityFormEndTime.value = "09:00";
  await actions.saveBatchAvailability();

  assert.equal(saved.value, null);
  assert.notEqual(actions.availabilityValidationMessage.value, "");
});
