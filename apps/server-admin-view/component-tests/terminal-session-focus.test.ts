import { computed, ref } from "vue";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { TerminalSessionRecord } from "@/lib/api/terminal";
import { useTerminalSessionActions } from "@/views/web-terminal/useTerminalSessionActions";

afterEach(() => {
  document.body.innerHTML = "";
});
const setup = () => {
  const session = { id: "session-a" } as TerminalSessionRecord;
  const selectedSessionId = ref(session.id);
  const focusTerminal = vi.fn();
  let finish!: () => void;
  const connect = vi.fn(
    () =>
      new Promise<void>((resolve) => {
        finish = resolve;
      }),
  );
  const isAttachedTo = vi.fn(() => false);
  const actions = useTerminalSessionActions({
    beginTargetCreate: vi.fn(),
    beginTargetEdit: vi.fn(),
    beginLocalSettings: vi.fn(),
    connect,
    createSession: vi.fn(),
    detach: vi.fn(),
    endSession: vi.fn(),
    focusTerminal,
    getTerminalSize: () => ({ cols: 80, rows: 24 }),
    isAttachedTo,
    onConnectStart: vi.fn(),
    reconnectAttachment: vi.fn(),
    selectedSession: computed(() => session),
    selectedSessionId,
    selectedTarget: ref(null),
    sessions: ref([session]),
    translate: (key) => key,
  });
  const tab = document.createElement("button");
  document.body.append(tab);
  tab.focus();
  return {
    actions,
    focusTerminal,
    finish: () => finish(),
    selectedSessionId,
    isAttachedTo,
  };
};

describe("terminal session focus after removing output autofocus", () => {
  it("focuses the terminal after an explicit tab switch", async () => {
    const { actions, focusTerminal, finish } = setup();
    const pending = actions.handleSessionTabChange("session-a");
    finish();
    await pending;
    expect(focusTerminal).toHaveBeenCalledOnce();
  });
  it("focuses an already attached session without reconnecting", async () => {
    const { actions, focusTerminal, isAttachedTo } = setup();
    isAttachedTo.mockReturnValue(true);
    await actions.handleSessionTabChange("session-a");
    expect(focusTerminal).toHaveBeenCalledOnce();
  });
  it.each(["other-field", "other-session"])(
    "does not steal focus after %s",
    async (reason) => {
      const { actions, focusTerminal, finish, selectedSessionId } = setup();
      const pending = actions.handleSessionTabChange("session-a");
      if (reason === "other-session") selectedSessionId.value = "session-b";
      else {
        const input = document.createElement("input");
        document.body.append(input);
        input.focus();
      }
      finish();
      await pending;
      expect(focusTerminal).not.toHaveBeenCalled();
    },
  );
  it("does not focus for background refresh connections", async () => {
    const { actions, focusTerminal, finish } = setup();
    const pending = actions.connectToSession({
      id: "session-a",
    } as TerminalSessionRecord);
    finish();
    await pending;
    expect(focusTerminal).not.toHaveBeenCalled();
  });
});
