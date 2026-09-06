import { flushPromises, mount } from "@vue/test-utils";
import { defineComponent, h, ref } from "vue";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type {
  RuntimeDebugReport,
  RuntimeDebugResponse,
  RuntimeDebugSample,
} from "../src/types/runtime-debug";

const api = vi.hoisted(() => ({
  getDebug: vi.fn(),
  startDebugCapture: vi.fn(),
  stopDebugCapture: vi.fn(),
  refreshDebugMemory: vi.fn(),
}));
vi.mock("../src/lib/api/runtime-health", () => ({ RuntimeHealthAPI: api }));
vi.mock("vue-i18n", () => ({ useI18n: () => ({ t: (key: string) => key }) }));
vi.mock("vue-sonner", () => ({ toast: { success: vi.fn(), error: vi.fn() } }));
import RuntimeDebugDialog from "../src/views/event-center/RuntimeDebugDialog.vue";
import { useRuntimeDebug } from "../src/views/event-center/useRuntimeDebug";
import { summarizeDebugSamples } from "../src/views/event-center/runtimeDebugPresentation";

const makeReport = (
  status: RuntimeDebugReport["capture"]["status"] = "idle",
): RuntimeDebugReport => ({
  schema_version: 1,
  generated_at: "2026-09-06T00:00:00Z",
  process: {
    pid: 123,
    version: "2.4.9",
    os: "linux",
    arch: "x86_64",
    logical_cpus: 4,
    uptime_ms: 1000,
  },
  capture: {
    id: null,
    status,
    started_at: null,
    finished_at: null,
    elapsed_ms: 0,
    duration_seconds: 60,
    sample_interval_ms: 1000,
    samples: [],
    errors: [],
    operations: {
      generation: 0,
      active: false,
      elapsed_ms: 0,
      dropped_operations: 0,
      operations: [],
    },
  },
  memory: null,
  memory_refreshing: false,
  queue: {
    queue_depth: 0,
    queue_depth_peak: 0,
    queue_wait_ms: 0,
    queue_wait_peak_ms: 0,
    active_operation_ms: 0,
    canceled_operations: 0,
  },
});
const hideDocument = (hidden: boolean) => {
  Object.defineProperty(document, "hidden", {
    configurable: true,
    value: hidden,
  });
  document.dispatchEvent(new Event("visibilitychange"));
};
const wrappers: Array<{ unmount: () => void }> = [];
const harness = (initiallyOpen = false) => {
  const open = ref(initiallyOpen);
  let debug!: ReturnType<typeof useRuntimeDebug>;
  const wrapper = mount(
    defineComponent({
      setup() {
        debug = useRuntimeDebug({ enabled: open });
        return () => h("div");
      },
    }),
  );
  wrappers.push(wrapper);
  return { open, debug, wrapper };
};

