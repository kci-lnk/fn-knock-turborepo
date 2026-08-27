import { ref } from "vue";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type {
  TerminalAttachmentRecord,
  TerminalSessionRecord,
  TerminalTargetRecord,
} from "@/lib/api/terminal";
import { useTerminalAttachment } from "@/views/web-terminal/useTerminalAttachment";
import { extractTerminalError } from "@/views/web-terminal/terminal-errors";
import { normalizeTerminalDimensions } from "@/views/web-terminal/terminal-dimensions";
import { useTerminalInputQueue } from "@/views/web-terminal/useTerminalInputQueue";
import { useTerminalSessions } from "@/views/web-terminal/useTerminalSessions";
import { useTerminalTargetEditor } from "@/views/web-terminal/useTerminalTargetEditor";
import { useTerminalTargetDeletion } from "@/views/web-terminal/useTerminalTargetDeletion";
import { useTerminalTargets } from "@/views/web-terminal/useTerminalTargets";
import { useTerminalViewport } from "@/views/web-terminal/useTerminalViewport";

const terminalApi = vi.hoisted(() => ({
  claimControl: vi.fn(),
  createAttachment: vi.fn(),
  deleteTarget: vi.fn(),
  deleteSession: vi.fn(),
  detachAttachment: vi.fn(),
  listSessions: vi.fn(),
  pollAttachmentEvents: vi.fn(),
  probeHostKey: vi.fn(),
  resizeAttachment: vi.fn(),
  sendInput: vi.fn(),
  testConnection: vi.fn(),
}));

vi.mock("@/lib/api/terminal", async () => {
  const actual =
    await vi.importActual<typeof import("@/lib/api/terminal")>(
      "@/lib/api/terminal",
    );
  return { ...actual, TerminalAPI: terminalApi };
});

const session = (
  overrides: Partial<TerminalSessionRecord> = {},
): TerminalSessionRecord => ({
  id: "session-1",
  targetId: "target-1",
  title: "Shell 1",
  phase: "running",
  cols: 120,
  rows: 32,
  errorCode: null,
  errorMessage: null,
  exitCode: null,
  createdAt: "2026-08-28T00:00:00Z",
  updatedAt: "2026-08-28T00:00:00Z",
  ...overrides,
});

const attachment = (
  id: string,
  overrides: Partial<TerminalAttachmentRecord> = {},
): TerminalAttachmentRecord => ({
  id,
  sessionId: "session-1",
  role: "controller",
  generation: 1,
  cursor: 0,
  expiresAt: "2026-08-28T00:02:00Z",
  transport: "http-polling",
  ...overrides,
});

const target = (
  overrides: Partial<TerminalTargetRecord> = {},
): TerminalTargetRecord => ({
  id: "target-1",
  name: "Production",
  host: "server.example.com",
  port: 22,
  username: "deploy",
  authMethod: "password",
  trustedHostKey: {
    algorithm: "ssh-ed25519",
    fingerprint: "SHA256:current",
  },
  credentialConfigured: true,
  passphraseConfigured: false,
  revision: 1,
  lastVerifiedAt: "2026-08-28T00:00:00Z",
  createdAt: "2026-08-28T00:00:00Z",
  updatedAt: "2026-08-28T00:00:00Z",
  ...overrides,
});

const deferred = <T>() => {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, reject, resolve };
};

const settleMicrotasks = async () => {
  for (let index = 0; index < 8; index += 1) await Promise.resolve();
};

const createAttachmentController = (
  getTerminalSize = () => ({ cols: 120, rows: 32 }),
) =>
  useTerminalAttachment({
    getTerminalSize,
    onOutput: vi.fn(),
    onReset: vi.fn(),
    onSessionState: vi.fn(),
  });

beforeEach(() => {
  vi.clearAllMocks();
  localStorage.clear();
  terminalApi.claimControl.mockReset();
  terminalApi.createAttachment.mockReset();
  terminalApi.deleteSession.mockReset();
  terminalApi.deleteTarget.mockReset();
  terminalApi.detachAttachment.mockReset().mockResolvedValue(undefined);
  terminalApi.listSessions.mockReset();
  terminalApi.pollAttachmentEvents.mockReset();
  terminalApi.probeHostKey.mockReset();
  terminalApi.resizeAttachment.mockReset().mockResolvedValue(undefined);
  terminalApi.sendInput.mockReset().mockResolvedValue(undefined);
  terminalApi.testConnection.mockReset();
});

