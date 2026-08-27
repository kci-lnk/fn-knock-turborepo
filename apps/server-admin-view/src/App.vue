<script setup lang="ts">
import { Toaster } from "@/components/ui/sonner";
import {
  computed,
  defineAsyncComponent,
  onMounted,
  onUnmounted,
  watch,
} from "vue";
import { useI18n } from "vue-i18n";
import "vue-sonner/style.css";
import { useThemeMode } from "@/components/ui/theme-toggle";
import { DYNAMIC_WHITE_THEME_COLOR_PRESET_KEY } from "@frontend-core/appearance";
import { useAppearanceState } from "@admin-shared/composables/useAppearanceState";
import { useDockerAdminAuthStore } from "./store/dockerAdminAuth";
import { setFnKnockLocale } from "@fn-knock/i18n/vue/admin";

const DockerAdminAccessGate = defineAsyncComponent(
  () => import("./components/DockerAdminAccessGate.vue"),
);
const DynamicWhiteBackground = defineAsyncComponent(
  () =>
    import("@admin-shared/components/appearance/DynamicWhiteBackground.vue"),
);
const dockerAdminAuthStore = useDockerAdminAuthStore();
const { activeThemeColorPreset } = useAppearanceState();
const { resolvedMode } = useThemeMode();
const i18n = useI18n();
const shouldRenderRouter = computed(() => dockerAdminAuthStore.canEnterApp);
const shouldShowDockerAdminGate = computed(() => {
  if (dockerAdminAuthStore.bootstrapError) return true;
  return (
    dockerAdminAuthStore.isBootstrapped &&
    dockerAdminAuthStore.isEnabled &&
    !dockerAdminAuthStore.isAuthenticated
  );
});
const isDynamicWhiteActive = computed(
  () =>
    resolvedMode.value === "light" &&
    activeThemeColorPreset.value === DYNAMIC_WHITE_THEME_COLOR_PRESET_KEY,
);
const dockerAdminGateMode = computed(() =>
  dockerAdminAuthStore.needsPasswordSetup ? "setup" : "login",
);
const dockerAdminGateError = computed(() => dockerAdminAuthStore.submitError);
const dockerAdminGateBootstrapError = computed(
  () => dockerAdminAuthStore.bootstrapError,
);
const dockerAdminGateShowRetry = computed(() =>
  Boolean(dockerAdminAuthStore.bootstrapError),
);
const toastOptions = {
  closeButton: false,
  duration: 2500,
};

const applySystemLocale = async (value: string | null | undefined) => {
  await setFnKnockLocale(i18n, value);
};

const bootstrapDockerAdmin = async (force = false) => {
  try {
    await dockerAdminAuthStore.bootstrap({ force });
  } catch (error) {
    console.error("Failed to bootstrap docker admin auth", error);
  }
};

const handleDockerAdminSubmit = async (
  password: string,
  rememberMe: boolean,
) => {
  try {
    await dockerAdminAuthStore.submitPassword(password, rememberMe);
  } catch (error) {
    console.error("Failed to submit docker admin password", error);
  }
};

const handleDockerAdminRetry = async () => {
  await bootstrapDockerAdmin(true);
};

const handleDockerAdminUnauthorized = () => {
  dockerAdminAuthStore.handleUnauthorized();
};

onMounted(() => {
  void bootstrapDockerAdmin();

  if (typeof window !== "undefined") {
    window.addEventListener(
      "fn-knock:docker-admin-auth-required",
      handleDockerAdminUnauthorized,
    );
  }
});

onUnmounted(() => {
  if (typeof window !== "undefined") {
    window.removeEventListener(
      "fn-knock:docker-admin-auth-required",
      handleDockerAdminUnauthorized,
    );
  }
});

watch(
  () => dockerAdminAuthStore.state?.locale?.default_locale,
  (next) => {
    if (next) {
      void applySystemLocale(next).catch((error) => {
        console.error("Failed to apply system locale", error);
      });
    }
  },
  { immediate: true },
);
</script>

<template>
  <DynamicWhiteBackground v-if="isDynamicWhiteActive" :active="true" />
  <div v-if="shouldRenderRouter" class="contents">
    <RouterView />
  </div>
  <DockerAdminAccessGate
    v-else-if="shouldShowDockerAdminGate"
    :mode="dockerAdminGateMode"
    :loading="
      dockerAdminAuthStore.isBootstrapping || dockerAdminAuthStore.isSubmitting
    "
    :error-message="dockerAdminGateError"
    :bootstrap-error-message="dockerAdminGateBootstrapError"
    :show-retry="dockerAdminGateShowRetry"
    :deployment-target="dockerAdminAuthStore.state?.deployment_target"
    @password-input="dockerAdminAuthStore.clearSubmitError"
    @submit="handleDockerAdminSubmit"
    @retry="handleDockerAdminRetry"
  />
  <main
    v-else
    class="grid min-h-dvh place-items-center bg-background px-6 text-foreground"
    role="status"
    aria-live="polite"
  >
    <div class="flex flex-col items-center gap-3 text-center">
      <span
        class="h-8 w-8 animate-spin rounded-full border-2 border-muted border-b-primary"
        aria-hidden="true"
      ></span>
      <p class="text-sm text-muted-foreground">fn-knock</p>
    </div>
  </main>
  <Toaster
    position="top-center"
    :duration="2000"
    :toast-options="toastOptions"
  />
</template>

<style>
[data-sonner-toast][data-styled="true"] {
  padding-right: 44px;
}

[data-sonner-toast][data-styled="true"] [data-close-button] {
  left: auto;
  right: 10px;
  top: 10px;
  bottom: auto;
  width: 24px;
  height: 24px;
  transform: none;
  opacity: 1;
  background: var(--normal-bg);
  border-color: var(--normal-border);
  color: var(--normal-text);
}

[data-sonner-toast][data-styled="true"] [data-close-button]:hover {
  background: var(--muted);
}

[data-sonner-toast][data-styled="true"] [data-close-button] svg {
  width: 14px;
  height: 14px;
}
</style>
