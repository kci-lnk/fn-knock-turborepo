import { flushPromises, mount } from "@vue/test-utils";
import { defineComponent, h, ref } from "vue";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useDashboardOnlineIps } from "../src/views/dashboard/useDashboardOnlineIps";
import { useHostActiveIps } from "../src/composables/useHostActiveIps";
import type {
  DashboardOnlineIpsPayload,
  IpLocationSnapshot,
} from "../src/types";

const mocks = vi.hoisted(() => ({
  online: vi.fn(),
  locations: vi.fn(),
  host: vi.fn(),
}));
vi.mock("../src/lib/api/dashboard", () => ({
  DashboardAPI: { getOnlineIps: mocks.online, getHostActiveIps: mocks.host },
}));
vi.mock("../src/lib/api/gateway", () => ({
  IpLocationAPI: { lookupBatch: mocks.locations },
}));
vi.mock("@fn-knock/i18n/vue/admin", () => ({ browserT: (key: string) => key }));
const wrappers: ReturnType<typeof mount>[] = [];
const payload = (count = 45): DashboardOnlineIpsPayload => ({
  items: Array.from({ length: count }, (_, i) => ({
    ip: `192.0.2.${i + 1}`,
    last_seen_at: new Date(1000000 + i * 1000).toISOString(),
    identity_count: 1,
  })),
  online_count: count,
  window_seconds: 120,
  timestamp: 2000000,
});
const location = (
  ip: string,
  status: IpLocationSnapshot["status"] = "success",
): IpLocationSnapshot => ({
  ip,
  normalizedIp: ip,
  status,
  location: status === "success" ? "Test location" : "",
  attempts: 0,
  maxAttempts: 3,
  updatedAt: 1,
  error: "",
});
function setup() {
  const open = ref(true);
  let resource!: ReturnType<typeof useDashboardOnlineIps>;
  const wrapper = mount(
    defineComponent({
      setup() {
        resource = useDashboardOnlineIps(open);
        return () => h("div");
      },
    }),
  );
  wrappers.push(wrapper);
  return { resource, open, wrapper };
}
beforeEach(() => {
  vi.useFakeTimers();
  mocks.online.mockReset().mockResolvedValue(payload());
  mocks.locations
    .mockReset()
    .mockImplementation(async (ips: string[]) => ips.map((ip) => location(ip)));
});
afterEach(() => {
  wrappers.splice(0).forEach((wrapper) => wrapper.unmount());
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe("online IP snapshot", () => {
  it.each([0, 20, 21])(
    "shows pagination only when %i addresses need another page",
    async (count) => {
      mocks.online.mockResolvedValueOnce(payload(count));
      const { resource: r } = setup();
      await flushPromises();
      expect(r.showPagination.value).toBe(count > 20);
      expect(r.hasPreviousPage.value).toBe(false);
      expect(r.hasNextPage.value).toBe(count > 20);
      if (count > 20) {
        r.page.value++;
        await flushPromises();
        expect(r.displayItems.value).toHaveLength(1);
        expect(r.hasPreviousPage.value).toBe(true);
        expect(r.hasNextPage.value).toBe(false);
      }
    },
  );
  it("sorts and pages a fixed snapshot, resolving only visible addresses and reusing results", async () => {
    const { resource: r } = setup();
    await flushPromises();
    expect(r.displayItems.value).toHaveLength(20);
    expect(r.displayItems.value[0]?.ip).toBe("192.0.2.45");
    expect(mocks.locations.mock.calls[0]?.[0]).toHaveLength(20);
    r.page.value = 2;
    await flushPromises();
    expect(r.displayItems.value[0]?.ip).toBe("192.0.2.25");
    expect(mocks.locations).toHaveBeenCalledTimes(2);
    r.page.value = 1;
    await flushPromises();
    expect(mocks.locations).toHaveBeenCalledTimes(2);
    r.order.value = "asc";
    await flushPromises();
    expect(r.page.value).toBe(1);
    expect(r.displayItems.value[0]?.ip).toBe("192.0.2.1");
    expect(r.showPagination.value).toBe(true);
    expect(r.hasPreviousPage.value).toBe(false);
    r.page.value = 3;
    await flushPromises();
    expect(r.displayItems.value).toHaveLength(5);
    expect(r.hasNextPage.value).toBe(false);
    expect(r.hasPreviousPage.value).toBe(true);
    await vi.advanceTimersByTimeAsync(15000);
    expect(mocks.online).toHaveBeenCalledTimes(1);
  });
  it("retains the previous page on refresh failure and resets on successful refresh", async () => {
    const { resource: r } = setup();
    await flushPromises();
    r.page.value = 2;
    mocks.online.mockRejectedValueOnce({
      response: { data: { message: "Update the gateway" } },
    });
    await r.refresh();
    expect(r.page.value).toBe(2);
    expect(r.snapshot.value?.online_count).toBe(45);
    expect(r.error.value).toBe("Update the gateway");
    mocks.online.mockResolvedValueOnce(payload(1));
    await r.refresh();
    expect(r.page.value).toBe(1);
    expect(r.error.value).toBe("");
    expect(r.displayItems.value).toHaveLength(1);
  });
  it("aborts old requests and ignores their responses after closing and reopening", async () => {
    let finish!: (value: DashboardOnlineIpsPayload) => void;
    mocks.online.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          finish = resolve;
        }),
    );
    const { resource: r, open } = setup();
    const signal = mocks.online.mock.calls[0]?.[0] as AbortSignal;
    open.value = false;
    expect(signal.aborted).toBe(true);
    open.value = true;
    await flushPromises();
    finish(payload(1));
    await flushPromises();
    expect(r.snapshot.value?.online_count).toBe(45);
    expect(r.loading.value).toBe(false);
  });
  it("keeps unknown addresses out of location queries and has stable time ties", async () => {
    const data = payload(3);
    data.items = ["192.0.2.2", "", "192.0.2.1"].map((ip) => ({
      ip,
      identity_count: 1,
      last_seen_at: new Date(1000).toISOString(),
    }));
    mocks.online.mockResolvedValueOnce(data);
    const { resource: r } = setup();
    await flushPromises();
    expect(r.displayItems.value.map((item) => item.ip)).toEqual([
      "",
      "192.0.2.1",
      "192.0.2.2",
    ]);
    expect(r.ipCount.value).toBe(2);
    expect(mocks.locations.mock.calls[0]?.[0]).toEqual([
      "192.0.2.1",
      "192.0.2.2",
    ]);
  });
  it("preserves nanosecond ordering before applying the IP tie breaker", async () => {
    const data = payload(2);
    data.items = [
      {
        ip: "192.0.2.1",
        identity_count: 1,
        last_seen_at: "2026-09-14T00:00:00.123Z",
      },
      {
        ip: "192.0.2.2",
        identity_count: 1,
        last_seen_at: "2026-09-14T00:00:00.123000001Z",
      },
    ];
    mocks.online.mockResolvedValueOnce(data);
    const { resource: r } = setup();
    await flushPromises();
    expect(r.displayItems.value[0]?.ip).toBe("192.0.2.2");
    r.order.value = "asc";
    expect(r.displayItems.value[0]?.ip).toBe("192.0.2.1");
  });

  it("recovers failed location lookups on manual refresh without refetching successful IPs", async () => {
    vi.spyOn(console, "error").mockImplementation(() => {});
    mocks.online.mockResolvedValue(payload(25));
    const { resource: r } = setup();
    await flushPromises();
    mocks.locations.mockRejectedValue(new Error("offline"));
    r.page.value = 2;
    await flushPromises();
    await vi.advanceTimersByTimeAsync(10000);
    expect(mocks.locations).toHaveBeenCalledTimes(4);
    mocks.locations.mockImplementation(async (ips: string[]) =>
      ips.map((ip) => location(ip)),
    );
    await r.refresh();
    await flushPromises();
    expect(mocks.locations).toHaveBeenCalledTimes(4);
    r.page.value = 2;
    await flushPromises();
    expect(mocks.locations).toHaveBeenCalledTimes(5);
    expect(
      r.displayItems.value.every(
        (item) => item.locationText === "Test location",
      ),
    ).toBe(true);
  });

  it("gives a manually retried location lookup a fresh retry budget", async () => {
    vi.spyOn(console, "error").mockImplementation(() => {});
    mocks.locations.mockRejectedValue(new Error("offline"));
    const { resource: r } = setup();
    await flushPromises();
    await vi.advanceTimersByTimeAsync(10000);
    expect(mocks.locations).toHaveBeenCalledTimes(3);
    mocks.online.mockResolvedValueOnce(payload());
    mocks.locations
      .mockRejectedValueOnce(new Error("still offline"))
      .mockImplementation(async (ips: string[]) =>
        ips.map((ip) => location(ip)),
      );
    await r.refresh();
    await flushPromises();
    await vi.advanceTimersByTimeAsync(2000);
    expect(mocks.locations).toHaveBeenCalledTimes(5);
    expect(r.displayItems.value[0]?.locationText).toBe("Test location");
  });

  it("handles empty and initial failure responses", async () => {
    mocks.online.mockRejectedValueOnce(new Error("offline"));
    const { resource: r } = setup();
    await flushPromises();
    expect(r.snapshot.value).toBeNull();
    expect(r.error.value).toBeTruthy();
    mocks.online.mockResolvedValueOnce(payload(0));
    await r.refresh();
    expect(r.displayItems.value).toEqual([]);
    expect(mocks.locations).not.toHaveBeenCalled();
  });
  it("does not schedule another location poll after unmount", async () => {
    let finish!: (items: IpLocationSnapshot[]) => void;
    mocks.locations.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          finish = resolve;
        }),
    );
    const { wrapper } = setup();
    await flushPromises();
    wrapper.unmount();
    finish([location("192.0.2.45", "queued")]);
    await flushPromises();
    await vi.advanceTimersByTimeAsync(10000);
    expect(mocks.locations).toHaveBeenCalledTimes(1);
  });
  it("stops both host and location polling when the existing host dialog closes", async () => {
    mocks.host.mockResolvedValue({
      items: [
        {
          ip: "192.0.2.1",
          last_seen_at: "2026-01-01T00:00:00Z",
          active_conns: 1,
        },
      ],
      window_seconds: 120,
    });
    mocks.locations.mockResolvedValue([location("192.0.2.1", "queued")]);
    const open = ref(true);
    wrappers.push(
      mount(
        defineComponent({
          setup() {
            useHostActiveIps("example.com", open);
            return () => h("div");
          },
        }),
      ),
    );
    await flushPromises();
    open.value = false;
    await flushPromises();
    await vi.advanceTimersByTimeAsync(15000);
    expect(mocks.host).toHaveBeenCalledTimes(1);
    expect(mocks.locations).toHaveBeenCalledTimes(1);
  });
  it("retries location failures three times then stops", async () => {
    vi.spyOn(console, "error").mockImplementation(() => {});
    mocks.locations.mockRejectedValue(new Error("offline"));
    const { resource: r } = setup();
    await flushPromises();
    await vi.advanceTimersByTimeAsync(10000);
    expect(mocks.locations).toHaveBeenCalledTimes(3);
    expect(r.displayItems.value[0]?.locationText).toBe(
      "admin.hostActiveIps.unavailable",
    );
  });
});
