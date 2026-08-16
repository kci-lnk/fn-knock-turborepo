import { DOMWrapper, mount } from "@vue/test-utils";
import { nextTick } from "vue";
import { createI18n } from "vue-i18n";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { WOLTarget } from "../src/lib/api/wol";
import WOLShutdownDialog from "../src/views/wol-management/WOLShutdownDialog.vue";

const i18n = createI18n({
  legacy: false,
  locale: "en",
  messages: {
    en: {
      common: { cancel: "Cancel" },
      admin: {
        wol: {
          ssh: {
            shutdownTitle: "Shut down",
            shutdownDescription: "Shut down {target} through {host}",
            shutdownWarning: "Unsaved data may be lost.",
            confirmShutdown: "Confirm shutdown",
            confirmShutdownCountdown: "Confirm shutdown ({seconds})",
          },
        },
      },
    },
  },
});

const target = {
  id: "desktop-1",
  name: "Office PC",
  ssh: {
    enabled: true,
    host: "192.0.2.10",
    port: 22,
    username: "operator",
    platform: "linux",
    authMethod: "privateKey",
    hostKeyAlgorithm: "ssh-ed25519",
    hostKeyFingerprint: "SHA256:example",
    credentialConfigured: true,
    passphraseConfigured: false,
  },
} as WOLTarget;

const confirmButton = () => {
  const element = document.body.querySelector<HTMLElement>(
    '[data-testid="wol-confirm-shutdown"]',
  );
  expect(element).not.toBeNull();
  return new DOMWrapper(element!);
};

describe("WOLShutdownDialog", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-08-17T00:00:00Z"));
    document.body.replaceChildren();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("cannot confirm before 3000ms and resets the deadline when reopened", async () => {
    const wrapper = mount(WOLShutdownDialog, {
      props: { open: false, target, loading: false },
      global: { plugins: [i18n] },
    });
    await wrapper.setProps({ open: true });
    await nextTick();

    expect(confirmButton().attributes("disabled")).toBeDefined();
    await vi.advanceTimersByTimeAsync(2_999);
    await confirmButton().trigger("click");
    expect(wrapper.emitted("confirm")).toBeUndefined();

    await vi.advanceTimersByTimeAsync(1);
    await nextTick();
    expect(confirmButton().attributes("disabled")).toBeUndefined();
    await confirmButton().trigger("click");
    expect(wrapper.emitted("confirm")).toHaveLength(1);

    await wrapper.setProps({ open: false });
    await wrapper.setProps({ open: true });
    expect(confirmButton().attributes("disabled")).toBeDefined();
    await confirmButton().trigger("keydown", { key: "Enter" });
    expect(wrapper.emitted("confirm")).toHaveLength(1);
  });

  it("cancels without sending a confirmation", async () => {
    const wrapper = mount(WOLShutdownDialog, {
      props: { open: false, target, loading: false },
      global: { plugins: [i18n] },
    });
    await wrapper.setProps({ open: true });
    await nextTick();

    const cancel = [...document.body.querySelectorAll("button")].find(
      (button) => button.textContent?.trim() === "Cancel",
    );
    expect(cancel).toBeDefined();
    await new DOMWrapper(cancel!).trigger("click");
    expect(wrapper.emitted("update:open")).toEqual([[false]]);
    expect(wrapper.emitted("confirm")).toBeUndefined();
  });
});
