import { effectScope, ref } from "vue";
import { flushPromises, mount } from "@vue/test-utils";
import { createI18n } from "vue-i18n";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  TerminalAPI,
  type TerminalSessionRecord,
  type TerminalTargetRecord,
} from "@/lib/api/terminal";
import { useTerminalPageLease } from "@/views/web-terminal/useTerminalPageLease";
import { useTerminalSessions } from "@/views/web-terminal/useTerminalSessions";
import { useTerminalTargetEditor } from "@/views/web-terminal/useTerminalTargetEditor";
import TerminalTargetEditorDialog from "@/views/web-terminal/TerminalTargetEditorDialog.vue";
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
  it("lets the server inherit connection persistence when creating sessions", async () => {
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
        pageId: "page-1",
      },
      expect.any(AbortSignal),
    );
    controller.dispose();
  });

  const target = (
    id = "target-1",
    persistent = true,
  ): TerminalTargetRecord => ({
    id,
    persistent,
    name: id,
    host: "example.com",
    port: 22,
    username: "operator",
    authMethod: "password",
    trustedHostKey: null,
    credentialConfigured: true,
    passphraseConfigured: false,
    revision: 1,
    lastVerifiedAt: null,
    createdAt: "",
    updatedAt: "",
  });
  const editor = () => {
    const scope = effectScope();
    const createTarget = vi.fn().mockResolvedValue(target());
    const updateTarget = vi.fn().mockResolvedValue(target("target-1", false));
    const controller = scope.run(() =>
      useTerminalTargetEditor({ createTarget, updateTarget }),
    )!;
    cleanups.push(() => {
      controller.close();
      scope.stop();
    });
    return { controller, createTarget, updateTarget };
  };

  it("defaults new connections to persistent and saves the choice with connection information", async () => {
    const { controller, createTarget } = editor();
    controller.beginCreate();
    expect(controller.draft.persistent).toBe(true);
    Object.assign(controller.draft, {
      name: "Host",
      host: "example.com",
      username: "user",
      clearCredential: true,
      trustedHostKey: { algorithm: "ssh-ed25519", fingerprint: "SHA256:test" },
      persistent: false,
    });
    await controller.save();
    expect(createTarget).toHaveBeenCalledWith(
      expect.objectContaining({ name: "Host", persistent: false }),
    );
  });

  it("edits connection persistence without an active session or SSH retest; Cancel discards changes", async () => {
    const { controller, updateTarget } = editor();
    const saved = target();
    controller.beginEdit(saved);
    controller.draft.persistent = false;
    expect(controller.requiresSessionTermination.value).toBe(false);
    expect(controller.canSave.value).toBe(true);
    expect(updateTarget).not.toHaveBeenCalled();
    controller.close();
    controller.beginEdit(saved);
    expect(controller.draft.persistent).toBe(true);
    controller.draft.persistent = false;
    await controller.save();
    expect(updateTarget).toHaveBeenCalledExactlyOnceWith(
      "target-1",
      expect.objectContaining({ persistent: false, revision: 1 }),
      false,
      undefined,
    );
    expect(saved.persistent).toBe(true);
    controller.beginEdit(target("target-2", true));
    expect(controller.draft.persistent).toBe(true);
  });

  it("keeps failed saves editable and prevents duplicate saves", async () => {
    const { controller, updateTarget } = editor();
    const saved = target();
    controller.beginEdit(saved);
    controller.draft.persistent = false;
    let fail!: (error: Error) => void;
    updateTarget.mockImplementation(
      () =>
        new Promise((_, reject) => {
          fail = reject;
        }),
    );
    const saving = controller.save();
    expect(controller.saving.value).toBe(true);
    await controller.save();
    expect(updateTarget).toHaveBeenCalledTimes(1);
    fail(new Error("offline"));
    await saving;
    expect(controller.open.value).toBe(true);
    expect(controller.error.value).toContain("offline");
    expect(controller.saving.value).toBe(false);
    expect(saved.persistent).toBe(true);
  });

  it("updates only the edited connection's active sessions and ignores an older poll", async () => {
    const controller = useTerminalSessions({ selectedTargetId: ref("local") });
    controller.sessions.value = [
      session(),
      { ...session("other"), targetId: "other" },
      { ...session("ended"), phase: "closed" },
    ];
    let finish!: (value: {
      runtimeId: string;
      sessions: TerminalSessionRecord[];
    }) => void;
    vi.spyOn(TerminalAPI, "listSessions").mockImplementation(
      () =>
        new Promise((resolve) => {
          finish = resolve;
        }),
    );
    const loading = controller.loadSessions();
    controller.applyTargetPersistence("local", false);
    finish({ runtimeId: "runtime", sessions: [session()] });
    await loading;
    expect(controller.sessions.value.map((s) => s.persistent)).toEqual([
      false,
      true,
      true,
    ]);
    controller.dispose();
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
  it("places the persistence checkbox in the host/port connection form and disables it while saving", async () => {
    const { controller, updateTarget } = editor();
    controller.beginEdit(target());
    const wrapper = mount(TerminalTargetEditorDialog, {
      props: { editor: controller, activeSessionCount: 0, onDelete: vi.fn() },
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
      expect(wrapper.get("#terminal-target-host").exists()).toBe(true);
      expect(wrapper.get("#terminal-target-port").exists()).toBe(true);
      const checkbox = wrapper.get('[role="checkbox"][aria-describedby]');
      expect(checkbox.attributes("aria-checked")).toBe("true");
      const help = wrapper.get(`#${checkbox.attributes("aria-describedby")}`);
      expect(help.text()).toContain("admin.webTerminal.persistenceDescription");
      await checkbox.trigger("click");
      expect(controller.draft.persistent).toBe(false);
      expect(updateTarget).not.toHaveBeenCalled();
      controller.saving.value = true;
      await flushPromises();
      expect(checkbox.attributes("disabled")).toBeDefined();
      controller.saving.value = false;
      await wrapper.get("form").trigger("submit");
      expect(updateTarget).toHaveBeenCalledWith(
        "target-1",
        expect.objectContaining({ persistent: false }),
        false,
        undefined,
      );
    } finally {
      wrapper.unmount();
    }
  });

  it("keeps the session rename form limited to the name", () => {
    const wrapper = mount(TerminalRenameDialog, {
      props: { open: true, value: "Shell", renaming: false },
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
    expect(wrapper.find('[role="checkbox"]').exists()).toBe(false);
    expect(wrapper.text()).not.toContain(
      "admin.webTerminal.persistenceDescription",
    );
    wrapper.unmount();
  });
});
