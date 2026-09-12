import { flushPromises, mount } from "@vue/test-utils";
import { createI18n } from "vue-i18n";
import { afterEach, describe, expect, it, vi } from "vitest";
import { SystemAPI } from "../src/lib/api/system";
import Page from "../src/views/system-settings/FnosCertificateSyncSettings.vue";
import type {
  FnosCertificateSyncDetails,
  FnosCertificateSyncItem,
} from "../src/types";
import { enAdmin } from "../../../packages/i18n/src/messages/admin/en";

vi.mock("@admin-shared/utils/toast", () => ({
  toast: { error: vi.fn(), success: vi.fn() },
}));
const wrappers: ReturnType<typeof mount>[] = [];
afterEach(() => {
  wrappers.splice(0).forEach((w) => w.unmount());
  vi.restoreAllMocks();
});
function item(
  action: FnosCertificateSyncItem["action"],
  status: FnosCertificateSyncItem["status"],
): FnosCertificateSyncItem {
  return {
    action,
    status,
    action_id: `${action}:1`,
    target_id: action === "create" ? "" : "1",
    domain: `${action}.example.test`,
    san: [],
    managed: action !== "create",
    source_ids: ["source"],
    references: status === "delete_blocked" ? ["gateway: example.test"] : [],
    source: "upload",
    renewal: false,
    valid_from: null,
    valid_to: null,
    fingerprint: null,
    reason: status === "delete_blocked" ? "gateway: example.test" : null,
    local: null,
  };
}
async function setup() {
  const details: FnosCertificateSyncDetails = {
    snapshot_version: "revision-1",
    availability: { available: true, reason: null },
    config: { auto_sync_enabled: false },
    runtime: {
      running: false,
      last_sync_at: null,
      last_result: null,
      last_error: null,
      failed_target_ids: [],
    },
    summary: {
      total: 3,
      syncable: 2,
      up_to_date: 0,
      create: 1,
      update: 0,
      delete: 1,
      adopt: 0,
    },
    certificates: [
      item("create", "pending_create"),
      item("delete", "pending_delete"),
      item("none", "delete_blocked"),
    ],
  };
  vi.spyOn(SystemAPI, "getFnosCertificateSyncDetails").mockResolvedValue(
    details,
  );
  vi.spyOn(SystemAPI, "syncFnosCertificates").mockResolvedValue({
    details,
    summary: {
      synced: 2,
      skipped: 0,
      failed: 0,
      rolled_back: false,
      created: 1,
      updated: 0,
      deleted: 1,
      adopted: 0,
    },
  });
  const wrapper = mount(Page, {
    global: {
      plugins: [
        createI18n({
          legacy: false,
          locale: "en",
          messages: { en: { admin: enAdmin } },
        }),
      ],
    },
  });
  wrappers.push(wrapper);
  await flushPromises();
  return wrapper;
}
describe("fnOS certificate lifecycle", () => {
  it("submits visible actions and the snapshot revision for sync all", async () => {
    const wrapper = await setup();
    const button = wrapper
      .findAll("button")
      .find((b) => b.text().includes("(2)"))!;
    await button.trigger("click");
    await flushPromises();
    expect(SystemAPI.syncFnosCertificates).toHaveBeenCalledWith(
      ["create:1", "delete:1"],
      "revision-1",
    );
  });
  it("creates without a target ID and disables deletion of a referenced certificate", async () => {
    const wrapper = await setup();
    const rows = wrapper.findAll("tbody tr");
    await rows[0]!.get("button").trigger("click");
    await flushPromises();
    expect(SystemAPI.syncFnosCertificates).toHaveBeenCalledWith(
      ["create:1"],
      "revision-1",
    );
    expect(rows[2]!.get("button").attributes("disabled")).toBeDefined();
    expect(rows[2]!.text()).toContain("gateway: example.test");
  });
  it("refreshes a stale preview after a rejected request", async () => {
    const wrapper = await setup();
    vi.mocked(SystemAPI.syncFnosCertificates).mockRejectedValueOnce(
      new Error("stale certificate synchronization plan"),
    );
    await wrapper.findAll("tbody tr")[0]!.get("button").trigger("click");
    await flushPromises();
    expect(SystemAPI.getFnosCertificateSyncDetails).toHaveBeenCalledTimes(2);
  });
});
