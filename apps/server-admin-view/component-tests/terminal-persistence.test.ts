import { ref } from "vue";
import { flushPromises, mount } from "@vue/test-utils";
import { createI18n } from "vue-i18n";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { TerminalAPI, type TerminalSessionRecord } from "@/lib/api/terminal";
import { useTerminalPageLease } from "@/views/web-terminal/useTerminalPageLease";
import { useTerminalSessions } from "@/views/web-terminal/useTerminalSessions";
import TerminalSessionToolbar from "@/views/web-terminal/TerminalSessionToolbar.vue";

const session = (
  id = "session-1",
  persistent = true,
): TerminalSessionRecord => ({
  id,
  persistent,
  backend: "local",
  targetId: "local",
  title: id,
  phase: "running",
  cols: 80,
  rows: 24,
  createdAt: "",
  updatedAt: "",
  errorCode: null,
  errorMessage: null,
  exitCode: null,
});
const leases: ReturnType<typeof useTerminalPageLease>[] = [];
const lease = () => {
  const onSuspend = vi.fn();
  const onResume = vi.fn().mockResolvedValue(undefined);
  const controller = useTerminalPageLease({ onSuspend, onResume });
  leases.push(controller);
  controller.start();
  return { controller, onSuspend, onResume };
};

beforeEach(() => {
  vi.useFakeTimers();
  vi.spyOn(TerminalAPI, "registerPage").mockResolvedValue({
    id: "page-1",
    expiresAt: "",
  });
  vi.spyOn(TerminalAPI, "heartbeatPage").mockResolvedValue({
    id: "page-1",
    expiresAt: "",
  });
  vi.spyOn(TerminalAPI, "releasePage").mockResolvedValue(undefined);
});
afterEach(() => {
  for (const controller of leases.splice(0)) controller.dispose();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

describe("terminal page ownership", () => {
  it("shares one registration across sessions and renews every 30 seconds", async () => {
    const { controller } = lease();
    expect(
      await Promise.all([controller.ensurePageId(), controller.ensurePageId()]),
    ).toEqual(["page-1", "page-1"]);
    await vi.advanceTimersByTimeAsync(30_000);
    expect(TerminalAPI.registerPage).toHaveBeenCalledTimes(1);
    expect(TerminalAPI.heartbeatPage).toHaveBeenCalledWith("page-1");
    expect(TerminalAPI.releasePage).not.toHaveBeenCalled();
  });

  it("releases on pagehide, never renews a hidden page, and releases only once on unmount", async () => {
    const { controller, onSuspend } = lease();
    await controller.ensurePageId();
    window.dispatchEvent(new Event("pagehide"));
    expect(onSuspend).toHaveBeenCalledTimes(1);
    await vi.advanceTimersByTimeAsync(150_000);
    expect(TerminalAPI.heartbeatPage).not.toHaveBeenCalled();
    await expect(controller.ensurePageId()).rejects.toMatchObject({
      name: "AbortError",
    });
    controller.dispose();
    expect(TerminalAPI.releasePage).toHaveBeenCalledExactlyOnceWith("page-1");
  });

  it("releases on route unmount and cleans up a registration that finishes after departure", async () => {
    let resolve!: (value: { id: string; expiresAt: string }) => void;
    vi.mocked(TerminalAPI.registerPage).mockImplementation(
      () =>
        new Promise((done) => {
          resolve = done;
        }),
    );
    const { controller } = lease();
    const pending = controller.ensurePageId();
    const rejected = expect(pending).rejects.toMatchObject({
      name: "AbortError",
    });
    controller.dispose();
    resolve({ id: "late-page", expiresAt: "" });
    await rejected;
    expect(TerminalAPI.releasePage).toHaveBeenCalledWith("late-page");
  });

  it("registers a new page and refreshes sessions after restoration from browser cache", async () => {
    const { controller, onResume } = lease();
    await controller.ensurePageId();
    window.dispatchEvent(new Event("pagehide"));
    vi.mocked(TerminalAPI.registerPage).mockResolvedValue({
      id: "page-2",
      expiresAt: "",
    });
    const restored = new Event("pageshow");
    Object.defineProperty(restored, "persisted", { value: true });
    window.dispatchEvent(restored);
    await flushPromises();
    expect(await controller.ensurePageId()).toBe("page-2");
    expect(onResume).toHaveBeenCalledTimes(1);
  });

  it("replaces expired leases without treating temporary network failure as departure", async () => {
    const { controller, onResume } = lease();
    await controller.ensurePageId();
    vi.mocked(TerminalAPI.heartbeatPage).mockRejectedValueOnce(
      new Error("offline"),
    );
    await vi.advanceTimersByTimeAsync(30_000);
    expect(TerminalAPI.releasePage).not.toHaveBeenCalled();
    vi.mocked(TerminalAPI.heartbeatPage).mockRejectedValueOnce({
      response: { status: 409 },
    });
    vi.mocked(TerminalAPI.registerPage).mockResolvedValue({
      id: "page-2",
      expiresAt: "",
    });
    await vi.advanceTimersByTimeAsync(30_000);
    expect(await controller.ensurePageId()).toBe("page-2");
    expect(onResume).toHaveBeenCalledTimes(1);
  });

  it.each([
    new Error("offline"),
    new DOMException("Session refresh superseded", "AbortError"),
  ])(
    "retries failed or superseded session restoration: %s",
    async (failure) => {
      const { controller, onResume } = lease();
      await controller.ensurePageId();
      window.dispatchEvent(new Event("pagehide"));
      onResume.mockRejectedValueOnce(failure);
      const restored = new Event("pageshow");
      Object.defineProperty(restored, "persisted", { value: true });
      window.dispatchEvent(restored);
      await flushPromises();
      expect(onResume).toHaveBeenCalledTimes(1);
      await vi.advanceTimersByTimeAsync(30_000);
      expect(onResume).toHaveBeenCalledTimes(2);
      await vi.advanceTimersByTimeAsync(30_000);
      expect(onResume).toHaveBeenCalledTimes(2);
    },
  );

  it("does not release a healthy lease when session restoration returns a conflict", async () => {
    const { controller, onResume } = lease();
    await controller.ensurePageId();
    window.dispatchEvent(new Event("pagehide"));
    vi.mocked(TerminalAPI.registerPage).mockResolvedValue({
      id: "page-2",
      expiresAt: "",
    });
    onResume.mockRejectedValue({ response: { status: 409 } });
    const restored = new Event("pageshow");
    Object.defineProperty(restored, "persisted", { value: true });
    window.dispatchEvent(restored);
    await flushPromises();
    await vi.advanceTimersByTimeAsync(30_000);
    expect(onResume).toHaveBeenCalledTimes(2);
    expect(TerminalAPI.releasePage).toHaveBeenCalledExactlyOnceWith("page-1");
    expect(await controller.ensurePageId()).toBe("page-2");
  });

  it("heartbeats the restored page while an old page request is still pending", async () => {
    const { controller } = lease();
    await controller.ensurePageId();
    let finish!: (page: { id: string; expiresAt: string }) => void;
    vi.mocked(TerminalAPI.heartbeatPage).mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          finish = resolve;
        }),
    );
    await vi.advanceTimersByTimeAsync(30_000);
    window.dispatchEvent(new Event("pagehide"));
    vi.mocked(TerminalAPI.registerPage).mockResolvedValue({
      id: "page-2",
      expiresAt: "",
    });
    const restored = new Event("pageshow");
    Object.defineProperty(restored, "persisted", { value: true });
    window.dispatchEvent(restored);
    await flushPromises();
    await vi.advanceTimersByTimeAsync(30_000);
    expect(TerminalAPI.heartbeatPage).toHaveBeenCalledWith("page-2");
    finish({ id: "page-1", expiresAt: "" });
    await flushPromises();
    expect(await controller.ensurePageId()).toBe("page-2");
  });

  it("does not open duplicate attachments when restoring overlaps a heartbeat", async () => {
    const { controller, onResume } = lease();
    await controller.ensurePageId();
    window.dispatchEvent(new Event("pagehide"));
    let finish!: () => void;
    onResume.mockImplementation(
      () =>
        new Promise<void>((resolve) => {
          finish = resolve;
        }),
    );
    const restored = new Event("pageshow");
    Object.defineProperty(restored, "persisted", { value: true });
    window.dispatchEvent(restored);
    await flushPromises();
    await vi.advanceTimersByTimeAsync(30_000);
    expect(onResume).toHaveBeenCalledTimes(1);
    finish();
    await flushPromises();
    await vi.advanceTimersByTimeAsync(30_000);
    expect(onResume).toHaveBeenCalledTimes(1);
  });

  it("invalidates an in-flight restoration when its page leaves again", async () => {
    const { controller, onResume } = lease();
    await controller.ensurePageId();
    window.dispatchEvent(new Event("pagehide"));
    let isCurrent!: () => boolean;
    let finish!: () => void;
    onResume.mockImplementation((guard: () => boolean) => {
      isCurrent = guard;
      return new Promise<void>((resolve) => {
        finish = resolve;
      });
    });
    const restored = new Event("pageshow");
    Object.defineProperty(restored, "persisted", { value: true });
    window.dispatchEvent(restored);
    await flushPromises();
    expect(isCurrent()).toBe(true);
    window.dispatchEvent(new Event("pagehide"));
    expect(isCurrent()).toBe(false);
    finish();
    await flushPromises();
    await vi.advanceTimersByTimeAsync(30_000);
    expect(onResume).toHaveBeenCalledTimes(1);
  });

  it("clears leases when the server runtime changes", async () => {
    const { controller } = lease();
    await controller.ensurePageId();
    controller.invalidate();
    vi.mocked(TerminalAPI.registerPage).mockResolvedValue({
      id: "new-runtime-page",
      expiresAt: "",
    });
    expect(await controller.ensurePageId()).toBe("new-runtime-page");
  });
});

