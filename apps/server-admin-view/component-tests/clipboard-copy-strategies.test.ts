import { afterEach, describe, expect, it, vi } from "vitest";
import { copyTextToClipboard } from "@admin-shared/utils/copyTextToClipboard";

const options = { strategy: "write-text-first", verify: false } as const;
afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  document.body.innerHTML = "";
});

describe("clipboard strategies", () => {
  it("starts writeText synchronously and never requests read permission or retries success", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    const readText = vi.fn();
    const write = vi.fn();
    vi.stubGlobal("navigator", { clipboard: { writeText, readText, write } });
    const copying = copyTextToClipboard("log", options);
    expect(writeText).toHaveBeenCalledExactlyOnceWith("log");
    await expect(copying).resolves.toEqual({
      method: "clipboard.writeText",
      verified: false,
    });
    expect(readText).not.toHaveBeenCalled();
    expect(write).not.toHaveBeenCalled();
  });
  it.each(["unavailable", "denied"])(
    "uses a separate textarea when the API is %s",
    async (mode) => {
      vi.stubGlobal(
        "navigator",
        mode === "denied"
          ? {
              clipboard: {
                writeText: vi.fn().mockRejectedValue(new Error("denied")),
              },
            }
          : {},
      );
      const original = document.createElement("textarea");
      original.value = "terminal-input";
      document.body.append(original);
      original.focus();
      const exec = vi.fn(() => {
        expect(document.activeElement).not.toBe(original);
        expect((document.activeElement as HTMLTextAreaElement).value).toBe(
          "  log\n中文",
        );
        return true;
      });
      Object.defineProperty(document, "execCommand", {
        configurable: true,
        value: exec,
      });
      await expect(
        copyTextToClipboard("  log\n中文", options),
      ).resolves.toEqual({ method: "execCommand", verified: false });
      expect(document.activeElement).toBe(original);
      expect(original.value).toBe("terminal-input");
      expect(document.querySelectorAll("textarea")).toHaveLength(1);
    },
  );
  it("does not mistake a dispatched copy event for a successful command", async () => {
    vi.stubGlobal("navigator", {});
    const setData = vi.fn();
    Object.defineProperty(document, "execCommand", {
      configurable: true,
      value: () => {
        const event = new Event("copy", { bubbles: true, cancelable: true });
        Object.defineProperty(event, "clipboardData", { value: { setData } });
        document.activeElement?.dispatchEvent(event);
        return false;
      },
    });
    await expect(copyTextToClipboard("log", options)).rejects.toThrow();
    expect(setData).toHaveBeenCalledWith("text/plain", "log");
    expect(document.querySelector("textarea")).toBeNull();
  });
  it("cancels fallback after the caller becomes stale", async () => {
    let reject!: (error: Error) => void;
    vi.stubGlobal("navigator", {
      clipboard: {
        writeText: () =>
          new Promise((_, r) => {
            reject = r;
          }),
      },
    });
    const exec = vi.fn();
    Object.defineProperty(document, "execCommand", {
      configurable: true,
      value: exec,
    });
    let current = true;
    const result = copyTextToClipboard("log", {
      ...options,
      shouldContinue: () => current,
    });
    current = false;
    reject(new Error("denied"));
    await expect(result).rejects.toThrow();
    expect(exec).not.toHaveBeenCalled();
  });
  it("keeps the existing verified strategy as the default", async () => {
    const readText = vi.fn().mockResolvedValue("a\r\nb");
    const writeText = vi.fn();
    vi.stubGlobal("navigator", { clipboard: { readText, writeText } });
    Object.defineProperty(document, "execCommand", {
      configurable: true,
      value: () => true,
    });
    await expect(copyTextToClipboard("a\nb")).resolves.toEqual({
      method: "execCommand",
      verified: true,
    });
    expect(readText).toHaveBeenCalledOnce();
    expect(writeText).not.toHaveBeenCalled();
  });
  it("accepts Unicode normalization performed by the OS clipboard", async () => {
    const readText = vi.fn().mockResolvedValue("é\r\n");
    const writeText = vi.fn();
    vi.stubGlobal("navigator", { clipboard: { readText, writeText } });
    Object.defineProperty(document, "execCommand", {
      configurable: true,
      value: () => true,
    });
    await expect(copyTextToClipboard("e\u0301\n")).resolves.toEqual({
      method: "execCommand",
      verified: true,
    });
    expect(writeText).not.toHaveBeenCalled();
  });
});
