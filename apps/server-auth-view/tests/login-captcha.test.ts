/// <reference types="node" />

import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { nextTick, ref } from "vue";

import { useLoginCaptcha } from "../src/composables/useLoginCaptcha";

const captchaConfig = {
  provider: "pow" as const,
  widget_mode: "normal" as const,
  available: true,
  unavailable_reason: null,
  pow: {},
  turnstile: { site_key: "site-key" },
};

describe("useLoginCaptcha", () => {
  it("tracks native PoW and Turnstile verification state", () => {
    let verifiedCount = 0;
    const captcha = useLoginCaptcha({
      canUseNativePow: true,
      translate: (key) => key,
      onError: () => undefined,
      onVerified: () => {
        verifiedCount += 1;
      },
    });
    captcha.captchaConfig.value = captchaConfig;

    captcha.handlePowStateChange(
      new CustomEvent("statechange", {
        detail: { state: "verified", payload: "pow-proof" },
      }),
    );
    assert.deepEqual(captcha.captchaSubmission.value, {
      provider: "pow",
      proof: "pow-proof",
    });

    captcha.handleTurnstileVerified("turnstile-token");
    assert.deepEqual(captcha.captchaSubmission.value, {
      provider: "turnstile",
      token: "turnstile-token",
    });
    assert.equal(captcha.isCaptchaVerified.value, true);
    assert.equal(verifiedCount, 2);

    captcha.resetCaptcha();
    assert.equal(captcha.isCaptchaVerified.value, false);
    assert.equal(captcha.captchaSubmission.value, null);
  });

  it("resolves fallback PoW and reports failures", async () => {
    const errors: string[] = [];
    const submission = ref<"success" | "failure">("success");
    const captcha = useLoginCaptcha({
      canUseNativePow: false,
      translate: (key) => key,
      onError: (message) => errors.push(message),
      resolvePowSubmission: async () => {
        if (submission.value === "failure") {
          throw new Error("solver failed");
        }
        return { provider: "pow", proof: "fallback-proof" };
      },
    });

    await captcha.handlePowFallbackVerify();
    assert.deepEqual(captcha.captchaSubmission.value, {
      provider: "pow",
      proof: "fallback-proof",
    });
    assert.equal(captcha.isPowFallbackLoading.value, false);

    submission.value = "failure";
    await captcha.handlePowFallbackVerify();
    assert.equal(captcha.captchaSubmission.value, null);
    assert.deepEqual(errors, ["solver failed"]);
  });

  it("resets only the active provider widget", () => {
    const canUseNativePow = ref(true);
    const resets: string[] = [];
    const captcha = useLoginCaptcha({
      canUseNativePow,
      translate: (key) => key,
      onError: () => undefined,
    });
    captcha.captchaConfig.value = captchaConfig;
    captcha.powWidgetRef.value = { reset: () => resets.push("pow") };
    captcha.turnstileWidgetRef.value = {
      reset: () => resets.push("turnstile"),
    };

    captcha.resetCaptchaWidgets();
    assert.deepEqual(resets, ["pow"]);

    captcha.captchaConfig.value = {
      ...captchaConfig,
      provider: "turnstile",
    };
    captcha.resetCaptchaWidgets();
    assert.deepEqual(resets, ["pow", "turnstile"]);
  });

  it("cancels fallback PoW when the active provider changes", async () => {
    let observedSignal: AbortSignal | undefined;
    const errors: string[] = [];
    const captcha = useLoginCaptcha({
      canUseNativePow: false,
      translate: (key) => key,
      onError: (message) => errors.push(message),
      resolvePowSubmission: (signal) => {
        observedSignal = signal;
        return new Promise((_, reject) => {
          signal?.addEventListener(
            "abort",
            () => reject(new DOMException("Aborted", "AbortError")),
            { once: true },
          );
        });
      },
    });
    captcha.captchaConfig.value = captchaConfig;
    await nextTick();

    captcha.handlePowStateChange(
      new CustomEvent("statechange", {
        detail: { state: "verified", payload: "stale-pow-proof" },
      }),
    );
    assert.equal(captcha.isCaptchaVerified.value, true);

    const pending = captcha.handlePowFallbackVerify();
    captcha.captchaConfig.value = {
      ...captchaConfig,
      provider: "turnstile",
    };
    await nextTick();
    await pending;

    assert.equal(observedSignal?.aborted, true);
    assert.equal(captcha.isPowFallbackLoading.value, false);
    assert.equal(captcha.isCaptchaVerified.value, false);
    assert.equal(captcha.captchaSubmission.value, null);
    assert.deepEqual(errors, []);
  });
});
