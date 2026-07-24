import { computed, ref } from "vue";
import { defineStore } from "pinia";
import { browserT } from "@fn-knock/i18n/vue/admin";
import type { DockerAdminBootstrapState } from "../types";
import { ConfigAPI } from "../lib/api";
import {
  buildDockerAdminDebugState,
  createDockerAdminDebugState,
  readDockerAdminDebugPassword,
  validateDockerAdminDebugPassword,
  writeDockerAdminDebugPassword,
  writeDockerAdminDebugStage,
} from "@runtime-debug";
import { applyAppearanceConfig } from "@admin-shared/composables/useAppearanceState";

export const useDockerAdminAuthStore = defineStore("dockerAdminAuth", () => {
  const state = ref<DockerAdminBootstrapState | null>(null);
  const isBootstrapped = ref(false);
  const isBootstrapping = ref(false);
  const isSubmitting = ref(false);
  const isDebugOverrideActive = ref(false);
  const bootstrapError = ref("");
  const submitError = ref("");
  let bootstrapPromise: Promise<DockerAdminBootstrapState> | null = null;

  const isEnabled = computed(() => state.value?.enabled === true);
  const passwordConfigured = computed(
    () => state.value?.password_configured === true,
  );
  const isAuthenticated = computed(() => {
    if (state.value?.enabled !== true) return true;
    return state.value.authenticated === true;
  });
  const needsPasswordSetup = computed(
    () => isEnabled.value && !passwordConfigured.value,
  );
  const needsLogin = computed(
    () => isEnabled.value && passwordConfigured.value && !isAuthenticated.value,
  );
  const authSource = computed(() => state.value?.auth_source ?? null);
  const canEnterApp = computed(
    () => isBootstrapped.value && (!isEnabled.value || isAuthenticated.value),
  );
  const currentLocaleConfig = () => state.value?.locale;
  const currentAppearanceConfig = () => state.value?.appearance;

  const applyState = (
    next: DockerAdminBootstrapState,
    options?: { debugOverride?: boolean },
  ) => {
    state.value = next;
    applyAppearanceConfig(next.appearance);
    isDebugOverrideActive.value = options?.debugOverride === true;
    isBootstrapped.value = true;
    bootstrapError.value = "";
    submitError.value = "";
    return next;
  };

  const bootstrap = async (options?: {
    force?: boolean;
  }): Promise<DockerAdminBootstrapState> => {
    if (bootstrapPromise && !options?.force) {
      return bootstrapPromise;
    }

    isBootstrapping.value = true;
    bootstrapError.value = "";
    bootstrapPromise = ConfigAPI.getDockerAdminBootstrap()
      .then((next) => {
        const debugState = buildDockerAdminDebugState(next);
        return applyState(debugState ?? next, {
          debugOverride: Boolean(debugState),
        });
      })
      .catch((error) => {
        bootstrapError.value =
          error instanceof Error
            ? error.message
            : browserT("admin.dockerAdmin.bootstrapFailed");
        throw error;
      })
      .finally(() => {
        isBootstrapping.value = false;
        bootstrapPromise = null;
      });

    return bootstrapPromise;
  };

  const submitPassword = async (password: string, rememberMe = false) => {
    isSubmitting.value = true;
    submitError.value = "";
    try {
      if (isDebugOverrideActive.value) {
        if (needsPasswordSetup.value) {
          const validationMessage = validateDockerAdminDebugPassword(password);
          if (validationMessage) {
            throw new Error(validationMessage);
          }

          writeDockerAdminDebugPassword(password);
          writeDockerAdminDebugStage("authenticated");
          return applyState(
            createDockerAdminDebugState(
              "authenticated",
              currentLocaleConfig(),
              currentAppearanceConfig(),
              rememberMe,
            ),
            {
              debugOverride: true,
            },
          );
        }

        const savedPassword = readDockerAdminDebugPassword();
        if (!savedPassword) {
          writeDockerAdminDebugStage("setup");
          return applyState(
            createDockerAdminDebugState(
              "setup",
              currentLocaleConfig(),
              currentAppearanceConfig(),
            ),
            {
              debugOverride: true,
            },
          );
        }

        if (savedPassword !== password) {
          throw new Error(browserT("admin.dockerAdmin.wrongPassword"));
        }

        writeDockerAdminDebugStage("authenticated");
        return applyState(
          createDockerAdminDebugState(
            "authenticated",
            currentLocaleConfig(),
            currentAppearanceConfig(),
            rememberMe,
          ),
          {
            debugOverride: true,
          },
        );
      }

      const next = needsPasswordSetup.value
        ? await ConfigAPI.setDockerAdminPassword(password)
        : await ConfigAPI.loginDockerAdmin(password, rememberMe);
      return applyState(next);
    } catch (error) {
      submitError.value =
        error instanceof Error
          ? error.message
          : browserT("admin.dockerAdmin.submitFailed");
      throw error;
    } finally {
      isSubmitting.value = false;
    }
  };

  const clearSubmitError = () => {
    submitError.value = "";
  };

  const logout = async () => {
    isSubmitting.value = true;
    submitError.value = "";
    try {
      if (isDebugOverrideActive.value) {
        const nextStage = readDockerAdminDebugPassword() ? "login" : "setup";
        writeDockerAdminDebugStage(nextStage);
        return applyState(
          createDockerAdminDebugState(
            nextStage,
            currentLocaleConfig(),
            currentAppearanceConfig(),
          ),
          {
            debugOverride: true,
          },
        );
      }

      const previousAuthSource = authSource.value;
      const next = await ConfigAPI.logoutDockerAdmin();
      if (
        previousAuthSource === "reauth_session" &&
        typeof window !== "undefined"
      ) {
        window.location.assign("/__auth__/logout");
        return applyState({
          ...next,
          authenticated: false,
          auth_source: null,
          session_expires_at: null,
        });
      }
      return applyState(next);
    } finally {
      isSubmitting.value = false;
    }
  };

  const handleUnauthorized = () => {
    if (!state.value?.enabled) return;
    if (isDebugOverrideActive.value) {
      const nextStage = readDockerAdminDebugPassword() ? "login" : "setup";
      writeDockerAdminDebugStage(nextStage);
      state.value = createDockerAdminDebugState(
        nextStage,
        currentLocaleConfig(),
        currentAppearanceConfig(),
      );
      return;
    }

    state.value = {
      ...state.value,
      authenticated: false,
      auth_source: null,
      session_expires_at: null,
    };
  };

  return {
    state,
    isBootstrapped,
    isBootstrapping,
    isSubmitting,
    isDebugOverrideActive,
    bootstrapError,
    submitError,
    isEnabled,
    passwordConfigured,
    isAuthenticated,
    authSource,
    needsPasswordSetup,
    needsLogin,
    canEnterApp,
    bootstrap,
    submitPassword,
    clearSubmitError,
    logout,
    handleUnauthorized,
  };
});
