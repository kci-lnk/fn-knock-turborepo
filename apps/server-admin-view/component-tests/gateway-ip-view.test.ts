import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { defineComponent, h } from "vue";
import { createI18n } from "vue-i18n";
import { createMemoryHistory, createRouter } from "vue-router";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { AppConfig } from "../src/types";
import type { GatewayLogIpGroups } from "../src/lib/api/gateway";

const api = vi.hoisted(() => ({
  dates: vi.fn(),
  entries: vi.fn(),
  groups: vi.fn(),
  credentials: vi.fn(),
  deleteDate: vi.fn(),
}));
vi.mock("../src/lib/api/config", () => ({
  ConfigAPI: { getTOTPStatus: api.credentials },
}));
vi.mock("../src/lib/api/gateway", () => ({
  GatewayLogsAPI: {
    getDates: api.dates,
    getEntries: api.entries,
    getIpGroups: api.groups,
    deleteDate: api.deleteDate,
  },
}));
vi.mock("../src/composables/useIpLocationBatch", () => ({
  useIpLocationBatch: () => ({ getSnapshot: vi.fn(), trackIps: vi.fn() }),
}));

import { useConfigStore } from "../src/store/config";
import { useGatewayRequestLogsResource } from "../src/views/gateway-request-logs/useGatewayRequestLogsResource";
import { useGatewayLogIpPresentation } from "../src/views/gateway-request-logs/useGatewayLogIpPresentation";
import GatewayLogIpGroupsView from "../src/views/gateway-request-logs/GatewayLogIpGroups.vue";
import { formatLogClock } from "../src/views/gateway-request-logs/ip-view-model";

const groupData = (page = 1): GatewayLogIpGroups => ({
  date: "2026-09-21",
  page,
  limit: 20,
  total: 45,
  total_ips: 44,
  total_requests: 501,
  items: [
    {
      client_ip: "36.142.108.111",
      requests: 301,
      hosts: ["example.com"],
      client_errors: 4,
      server_errors: 0,
      waf_hits: 2,
      first_seen: "2026-09-21T10:00:00+08:00",
      last_seen: "2026-09-21T14:32:08+08:00",
    },
  ],
});
const i18n = () =>
  createI18n({
    legacy: false,
    locale: "en",
    missingWarn: false,
    fallbackWarn: false,
  });
const setup = async (url = "/logs") => {
  const pinia = createPinia();
  setActivePinia(pinia);
  useConfigStore().config = {} as AppConfig;
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [{ path: "/logs", component: { template: "<div/>" } }],
  });
  await router.push(url);
  await router.isReady();
  let resource!: ReturnType<typeof useGatewayRequestLogsResource>;
  let presentation!: ReturnType<typeof useGatewayLogIpPresentation>;
  const wrapper = mount(
    defineComponent({
      setup() {
        resource = useGatewayRequestLogsResource();
        presentation = useGatewayLogIpPresentation(resource);
        return () => h("div", { ref: presentation.setRootRef });
      },
    }),
    { global: { plugins: [pinia, router, i18n()] } },
  );
  await flushPromises();
  return { resource, wrapper, router, presentation };
};

beforeEach(() => {
  localStorage.clear();
  vi.clearAllMocks();
  api.dates.mockResolvedValue({
    today: "2026-09-21",
    dates: ["2026-09-21", "2026-09-20"],
    logs_dir: "/logs",
  });
  api.credentials.mockResolvedValue({ credentials: [] });
  api.entries.mockImplementation(async (params) => ({
    date: params.date,
    available_dates: ["2026-09-21", "2026-09-20"],
    items: [],
    next_cursor: "",
    logs_dir: "/logs",
  }));
  api.groups.mockImplementation(async (params) => groupData(params.page));
});

