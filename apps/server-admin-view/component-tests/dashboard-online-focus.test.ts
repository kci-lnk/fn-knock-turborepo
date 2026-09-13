import { flushPromises, mount } from "@vue/test-utils";
import { createPinia } from "pinia";
import { createMemoryHistory, createRouter } from "vue-router";
import { afterEach, describe, expect, it, vi } from "vitest";
import { createFnKnockI18n } from "@fn-knock/i18n/vue/admin";
import Dashboard from "../src/views/Dashboard.vue";

vi.mock("../src/lib/api/dashboard", () => ({
  DashboardAPI: {
    getStats: vi.fn(async () => ({
      rangeSec: 900,
      now: { online: 0, error5xxTotal: 0 },
      totals: { inBytes: 0, outBytes: 0, error5xx: 0 },
      errors: { error5xx1d: 0, error5xx1w: 0 },
      traffic: { echarts: { series: [] } },
    })),
    getRealtime: vi.fn(async () => ({
      active_conns: 0,
      total_in: 0,
      total_out: 0,
      by_host: [],
      by_stream: [],
      timestamp: Date.now(),
    })),
    getOnlineIps: vi.fn(async () => ({
      items: [],
      online_count: 0,
      window_seconds: 120,
      timestamp: Date.now(),
    })),
  },
}));
vi.mock("../src/lib/api/security", () => ({
  SecurityAPI: {
    getOverview: vi.fn(async () => ({
      totals: {},
      series: { failedLogins: [], blockedScanners: [], wafEvents: [] },
    })),
  },
}));
vi.mock("../src/lib/api/ddns", () => ({
  DDNSAPI: {
    getStatus: vi.fn(async () => ({
      enabled: false,
      updateScope: "dual_stack",
      targets: [],
    })),
  },
}));
const wrappers: ReturnType<typeof mount>[] = [];
afterEach(() => {
  wrappers.splice(0).forEach((wrapper) => wrapper.unmount());
  document.body.innerHTML = "";
});

describe("Dashboard online dialog focus", () => {
  it("returns focus to its lazy trigger when the dialog is removed", async () => {
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [{ path: "/", component: Dashboard }],
    });
    await router.push("/");
    const i18n = await createFnKnockI18n({
      scope: "admin",
      defaultLocale: "en",
    });
    const wrapper = mount(Dashboard, {
      attachTo: document.body,
      global: {
        plugins: [createPinia(), router, i18n],
        stubs: { TimeSeriesChart: true },
      },
    });
    wrappers.push(wrapper);
    await flushPromises();
    const trigger = wrapper.get<HTMLButtonElement>(
      'button[aria-haspopup="dialog"]',
    );
    trigger.element.focus();
    await trigger.trigger("click");
    await vi.waitFor(() =>
      expect(document.querySelector('[role="dialog"]')).not.toBeNull(),
    );
    await flushPromises();
    expect(trigger.attributes("aria-expanded")).toBe("true");
    (
      document.querySelector('[data-slot="dialog-close"]') as HTMLButtonElement
    ).click();
    await vi.waitFor(() => {
      expect(document.querySelector('[role="dialog"]')).toBeNull();
      expect(document.activeElement).toBe(trigger.element);
      expect(trigger.attributes("aria-expanded")).toBe("false");
    });
  });
});