afterEach(() => {
  vi.useRealTimers();
});

describe("SSH terminal attachment behavior", () => {
  it("normalizes transient mobile dimensions before attachment creation", async () => {
    const blockedPoll = deferred<never>();
    terminalApi.createAttachment.mockResolvedValue(attachment("attachment-1"));
    terminalApi.pollAttachmentEvents.mockReturnValue(blockedPoll.promise);
    const controller = createAttachmentController(() => ({
      cols: 0,
      rows: Number.POSITIVE_INFINITY,
    }));

    await controller.attach(session());
    expect(terminalApi.createAttachment).toHaveBeenCalledWith(
      "session-1",
      { cols: 40, rows: 32 },
      expect.any(AbortSignal),
    );
    expect(normalizeTerminalDimensions({ cols: 999, rows: 1 })).toEqual({
      cols: 400,
      rows: 12,
    });

    await controller.detach();
  });

  it("rebuilds only the attachment after long-poll transport failure", async () => {
    vi.useFakeTimers();
    const blockedPoll = deferred<never>();
    terminalApi.createAttachment
      .mockResolvedValueOnce(attachment("attachment-1"))
      .mockResolvedValueOnce(attachment("attachment-2"));
    terminalApi.pollAttachmentEvents
      .mockRejectedValueOnce(new Error("poll transport failed"))
      .mockReturnValueOnce(blockedPoll.promise);

    const controller = createAttachmentController();
    await controller.attach(session());
    await settleMicrotasks();

    expect(controller.state.value).toMatchObject({
      kind: "reconnecting",
      sessionId: "session-1",
    });
    expect(terminalApi.detachAttachment).toHaveBeenCalledWith("attachment-1");

    await vi.advanceTimersByTimeAsync(500);
    await settleMicrotasks();

    expect(terminalApi.createAttachment).toHaveBeenCalledTimes(2);
    expect(controller.attachment.value?.id).toBe("attachment-2");
    expect(terminalApi.deleteSession).not.toHaveBeenCalled();

    await controller.detach();
  });

  it("applies controller fencing events and retries input with one sequence", async () => {
    const blockedPoll = deferred<never>();
    terminalApi.createAttachment.mockResolvedValue(attachment("attachment-1"));
    terminalApi.pollAttachmentEvents
      .mockResolvedValueOnce({
        events: [
          {
            type: "control",
            cursor: 1,
            role: "viewer",
            generation: 7,
          },
          {
            type: "control",
            cursor: 2,
            role: "controller",
            generation: 6,
          },
        ],
        nextCursor: 2,
      })
      .mockReturnValueOnce(blockedPoll.promise);
    terminalApi.claimControl.mockResolvedValue(
      attachment("attachment-1", { generation: 8, role: "controller" }),
    );
    terminalApi.sendInput
      .mockRejectedValueOnce(new Error("transient network failure"))
      .mockResolvedValueOnce(undefined);

    const controller = createAttachmentController();
    await controller.attach(session());
    await settleMicrotasks();

    expect(controller.attachment.value).toMatchObject({
      role: "viewer",
      generation: 7,
    });
    expect(controller.canClaimControl.value).toBe(true);
    expect(controller.canInput.value).toBe(false);

    await controller.claimControl();
    expect(terminalApi.claimControl).toHaveBeenCalledWith(
      "attachment-1",
      7,
      expect.any(AbortSignal),
    );
    expect(controller.canInput.value).toBe(true);

    await controller.sendInput("aGVsbG8=");
    expect(terminalApi.sendInput).toHaveBeenCalledTimes(2);
    expect(terminalApi.sendInput.mock.calls[0]).toEqual(
      terminalApi.sendInput.mock.calls[1],
    );
    expect(terminalApi.sendInput).toHaveBeenLastCalledWith(
      "attachment-1",
      {
        dataBase64: "aGVsbG8=",
        sequence: 1,
        generation: 8,
      },
      expect.any(AbortSignal),
    );

    await controller.detach();
  });

  it("keeps creating and authentication phases read-only", async () => {
    const blockedPoll = deferred<never>();
    terminalApi.createAttachment.mockResolvedValue(attachment("attachment-1"));
    terminalApi.pollAttachmentEvents
      .mockResolvedValueOnce({ events: [], nextCursor: 0 })
      .mockReturnValueOnce(blockedPoll.promise);

    const controller = createAttachmentController();
    await controller.attach(session({ phase: "authenticating" }));
    await settleMicrotasks();

    expect(controller.canInput.value).toBe(false);
    expect(controller.canClaimControl.value).toBe(false);
    expect(controller.readOnly.value).toBe(true);

    controller.syncSession(session({ phase: "running" }));
    expect(controller.canInput.value).toBe(true);
    expect(controller.readOnly.value).toBe(false);

    controller.syncSession(session({ phase: "authenticating" }));
    expect(controller.livePhase.value).toBe("running");
    expect(controller.canInput.value).toBe(true);

    await controller.detach();
  });

  it("clears before a reset snapshot and disables control on terminal status", async () => {
    const firstPoll = deferred<{
      events: Array<
        | {
            type: "output";
            cursor: number;
            dataBase64: string;
            reset: boolean;
          }
        | { type: "status"; cursor: number; phase: "lost" }
      >;
      nextCursor: number;
    }>();
    const blockedPoll = deferred<never>();
    terminalApi.createAttachment.mockResolvedValue(attachment("attachment-1"));
    terminalApi.pollAttachmentEvents
      .mockReturnValueOnce(firstPoll.promise)
      .mockReturnValueOnce(blockedPoll.promise);
    const calls: string[] = [];
    const controller = useTerminalAttachment({
      getTerminalSize: () => ({ cols: 120, rows: 32 }),
      onOutput: () => calls.push("output"),
      onReset: () => calls.push("reset"),
      onSessionState: () => calls.push("status"),
    });

    await controller.attach(session());
    calls.length = 0;
    firstPoll.resolve({
      events: [
        {
          type: "output",
          cursor: 1,
          dataBase64: "c25hcHNob3Q=",
          reset: true,
        },
        { type: "status", cursor: 2, phase: "lost" },
      ],
      nextCursor: 2,
    });
    await settleMicrotasks();

    expect(calls).toEqual(["reset", "output", "status"]);
    expect(controller.livePhase.value).toBe("lost");
    expect(controller.canInput.value).toBe(false);
    expect(controller.canClaimControl.value).toBe(false);
    expect(controller.readOnly.value).toBe(true);
    await controller.detach();
  });

  it("reports input API failures without mutating a readonly connection state", async () => {
    const connectionState = ref<"idle" | "connecting" | "connected" | "error">(
      "connected",
    );
    const onSendError = vi.fn();
    const sendInput = vi.fn().mockRejectedValue(
      Object.assign(new Error("Request failed"), {
        response: {
          data: {
            errorCode: "controller_conflict",
            message: "Another attachment controls this session",
          },
        },
      }),
    );
    const controller = useTerminalInputQueue({
      activeAttachment: ref(attachment("attachment-1")),
      connectionState,
      onSendError,
      selectedSessionId: ref("session-1"),
      sendInput,
      translate: (key) => key,
    });

    await expect(controller.sendTerminalPayloadNow("whoami\r")).rejects.toThrow(
      "Request failed",
    );
    expect(onSendError).toHaveBeenCalledOnce();
    expect(connectionState.value).toBe("connected");
  });

  it("splits large UTF-8 input into bounded ordered requests", async () => {
    const sendInput = vi.fn().mockResolvedValue(undefined);
    const controller = useTerminalInputQueue({
      activeAttachment: ref(attachment("attachment-1")),
      connectionState: ref("connected"),
      onSendError: vi.fn(),
      selectedSessionId: ref("session-1"),
      sendInput,
      translate: (key) => key,
    });
    const payload = `${"界".repeat(900)}\u001b[M${String.fromCharCode(32, 33, 34)}`;

    await controller.sendTerminalPayloadNow(payload);

    expect(sendInput.mock.calls.length).toBeGreaterThan(1);
    const decoded = sendInput.mock.calls
      .map(([, encoded]) =>
        Uint8Array.from(atob(encoded), (char) => char.charCodeAt(0)),
      )
      .reduce((combined, chunk) => {
        const next = new Uint8Array(combined.length + chunk.length);
        next.set(combined);
        next.set(chunk, combined.length);
        return next;
      }, new Uint8Array());
    expect([...decoded]).toEqual([
      ...new TextEncoder().encode("界".repeat(900)),
      0x1b,
      0x5b,
      0x4d,
      32,
      33,
      34,
    ]);
    for (const [, encoded] of sendInput.mock.calls) {
      expect(atob(encoded).length).toBeLessThanOrEqual(1024);
    }
  });

  it("does not drop a large input event while batching", async () => {
    const sendInput = vi.fn().mockResolvedValue(undefined);
    const controller = useTerminalInputQueue({
      activeAttachment: ref(attachment("attachment-1")),
      connectionState: ref("connected"),
      onSendError: vi.fn(),
      selectedSessionId: ref("session-1"),
      sendInput,
      translate: (key) => key,
    });
    const payload = "a".repeat(70 * 1024);

    controller.queueTerminalInput(payload);
    await controller.flushPendingInput();

    expect(sendInput).toHaveBeenCalledTimes(70);
    expect(
      sendInput.mock.calls.reduce(
        (bytes, [, encoded]) => bytes + atob(encoded).length,
        0,
      ),
    ).toBe(payload.length);
  });
});

