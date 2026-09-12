import { flushPromises, mount } from "@vue/test-utils";
import { defineComponent, h, KeepAlive, ref } from "vue";
import { beforeEach, describe, expect, it, vi } from "vitest";
import CursorPaginationDock from "../src/components/CursorPaginationDock.vue";

const mocks = vi.hoisted(() => ({
  getLogs: vi.fn(),
  poll: null as null | ((signal: AbortSignal) => Promise<void>),
  error: vi.fn(),
  query: {} as Record<string, string>,
  config: {} as object | null,
  loadConfig: vi.fn(),
  start: vi.fn(),
  stop: vi.fn(),
}));
vi.mock("vue-i18n", () => ({ useI18n: () => ({ t: (key: string) => key }) }));
vi.mock("vue-router", () => ({ useRoute: () => ({ query: mocks.query }) }));
vi.mock("../src/store/config", () => ({
  useConfigStore: () => ({
    config: mocks.config,
    loadConfig: mocks.loadConfig,
  }),
}));
vi.mock("../src/lib/api/gateway", () => ({
  WAFAPI: { getLogs: mocks.getLogs, drainEvents: vi.fn(), deleteLogs: vi.fn() },
}));
vi.mock("../src/composables/useIpLocationBatch", () => ({
  useIpLocationBatch: () => ({ trackIps: vi.fn(), getSnapshot: vi.fn() }),
}));
vi.mock("@admin-shared/utils/toast", () => ({ toast: { error: mocks.error } }));
vi.mock("../src/composables/useVisibilityPolling", () => ({
  createVisibilityPoller: (options: { task: typeof mocks.poll }) => {
    mocks.poll = options.task;
    return { start: mocks.start, stop: mocks.stop };
  },
}));
import { useWafLogsResource } from "../src/views/waf-logs/useWafLogsResource";

const page = (id: string, next = "") => ({
  items: [{ trace_id: id }],
  next_cursor: next,
  available_dates: ["2026-09-12"],
  date: "2026-09-12",
});
async function setup() {
  mocks.getLogs.mockResolvedValueOnce(page("first", "50"));
  let resource!: ReturnType<typeof useWafLogsResource>;
  const wrapper = mount(
    defineComponent({
      setup() {
        resource = useWafLogsResource();
        return () => h("div");
      },
    }),
  );
  await flushPromises();
  return { resource, wrapper };
}
beforeEach(() => {
  vi.clearAllMocks();
  mocks.config = {};
  mocks.query = {};
  mocks.getLogs.mockReset();
});
describe("WAF pagination", () => {
  it("lets paging supersede a silent refresh and ignores its stale result", async () => {
    const { resource: r, wrapper } = await setup();
    let finish!: (value: ReturnType<typeof page>) => void;
    mocks.getLogs.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          finish = resolve;
        }),
    );
    const background = mocks.poll!(new AbortController().signal);
    expect(r.loading.value).toBe(false);
    mocks.getLogs.mockResolvedValueOnce(page("second", "100"));
    await r.handleLoadOlder();
    expect(mocks.getLogs.mock.lastCall?.[0].cursor).toBe("50");
    finish(page("stale", "50"));
    await background;
    expect(r.entries.value[0]?.trace_id).toBe("second");
    expect(r.currentCursor.value).toBe("50");
    expect(r.loading.value).toBe(false);
    wrapper.unmount();
  });
  it("restores failed navigation, retries, returns to first and resets filters", async () => {
    const { resource: r, wrapper } = await setup();
    mocks.getLogs.mockRejectedValueOnce(new Error("offline"));
    await r.handleLoadOlder();
    expect(r.currentCursor.value).toBe("");
    expect(r.cursorHistory.value).toEqual([]);
    expect(r.entries.value[0]?.trace_id).toBe("first");
    expect(r.canLoadOlder.value).toBe(true);
    expect(mocks.error).toHaveBeenCalled();
    mocks.getLogs.mockResolvedValueOnce(page("second"));
    await r.handleLoadOlder();
    expect(r.canLoadOlder.value).toBe(false);
    expect(r.canLoadNewer.value).toBe(true);
    mocks.getLogs.mockResolvedValueOnce(page("first", "50"));
    await r.handleLoadNewer();
    expect(r.currentCursor.value).toBe("");
    mocks.getLogs.mockResolvedValueOnce(page("second"));
    await r.handleLoadOlder();
    mocks.getLogs.mockResolvedValueOnce(page("first", "50"));
    await r.handleLoadFirst();
    expect(r.cursorHistory.value).toEqual([]);
    mocks.getLogs.mockResolvedValue(page("filtered"));
    r.searchQuery.value = "test";
    await r.handleSearch();
    await r.handleDateChange("2026-09-11");
    await r.handleLimitChange("20");
    expect(mocks.getLogs.mock.lastCall?.[0]).toMatchObject({
      cursor: undefined,
      limit: "20",
      search: "test",
    });
    wrapper.unmount();
  });
  it("wires inline and floating controls to the same callbacks", async () => {
    const next = vi.fn();
    const wrapper = mount(CursorPaginationDock, {
      props: {
        canLoadNewer: false,
        canLoadOlder: true,
        cursorPageLabel: "Page 1",
        handleLimitChange: vi.fn(),
        handleLoadFirst: vi.fn(),
        handleLoadNewer: vi.fn(),
        handleLoadOlder: next,
        labels: {
          ariaLabel: "logs",
          canLoadOlder: "more",
          firstPage: "first",
          lastPage: "last",
          nextPage: "next",
          pageSize: "size",
          pageSizeOption: (v: string) => v,
          previousPage: "previous",
        },
        limit: "50",
        limitOptions: ["50"],
        loading: false,
        shouldFloat: true,
      },
      global: {
        stubs: {
          FloatingActionDock: {
            template: '<div><slot name="inline"/><slot name="floating"/></div>',
          },
        },
      },
    });
    const buttons = wrapper
      .findAll("button")
      .filter(
        (button) =>
          button.text().includes("next") ||
          button.attributes("aria-label") === "next",
      );
    expect(buttons).toHaveLength(2);
    for (const button of buttons) await button.trigger("click");
    expect(next).toHaveBeenCalledTimes(2);
    await wrapper.setProps({ loading: true });
    for (const button of buttons)
      expect(button.attributes("disabled")).toBeDefined();
    wrapper.unmount();
  });
});

