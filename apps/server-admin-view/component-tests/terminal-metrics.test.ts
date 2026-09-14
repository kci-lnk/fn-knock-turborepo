import { defineComponent, h, ref } from "vue";
import { flushPromises, mount } from "@vue/test-utils";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createI18n } from "vue-i18n";
import { useTerminalMetrics } from "@/views/web-terminal/useTerminalMetrics";
import TerminalResourceStatusBar from "@/views/web-terminal/TerminalResourceStatusBar.vue";
import type {
  TerminalAttachmentRecord,
  TerminalMetrics,
} from "@/lib/api/terminal";
import { enAdmin as en } from "../../../packages/i18n/src/messages/admin/en";

const api = vi.hoisted(() => ({ getAttachmentMetrics: vi.fn() }));
vi.mock("@/lib/api/terminal", () => ({ TerminalAPI: api }));
const snapshot = (): TerminalMetrics => ({
  sampledAt: new Date().toISOString(),
  sampleAgeMs: 0,
  platform: "linux",
  status: "available",
  cpu: { value: 30, status: "available", reason: null },
  memory: {
    usedBytes: 1024 ** 3,
    totalBytes: 4 * 1024 ** 3,
    percent: 25,
    status: "available",
    reason: null,
  },
  disk: {
    usedBytes: 20,
    totalBytes: 100,
    percent: 20,
    status: "available",
    reason: null,
  },
  uptime: { value: 90061, status: "available", reason: null },
});
const record = (id = "a", sessionId = "s"): TerminalAttachmentRecord => ({
  id,
  sessionId,
  role: "viewer",
  transport: "http-polling",
  generation: 1,
  cursor: 0,
  expiresAt: "",
});
function harness() {
  const attachment = ref<TerminalAttachmentRecord | null>(record());
  const sessionId = ref<string | null>("s");
  const connected = ref(true);
  let state!: ReturnType<typeof useTerminalMetrics>;
  const wrapper = mount(
    defineComponent({
      setup() {
        state = useTerminalMetrics({ attachment, sessionId, connected });
        return () => h("div");
      },
    }),
  );
  return { wrapper, state, attachment, sessionId, connected };
}
let wrappers: { unmount: () => void }[] = [];
beforeEach(() => {
  vi.useFakeTimers({
    toFake: [
      "Date",
      "performance",
      "setTimeout",
      "clearTimeout",
      "setInterval",
      "clearInterval",
    ],
  });
  vi.setSystemTime(new Date("2026-09-14T12:00:00Z"));
  vi.spyOn(document, "visibilityState", "get").mockReturnValue("visible");
  api.getAttachmentMetrics
    .mockReset()
    .mockImplementation(async () => snapshot());
});
afterEach(() => {
  wrappers.forEach((w) => w.unmount());
  wrappers = [];
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe("terminal metrics lifecycle", () => {
  it("uses cache age rather than wall clocks to detect stale data", async () => {
    const data = snapshot();
    data.sampledAt = "2000-01-01T00:00:00Z";
    data.sampleAgeMs = 4000;
    api.getAttachmentMetrics.mockResolvedValue(data);
    const h = harness();
    wrappers.push(h.wrapper);
    await flushPromises();
    expect(h.state.metricsStale.value).toBe(false);
    vi.setSystemTime(new Date("2100-01-01T00:00:00Z"));
    await vi.advanceTimersByTimeAsync(1000);
    expect(h.state.metricsStale.value).toBe(false);
    h.connected.value = false;
    await vi.advanceTimersByTimeAsync(11000);
    expect(h.state.metricsStale.value).toBe(true);
  });
  it("does not refresh the age of a cached sample", async () => {
    const data = snapshot();
    api.getAttachmentMetrics.mockResolvedValue(data);
    const h = harness();
    wrappers.push(h.wrapper);
    await flushPromises();
    data.sampleAgeMs = 17000;
    await vi.advanceTimersByTimeAsync(5000);
    expect(h.state.metricsStale.value).toBe(true);
  });
  it("clears request deadlines on unmount even if an adapter ignores abort", () => {
    api.getAttachmentMetrics.mockImplementation(() => new Promise(() => {}));
    const h = harness();
    const signal = api.getAttachmentMetrics.mock.calls[0][1] as AbortSignal;
    h.wrapper.unmount();
    expect(signal.aborted).toBe(true);
    expect(vi.getTimerCount()).toBe(0);
  });
  it("polls for viewers and stops on disconnect and unmount", async () => {
    const h = harness();
    wrappers.push(h.wrapper);
    await flushPromises();
    expect(h.state.metrics.value?.cpu.value).toBe(30);
    expect(api.getAttachmentMetrics).toHaveBeenCalledTimes(1);
    await vi.advanceTimersByTimeAsync(5000);
    expect(api.getAttachmentMetrics).toHaveBeenCalledTimes(2);
    h.connected.value = false;
    await vi.advanceTimersByTimeAsync(15000);
    expect(api.getAttachmentMetrics).toHaveBeenCalledTimes(2);
    h.wrapper.unmount();
    wrappers = [];
    expect(vi.getTimerCount()).toBe(0);
  });
  it("clears data immediately and rejects late responses after session changes", async () => {
    let resolve!: (m: TerminalMetrics) => void;
    api.getAttachmentMetrics.mockImplementationOnce(
      () =>
        new Promise<TerminalMetrics>((r) => {
          resolve = r;
        }),
    );
    const h = harness();
    wrappers.push(h.wrapper);
    const signal = api.getAttachmentMetrics.mock.calls[0][1] as AbortSignal;
    h.sessionId.value = "other";
    expect(signal.aborted).toBe(true);
    expect(h.state.metrics.value).toBeNull();
    resolve(snapshot());
    await flushPromises();
    expect(h.state.metrics.value).toBeNull();
    h.attachment.value = record("b", "other");
    await flushPromises();
    expect(api.getAttachmentMetrics).toHaveBeenLastCalledWith(
      "b",
      expect.any(AbortSignal),
    );
    expect(h.state.metrics.value?.cpu.value).toBe(30);
  });
  it("pauses while hidden and refreshes immediately on return", async () => {
    const h = harness();
    wrappers.push(h.wrapper);
    await flushPromises();
    vi.spyOn(document, "visibilityState", "get").mockReturnValue("hidden");
    document.dispatchEvent(new Event("visibilitychange"));
    await vi.advanceTimersByTimeAsync(20000);
    expect(api.getAttachmentMetrics).toHaveBeenCalledTimes(1);
    expect(h.state.metricsStale.value).toBe(true);
    vi.spyOn(document, "visibilityState", "get").mockReturnValue("visible");
    document.dispatchEvent(new Event("visibilitychange"));
    await flushPromises();
    expect(api.getAttachmentMetrics).toHaveBeenCalledTimes(2);
    expect(h.state.metricsStale.value).toBe(false);
  });
  it("backs off after failure and preserves the last useful sample with stale status", async () => {
    const h = harness();
    wrappers.push(h.wrapper);
    await flushPromises();
    api.getAttachmentMetrics.mockRejectedValue(new Error("offline"));
    await vi.advanceTimersByTimeAsync(5000);
    expect(h.state.metricsFailed.value).toBe(true);
    await vi.advanceTimersByTimeAsync(11000);
    expect(api.getAttachmentMetrics).toHaveBeenCalledTimes(2);
    expect(h.state.metricsStale.value).toBe(true);
    expect(h.state.metrics.value?.cpu.value).toBe(30);
    await vi.advanceTimersByTimeAsync(4000);
    expect(api.getAttachmentMetrics).toHaveBeenCalledTimes(3);
  });
});

describe("terminal resource status bar", () => {
  it("shows partial availability separately from the shell connection", () => {
    const data = snapshot();
    data.status = "partial";
    data.cpu = {
      value: null,
      status: "unavailable",
      reason: "missing_command",
    };
    const wrapper = mount(TerminalResourceStatusBar, {
      props: {
        metrics: data,
        loading: false,
        failed: false,
        stale: false,
        connectionState: "connected",
        connectionLabel: "Connected",
      },
      global: {
        plugins: [
          createI18n({
            legacy: false,
            locale: "en",
            messages: { en: { admin: en } },
          }),
        ],
      },
    });
    wrappers.push(wrapper);
    expect(wrapper.text()).toContain("Connected");
    expect(wrapper.get('[data-metric="cpu"]').text()).toContain("—");
    expect(wrapper.get('[data-metric="cpu"]').attributes("title")).toContain(
      "unavailable",
    );
    expect(wrapper.get('[data-metric="memory"]').text()).toContain(
      "1 GiB / 4 GiB",
    );
    expect(wrapper.get('[data-metric="uptime"]').text()).toContain("1d 1h 1m");
    expect(wrapper.text()).not.toContain("Load");
  });
});