describe("runtime debug capture lifecycle", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.clearAllMocks();
    hideDocument(false);
    api.getDebug.mockResolvedValue({ data: makeReport() });
    api.startDebugCapture.mockResolvedValue({ data: makeReport("running") });
    api.stopDebugCapture.mockResolvedValue({ data: makeReport("stopped") });
    api.refreshDebugMemory.mockResolvedValue({ data: makeReport() });
  });
  afterEach(() => {
    for (const wrapper of wrappers.splice(0)) wrapper.unmount();
    hideDocument(false);
    vi.useRealTimers();
  });

  it("renders capture controls, partial memory support and diagnostic export without starting work on open", async () => {
    const data = makeReport();
    data.memory = {
      status: "unsupported",
      collected_at: data.generated_at,
      rss_bytes: 40 * 1024 * 1024,
      anonymous_bytes: null,
      file_bytes: null,
      swap_bytes: null,
      threads: null,
      categories: [],
      largest_anonymous_regions: [],
      allocator: null,
      errors: [],
    };
    api.getDebug.mockResolvedValue({ data });
    const slots = { template: "<div><slot /></div>" };
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText },
    });
    const wrapper = mount(RuntimeDebugDialog, {
      props: { open: true },
      global: {
        stubs: {
          Dialog: slots,
          DialogContent: slots,
          DialogHeader: slots,
          DialogTitle: slots,
          DialogDescription: slots,
        },
      },
    });
    wrappers.push(wrapper);
    await flushPromises();
    expect(wrapper.text()).toContain(
      "admin.eventCenter.runtime.debug.memoryStatus.unsupported",
    );
    expect(wrapper.text()).toContain("40.00 MiB");
    expect(api.startDebugCapture).not.toHaveBeenCalled();
    expect(api.refreshDebugMemory).not.toHaveBeenCalled();
    const button = (key: string) =>
      wrapper
        .findAll("button")
        .find(
          (item) => item.text() === `admin.eventCenter.runtime.debug.${key}`,
        )!;
    await button("copy").trigger("click");
    expect(JSON.parse(writeText.mock.calls[0]![0])).toEqual(data);
    await button("start").trigger("click");
    await flushPromises();
    expect(api.startDebugCapture).toHaveBeenCalledTimes(1);
    expect(wrapper.text()).toContain(
      "admin.eventCenter.runtime.debug.status.running",
    );
    await button("stop").trigger("click");
    await flushPromises();
    expect(api.stopDebugCapture).toHaveBeenCalledTimes(1);
    expect(wrapper.text()).toContain(
      "admin.eventCenter.runtime.debug.status.stopped",
    );
  });

  it("does no polling or capture while closed, and only reads cached data when opened", async () => {
    const { open, debug } = harness();
    await vi.advanceTimersByTimeAsync(10_000);
    expect(api.getDebug).not.toHaveBeenCalled();
    open.value = true;
    await flushPromises();
    expect(api.getDebug).toHaveBeenCalledTimes(1);
    expect(api.startDebugCapture).not.toHaveBeenCalled();
    expect(api.refreshDebugMemory).not.toHaveBeenCalled();
    await debug.start();
    expect(debug.running.value).toBe(true);
    expect(debug.remainingSeconds.value).toBe(60);
    open.value = false;
    await flushPromises();
    await vi.advanceTimersByTimeAsync(10_000);
    expect(api.getDebug).toHaveBeenCalledTimes(1);
    expect(api.stopDebugCapture).not.toHaveBeenCalled();
  });

  it("polls at most every two seconds, pauses when hidden, and resumes without overlapping", async () => {
    const { debug } = harness(true);
    await flushPromises();
    await debug.refresh();
    await vi.advanceTimersByTimeAsync(1_999);
    expect(api.getDebug).toHaveBeenCalledTimes(1);
    await vi.advanceTimersByTimeAsync(1);
    expect(api.getDebug).toHaveBeenCalledTimes(2);
    hideDocument(true);
    await vi.advanceTimersByTimeAsync(10_000);
    expect(api.getDebug).toHaveBeenCalledTimes(2);
    hideDocument(false);
    await flushPromises();
    expect(api.getDebug).toHaveBeenCalledTimes(3);
    hideDocument(true);
    hideDocument(false);
    await flushPromises();
    expect(api.getDebug).toHaveBeenCalledTimes(3);
  });

  it("aborts and ignores an old read after a capture starts", async () => {
    const { debug } = harness(true);
    await flushPromises();
    let resolveOld!: (data: RuntimeDebugResponse) => void;
    let oldSignal!: AbortSignal;
    api.getDebug.mockImplementationOnce((signal: AbortSignal) => {
      oldSignal = signal;
      return new Promise((resolve) => {
        resolveOld = resolve;
      });
    });
    await vi.advanceTimersByTimeAsync(2_000);
    expect(debug.loading.value).toBe(true);
    await debug.start();
    expect(oldSignal.aborted).toBe(true);
    resolveOld({ success: true, data: makeReport("idle") });
    await flushPromises();
    expect(debug.report.value?.capture.status).toBe("running");
  });

  it("does not publish a mutation result after closing and fetches authoritative status on reopen", async () => {
    const { debug, open } = harness(true);
    await flushPromises();
    let resolveStart!: (data: RuntimeDebugResponse) => void;
    let startSignal!: AbortSignal;
    api.startDebugCapture.mockImplementationOnce((signal: AbortSignal) => {
      startSignal = signal;
      return new Promise((resolve) => {
        resolveStart = resolve;
      });
    });
    const startPromise = debug.start();
    open.value = false;
    await flushPromises();
    expect(startSignal.aborted).toBe(true);
    resolveStart({ success: true, data: makeReport("running") });
    await startPromise;
    expect(debug.report.value?.capture.status).toBe("idle");
    expect(api.stopDebugCapture).not.toHaveBeenCalled();
    api.getDebug.mockResolvedValue({ data: makeReport("completed") });
    open.value = true;
    await flushPromises();
    await vi.advanceTimersByTimeAsync(2_000);
    expect(debug.report.value?.capture.status).toBe("completed");
  });

  it("reports unsupported versions and stops retry polling until requested", async () => {
    api.getDebug.mockRejectedValueOnce({ response: { status: 404 } });
    const { debug } = harness(true);
    await flushPromises();
    expect(debug.error.value).toBe(true);
    expect(debug.unavailable.value).toBe(true);
    await vi.advanceTimersByTimeAsync(10_000);
    expect(api.getDebug).toHaveBeenCalledTimes(1);
    await debug.refresh();
    expect(debug.unavailable.value).toBe(false);
    expect(debug.report.value?.capture.status).toBe("idle");
  });

  it("deduplicates mutations and keeps stopped results available", async () => {
    const { debug } = harness(true);
    await flushPromises();
    await debug.start();
    let resolveStop!: (data: RuntimeDebugResponse) => void;
    api.stopDebugCapture.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          resolveStop = resolve;
        }),
    );
    const stopped = debug.stop();
    await debug.stop();
    expect(api.stopDebugCapture).toHaveBeenCalledTimes(1);
    resolveStop({ success: true, data: makeReport("stopped") });
    await stopped;
    expect(debug.report.value?.capture.status).toBe("stopped");
  });
});