describe("session persistence", () => {
  it("creates default-persistent sessions with page ownership", async () => {
    vi.spyOn(TerminalAPI, "createSession").mockResolvedValue(session());
    const controller = useTerminalSessions({
      selectedTargetId: ref("local"),
      getPageId: async () => "page-1",
    });
    await controller.createSession("local", { cols: 80, rows: 24 });
    expect(TerminalAPI.createSession).toHaveBeenCalledWith(
      "local",
      {
        cols: 80,
        rows: 24,
        persistent: true,
        pageId: "page-1",
      },
      expect.any(AbortSignal),
    );
    controller.dispose();
  });

  it("saves only the requested session and leaves server state unchanged on failure", async () => {
    const controller = useTerminalSessions({ selectedTargetId: ref("local") });
    controller.sessions.value = [session(), session("session-2")];
    const update = vi
      .spyOn(TerminalAPI, "updateSessionPersistence")
      .mockResolvedValue(session("session-1", false));
    await controller.setSessionPersistence("session-1", false);
    expect(controller.sessions.value.map((item) => item.persistent)).toEqual([
      false,
      true,
    ]);
    update.mockRejectedValue(new Error("offline"));
    await expect(
      controller.setSessionPersistence("session-1", true),
    ).rejects.toThrow("offline");
    expect(controller.sessions.value[0]?.persistent).toBe(false);
    expect(controller.savingPersistence.value).toBe(false);
    controller.dispose();
  });

  it("disables further saves while pending and ignores older list responses", async () => {
    let finish!: (value: TerminalSessionRecord) => void;
    vi.spyOn(TerminalAPI, "updateSessionPersistence").mockImplementation(
      () =>
        new Promise((resolve) => {
          finish = resolve;
        }),
    );
    let finishList!: (value: {
      runtimeId: string;
      sessions: TerminalSessionRecord[];
    }) => void;
    vi.spyOn(TerminalAPI, "listSessions").mockImplementation(
      () =>
        new Promise((resolve) => {
          finishList = resolve;
        }),
    );
    const controller = useTerminalSessions({ selectedTargetId: ref("local") });
    controller.sessions.value = [session()];
    const loading = controller.loadSessions();
    const saving = controller.setSessionPersistence("session-1", false);
    expect(controller.savingPersistence.value).toBe(true);
    await controller.setSessionPersistence("session-1", true);
    expect(TerminalAPI.updateSessionPersistence).toHaveBeenCalledTimes(1);
    finish(session("session-1", false));
    await saving;
    finishList({ runtimeId: "runtime", sessions: [session()] });
    await loading;
    expect(controller.sessions.value[0]?.persistent).toBe(false);
    controller.dispose();
  });

  it("shows a checked, accessible control with visible guidance and disables ended sessions", async () => {
    const change = vi.fn();
    const wrapper = mount(TerminalSessionToolbar, {
      props: {
        connectionState: "connected",
        canClaimControl: false,
        claimControl: vi.fn(),
        createSession: vi.fn(),
        destroySelectedSession: vi.fn(),
        destroySessionDescription: "",
        handleSessionTabChange: vi.fn(),
        isBooting: false,
        isCreating: false,
        isKilling: false,
        isRenamingSession: false,
        isSavingPersistence: false,
        setSessionPersistence: change,
        keepTerminalFocused: vi.fn(),
        openRenameDialog: vi.fn(),
        openSendDialog: vi.fn(),
        openTargetDrawer: vi.fn(),
        reconnectSession: vi.fn(),
        selectedSession: session(),
        selectedSessionId: "session-1",
        selectedTarget: null,
        sessions: [session()],
        statusTone: "connected",
        toolbarDisabled: false,
      },
      global: {
        plugins: [
          createI18n({
            legacy: false,
            locale: "en",
            missingWarn: false,
            fallbackWarn: false,
            messages: { en: {} },
          }),
        ],
      },
    });
    try {
      const checkbox = wrapper.get('[role="checkbox"]');
      expect(checkbox.attributes("aria-checked")).toBe("true");
      const help = wrapper.get(`#${checkbox.attributes("aria-describedby")}`);
      expect(help.text()).toContain("admin.webTerminal.persistenceDescription");
      expect(help.text()).toContain(
        "admin.webTerminal.persistenceLeaseDescription",
      );
      await checkbox.trigger("click");
      expect(change).toHaveBeenCalledWith(false);
      await wrapper.setProps({ isSavingPersistence: true });
      expect(checkbox.attributes("disabled")).toBeDefined();
      await wrapper.setProps({
        isSavingPersistence: false,
        selectedSession: { ...session(), phase: "closed" },
      });
      expect(checkbox.attributes("disabled")).toBeDefined();
      await wrapper.setProps({ selectedSession: null });
      expect(checkbox.attributes("disabled")).toBeDefined();
    } finally {
      wrapper.unmount();
    }
  });
});
