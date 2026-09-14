import assert from "node:assert/strict";
import test from "node:test";
import { computed, ref } from "vue";
import type { HostMapping } from "../src/types";
import {
  createDefaultMapping,
  getMappingDisplayTitle,
  createDefaultStaticServe,
} from "../src/views/subdomain-proxy/model";
import {
  useSubdomainBatchEdit,
  type BatchEditRow,
} from "../src/views/subdomain-proxy/useSubdomainBatchEdit";
const mapping = (host: string): HostMapping => ({
  ...createDefaultMapping(),
  host,
  target: `http://${host}:8080`,
  title: "Automatic",
  title_override: "",
});
const setup = (
  initial = [mapping("one.test"), mapping("two.test"), mapping("three.test")],
) => {
  const mappings = ref(initial);
  const calls: HostMapping[][] = [];
  const previous: ReadonlyMap<string, string>[] = [];
  const gateway = {
    gateway_proxy_headers: ["one.test", "unrelated.test"],
    gateway_host_response: ["one.test"],
  };
  const writes: string[] = [];
  let catalogFailure = false,
    gatewayFailure = false,
    pruneRemovedHosts = false,
    saved = 0,
    completed = 0;
  const editor = useSubdomainBatchEdit({
    allMappings: computed(() => mappings.value),
    isSavingMappings: ref(false),
    isWindows: () => false,
    isAuthServiceTarget: (target) => target === "http://auth:7997",
    saveHostMappings: async (next, _icons, _titles, renames) => {
      calls.push(next);
      if (catalogFailure) throw new Error("Catalog conflict");
      previous.push(renames ?? new Map());
      mappings.value = next;
      if (pruneRemovedHosts) {
        const removed = new Set(renames?.values());
        for (const kind of [
          "gateway_proxy_headers",
          "gateway_host_response",
        ] as const) {
          gateway[kind] = gateway[kind].filter((host) => !removed.has(host));
        }
      }
    },
    reloadMappings: async () => {},
    readGatewayHosts: async (kind) => [...gateway[kind]],
    writeGatewayHosts: async (kind, hosts) => {
      writes.push(kind);
      if (gatewayFailure && kind === "gateway_host_response")
        throw new Error("Gateway offline");
      gateway[kind] = [...hosts];
    },
    translate: (key) => key,
    onSaved: () => {
      saved++;
    },
  });
  return {
    editor,
    mappings,
    calls,
    previous,
    gateway,
    writes,
    open: (hosts = ["one.test", "two.test"]) =>
      editor.openDialog(hosts, () => {
        completed++;
      }),
    pruneAfterSave: () => {
      pruneRemovedHosts = true;
    },
    failCatalog: (value: boolean) => {
      catalogFailure = value;
    },
    failGateway: (value: boolean) => {
      gatewayFailure = value;
    },
    get saved() {
      return saved;
    },
    get completed() {
      return completed;
    },
  };
};
test("independent edits save once, preserving other settings and updating application display names", async () => {
  const ctx = setup();
  const before = JSON.parse(JSON.stringify(ctx.mappings.value));
  ctx.open();
  ctx.editor.rows.value[0]!.title = "First app";
  ctx.editor.rows.value[1]!.title = "Second app";
  ctx.editor.rows.value[1]!.target = "https://new-target.test";
  assert.deepEqual(JSON.parse(JSON.stringify(ctx.mappings.value)), before);
  await ctx.editor.save();
  assert.equal(ctx.calls.length, 1);
  assert.equal(getMappingDisplayTitle(ctx.mappings.value[0]!), "First app");
  assert.equal(getMappingDisplayTitle(ctx.mappings.value[1]!), "Second app");
  assert.deepEqual(ctx.mappings.value[0], {
    ...before[0],
    title_override: "First app",
  });
  assert.deepEqual(ctx.mappings.value[1], {
    ...before[1],
    title_override: "Second app",
    target: "https://new-target.test",
  });
  assert.deepEqual(ctx.mappings.value[2], before[2]);
  assert.equal(ctx.completed, 1);
  assert.equal(ctx.saved, 1);
  assert.equal(ctx.editor.open.value, false);
});
test("no-op does not save; cancel confirms dirty drafts without clearing selection", async () => {
  const ctx = setup();
  ctx.open();
  await ctx.editor.save();
  assert.equal(ctx.calls.length, 0);
  ctx.editor.requestClose();
  assert.equal(ctx.editor.open.value, false);
  ctx.open();
  ctx.editor.rows.value[0]!.title = "Draft";
  ctx.editor.requestClose();
  assert.equal(ctx.editor.discardOpen.value, true);
  assert.equal(ctx.editor.open.value, true);
  ctx.editor.discard();
  assert.equal(ctx.completed, 0);
  assert.equal(ctx.mappings.value[0]!.title_override, "");
});
test("clearing a manual title restores automatic title", async () => {
  const ctx = setup();
  ctx.mappings.value[0]!.title_override = "Manual";
  ctx.open();
  ctx.editor.rows.value[0]!.title = "";
  await ctx.editor.save();
  assert.equal(getMappingDisplayTitle(ctx.mappings.value[0]!), "Automatic");
});
for (const [name, edit] of [
  [
    "invalid host",
    (rows) => {
      rows[0]!.host = "bad host";
    },
  ],
  [
    "empty host",
    (rows) => {
      rows[0]!.host = "";
    },
  ],
  [
    "duplicate hosts",
    (rows) => {
      rows[0]!.host = "new.test";
      rows[1]!.host = "NEW.TEST";
    },
  ],
  [
    "occupied host",
    (rows) => {
      rows[0]!.host = "three.test";
    },
  ],
  [
    "domain swap",
    (rows) => {
      rows[0]!.host = "two.test";
      rows[1]!.host = "one.test";
    },
  ],
  [
    "invalid target",
    (rows) => {
      rows[0]!.target = "ftp://bad.test";
    },
  ],
  [
    "authentication target",
    (rows) => {
      rows[0]!.target = "http://auth:7997";
    },
  ],
] as [string, (rows: BatchEditRow[]) => void][]) {
  test(`${name} blocks the whole batch`, async () => {
    const ctx = setup();
    ctx.open();
    edit(ctx.editor.rows.value);
    await ctx.editor.save();
    assert.equal(ctx.calls.length, 0);
    assert.equal(ctx.editor.open.value, true);
    assert.ok(ctx.editor.errors.value.some((row) => row.host || row.target));
  });
}
test("static paths keep their type and settings and reject unsafe paths", async () => {
  const item = {
    ...mapping("one.test"),
    target_type: "directory" as const,
    target: "",
    static_serve: {
      ...createDefaultStaticServe("directory"),
      path: "/srv/site",
    },
  };
  const ctx = setup([item]);
  ctx.open([item.host]);
  ctx.editor.rows.value[0]!.target = "../unsafe";
  await ctx.editor.save();
  assert.equal(ctx.calls.length, 0);
  ctx.editor.rows.value[0]!.target = "/srv/new-site";
  await ctx.editor.save();
  assert.deepEqual(ctx.calls[0]![0]!.static_serve, {
    ...item.static_serve,
    path: "/srv/new-site",
  });
  assert.equal(ctx.calls[0]![0]!.target_type, "directory");
});
test("catalog failure keeps draft for retry", async () => {
  const ctx = setup();
  ctx.open();
  ctx.editor.rows.value[0]!.title = "Draft";
  ctx.failCatalog(true);
  await ctx.editor.save();
  assert.equal(ctx.editor.rows.value[0]!.title, "Draft");
  assert.equal(ctx.completed, 0);
  assert.equal(ctx.editor.mappingsSaved.value, false);
  ctx.failCatalog(false);
  await ctx.editor.save();
  assert.equal(ctx.saved, 1);
});
test("external changes block overwrite; automatic metadata refresh is preserved", async () => {
  const ctx = setup();
  ctx.open();
  ctx.editor.rows.value[0]!.title = "Draft";
  ctx.mappings.value[2]!.disabled = true;
  await ctx.editor.save();
  assert.equal(ctx.calls.length, 0);
  assert.match(ctx.editor.error.value, /conflict/);
  ctx.editor.discard();
  ctx.open();
  ctx.editor.rows.value[0]!.title = "Draft";
  ctx.mappings.value[0]!.title = "Refreshed title";
  await ctx.editor.save();
  assert.equal(ctx.calls[0]![0]!.title, "Refreshed title");
});
test("rename migrates both gateway lists and retries only failed steps", async () => {
  const ctx = setup();
  ctx.open();
  ctx.editor.rows.value[0]!.host = "NEW.test";
  ctx.failGateway(true);
  await ctx.editor.save();
  assert.equal(ctx.calls.length, 1);
  assert.equal(ctx.previous[0]!.get("new.test"), "one.test");
  assert.deepEqual([...ctx.gateway.gateway_proxy_headers].sort(), [
    "new.test",
    "unrelated.test",
  ]);
  assert.equal(ctx.editor.mappingsSaved.value, true);
  assert.match(ctx.editor.error.value, /partial/);
  assert.equal(ctx.completed, 0);
  ctx.failGateway(false);
  await ctx.editor.save();
  assert.equal(ctx.calls.length, 1);
  assert.deepEqual(ctx.writes, [
    "gateway_proxy_headers",
    "gateway_host_response",
    "gateway_host_response",
  ]);
  assert.deepEqual(ctx.gateway.gateway_host_response, ["new.test"]);
  assert.equal(ctx.completed, 1);
});
test("gateway retry refuses to overwrite concurrently changed settings", async () => {
  const ctx = setup();
  ctx.open();
  ctx.editor.rows.value[0]!.host = "new.test";
  ctx.failGateway(true);
  await ctx.editor.save();
  ctx.gateway.gateway_host_response = ["one.test", "another.test"];
  ctx.failGateway(false);
  await ctx.editor.save();
  assert.match(ctx.editor.error.value, /conflict/);
  assert.deepEqual(ctx.gateway.gateway_host_response, [
    "one.test",
    "another.test",
  ]);
  assert.equal(ctx.completed, 0);
});
test("saving blocks repeated submissions and closing", async () => {
  const ctx = setup();
  ctx.open();
  ctx.editor.rows.value[0]!.host = "new.test";
  const pending = ctx.editor.save();
  await ctx.editor.save();
  ctx.editor.requestClose();
  assert.equal(ctx.editor.open.value, true);
  assert.equal(ctx.editor.discardOpen.value, false);
  await pending;
  assert.equal(ctx.calls.length, 1);
});
test("authentication mappings cannot be included", () => {
  const ctx = setup([{ ...mapping("auth.test"), target: "http://auth:7997" }]);
  ctx.open(["auth.test"]);
  assert.equal(ctx.editor.open.value, false);
  assert.deepEqual(ctx.editor.rows.value, []);
});

