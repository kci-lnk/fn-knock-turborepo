import { flushPromises, mount } from "@vue/test-utils";
import { defineComponent, h, ref } from "vue";
import { createI18n } from "vue-i18n";
import { beforeEach, describe, expect, it, vi } from "vitest";

const api = vi.hoisted(() => ({
  getPathWhitelist: vi.fn(),
  updatePathWhitelist: vi.fn(),
  resolveFalsePositive: vi.fn(),
}));
const toast = vi.hoisted(() => ({ error: vi.fn(), success: vi.fn() }));

vi.mock("@/lib/api/security", () => ({
  ScannerAPI: {
    getPathWhitelist: api.getPathWhitelist,
    updatePathWhitelist: api.updatePathWhitelist,
    resolveFalsePositive: api.resolveFalsePositive,
  },
}));
vi.mock("@admin-shared/utils/toast", () => ({ toast }));

import type { ScannerBlacklistRecord } from "../src/lib/api/security";
import BlacklistHitsTable from "@admin-shared/components/session/BlacklistHitsTable.vue";
import { useScannerFalsePositive } from "../src/views/session-management/useScannerFalsePositive";
import { useScannerPathWhitelistSettings } from "../src/views/system-settings/scanner-path-whitelist/useScannerPathWhitelistSettings";

const i18n = () =>
  createI18n({
    legacy: false,
    locale: "en",
    missingWarn: false,
    fallbackWarn: false,
    messages: { en: {} },
  });

const mountWhitelistHook = () => {
  let model!: ReturnType<typeof useScannerPathWhitelistSettings>;
  const harness = defineComponent({
    setup() {
      model = useScannerPathWhitelistSettings();
      return () => h("div");
    },
  });
  const wrapper = mount(harness, { global: { plugins: [i18n()] } });
  return { model, wrapper };
};

describe("scanner path whitelist settings hook", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    api.getPathWhitelist.mockResolvedValue({
      paths: ["/saved"],
      defaultPaths: ["/", "/default"],
    });
    api.updatePathWhitelist.mockImplementation(
      async ({ paths }: { paths: string[] }) => ({
        paths,
        defaultPaths: ["/", "/default"],
      }),
    );
  });

  it("loads, edits, validates, saves, restores, and discards one draft", async () => {
    const { model, wrapper } = mountWhitelistHook();
    expect(model.isLoading).toBe(true);
    await flushPromises();
    expect(model.entries.map((entry) => entry.value)).toEqual(["/saved"]);
    expect(model.isDirty).toBe(false);

    model.addEntry();
    const added = model.entries.at(-1)!;
    expect(model.entryErrors[added.id]).toBeTruthy();
    await model.saveSettings();
    expect(api.updatePathWhitelist).not.toHaveBeenCalled();

    model.setEntryPath(added.id, "/custom/?source=test#section");
    expect(model.isDirty).toBe(true);
    await model.saveSettings();
    expect(api.updatePathWhitelist).toHaveBeenCalledWith({
      paths: ["/saved", "/custom"],
    });
    expect(model.isDirty).toBe(false);

    model.restoreDefaults();
    expect(model.entries.map((entry) => entry.value)).toEqual([
      "/",
      "/default",
    ]);
    expect(model.isDirty).toBe(true);
    expect(api.updatePathWhitelist).toHaveBeenCalledTimes(1);
    model.discardChanges();
    expect(model.entries.map((entry) => entry.value)).toEqual([
      "/saved",
      "/custom",
    ]);
    expect(model.isDirty).toBe(false);
    wrapper.unmount();
  });

  it("exposes a recoverable load error instead of an empty editable draft", async () => {
    api.getPathWhitelist.mockRejectedValueOnce(new Error("load failed"));
    const { model, wrapper } = mountWhitelistHook();
    await flushPromises();

    expect(model.hasSettings).toBe(false);
    expect(model.loadError).toBe("load failed");
    expect(model.entries).toEqual([]);

    api.getPathWhitelist.mockResolvedValueOnce({
      paths: ["/recovered"],
      defaultPaths: ["/"],
    });
    await model.fetchSettings();
    expect(model.loadError).toBe("");
    expect(model.hasSettings).toBe(true);
    expect(model.entries.map((entry) => entry.value)).toEqual(["/recovered"]);
    wrapper.unmount();
  });
});

