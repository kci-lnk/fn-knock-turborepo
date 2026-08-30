import { flushPromises, mount } from "@vue/test-utils";
import { defineComponent, h } from "vue";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { DashboardStats } from "../src/types";

const apiMocks = vi.hoisted(() => ({
  getDdnsStatus: vi.fn(),
  getOverview: vi.fn(),
  getStats: vi.fn(),
}));

vi.mock("../src/lib/api/dashboard", () => ({
  DashboardAPI: { getStats: apiMocks.getStats },
}));

vi.mock("../src/lib/api/ddns", () => ({
  DDNSAPI: { getStatus: apiMocks.getDdnsStatus },
}));

vi.mock("../src/lib/api/security", () => ({
  SecurityAPI: { getOverview: apiMocks.getOverview },
}));

vi.mock("@admin-shared/utils/toast", () => ({
  toast: { error: vi.fn() },
}));

vi.mock("../src/composables/useVisibilityPolling", () => ({
  createVisibilityPoller: () => ({
    start: vi.fn(),
    stop: vi.fn(),
    sync: vi.fn(),
  }),
}));

import { useDashboardData } from "../src/views/dashboard/useDashboardData";

const dashboardStats = (rangeSec: number) =>
  ({
    rangeSec,
    now: { online: 0, error5xxTotal: 0 },
    totals: { inBytes: 0, outBytes: 0, error5xx: 0 },
    errors: { error5xx1d: 0, error5xx1w: 0 },
    traffic: { echarts: { series: [] } },
  }) as DashboardStats;

describe("useDashboardData range refresh", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    apiMocks.getDdnsStatus.mockResolvedValue({ enabled: false });
    apiMocks.getOverview.mockImplementation((rangeSec: number) =>
      Promise.resolve({ rangeSec, totals: {}, series: {} }),
    );
  });

  it("reloads the latest range when it changes during an active request", async () => {
    const requests: Array<{
      rangeSec: number;
      resolve: (value: DashboardStats) => void;
      signal: AbortSignal;
    }> = [];
    apiMocks.getStats.mockImplementation(
      (rangeSec: number, _options: undefined, signal: AbortSignal) =>
        new Promise((resolve) =>
          requests.push({ rangeSec, resolve, signal }),
        ),
    );

    let dashboardData!: ReturnType<typeof useDashboardData>;
    const wrapper = mount(
      defineComponent({
        setup() {
          dashboardData = useDashboardData({
            disposeTunnelStatus: vi.fn(),
            scheduleTunnelStatusLoad: vi.fn(),
            startRealtimePolling: vi.fn(),
            stopRealtimePolling: vi.fn(),
            translate: (key) => key,
          });
          return () => h("div");
        },
      }),
    );

    await flushPromises();
    expect(requests.map(({ rangeSec }) => rangeSec)).toEqual([3600]);

    dashboardData.rangeKey.value = "15m";
    await flushPromises();

    expect(requests.map(({ rangeSec }) => rangeSec)).toEqual([3600, 900]);
    expect(requests[0]!.signal.aborted).toBe(true);
    expect(dashboardData.stats.value).toBeNull();
    expect(dashboardData.isInitializing.value).toBe(true);

    requests[1]!.resolve(dashboardStats(900));
    await flushPromises();

    expect(dashboardData.stats.value?.rangeSec).toBe(900);
    expect(dashboardData.isInitializing.value).toBe(false);

    requests[0]!.resolve(dashboardStats(3600));
    await flushPromises();
    expect(dashboardData.stats.value?.rangeSec).toBe(900);
    wrapper.unmount();
  });
});