it("does not poll before configuration is ready or fetch after unmount", async () => {
  mocks.config = null;
  let finish!: () => void;
  mocks.loadConfig.mockImplementationOnce(
    () =>
      new Promise<void>((resolve) => {
        finish = resolve;
      }),
  );
  const child = defineComponent({
    setup() {
      useWafLogsResource();
      return () => h("div");
    },
  });
  const wrapper = mount(
    defineComponent({
      setup: () => () => h(KeepAlive, null, { default: () => h(child) }),
    }),
  );
  await flushPromises();
  expect(mocks.start).not.toHaveBeenCalled();
  wrapper.unmount();
  mocks.getLogs.mockClear();
  finish();
  await flushPromises();
  expect(mocks.getLogs).not.toHaveBeenCalled();
});
it("pauses a cached WAF page and restarts polling only when reactivated", async () => {
  mocks.getLogs.mockResolvedValue(page("first", "50"));
  const active = ref(true);
  const child = defineComponent({
    setup() {
      useWafLogsResource();
      return () => h("div");
    },
  });
  const wrapper = mount(
    defineComponent({
      setup: () => () =>
        h(KeepAlive, null, { default: () => (active.value ? h(child) : null) }),
    }),
  );
  await flushPromises();
  expect(mocks.start).toHaveBeenCalledTimes(1);
  active.value = false;
  await flushPromises();
  expect(mocks.stop).toHaveBeenCalledTimes(1);
  active.value = true;
  await flushPromises();
  expect(mocks.start).toHaveBeenCalledTimes(2);
  wrapper.unmount();
});

it("does not consume trace retries while a foreground request is pending", async () => {
  mocks.query = { trace_id: "trace-1" };
  let finish!: (value: ReturnType<typeof page>) => void;
  mocks.getLogs.mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        finish = resolve;
      }),
  );
  const wrapper = mount(
    defineComponent({
      setup() {
        useWafLogsResource();
        return () => h("div");
      },
    }),
  );
  await flushPromises();
  const controller = new AbortController();
  for (let i = 0; i < 13; i++) await mocks.poll!(controller.signal);
  finish({ ...page(""), items: [] });
  await flushPromises();
  mocks.getLogs.mockResolvedValue({ ...page(""), items: [] });
  await mocks.poll!(controller.signal);
  expect(mocks.getLogs).toHaveBeenCalledTimes(2);
  wrapper.unmount();
});