type FalsePositiveHarness = ReturnType<typeof useScannerFalsePositive>;

const mountFalsePositiveHook = (fetchBlacklist = vi.fn(async () => {})) => {
  const record = ref<ScannerBlacklistRecord | null>({
    ip: "203.0.113.20",
    blockedAt: Date.now(),
    windowMinutes: 5,
    threshold: 3,
    hits: [],
  });
  const open = ref(true);
  const selectedIps = ref(new Set(["203.0.113.20", "203.0.113.21"]));
  const clearSelection = vi.fn(() => {
    selectedIps.value = new Set();
  });
  let model!: FalsePositiveHarness;
  const harness = defineComponent({
    setup() {
      model = useScannerFalsePositive({
        clearSelection,
        detailRecord: record,
        isDetailsModalOpen: open,
        fetchBlacklist,
      });
      return () => h("div");
    },
  });
  const wrapper = mount(harness, { global: { plugins: [i18n()] } });
  return {
    clearSelection,
    fetchBlacklist,
    model,
    open,
    record,
    selectedIps,
    wrapper,
  };
};

describe("scanner false-positive hook", () => {
  beforeEach(() => vi.clearAllMocks());

  it("prevents duplicate requests and clears the detail after unblocking", async () => {
    let finish!: (value: unknown) => void;
    api.resolveFalsePositive.mockReturnValue(
      new Promise((resolve) => {
        finish = resolve;
      }),
    );
    const context = mountFalsePositiveHook();

    const pending = context.model.resolveFalsePositive("/legitimate/");
    void context.model.resolveFalsePositive("/legitimate/");
    expect(api.resolveFalsePositive).toHaveBeenCalledTimes(1);
    expect(api.resolveFalsePositive).toHaveBeenCalledWith({
      ip: "203.0.113.20",
      path: "/legitimate/",
    });
    expect(context.model.isResolvingFalsePositive.value).toBe(true);

    finish({
      ip: "203.0.113.20",
      path: "/legitimate",
      added: true,
      unblocked: true,
    });
    await pending;
    expect(context.open.value).toBe(false);
    expect(context.record.value).toBeNull();
    expect(context.selectedIps.value.size).toBe(0);
    expect(context.clearSelection).toHaveBeenCalledOnce();
    expect(context.fetchBlacklist).toHaveBeenCalledOnce();
    context.wrapper.unmount();
  });

  it("retains the detail when the request fails", async () => {
    api.resolveFalsePositive.mockRejectedValue(new Error("failed"));
    const context = mountFalsePositiveHook();
    await context.model.resolveFalsePositive("/legitimate");
    expect(context.open.value).toBe(true);
    expect(context.record.value?.ip).toBe("203.0.113.20");
    expect(context.fetchBlacklist).not.toHaveBeenCalled();
    expect(toast.error).toHaveBeenCalledOnce();
    context.wrapper.unmount();
  });
});

describe("blacklist hit table actions", () => {
  const rows = [
    {
      key: "one",
      time: "2026-08-19 02:00",
      path: "/legitimate",
      interval: "1s",
    },
  ];

  it("adds an action column only when the caller supplies the slot", () => {
    const plain = mount(BlacklistHitsTable, {
      props: { rows },
      global: { plugins: [i18n()] },
    });
    expect(plain.findAll("th")).toHaveLength(3);
    plain.unmount();

    const actionable = mount(BlacklistHitsTable, {
      props: { rows },
      slots: {
        action: ({ row }: { row: (typeof rows)[number] }) =>
          h("button", { "data-path": row.path }, "Allow"),
      },
      global: { plugins: [i18n()] },
    });
    expect(actionable.findAll("th")).toHaveLength(4);
    expect(actionable.get("button").attributes("data-path")).toBe(
      "/legitimate",
    );
    actionable.unmount();
  });
});
