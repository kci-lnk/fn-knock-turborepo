import { mount } from "@vue/test-utils";
import { createI18n } from "vue-i18n";
import { afterEach, describe, expect, it, vi } from "vitest";
import { defineComponent, ref } from "vue";
import { AcmeAPI } from "../src/lib/api/acme";
import { useAcmeJobPolling } from "../src/views/ssl-settings/useAcmeJobPolling";

const wrappers: ReturnType<typeof mount>[] = [];
afterEach(() => {
  for (const wrapper of wrappers.splice(0)) wrapper.unmount();
  vi.restoreAllMocks();
  vi.useRealTimers();
});

function setup() {
  let polling!: ReturnType<typeof useAcmeJobPolling>;
  const runtimeLocked = ref(false);
  const refreshOverview = vi.fn(async () => {
    // A stopped executor can retain its runtime lock during file output. The
    // overview consequently selects the same terminal job again. Bound the
    // callback so a regression fails assertions rather than looping forever.
    if (refreshOverview.mock.calls.length < 3) {
      await polling.selectJob("job-1", true);
    }
  });
  wrappers.push(
    mount(
      defineComponent({
        setup() {
          polling = useAcmeJobPolling({
            refreshOverview,
            isRuntimeLocked: () => runtimeLocked.value,
          });
          return () => null;
        },
      }),
      {
        global: {
          plugins: [createI18n({ legacy: false, locale: "en", messages: {} })],
        },
      },
    ),
  );
  const poll = vi.spyOn(AcmeAPI, "poll").mockResolvedValue({
    job: { id: "job-1", status: "stopped" },
    logs: [],
    analysis: null,
  } as Awaited<ReturnType<typeof AcmeAPI.poll>>);
  return { polling, refreshOverview, poll, runtimeLocked };
}

describe("ACME job completion refresh", () => {
  it("does not recursively reload an overview when selecting a terminal job", async () => {
    const { polling, refreshOverview, poll } = setup();
    await polling.selectJob("job-1", true);
    expect(polling.job.value?.status).toBe("stopped");
    expect(refreshOverview).not.toHaveBeenCalled();
    expect(poll).toHaveBeenCalledTimes(1);
  });

  it("refreshes once after polling completion even while the overview retains the job lock", async () => {
    const { polling, refreshOverview, poll } = setup();
    await polling.selectJob("job-1", false);
    await polling.refreshLogs();
    expect(refreshOverview).toHaveBeenCalledTimes(1);
    expect(poll).toHaveBeenCalledTimes(3);
  });
});

it("polls at the normal interval until output cleanup releases the runtime lock", async () => {
  vi.useFakeTimers();
  const { polling, refreshOverview, poll, runtimeLocked } = setup();
  runtimeLocked.value = true;
  refreshOverview.mockImplementation(async () => {});
  await polling.selectJob("job-1", true);
  await vi.advanceTimersByTimeAsync(0);
  expect(poll).toHaveBeenCalledTimes(2);
  expect(refreshOverview).toHaveBeenCalledTimes(1);
  await vi.advanceTimersByTimeAsync(1_999);
  expect(poll).toHaveBeenCalledTimes(2);
  runtimeLocked.value = false;
  await vi.advanceTimersByTimeAsync(1);
  expect(poll).toHaveBeenCalledTimes(3);
  await vi.advanceTimersByTimeAsync(10_000);
  expect(poll).toHaveBeenCalledTimes(3);
});
