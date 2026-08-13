import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { defineComponent, h } from "vue";
import { createI18n } from "vue-i18n";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { AppConfig } from "../src/types";

const api = vi.hoisted(() => ({
  drainWafEvents: vi.fn(),
  getGatewayDates: vi.fn(),
  getGatewayEntries: vi.fn(),
  getTotpStatus: vi.fn(),
  getWafLogs: vi.fn(),
}));

vi.mock("../src/lib/api/config", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../src/lib/api/config")>();
  return {
    ...actual,
    ConfigAPI: {
      ...actual.ConfigAPI,
      getTOTPStatus: api.getTotpStatus,
    },
  };
});

vi.mock("../src/lib/api/gateway", async (importOriginal) => {
  const actual = await importOriginal<
    typeof import("../src/lib/api/gateway")
  >();
  return {
    ...actual,
    GatewayLogsAPI: {
      ...actual.GatewayLogsAPI,
      getDates: api.getGatewayDates,
      getEntries: api.getGatewayEntries,
    },
    WAFAPI: {
      ...actual.WAFAPI,
      drainEvents: api.drainWafEvents,
      getLogs: api.getWafLogs,
    },
  };
});

vi.mock("../src/composables/useIpLocationBatch", () => ({
  useIpLocationBatch: () => ({
    getSnapshot: vi.fn(),
    trackIps: vi.fn(),
  }),
}));

vi.mock("vue-router", async (importOriginal) => {
  const actual = await importOriginal<typeof import("vue-router")>();
  return {
    ...actual,
    useRoute: () => ({ query: {} }),
  };
});

import { useConfigStore } from "../src/store/config";
import { useGatewayRequestLogsResource } from "../src/views/gateway-request-logs/useGatewayRequestLogsResource";
import { useWafLogsResource } from "../src/views/waf-logs/useWafLogsResource";

const deferred = <T>() => {
  let resolve: ((value: T) => void) | undefined;
  const promise = new Promise<T>((next) => {
    resolve = next;
  });
  return {
    promise,
    resolve: (value: T) => resolve?.(value),
  };
};

const mountResource = <T>(setupResource: () => T) => {
  let resource: T | undefined;
  const pinia = createPinia();
  setActivePinia(pinia);
  useConfigStore().config = {} as AppConfig;
  const component = defineComponent({
    setup() {
      resource = setupResource();
      return () => h("div");
    },
  });
  const i18n = createI18n({
    legacy: false,
    locale: "en",
    missingWarn: false,
    fallbackWarn: false,
  });
  const wrapper = mount(component, { global: { plugins: [pinia, i18n] } });
  if (!resource) throw new Error("resource setup did not run");
  return { resource, wrapper };
};

beforeEach(() => {
  vi.clearAllMocks();
  api.drainWafEvents.mockResolvedValue({});
  api.getGatewayDates.mockResolvedValue({
    dates: ["2026-08-14"],
    logs_dir: "",
    today: "2026-08-14",
  });
  api.getGatewayEntries.mockResolvedValue({
    available_dates: ["2026-08-14"],
    date: "2026-08-14",
    items: [],
    logs_dir: "",
    next_cursor: "",
  });
  api.getTotpStatus.mockResolvedValue({ credentials: [] });
  api.getWafLogs.mockResolvedValue({
    available_dates: ["2026-08-14"],
    date: "2026-08-14",
    items: [],
    next_cursor: "",
  });
});

describe("latest log requests", () => {
  it("keeps a slow gateway response from overwriting a newer filter", async () => {
    const { resource, wrapper } = mountResource(
      useGatewayRequestLogsResource,
    );
    await flushPromises();
    const first = deferred<Record<string, unknown>>();
    const second = deferred<Record<string, unknown>>();
    api.getGatewayEntries
      .mockImplementationOnce(() => first.promise)
      .mockImplementationOnce(() => second.promise);

    resource.searchQuery.value = "first";
    const firstLoad = resource.handleSearch();
    resource.searchQuery.value = "second";
    const secondLoad = resource.handleSearch();
    const newestEntry = { id: "newest" };
    second.resolve({
      available_dates: ["2026-08-14"],
      date: "2026-08-14",
      items: [newestEntry],
      logs_dir: "",
      next_cursor: "",
    });
    await secondLoad;
    first.resolve({
      available_dates: ["2026-08-14"],
      date: "2026-08-14",
      items: [{ id: "stale" }],
      logs_dir: "",
      next_cursor: "",
    });
    await firstLoad;

    expect(resource.entries.value).toEqual([newestEntry]);
    wrapper.unmount();
  });

  it("does not drop a newer WAF filter while another request is loading", async () => {
    const { resource, wrapper } = mountResource(useWafLogsResource);
    await flushPromises();
    const first = deferred<Record<string, unknown>>();
    const second = deferred<Record<string, unknown>>();
    api.getWafLogs
      .mockImplementationOnce(() => first.promise)
      .mockImplementationOnce(() => second.promise);

    const firstLoad = resource.handleDateChange("2026-08-13");
    const secondLoad = resource.handleDateChange("2026-08-14");
    const newestEntry = { id: "newest" };
    second.resolve({
      available_dates: ["2026-08-14"],
      date: "2026-08-14",
      items: [newestEntry],
      next_cursor: "",
    });
    await secondLoad;
    first.resolve({
      available_dates: ["2026-08-13"],
      date: "2026-08-13",
      items: [{ id: "stale" }],
      next_cursor: "",
    });
    await firstLoad;

    expect(resource.entries.value).toEqual([newestEntry]);
    wrapper.unmount();
  });
});