describe("runtime debug sample interpretation", () => {
  const sample = (
    elapsed: number,
    cpu: number | null,
    rss: number,
  ): RuntimeDebugSample => ({
    at: "2026-09-06T00:00:00Z",
    elapsed_ms: elapsed,
    resource: {
      collected_at: "2026-09-06T00:00:00Z",
      errors: [],
      cpu_percent: cpu,
      rss_bytes: rss,
      anonymous_bytes: null,
      file_bytes: null,
      swap_bytes: null,
      threads: 1,
      thread_cpu:
        cpu == null
          ? []
          : [{ tid: 12, name: "sqlite-primary", cpu_percent: cpu }],
    },
    queue_depth: 0,
    active_operation_ms: 0,
  });
  it("weights CPU by actual elapsed intervals and measures RSS start-to-end independently", () => {
    const result = summarizeDebugSamples([
      sample(0, null, 1000),
      sample(1000, 10, 2000),
      sample(4000, 30, 1500),
    ]);
    expect(result.averageCpu).toBe(25);
    expect(result.maxCpu).toBe(30);
    expect(result.rssDelta).toBe(500);
    expect(result.threads[0]?.average).toBe(25);
  });
  it("keeps unavailable CPU unknown instead of displaying zero", () => {
    const result = summarizeDebugSamples([
      sample(0, null, 1000),
      sample(1000, null, 1000),
    ]);
    expect(result.averageCpu).toBeNull();
    expect(result.maxCpu).toBeNull();
    expect(result.rssDelta).toBe(0);
    expect(result.threads).toEqual([]);
  });
});
