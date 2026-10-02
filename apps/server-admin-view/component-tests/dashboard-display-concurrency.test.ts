import { flushPromises } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { AppConfig, DashboardDisplayConfig } from "../src/types";
import { DEFAULT_SIDEBAR_MENU_ORDER } from "../src/views/layout/sidebarNavigation";

const api = vi.hoisted(() => ({
  getConfig: vi.fn(),
  updateDashboardDisplayConfig: vi.fn(),
}));
vi.mock("@/lib/api/config", () => ({ ConfigAPI: api }));
import { useConfigStore } from "../src/store/config";
import { useDateTimeDisplayState } from "@admin-shared/composables/useDateTimeDisplayState";

const display = (): DashboardDisplayConfig => ({
  sidebar_collapsed: false,
  show_entry_status_module: true,
  show_console_app_list: false,
  sidebar_menu_order: [...DEFAULT_SIDEBAR_MENU_ORDER],
  date_time_display_mode: "human_friendly",
});
const snapshot = (preferences: DashboardDisplayConfig) => ({
  config: {
    dashboard_display: preferences,
    host_mappings: [],
  } as unknown as AppConfig,
  hostMappingsRevision: null,
  hostMappingCatalogRevision: null,
});
const deferred = <T>() => {
  let resolve!: (value: T) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<T>((done, fail) => {
    resolve = done;
    reject = fail;
  });
  return { promise, resolve, reject };
};

beforeEach(() => {
  vi.resetAllMocks();
  setActivePinia(createPinia());
  const store = useConfigStore();
  store.config = snapshot(display()).config;
  store.isLoading = false;
});

describe("dashboard display save concurrency", () => {
  it("serializes partial saves so older responses cannot replace newer preferences", async () => {
    const first = deferred<DashboardDisplayConfig>();
    const store = useConfigStore();
    api.updateDashboardDisplayConfig
      .mockReturnValueOnce(first.promise)
      .mockResolvedValueOnce({
        ...display(),
        sidebar_collapsed: true,
        date_time_display_mode: "full",
      });
    const collapse = store.saveDashboardDisplayConfig({
      sidebar_collapsed: true,
    });
    const dateMode = store.saveDashboardDisplayConfig({
      date_time_display_mode: "full",
    });
    await flushPromises();
    expect(api.updateDashboardDisplayConfig).toHaveBeenCalledTimes(1);
    first.resolve({ ...display(), sidebar_collapsed: true });
    await Promise.all([collapse, dateMode]);
    expect(api.updateDashboardDisplayConfig).toHaveBeenNthCalledWith(2, {
      date_time_display_mode: "full",
    });
    expect(store.config!.dashboard_display).toMatchObject({
      sidebar_collapsed: true,
      date_time_display_mode: "full",
    });
  });

  it("does not let a stale config read undo a completed display save", async () => {
    const read = deferred<ReturnType<typeof snapshot>>();
    const store = useConfigStore();
    api.getConfig.mockReturnValueOnce(read.promise);
    const reload = store.loadConfig({ force: true });
    api.updateDashboardDisplayConfig.mockResolvedValueOnce({
      ...display(),
      sidebar_collapsed: true,
      date_time_display_mode: "full",
    });
    await store.saveDashboardDisplayConfig({
      sidebar_collapsed: true,
      date_time_display_mode: "full",
    });
    read.resolve(snapshot(display()));
    await reload;
    expect(store.config!.dashboard_display).toMatchObject({
      sidebar_collapsed: true,
      date_time_display_mode: "full",
    });
    expect(useDateTimeDisplayState().dateTimeDisplayMode.value).toBe("full");
  });

  it("allows a fresh server read to apply another client's preference", async () => {
    const store = useConfigStore();
    api.updateDashboardDisplayConfig.mockResolvedValueOnce({
      ...display(),
      sidebar_collapsed: true,
    });
    await store.saveDashboardDisplayConfig({ sidebar_collapsed: true });
    api.getConfig.mockResolvedValueOnce(snapshot(display()));
    await store.loadConfig({ force: true });
    expect(store.config!.dashboard_display!.sidebar_collapsed).toBe(false);
  });

  it("continues later partial saves after an earlier save fails", async () => {
    const store = useConfigStore();
    api.updateDashboardDisplayConfig
      .mockRejectedValueOnce(new Error("save failed"))
      .mockResolvedValueOnce({ ...display(), date_time_display_mode: "full" });
    const failed = store.saveDashboardDisplayConfig({
      sidebar_collapsed: true,
    });
    const next = store.saveDashboardDisplayConfig({
      date_time_display_mode: "full",
    });
    await expect(failed).rejects.toThrow("save failed");
    await next;
    expect(store.config!.dashboard_display).toMatchObject({
      sidebar_collapsed: false,
      date_time_display_mode: "full",
    });
  });
});
