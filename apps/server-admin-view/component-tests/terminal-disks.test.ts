import { defineComponent, h, ref, nextTick } from "vue";
import { flushPromises, mount } from "@vue/test-utils";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createI18n } from "vue-i18n";
import { useTerminalDisks } from "@/views/web-terminal/useTerminalDisks";
import TerminalDiskPopover from "@/views/web-terminal/TerminalDiskPopover.vue";
import type {
  TerminalAttachmentRecord,
  TerminalDisks,
} from "@/lib/api/terminal";
import { enAdmin as en } from "../../../packages/i18n/src/messages/admin/en";
const api = vi.hoisted(() => ({ getAttachmentDisks: vi.fn() }));
vi.mock("@/lib/api/terminal", () => ({ TerminalAPI: api }));
const snapshot = (): TerminalDisks => ({
  sampledAt: "2026-09-14T12:00:00Z",
  sampleAgeMs: 0,
  status: "available",
  reason: null,
  disks: [
    {
      filesystem: "/dev/root",
      mountPoint: "/",
      availableBytes: 512 * 1024,
      capacity: {
        usedBytes: 512 * 1024,
        totalBytes: 1024 ** 2,
        percent: 50,
        status: "available",
        reason: null,
      },
    },
    {
      filesystem: "server:/Data",
      mountPoint: "/Volumes/My Disk",
      availableBytes: 1024,
      capacity: {
        usedBytes: 4096,
        totalBytes: 5120,
        percent: 80,
        status: "available",
        reason: null,
      },
    },
  ],
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
const wrappers: { unmount(): void }[] = [];
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
  vi.spyOn(document, "visibilityState", "get").mockReturnValue("visible");
  api.getAttachmentDisks.mockReset().mockResolvedValue(snapshot());
});
afterEach(() => {
  wrappers.splice(0).forEach((w) => w.unmount());
  vi.useRealTimers();
  vi.restoreAllMocks();
});
function harness() {
  const attachment = ref<TerminalAttachmentRecord | null>(record());
  const sessionId = ref<string | null>("s");
  const connected = ref(true);
  let state!: ReturnType<typeof useTerminalDisks>;
  wrappers.push(
    mount(
      defineComponent({
        setup() {
          state = useTerminalDisks({ attachment, sessionId, connected });
          return () => h("div");
        },
      }),
    ),
  );
  return { state, attachment, sessionId, connected };
}
describe("disk sampling", () => {
  it("collects for viewers only while details are open and the page is visible", async () => {
    const { state } = harness();
    expect(api.getAttachmentDisks).not.toHaveBeenCalled();
    state.diskDetailsOpen.value = true;
    await flushPromises();
    expect(state.disks.value?.disks).toHaveLength(2);
    await vi.advanceTimersByTimeAsync(5000);
    expect(api.getAttachmentDisks).toHaveBeenCalledTimes(2);
    vi.spyOn(document, "visibilityState", "get").mockReturnValue("hidden");
    document.dispatchEvent(new Event("visibilitychange"));
    await vi.advanceTimersByTimeAsync(10000);
    expect(api.getAttachmentDisks).toHaveBeenCalledTimes(2);
    vi.spyOn(document, "visibilityState", "get").mockReturnValue("visible");
    document.dispatchEvent(new Event("visibilitychange"));
    await flushPromises();
    expect(api.getAttachmentDisks).toHaveBeenCalledTimes(3);
    state.diskDetailsOpen.value = false;
    await vi.advanceTimersByTimeAsync(15000);
    expect(api.getAttachmentDisks).toHaveBeenCalledTimes(3);
  });
  it("aborts on close and discards late responses across session switches", async () => {
    let resolve!: (value: TerminalDisks) => void;
    api.getAttachmentDisks.mockImplementation(
      () =>
        new Promise((r) => {
          resolve = r;
        }),
    );
    const { state, attachment, sessionId } = harness();
    state.diskDetailsOpen.value = true;
    const signal = api.getAttachmentDisks.mock.calls[0]![1] as AbortSignal;
    state.diskDetailsOpen.value = false;
    expect(signal.aborted).toBe(true);
    sessionId.value = "other";
    attachment.value = record("b", "other");
    resolve(snapshot());
    await flushPromises();
    expect(state.disks.value).toBeNull();
    expect(state.diskDetailsOpen.value).toBe(false);
  });
  it("retains a stale sample when enumeration fails and backs off", async () => {
    const { state } = harness();
    state.diskDetailsOpen.value = true;
    await flushPromises();
    api.getAttachmentDisks.mockRejectedValue(new Error("timeout"));
    await vi.advanceTimersByTimeAsync(16000);
    expect(state.disksFailed.value).toBe(true);
    expect(state.disksStale.value).toBe(true);
    expect(state.disks.value?.disks).toHaveLength(2);
    expect(api.getAttachmentDisks).toHaveBeenCalledTimes(2);
  });
});
function popover() {
  const open = ref(false);
  const wrapper = mount(
    defineComponent({
      setup() {
        return () =>
          h(
            TerminalDiskPopover,
            {
              open: open.value,
              "onUpdate:open": (value: boolean) => {
                open.value = value;
              },
              disks: snapshot(),
              loading: false,
              failed: false,
              stale: false,
              disabled: false,
            },
            () => "Disk 50%",
          );
      },
    }),
    {
      attachTo: document.body,
      global: {
        plugins: [
          createI18n({
            legacy: false,
            locale: "en",
            messages: { en: { admin: en } },
          }),
        ],
      },
    },
  );
  wrappers.push(wrapper);
  return { wrapper, open, trigger: wrapper.get("button") };
}
describe("disk details interaction", () => {
  it("renders loading, partial results, stale values, and unavailable reasons", async () => {
    const wrapper = mount(TerminalDiskPopover, {
      props: {
        open: true,
        disks: null,
        loading: true,
        failed: false,
        stale: false,
        disabled: false,
      },
      attachTo: document.body,
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
    await flushPromises();
    expect(document.querySelector('[aria-busy="true"]')).not.toBeNull();
    await wrapper.setProps({
      loading: false,
      disks: { ...snapshot(), status: "partial", reason: "collection_failed" },
    });
    expect(document.body.textContent).toContain(
      "Some filesystems could not be read",
    );
    expect(document.querySelectorAll("[data-disk-row]")).toHaveLength(2);
    await wrapper.setProps({ stale: true });
    expect(document.body.textContent).toContain(en.webTerminal.metrics.stale);
    await wrapper.setProps({
      disks: {
        ...snapshot(),
        status: "unavailable",
        reason: "missing_command",
        disks: [],
      },
      stale: false,
      failed: true,
    });
    expect(document.body.textContent).toContain(
      en.webTerminal.metrics.reason.missing_command,
    );
    expect(document.querySelectorAll("[data-disk-row]")).toHaveLength(0);
  });

  it("opens on hover, stays open over the panel, then closes on leaving", async () => {
    const { trigger, open } = popover();
    await trigger.trigger("pointerenter", { pointerType: "mouse" });
    await vi.advanceTimersByTimeAsync(150);
    await flushPromises();
    expect(open.value).toBe(true);
    expect(document.querySelectorAll("[data-disk-row]")).toHaveLength(2);
    expect(document.body.textContent).toContain("/Volumes/My Disk");
    expect(document.body.textContent).toContain("Mounted filesystems");
    expect(document.body.textContent).not.toContain(
      "admin.webTerminal.metrics.",
    );
    await trigger.trigger("pointerleave", { pointerType: "mouse" });
    const panel = document.querySelector('[data-slot="popover-content"]')!;
    panel.dispatchEvent(
      new PointerEvent("pointerenter", { pointerType: "mouse" }),
    );
    await vi.advanceTimersByTimeAsync(250);
    expect(open.value).toBe(true);
    panel.dispatchEvent(
      new PointerEvent("pointerleave", { pointerType: "mouse" }),
    );
    await vi.advanceTimersByTimeAsync(250);
    expect(open.value).toBe(false);
  });
  it("toggles on touch without synthetic clicks and supports keyboard focus", async () => {
    const { trigger, open } = popover();
    await trigger.trigger("pointerenter", { pointerType: "touch" });
    await vi.advanceTimersByTimeAsync(200);
    expect(open.value).toBe(false);
    await trigger.trigger("pointerdown", { pointerType: "touch" });
    await trigger.trigger("click", { detail: 1 });
    expect(open.value).toBe(true);
    await trigger.trigger("pointerdown", { pointerType: "touch" });
    expect(open.value).toBe(false);
    (trigger.element as HTMLButtonElement).focus();
    await nextTick();
    expect(open.value).toBe(true);
    document.dispatchEvent(
      new KeyboardEvent("keydown", { key: "Escape", bubbles: true }),
    );
    await nextTick();
    expect(open.value).toBe(false);
  });
});