test("multiple renames keep each mapping's gateway preferences and clear stale destination settings", async () => {
  const ctx = setup();
  ctx.mappings.value[0]!.sync_id = "identity-one";
  ctx.mappings.value[1]!.sync_id = "identity-two";
  ctx.gateway.gateway_proxy_headers = [
    "one.test",
    "new-two.test",
    "unrelated.test",
  ];
  ctx.gateway.gateway_host_response = ["two.test", "new-one.test"];
  ctx.open();
  ctx.editor.rows.value[0]!.host = "new-one.test";
  ctx.editor.rows.value[1]!.host = "new-two.test";
  await ctx.editor.save();
  assert.deepEqual([...ctx.gateway.gateway_proxy_headers].sort(), [
    "new-one.test",
    "unrelated.test",
  ]);
  assert.deepEqual(ctx.gateway.gateway_host_response, ["new-two.test"]);
  assert.equal(ctx.mappings.value[0]!.sync_id, "identity-one");
  assert.equal(ctx.mappings.value[1]!.sync_id, "identity-two");
  assert.equal(ctx.previous[0]!.get("new-two.test"), "two.test");
  assert.equal(ctx.calls.length, 1);
});

test("retry confirms runtime synchronization even when configuration was already persisted", async () => {
  const ctx = setup();
  ctx.open();
  ctx.editor.rows.value[0]!.host = "new.test";
  ctx.failGateway(true);
  await ctx.editor.save();
  // Simulate the server accepting a request whose response was lost.
  ctx.gateway.gateway_host_response = ["new.test"];
  ctx.failGateway(false);
  await ctx.editor.save();
  assert.equal(ctx.calls.length, 1);
  assert.deepEqual(ctx.writes, [
    "gateway_proxy_headers",
    "gateway_host_response",
    "gateway_host_response",
  ]);
  assert.equal(ctx.saved, 1);
});

