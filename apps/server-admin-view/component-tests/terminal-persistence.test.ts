import { effectScope, ref } from "vue";
import { flushPromises, mount } from "@vue/test-utils";
import { createI18n } from "vue-i18n";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { TerminalAPI, type TerminalSessionRecord } from "@/lib/api/terminal";
import { useTerminalPageLease } from "@/views/web-terminal/useTerminalPageLease";
import { useTerminalSessions } from "@/views/web-terminal/useTerminalSessions";
import { useTerminalDialogs } from "@/views/web-terminal/useTerminalDialogs";
import TerminalRenameDialog from "@/views/web-terminal/TerminalRenameDialog.vue";
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
const cleanups: Array<() => void> = [];
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
  for (const cleanup of cleanups.splice(0)) cleanup();
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

  const editor = () => {
    const scope = effectScope();
    const result = scope.run(() => {
      const controller = useTerminalSessions({
        selectedTargetId: ref("local"),
      });
      controller.sessions.value = [session(), session("session-2")];
      controller.selectSession("session-1");
      const dialogs = useTerminalDialogs({
        activeAttachment: ref(null),
        cancelRenameSession: controller.cancelRename,
        clearArmedModifier: vi.fn(),
        focusTerminal: vi.fn(),
        selectedSession: controller.selectedSession,
        sendPayloadNow: vi.fn(),
        sessions: controller.sessions,
        translate: (key) => key,
        updateSessionTitle: controller.renameSession,
      });
      return { controller, dialogs };
    })!;
    cleanups.push(() => {
      result.controller.dispose();
      scope.stop();
    });
    return result;
  };

  it("keeps persistence changes in the form until Save and discards Cancel", async () => {
    const update = vi
      .spyOn(TerminalAPI, "updateSession")
      .mockResolvedValue(session("session-1", false));
    const { controller, dialogs } = editor();
    dialogs.openRenameDialog();
    expect(dialogs.renameDialogPersistent.value).toBe(true);
    dialogs.renameDialogPersistent.value = false;
    expect(update).not.toHaveBeenCalled();
    expect(controller.sessions.value[0]?.persistent).toBe(true);
    dialogs.renameDialogOpen.value = false;
    dialogs.openRenameDialog();
    expect(dialogs.renameDialogPersistent.value).toBe(true);
    dialogs.renameDialogPersistent.value = false;
    dialogs.renameDialogValue.value = "New name";
    update.mockResolvedValue({
      ...session("session-1", false),
      title: "New name",
    });
    await dialogs.submitRenameDialog();
    expect(update).toHaveBeenCalledExactlyOnceWith(
      "session-1",
      { title: "New name", persistent: false },
      expect.any(AbortSignal),
    );
    expect(controller.sessions.value[0]).toMatchObject({
      title: "New name",
      persistent: false,
    });
    expect(controller.sessions.value[1]?.persistent).toBe(true);
    expect(dialogs.renameDialogOpen.value).toBe(false);
  });

  it("keeps a failed save open and preserves the server state and editable draft", async () => {
    vi.spyOn(TerminalAPI, "updateSession").mockRejectedValue(
      new Error("offline"),
    );
    const { controller, dialogs } = editor();
    dialogs.openRenameDialog();
    dialogs.renameDialogPersistent.value = false;
    dialogs.renameDialogValue.value = "Changed";
    await dialogs.submitRenameDialog();
    expect(dialogs.renameDialogOpen.value).toBe(true);
    expect(dialogs.renameDialogPersistent.value).toBe(false);
    expect(dialogs.renameDialogValue.value).toBe("Changed");
    expect(dialogs.isRenamingSession.value).toBe(false);
    expect(controller.sessions.value[0]).toMatchObject({
      title: "session-1",
      persistent: true,
    });
  });

  it("prevents duplicate saves and ignores list responses older than the edit", async () => {
    let finish!: (value: TerminalSessionRecord) => void;
    const update = vi.spyOn(TerminalAPI, "updateSession").mockImplementation(
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
    const { controller, dialogs } = editor();
    const loading = controller.loadSessions();
    dialogs.openRenameDialog();
    dialogs.renameDialogPersistent.value = false;
    const saving = dialogs.submitRenameDialog();
    expect(dialogs.isRenamingSession.value).toBe(true);
    await dialogs.submitRenameDialog();
    expect(update).toHaveBeenCalledTimes(1);
    finish(session("session-1", false));
    await saving;
    finishList({ runtimeId: "runtime", sessions: [session()] });
    await loading;
    expect(controller.sessions.value[0]?.persistent).toBe(false);
  });

  it("does not apply an old save to another session's newly opened form", async () => {
    let finish!: (value: TerminalSessionRecord) => void;
    vi.spyOn(TerminalAPI, "updateSession").mockImplementation(
      () =>
        new Promise((resolve) => {
          finish = resolve;
        }),
    );
    const { controller, dialogs } = editor();
    dialogs.openRenameDialog();
    dialogs.renameDialogPersistent.value = false;
    const saving = dialogs.submitRenameDialog();
    controller.selectSession("session-2");
    expect(dialogs.renameDialogOpen.value).toBe(false);
    dialogs.openRenameDialog();
    finish(session("session-1", false));
    await saving;
    expect(dialogs.renameDialogOpen.value).toBe(true);
    expect(dialogs.renameDialogValue.value).toBe("session-2");
    expect(dialogs.renameDialogPersistent.value).toBe(true);
  });

  it("saves the name alone when persistence is unchanged or the session has ended", async () => {
    const update = vi
      .spyOn(TerminalAPI, "updateSession")
      .mockResolvedValue(session());
    const { controller, dialogs } = editor();
    dialogs.openRenameDialog();
    await dialogs.submitRenameDialog();
    expect(update).toHaveBeenLastCalledWith(
      "session-1",
      { title: "session-1" },
      expect.any(AbortSignal),
    );
    dialogs.openRenameDialog();
    dialogs.renameDialogPersistent.value = false;
    controller.updateSessionPhase("session-1", "closed");
    expect(dialogs.renameDialogPersistenceDisabled.value).toBe(true);
    await dialogs.submitRenameDialog();
    expect(update).toHaveBeenLastCalledWith(
      "session-1",
      { title: "session-1" },
      expect.any(AbortSignal),
    );
  });

  it("keeps persistence controls and their guidance out of the terminal toolbar", async () => {
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
      expect(wrapper.find('[role="checkbox"]').exists()).toBe(false);
      expect(wrapper.text()).not.toContain(
        "admin.webTerminal.persistenceDescription",
      );
    } finally {
      wrapper.unmount();
    }
  });
  it("shows the accessible persistence field inside the edit form, saving only on submit", async () => {
    const wrapper = mount(TerminalRenameDialog, {
      props: {
        open: true,
        value: "Shell",
        renaming: false,
        persistent: true,
        persistenceDisabled: false,
      },
      global: {
        stubs: { DialogContent: { template: "<div><slot /></div>" } },
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
      await flushPromises();
      const checkbox = wrapper.get('[role="checkbox"]');
      expect(checkbox.attributes("aria-checked")).toBe("true");
      const help = wrapper.get(`#${checkbox.attributes("aria-describedby")}`);
      expect(help.text()).toContain("admin.webTerminal.persistenceDescription");
      await checkbox.trigger("click");
      expect(wrapper.emitted("update:persistent")).toEqual([[false]]);
      expect(wrapper.emitted("submit")).toBeUndefined();
      await wrapper.get("form").trigger("submit");
      expect(wrapper.emitted("submit")).toHaveLength(1);
      await wrapper.setProps({ renaming: true });
      expect(checkbox.attributes("disabled")).toBeDefined();
      await wrapper.setProps({ renaming: false, persistenceDisabled: true });
      expect(checkbox.attributes("disabled")).toBeDefined();
    } finally {
      wrapper.unmount();
    }
  });
});