describe("SSH terminal viewport behavior", () => {
  it("opens and closes the mobile target drawer independently of sidebar state", () => {
    const requestAnimationFrame = vi
      .spyOn(window, "requestAnimationFrame")
      .mockImplementation((callback) => {
        callback(0);
        return 1;
      });
    const controller = useTerminalViewport({
      focusTerminal: vi.fn(),
      scheduleFit: vi.fn(),
      syncTerminalTextInputAnchor: vi.fn(),
    });

    controller.setTargetDrawerOpen(true);
    expect(controller.targetDrawerOpen.value).toBe(true);
    controller.closeTargetDrawer();
    expect(controller.targetDrawerOpen.value).toBe(false);

    controller.toggleSidebar();
    expect(localStorage.getItem("fn-knock:terminal:sidebar-collapsed")).toBe(
      "true",
    );
    expect(controller.targetDrawerOpen.value).toBe(false);
    requestAnimationFrame.mockRestore();
  });
});

describe("SSH terminal collection and editor behavior", () => {
  it("preserves stable terminal API messages and error codes", () => {
    expect(
      extractTerminalError({
        message: "Request failed with status code 409",
        response: {
          data: {
            errorCode: "target_revision_conflict",
            message: "The SSH target was updated in another browser",
          },
        },
      }),
    ).toEqual({
      activeSessionCount: null,
      confirmationToken: null,
      errorCode: "target_revision_conflict",
      message: "The SSH target was updated in another browser",
    });
  });

  it("invalidates the selected session when runtimeId changes", async () => {
    const onRuntimeChanged = vi.fn();
    terminalApi.listSessions
      .mockResolvedValueOnce({ runtimeId: "runtime-1", sessions: [session()] })
      .mockResolvedValueOnce({ runtimeId: "runtime-2", sessions: [] });
    const controller = useTerminalSessions({
      selectedTargetId: ref("target-1"),
      onRuntimeChanged,
    });

    await controller.loadSessions();
    expect(controller.selectedSessionId.value).toBe("session-1");

    await controller.loadSessions();
    expect(onRuntimeChanged).toHaveBeenCalledWith("runtime-1", "runtime-2");
    expect(controller.selectedSessionId.value).toBe("");
    expect(controller.sessions.value).toEqual([]);
    controller.dispose();
  });

  it("retries target mutations with force only after a server conflict", async () => {
    const conflict = Object.assign(new Error("Request failed"), {
      response: {
        data: {
          activeSessionCount: 2,
          confirmationToken: "confirm-update-1",
          errorCode: "conflict",
          message: "terminal target has 2 active session(s)",
        },
      },
    });
    const changedConflict = Object.assign(new Error("Request failed"), {
      response: {
        data: {
          activeSessionCount: 4,
          confirmationToken: "confirm-update-2",
          errorCode: "conflict",
          message: "terminal target now has 4 active session(s)",
        },
      },
    });
    const updateTarget = vi
      .fn()
      .mockRejectedValueOnce(conflict)
      .mockRejectedValueOnce(changedConflict)
      .mockResolvedValueOnce(target({ revision: 2 }));
    const editor = useTerminalTargetEditor({
      createTarget: vi.fn(),
      updateTarget,
    });
    editor.beginEdit(target());
    editor.draft.clearCredential = true;

    await expect(editor.save(false)).resolves.toBeNull();
    expect(updateTarget.mock.calls[0]?.[2]).toBe(false);
    expect(editor.forceConfirmationRequired.value).toBe(true);
    expect(editor.conflictingActiveSessionCount.value).toBe(2);
    expect(editor.forceConfirmationToken.value).toBe("confirm-update-1");

    await expect(editor.save(true)).resolves.toBeNull();
    expect(updateTarget.mock.calls[1]?.[2]).toBe(true);
    expect(updateTarget.mock.calls[1]?.[3]).toBe("confirm-update-1");
    expect(editor.conflictingActiveSessionCount.value).toBe(4);
    expect(editor.forceConfirmationToken.value).toBe("confirm-update-2");

    await expect(editor.save(true)).resolves.toMatchObject({ revision: 2 });
    expect(updateTarget.mock.calls[2]?.[2]).toBe(true);
    expect(updateTarget.mock.calls[2]?.[3]).toBe("confirm-update-2");

    terminalApi.deleteTarget
      .mockRejectedValueOnce(conflict)
      .mockResolvedValueOnce(undefined);
    const targets = useTerminalTargets();
    await expect(targets.deleteTarget("target-1", 1, false)).rejects.toThrow();
    await targets.deleteTarget("target-1", 1, true, "confirm-delete-1");
    expect(terminalApi.deleteTarget.mock.calls[0]?.slice(0, 4)).toEqual([
      "target-1",
      1,
      false,
      undefined,
    ]);
    expect(terminalApi.deleteTarget.mock.calls[1]?.slice(0, 4)).toEqual([
      "target-1",
      1,
      true,
      "confirm-delete-1",
    ]);
    targets.dispose();
  });

  it("prefers the server active-session count in force-delete confirmation", async () => {
    const conflict = Object.assign(new Error("Request failed"), {
      response: {
        data: {
          activeSessionCount: 3,
          confirmationToken: "confirm-delete-1",
          errorCode: "conflict",
          message: "terminal target has 3 active session(s)",
        },
      },
    });
    const changedConflict = Object.assign(new Error("Request failed"), {
      response: {
        data: {
          activeSessionCount: 4,
          confirmationToken: "confirm-delete-2",
          errorCode: "conflict",
          message: "terminal target now has 4 active session(s)",
        },
      },
    });
    const deleteTarget = vi
      .fn()
      .mockRejectedValueOnce(conflict)
      .mockRejectedValueOnce(changedConflict)
      .mockResolvedValueOnce(undefined);
    const controller = useTerminalTargetDeletion({
      activeSessionCount: () => 0,
      attachedSessionId: ref(""),
      deleteTarget,
      detach: vi.fn(),
      removeSessionsForTarget: vi.fn(),
      sessions: ref([]),
      translate: (_key, fallback) => fallback,
    });

    await controller.deleteTarget(target());
    expect(controller.pendingForceDeleteActiveCount.value).toBe(3);
    expect(deleteTarget).toHaveBeenLastCalledWith(
      "target-1",
      1,
      false,
      undefined,
    );

    await controller.confirmForceDeleteTarget();
    expect(deleteTarget).toHaveBeenLastCalledWith(
      "target-1",
      1,
      true,
      "confirm-delete-1",
    );
    expect(controller.pendingForceDeleteActiveCount.value).toBe(4);
    expect(controller.pendingForceDeleteConfirmationToken.value).toBe(
      "confirm-delete-2",
    );

    await controller.confirmForceDeleteTarget();
    expect(deleteTarget).toHaveBeenLastCalledWith(
      "target-1",
      1,
      true,
      "confirm-delete-2",
    );
    controller.dispose();
  });

  it("uses one test action to probe, confirm, and authenticate", async () => {
    terminalApi.probeHostKey.mockResolvedValue({
      algorithm: "ssh-ed25519",
      fingerprint: "SHA256:current",
      host: "server.example.com",
      port: 22,
    });
    terminalApi.testConnection.mockResolvedValue({
      success: true,
      latencyMs: 20,
      verificationToken: "verified-connection-token",
    });
    const editor = useTerminalTargetEditor({
      createTarget: vi.fn(),
      updateTarget: vi.fn(),
    });

    editor.beginCreate();
    editor.draft.name = "my nas";
    editor.setEndpoint("host", "server.example.com");
    editor.draft.username = "deploy";
    editor.draft.secret = "temporary-password";

    await expect(editor.testConnection()).resolves.toBe(false);
    expect(terminalApi.probeHostKey).toHaveBeenCalledTimes(1);
    expect(terminalApi.testConnection).not.toHaveBeenCalled();
    expect(editor.pendingHostKey.value).toEqual({
      algorithm: "ssh-ed25519",
      fingerprint: "SHA256:current",
    });

    await expect(editor.confirmHostKey()).resolves.toBe(true);
    expect(terminalApi.testConnection).toHaveBeenCalledTimes(1);
    expect(editor.tested.value).toBe(true);
    expect(editor.draft.trustedHostKey).toEqual({
      algorithm: "ssh-ed25519",
      fingerprint: "SHA256:current",
    });
  });

  it("ignores stale host-key and connection-test responses", async () => {
    const oldProbe = deferred<{
      algorithm: string;
      fingerprint: string;
      host: string;
      port: number;
    }>();
    const oldTest = deferred<{
      success: boolean;
      latencyMs: number;
      verificationToken: string;
    }>();
    terminalApi.probeHostKey.mockReturnValue(oldProbe.promise);
    const createTarget =
      vi.fn<(payload: unknown) => Promise<TerminalTargetRecord>>();
    const updateTarget =
      vi.fn<(id: string, payload: unknown) => Promise<TerminalTargetRecord>>();
    const editor = useTerminalTargetEditor({
      createTarget,
      updateTarget,
    });

    editor.beginCreate();
    editor.draft.name = "Production";
    editor.setEndpoint("host", "old.example.com");
    editor.draft.username = "deploy";
    editor.draft.secret = "temporary-password";
    const probeRequest = editor.testConnection();
    editor.setEndpoint("host", "new.example.com");
    oldProbe.resolve({
      algorithm: "ssh-ed25519",
      fingerprint: "SHA256:stale",
      host: "old.example.com",
      port: 22,
    });
    await probeRequest;
    expect(editor.pendingHostKey.value).toBeNull();

    terminalApi.probeHostKey.mockResolvedValue({
      algorithm: "ssh-ed25519",
      fingerprint: "SHA256:current",
      host: "new.example.com",
      port: 22,
    });
    terminalApi.testConnection.mockReturnValue(oldTest.promise);
    editor.draft.trustedHostKey = {
      algorithm: "ssh-ed25519",
      fingerprint: "SHA256:current",
    };
    const testRequest = editor.testConnection();
    await settleMicrotasks();
    expect(terminalApi.testConnection).toHaveBeenCalledTimes(1);
    editor.draft.username = "changed-after-request";
    oldTest.resolve({
      success: true,
      latencyMs: 20,
      verificationToken: "stale-verification-token",
    });

    await expect(testRequest).resolves.toBe(false);
    expect(editor.tested.value).toBe(false);
    editor.close();
    expect(editor.draft.secret).toBe("");
    expect(editor.draft.passphrase).toBe("");
  });
});
