import assert from "node:assert/strict";
import test from "node:test";
import { computed, ref } from "vue";
import { useAsyncAction } from "../../../packages/admin-shared/src/composables/useAsyncAction";
import type { HostMapping } from "../src/types";
import { toHostMappingUpdatePayload } from "../src/lib/api/host-mapping-payload";
import {
  createDefaultMapping,
  createDefaultStaticServe,
} from "../src/views/subdomain-proxy/model";
import { createDefaultLocation } from "../src/views/system-settings/gateway-locations/gatewayLocationModel";
import { useSubdomainBatchActions } from "../src/views/subdomain-proxy/useSubdomainBatchActions";

const fixture = (): HostMapping[] => [
  {
    ...createDefaultMapping(),
    host: "proxy.test",
    target: "http://app:8080",
    use_auth: false,
    disabled: true,
    access_mode: "strict_whitelist",
    availability: { enabled: true, start_time: "09:00", end_time: "18:00" },
    basic_auth: { enabled: true, username: "upstream", password: "secret" },
    advanced_auth: {
      enabled: true,
      groups: [],
      idle_ttl_seconds: 86400,
      max_lifetime_seconds: 2592000,
      policy_version: "existing-policy",
    },
    locations: [
      {
        ...createDefaultLocation(),
        path: "/admin",
        auth_mode: "require_login",
      },
      { ...createDefaultLocation(), path: "/public", auth_mode: "public" },
    ],
  },
  {
    ...createDefaultMapping(),
    host: "static.test",
    target_type: "directory",
    static_serve: createDefaultStaticServe("directory"),
  },
  { ...createDefaultMapping(), host: "unselected.test" },
  {
    ...createDefaultMapping(),
    host: "auth-role.test",
    service_role: "auth",
    use_auth: false,
  },
  {
    ...createDefaultMapping(),
    host: "auth-target.test",
    target: "http://auth:7997",
    use_auth: false,
  },
];
const setup = () => {
  const mappings = ref(fixture());
  const calls: HostMapping[][] = [];
  const errors: unknown[] = [];
  const asyncAction = useAsyncAction({
    onError: (error) => errors.push(error),
  });
  let failure: unknown;
  let wait: Promise<void> | undefined;
  let completed = 0;
  const actions = useSubdomainBatchActions({
    allMappings: computed(() => mappings.value),
    isAuthServiceTarget: (target) => target === "http://auth:7997",
    isSavingMappings: asyncAction.isPending,
    runSaveMappings: asyncAction.run,
    saveHostMappings: async (next) => {
      calls.push(next);
      await wait;
      if (failure) throw failure;
      mappings.value = next;
    },
    translate: (key, params) => `${key}:${params?.count ?? ""}`,
  });
  return {
    actions,
    mappings,
    calls,
    errors,
    open: (
      mutation: "require-auth" | "public-access",
      hosts = [
        "proxy.test",
        "static.test",
        "auth-role.test",
        "auth-target.test",
      ],
    ) => actions.openBatchMutation(hosts, mutation, () => completed++),
    fail: (error: unknown) => {
      failure = error;
    },
    delay: (promise: Promise<void>) => {
      wait = promise;
    },
    get completed() {
      return completed;
    },
  };
};

for (const mutation of ["require-auth", "public-access"] as const) {
  test(`${mutation} changes only use_auth for selected regular proxy/static mappings`, async () => {
    const ctx = setup();
    const before = structuredClone(fixture());
    ctx.open(mutation);
    assert.equal(ctx.actions.batchSelectedCount.value, 2);
    assert.match(ctx.actions.batchMutationTitle.value, /:2$/);
    assert.equal(ctx.calls.length, 0);
    await ctx.actions.confirmBatchMutation();
    assert.deepEqual(
      ctx.calls[0],
      before.map((mapping, index) =>
        index < 2
          ? { ...mapping, use_auth: mutation === "require-auth" }
          : mapping,
      ),
    );
    const payload = toHostMappingUpdatePayload(ctx.calls[0]![0]!);
    assert.equal(payload.use_auth, mutation === "require-auth");
    assert.deepEqual(payload.locations, before[0]!.locations);
    // Advanced policies stay with the existing server-side normalization flow.
    assert.equal("advanced_auth" in payload, false);
    assert.equal(ctx.completed, 1);
    assert.equal(ctx.actions.isBatchMutationOpen.value, false);
    assert.equal(ctx.actions.batchSelectedCount.value, 0);
  });
}

test("empty, auth-only, missing and cancelled selections never save", async () => {
  const ctx = setup();
  for (const hosts of [
    [],
    ["auth-role.test", "auth-target.test"],
    ["missing.test"],
  ]) {
    ctx.open("require-auth", hosts);
    assert.equal(ctx.actions.isBatchMutationOpen.value, false);
    await ctx.actions.confirmBatchMutation();
  }
  ctx.open("public-access", ["proxy.test", "proxy.test"]);
  assert.equal(ctx.actions.batchSelectedCount.value, 1);
  ctx.actions.closeBatchMutation();
  await ctx.actions.confirmBatchMutation();
  assert.equal(ctx.calls.length, 0);
  assert.equal(ctx.completed, 0);
});

test("pending saves block duplicate submits, close and changing the action", async () => {
  const ctx = setup();
  let release!: () => void;
  ctx.delay(
    new Promise<void>((resolve) => {
      release = resolve;
    }),
  );
  ctx.open("require-auth");
  const saving = ctx.actions.confirmBatchMutation();
  ctx.actions.closeBatchMutation();
  ctx.open("public-access");
  await ctx.actions.confirmBatchMutation();
  assert.equal(ctx.calls.length, 1);
  assert.equal(ctx.actions.isBatchMutationOpen.value, true);
  assert.match(ctx.actions.batchMutationTitle.value, /batchRequireAuthTitle/);
  release();
  await saving;
  assert.equal(ctx.completed, 1);
  assert.equal(ctx.actions.isBatchMutationOpen.value, false);
});

for (const error of [new Error("offline"), { response: { status: 409 } }]) {
  test(`failed save (${error instanceof Error ? "network" : "409"}) uses shared error handling and keeps selection for retry`, async () => {
    const ctx = setup();
    ctx.fail(error);
    ctx.open("public-access");
    await ctx.actions.confirmBatchMutation();
    assert.deepEqual(ctx.errors, [error]);
    assert.equal(ctx.completed, 0);
    assert.equal(ctx.actions.batchSelectedCount.value, 2);
    assert.equal(ctx.actions.isBatchMutationOpen.value, true);
    assert.deepEqual(ctx.mappings.value, fixture());
    // Shared page error handling refreshes the catalog on 409. Retry must use it.
    ctx.mappings.value[0]!.title_override = "Refreshed title";
    ctx.fail(undefined);
    await ctx.actions.confirmBatchMutation();
    assert.equal(ctx.calls[1]![0]!.title_override, "Refreshed title");
    assert.equal(ctx.completed, 1);
  });
}
