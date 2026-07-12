import { computed, ref, toValue, type MaybeRefOrGetter } from "vue";

import type {
  CaptchaPublicSettings,
  CaptchaSubmission,
} from "@frontend-core/captcha/types";
import { extractErrorMessage } from "@frontend-core/errors/extractErrorMessage";
import {
  buildPowSubmission,
  CaptchaError,
  normalizePowChallenge,
  solvePowChallenge,
} from "@/lib/captcha";

interface ResettableCaptchaWidget {
  reset: () => void;
}

interface UseLoginCaptchaOptions {
  canUseNativePow: MaybeRefOrGetter<boolean>;
  translate: (key: string) => string;
  onError: (message: string) => void;
  onVerified?: () => void;
  resolvePowSubmission?: () => Promise<CaptchaSubmission>;
}

type PowStateChangeEvent = CustomEvent<{
  state?: string;
  payload?: string;
}>;

const resolveDefaultPowSubmission = async (): Promise<CaptchaSubmission> => {
  const { CaptchaAPI } = await import("@/lib/api");
  const challenge = normalizePowChallenge(await CaptchaAPI.getPowChallenge());
  const number = await solvePowChallenge(challenge);
  return buildPowSubmission(challenge, number);
};

export const useLoginCaptcha = (options: UseLoginCaptchaOptions) => {
  const captchaConfig = ref<CaptchaPublicSettings | null>(null);
  const powWidgetRef = ref<ResettableCaptchaWidget | null>(null);
  const turnstileWidgetRef = ref<ResettableCaptchaWidget | null>(null);
  const isCaptchaVerified = ref(false);
  const captchaSubmission = ref<CaptchaSubmission | null>(null);
  const isPowFallbackLoading = ref(false);
  const isCaptchaConfigLoading = ref(true);

  const activeCaptchaProvider = computed(
    () => captchaConfig.value?.provider ?? null,
  );
  const isCaptchaProviderAvailable = computed(
    () => captchaConfig.value?.available ?? false,
  );
  const captchaUnavailableReason = computed(
    () =>
      captchaConfig.value?.unavailable_reason ||
      options.translate("auth.captchaConfigLoadFailed"),
  );
  const hasTurnstileSiteKey = computed(
    () => !!captchaConfig.value?.turnstile.site_key.trim(),
  );
  const powWidgetStrings = computed(() =>
    JSON.stringify({
      label: options.translate("auth.notRobot"),
      verified: options.translate("auth.verified"),
      verifying: options.translate("auth.verifying"),
      wait: options.translate("auth.wait"),
      error: options.translate("auth.verifyError"),
    }),
  );

  const resetCaptcha = () => {
    isCaptchaVerified.value = false;
    captchaSubmission.value = null;
  };

  const resetCaptchaWidgets = () => {
    resetCaptcha();
    if (
      activeCaptchaProvider.value === "pow" &&
      toValue(options.canUseNativePow)
    ) {
      powWidgetRef.value?.reset();
    }
    if (activeCaptchaProvider.value === "turnstile") {
      turnstileWidgetRef.value?.reset();
    }
  };

  const setVerifiedSubmission = (submission: CaptchaSubmission) => {
    captchaSubmission.value = submission;
    isCaptchaVerified.value = true;
    options.onVerified?.();
  };

  const handlePowStateChange = (event: PowStateChangeEvent) => {
    if (
      event.detail?.state === "verified" &&
      typeof event.detail.payload === "string" &&
      event.detail.payload
    ) {
      setVerifiedSubmission({
        provider: "pow",
        proof: event.detail.payload,
      });
      return;
    }

    resetCaptcha();
  };

  const handlePowFallbackVerify = async () => {
    if (isPowFallbackLoading.value) return;
    isPowFallbackLoading.value = true;
    try {
      const resolveSubmission =
        options.resolvePowSubmission ?? resolveDefaultPowSubmission;
      setVerifiedSubmission(await resolveSubmission());
    } catch (error) {
      resetCaptcha();
      options.onError(
        error instanceof CaptchaError
          ? options.translate(`auth.${error.code}`)
          : extractErrorMessage(error, options.translate("auth.captchaFailed")),
      );
    } finally {
      isPowFallbackLoading.value = false;
    }
  };

  const handleTurnstileVerified = (token: string) => {
    setVerifiedSubmission({ provider: "turnstile", token });
  };

  const handleTurnstileError = (message: string) => {
    resetCaptcha();
    options.onError(message);
  };

  return {
    activeCaptchaProvider,
    captchaConfig,
    captchaSubmission,
    captchaUnavailableReason,
    handlePowFallbackVerify,
    handlePowStateChange,
    handleTurnstileError,
    handleTurnstileVerified,
    hasTurnstileSiteKey,
    isCaptchaConfigLoading,
    isCaptchaProviderAvailable,
    isCaptchaVerified,
    isPowFallbackLoading,
    powWidgetRef,
    powWidgetStrings,
    resetCaptcha,
    resetCaptchaWidgets,
    turnstileWidgetRef,
  };
};