test("catalog cleanup of old hosts does not block gateway migration or its retry", async () => {
  const ctx = setup();
  ctx.open();
  ctx.pruneAfterSave();
  ctx.editor.rows.value[0]!.host = "new.test";
  ctx.failGateway(true);
  await ctx.editor.save();
  assert.deepEqual([...ctx.gateway.gateway_proxy_headers].sort(), [
    "new.test",
    "unrelated.test",
  ]);
  assert.match(ctx.editor.error.value, /Gateway offline/);
  ctx.failGateway(false);
  await ctx.editor.save();
  assert.deepEqual(ctx.gateway.gateway_host_response, ["new.test"]);
  assert.equal(ctx.calls.length, 1);
  assert.equal(ctx.saved, 1);
});

test("equivalent refreshed config with reordered object keys does not block editing", async () => {
  const ctx = setup();
  ctx.open();
  ctx.editor.rows.value[0]!.title = "Draft";
  ctx.mappings.value = ctx.mappings.value.map(
    (mapping) =>
      Object.fromEntries(Object.entries(mapping).reverse()) as HostMapping,
  );
  await ctx.editor.save();
  assert.equal(ctx.calls.length, 1);
  assert.equal(ctx.saved, 1);
});

for (const host of [
  "bad!host.test",
  "bad%2etest",
  "bad..test",
  "-bad.test",
  "bad-.test",
  "a".repeat(64) + ".test",
]) {
  test(`invalid domain ${host} cannot be saved`, async () => {
    const ctx = setup();
    ctx.open();
    ctx.editor.rows.value[0]!.host = host;
    await ctx.editor.save();
    assert.equal(ctx.calls.length, 0);
    assert.match(ctx.editor.errors.value[0]!.host, /invalidHost/);
  });
}

for (const host of [
  "localhost",
  "*.example.test",
  "internal_app.test",
  "192.0.2.10",
  "[2001:db8::1]",
  "bücher.test",
]) {
  test(`valid host ${host} remains supported`, async () => {
    const ctx = setup();
    ctx.open();
    ctx.editor.rows.value[0]!.host = host;
    await ctx.editor.save();
    assert.equal(ctx.saved, 1);
    assert.equal(ctx.mappings.value[0]!.host, host);
  });
}
