import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { PanelConnection } from "../src/lib/api/panel-sync-api";
import {
  composePanelEndpointUrl,
  createPanelSyncForm,
  isPanelAutoSyncReady,
  nextPanelConnectionName,
  panelApiPaths,
  panelConnectionToForm,
  panelFormToUpdate,
  splitPanelEndpointUrl,
} from "../src/views/panel-sync/panel-sync-model";

const connection = {
  id: "connection-1",
  name: "NAS",
  provider: "one_nav",
  base_url: "https://nav.example.test",
  api_path: panelApiPaths.one_nav,
  allow_invalid_tls: false,
  grouping: {
    mode: "mirror",
    namespace: "fn-knock",
    single_group_name: "",
  },
  auto_sync: { enabled: false, interval_minutes: 60 },
  credential_configured: true,
  verified_at: "2026-08-19T00:00:00Z",
  verified_version: null,
  created_at: "2026-08-19T00:00:00Z",
  updated_at: "2026-08-19T00:00:00Z",
  last_run: null,
  next_sync_at: null,
} satisfies PanelConnection;

describe("panel synchronization editor model", () => {
  it("uses an automatic unique name and enables automation by default", () => {
    const form = createPanelSyncForm(["Sun-Panel-1", "Sun-Panel-3"]);
    assert.equal(form.name, "Sun-Panel-2");
    assert.equal(form.endpoint_url, "");
    assert.equal(form.auto_sync.enabled, true);
    assert.equal(form.auto_sync.interval_minutes, 60);
    assert.equal(form.grouping.namespace, "fn-knock");
    assert.equal(nextPanelConnectionName("one_nav", ["OneNav-1"]), "OneNav-2");
  });

  it("combines and splits the complete API endpoint URL", () => {
    const endpoint = composePanelEndpointUrl(
      "https://nav.example.test",
      "/index.php?c=api",
    );
    assert.equal(endpoint, "https://nav.example.test/index.php?c=api");
    assert.deepEqual(splitPanelEndpointUrl(endpoint), {
      base_url: "https://nav.example.test",
      api_path: "/index.php?c=api",
    });
  });

  it("never copies a stored credential back into an edit form", () => {
    const form = panelConnectionToForm(connection);
    assert.equal(form.credential, "");
    assert.equal(form.clear_credential, false);
    assert.equal(panelFormToUpdate(form, connection).credential, undefined);
  });

  it("requires explicit credential clearing and blocks automation while clearing", () => {
    const form = panelConnectionToForm(connection);
    assert.equal(isPanelAutoSyncReady(connection, form), true);
    form.clear_credential = true;
    const update = panelFormToUpdate(form, connection);
    assert.equal(update.clear_credential, true);
    assert.equal(isPanelAutoSyncReady(connection, form), false);
  });

  it("keeps the provider immutable by omitting it from update payloads", () => {
    const update = panelFormToUpdate(
      panelConnectionToForm(connection),
      connection,
    );
    assert.equal("provider" in update, false);
  });

  it("preserves stored endpoint fields when the effective URL is unchanged", () => {
    const proxied = {
      ...connection,
      base_url: "https://nav.example.test/reverse-proxy",
      api_path: "/index.php?c=api",
    };
    const update = panelFormToUpdate(panelConnectionToForm(proxied), proxied);
    assert.equal(update.base_url, proxied.base_url);
    assert.equal(update.api_path, proxied.api_path);
  });

  it("requires another test after endpoint or credential changes", () => {
    const form = panelConnectionToForm(connection);
    form.endpoint_url = "https://other.example.test/index.php?c=api";
    assert.equal(isPanelAutoSyncReady(connection, form), false);

    form.endpoint_url = composePanelEndpointUrl(
      connection.base_url,
      connection.api_path,
    );
    form.credential = "replacement-token";
    assert.equal(isPanelAutoSyncReady(connection, form), false);
  });
});