describe("gateway IP view", () => {
  it("keeps the initial request view and remembers switching to IPs", async () => {
    const { resource, wrapper } = await setup();
    expect(api.groups).not.toHaveBeenCalled();
    await resource.setViewMode("ips");
    expect(api.groups).toHaveBeenLastCalledWith(
      expect.objectContaining({
        date: "2026-09-21",
        page: 1,
        sort: "requests",
      }),
    );
    await flushPromises();
    wrapper.unmount();
    const again = await setup();
    expect(again.resource.isIpOverview.value).toBe(true);
    again.wrapper.unmount();
  });

  it("drills down with exact IP plus inherited filters and restores the original page on browser back", async () => {
    const { resource, wrapper, router, presentation } = await setup();
    await resource.setViewMode("ips");
    resource.searchQuery.value = "/api";
    await resource.handleStatusChange("4xx");
    await resource.handleIpSortChange("last_seen");
    await resource.handleLoadOlder();
    expect(resource.ipGroups.value?.page).toBe(2);
    const overviewCalls = api.groups.mock.calls.length;
    wrapper.element.scrollTop = 240;
    presentation.handleOpenIp("36.142.108.111");
    await flushPromises();
    expect(wrapper.element.scrollTop).toBe(0);
    expect(resource.isIpOverview.value).toBe(false);
    expect(resource.detailRequests.value).toBe(301);
    expect(api.groups.mock.calls.length).toBe(overviewCalls);
    expect(api.entries).toHaveBeenLastCalledWith(
      expect.objectContaining({
        client_ip: "36.142.108.111",
        search: "/api",
        status: "4xx",
        date: "2026-09-21",
        cursor: undefined,
      }),
    );
    await resource.showAllIpRequests();
    expect(api.entries).toHaveBeenLastCalledWith(
      expect.objectContaining({
        client_ip: "36.142.108.111",
        search: undefined,
        status: undefined,
      }),
    );
    router.back();
    await flushPromises();
    expect(resource.isIpOverview.value).toBe(true);
    expect(resource.searchQuery.value).toBe("/api");
    expect(resource.selectedStatus.value).toBe("4xx");
    expect(resource.ipSort.value).toBe("last_seen");
    expect(resource.ipGroups.value?.page).toBe(2);
    expect(wrapper.element.scrollTop).toBe(240);
    expect(api.groups.mock.calls.length).toBe(overviewCalls + 1);
    wrapper.unmount();
  });

  it("honors direct-linked date and unknown source and supports returning without history", async () => {
    const { resource, wrapper, router } = await setup(
      "/logs?client_ip=unknown&log_date=2026-09-20",
    );
    expect(api.entries).toHaveBeenLastCalledWith(
      expect.objectContaining({ date: "2026-09-20", client_ip: "unknown" }),
    );
    await resource.backToIps();
    await flushPromises();
    expect(router.currentRoute.value.query.client_ip).toBeUndefined();
    expect(resource.isIpOverview.value).toBe(true);
    wrapper.unmount();
  });

  it("keeps the detail URL date in sync without losing the exact IP", async () => {
    const { resource, wrapper, router } = await setup(
      "/logs?client_ip=36.142.108.111&log_date=2026-09-21",
    );
    const before = api.entries.mock.calls.length;
    await resource.handleDateChange("2026-09-20");
    await flushPromises();
    expect(router.currentRoute.value.query.log_date).toBe("2026-09-20");
    expect(api.entries.mock.calls.length).toBe(before + 1);
    await router.push({
      query: { client_ip: "36.142.108.111", log_date: "2026-09-21" },
    });
    await flushPromises();
    expect(resource.selectedDate.value).toBe("2026-09-21");
    wrapper.unmount();
  });

  it("does not replace an empty today with yesterday's logs", async () => {
    api.dates.mockResolvedValueOnce({
      today: "2026-09-21",
      dates: ["2026-09-20"],
      logs_dir: "/logs",
    });
    const { resource, wrapper } = await setup();
    expect(resource.selectedDate.value).toBe("2026-09-21");
    expect(resource.availableDates.value).toContain("2026-09-21");
    wrapper.unmount();
  });

  it("does not let a slow aggregate replace newer filters and exposes a retryable failure", async () => {
    const { resource, wrapper } = await setup();
    let resolve!: (data: GatewayLogIpGroups) => void;
    api.groups.mockImplementationOnce(
      () =>
        new Promise<GatewayLogIpGroups>((r) => {
          resolve = r;
        }),
    );
    const first = resource.setViewMode("ips");
    await resource.handleStatusChange("5xx");
    resolve({ ...groupData(), total_requests: 999 });
    await first;
    expect(resource.ipGroups.value?.total_requests).toBe(501);
    api.groups.mockRejectedValueOnce(new Error("unavailable"));
    await resource.retry();
    expect(resource.loadError.value).toBe(true);
    expect(resource.ipGroups.value).toBeNull();
    await resource.retry();
    expect(resource.loadError.value).toBe(false);
    expect(resource.ipGroups.value?.total_requests).toBe(501);
    wrapper.unmount();
  });

  it("invalidates the overview after deleting the selected day", async () => {
    const { resource, wrapper, router } = await setup();
    await resource.setViewMode("ips");
    await resource.openIp("36.142.108.111");
    await flushPromises();
    api.deleteDate.mockResolvedValueOnce({
      deleted: true,
      available_dates: ["2026-09-20"],
    });
    await resource.deleteSelectedDate();
    expect(resource.detailRequests.value).toBe(501);
    expect(router.currentRoute.value.query.log_date).toBe("2026-09-20");
    const calls = api.groups.mock.calls.length;
    await resource.backToIps();
    await flushPromises();
    expect(api.groups.mock.calls.length).toBeGreaterThan(calls);
    expect(resource.selectedDate.value).toBe("2026-09-20");
    wrapper.unmount();
  });

  it("continues loading requests when date discovery fails", async () => {
    api.dates.mockRejectedValueOnce(new Error("dates unavailable"));
    const { resource, wrapper } = await setup();
    expect(api.entries).toHaveBeenCalledTimes(1);
    expect(resource.loading.value).toBe(false);
    expect(resource.availableDates.value).toContain(
      resource.selectedDate.value,
    );
    wrapper.unmount();
  });

  it("ignores stale refresh metadata after a new date filter", async () => {
    const { resource, wrapper } = await setup();
    let resolve!: (value: unknown) => void;
    api.dates.mockImplementationOnce(
      () =>
        new Promise((r) => {
          resolve = r;
        }),
    );
    const refresh = resource.refreshAll();
    expect(resource.loading.value).toBe(true);
    await resource.handleDateChange("2026-09-20");
    const calls = api.entries.mock.calls.length;
    resolve({ today: "2026-09-21", dates: ["2026-09-21"], logs_dir: "/stale" });
    await refresh;
    expect(resource.selectedDate.value).toBe("2026-09-20");
    expect(api.entries.mock.calls.length).toBe(calls);
    wrapper.unmount();
  });

  it("renders IP links and exact recorded times on desktop and mobile", async () => {
    const wrapper = mount(GatewayLogIpGroupsView, {
      props: {
        data: groupData(),
        loading: false,
        sort: "requests",
        location: () => "Gansu",
      },
      global: { plugins: [i18n()] },
    });
    expect(wrapper.text()).toContain("14:32:08");
    const button = wrapper
      .findAll("button")
      .find((node) => node.text() === "36.142.108.111");
    await button?.trigger("click");
    expect(wrapper.emitted("open")?.[0]).toEqual(["36.142.108.111"]);
    expect(formatLogClock("2026-09-21T00:00:01+08:00")).toBe("00:00:01");
    expect(formatLogClock("")).toBe("—");
    wrapper.unmount();
  });
});
