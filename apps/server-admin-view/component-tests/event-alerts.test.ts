import { createPinia, setActivePinia } from "pinia";
import { beforeEach, expect, it, vi } from "vitest";
const getEvents = vi.hoisted(() => vi.fn());
vi.mock("../src/lib/api/events", () => ({ EventCenterAPI: { getEvents } }));
import { useEventAlertsStore } from "../src/store/event-alerts";
beforeEach(() => {
  setActivePinia(createPinia());
  getEvents.mockReset();
});
it("queries only CRITICAL and retains successful state on failures, then clears after deletion", async () => {
  const store = useEventAlertsStore();
  expect(store.hasCriticalEvents).toBe(false);
  getEvents.mockResolvedValueOnce({ success: true, data: { total: 1 } });
  await store.refresh();
  expect(getEvents).toHaveBeenCalledWith(
    { page: 1, limit: "1", search: "", level: "CRITICAL" },
    undefined,
  );
  expect(store.hasCriticalEvents).toBe(true);
  getEvents.mockRejectedValueOnce(new Error("offline"));
  await store.refresh();
  expect(store.hasCriticalEvents).toBe(true);
  getEvents.mockResolvedValueOnce({ success: true, data: { total: 0 } });
  await store.refresh();
  expect(store.hasCriticalEvents).toBe(false);
});
it("ignores older or aborted refresh results", async () => {
  const store = useEventAlertsStore();
  let finish!: (value: unknown) => void;
  getEvents.mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        finish = resolve;
      }),
  );
  const older = store.refresh();
  getEvents.mockResolvedValueOnce({ success: true, data: { total: 0 } });
  await store.refresh();
  finish({ success: true, data: { total: 1 } });
  await older;
  expect(store.hasCriticalEvents).toBe(false);
  const controller = new AbortController();
  controller.abort();
  getEvents.mockResolvedValueOnce({ success: true, data: { total: 1 } });
  await store.refresh(controller.signal);
  expect(store.hasCriticalEvents).toBe(false);
});
