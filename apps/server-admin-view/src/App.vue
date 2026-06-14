<script setup lang="ts">
import { Toaster } from "@/components/ui/sonner";
import { extractErrorMessage } from "@admin-shared/composables/useAsyncAction";
import { toast } from "@admin-shared/utils/toast";
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import "vue-sonner/style.css";
import DockerAdminAccessGate from "./components/DockerAdminAccessGate.vue";
import WelcomeScreen from "./components/WelcomeScreen.vue";
import { ConfigAPI } from "./lib/api";
import { useDockerAdminAuthStore } from "./store/dockerAdminAuth";
import { normalizeLocale } from "@fn-knock/i18n";
import { applyDocumentLocale } from "@fn-knock/i18n/vue";

const WELCOME_GUIDE_STORAGE_KEY = "fn_knock:welcome-guide:completed";
const dockerAdminAuthStore = useDockerAdminAuthStore();
const { t, locale } = useI18n();

const readWelcomeGuideLocalFlag = () => {
  if (typeof window === "undefined") return false;
  return window.localStorage.getItem(WELCOME_GUIDE_STORAGE_KEY) === "1";
};

const writeWelcomeGuideLocalFlag = () => {
  if (typeof window === "undefined") return;
  window.localStorage.setItem(WELCOME_GUIDE_STORAGE_KEY, "1");
};

const hasLocalWelcomeGuideCompletion = readWelcomeGuideLocalFlag();
const isWelcomeVisible = ref(false);
const isWelcomeResolved = ref(hasLocalWelcomeGuideCompletion);
const isSavingWelcomeStatus = ref(false);
const hasLoadedWelcomeGuide = ref(false);
const showWelcomeBootMask = computed(
  () =>
    (!dockerAdminAuthStore.isBootstrapped &&
      !dockerAdminAuthStore.bootstrapError) ||
    (dockerAdminAuthStore.canEnterApp &&
      !isWelcomeResolved.value &&
      !isWelcomeVisible.value),
);
const shouldRenderRouter = computed(() => dockerAdminAuthStore.canEnterApp);
const shouldShowDockerAdminGate = computed(() => {
  if (dockerAdminAuthStore.bootstrapError) return true;
  return (
    dockerAdminAuthStore.isBootstrapped &&
    dockerAdminAuthStore.isEnabled &&
    !dockerAdminAuthStore.isAuthenticated
  );
});
const dockerAdminGateMode = computed(() =>
  dockerAdminAuthStore.needsPasswordSetup ? "setup" : "login",
);
const dockerAdminGateError = computed(
  () => dockerAdminAuthStore.submitError || dockerAdminAuthStore.bootstrapError,
);
const dockerAdminGateShowRetry = computed(() =>
  Boolean(dockerAdminAuthStore.bootstrapError),
);
const toastOptions = {
  closeButton: false,
  duration: 2500
};

const applySystemLocale = (value: string | null | undefined) => {
  const next = normalizeLocale(value) ?? "zh-CN";
  locale.value = next;
  applyDocumentLocale(next);
};

const loadWelcomeGuideStatus = async () => {
  try {
    const status = await ConfigAPI.getWelcomeGuideStatus();
    if (status.completed === true) {
      writeWelcomeGuideLocalFlag();
      isWelcomeVisible.value = false;
      return;
    }

    isWelcomeVisible.value = true;
  } catch (error) {
    console.error("Failed to load welcome guide status", error);
    isWelcomeVisible.value = false;
  } finally {
    isWelcomeResolved.value = true;
  }
};

const initializeWelcomeGuide = async () => {
  if (!dockerAdminAuthStore.canEnterApp || hasLoadedWelcomeGuide.value) {
    return;
  }

  hasLoadedWelcomeGuide.value = true;

  if (hasLocalWelcomeGuideCompletion) {
    isWelcomeResolved.value = true;
    void syncWelcomeGuideCompletion(false);
    return;
  }

  isWelcomeResolved.value = false;
  await loadWelcomeGuideStatus();
};

const resetWelcomeGuideGate = () => {
  hasLoadedWelcomeGuide.value = false;
  isWelcomeVisible.value = false;
  isWelcomeResolved.value = true;
};

const syncWelcomeGuideCompletion = async (showErrorToast: boolean) => {
  if (isSavingWelcomeStatus.value) return;

  isSavingWelcomeStatus.value = true;
  try {
    await ConfigAPI.completeWelcomeGuide();
  } catch (error) {
    console.error("Failed to save welcome guide status", error);
    if (showErrorToast) {
      toast.error(t("admin.welcomeGuide.saveStatusFailed"), {
        description: extractErrorMessage(error, t("common.tryLater")),
      });
    }
  } finally {
    isSavingWelcomeStatus.value = false;
  }
};

const handleWelcomeStart = () => {
  writeWelcomeGuideLocalFlag();
  isWelcomeResolved.value = true;
  isWelcomeVisible.value = false;
  void syncWelcomeGuideCompletion(false);
};

const bootstrapDockerAdmin = async (force = false) => {
  try {
    await dockerAdminAuthStore.bootstrap({ force });
  } catch (error) {
    console.error("Failed to bootstrap docker admin auth", error);
    resetWelcomeGuideGate();
    return;
  }

  if (dockerAdminAuthStore.canEnterApp) {
    await initializeWelcomeGuide();
    return;
  }

  resetWelcomeGuideGate();
};

const handleDockerAdminSubmit = async (password: string) => {
  try {
    await dockerAdminAuthStore.submitPassword(password);
    await initializeWelcomeGuide();
  } catch (error) {
    console.error("Failed to submit docker admin password", error);
  }
};

const handleDockerAdminRetry = async () => {
  await bootstrapDockerAdmin(true);
};

const handleDockerAdminUnauthorized = () => {
  dockerAdminAuthStore.handleUnauthorized();
  resetWelcomeGuideGate();
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
  () => dockerAdminAuthStore.canEnterApp,
  (canEnterApp) => {
    if (!canEnterApp) {
      resetWelcomeGuideGate();
      return;
    }

    void initializeWelcomeGuide();
  },
);

watch(
  () => dockerAdminAuthStore.state?.locale?.default_locale,
  (next) => {
    if (next) applySystemLocale(next);
  },
  { immediate: true },
);
</script>

<template>
  <RouterView v-if="shouldRenderRouter" />
  <DockerAdminAccessGate
    v-else-if="shouldShowDockerAdminGate"
    :mode="dockerAdminGateMode"
    :loading="
      dockerAdminAuthStore.isBootstrapping || dockerAdminAuthStore.isSubmitting
    "
    :error-message="dockerAdminGateError"
    :show-retry="dockerAdminGateShowRetry"
    @submit="handleDockerAdminSubmit"
    @retry="handleDockerAdminRetry"
  />
  <div v-if="showWelcomeBootMask" class="welcome-boot-mask"></div>
  <WelcomeScreen
    :visible="shouldRenderRouter && isWelcomeVisible"
    :pending="isSavingWelcomeStatus"
    @start="handleWelcomeStart"
  />
  <Toaster
    position="top-center"
    :duration="2000"
    :toast-options="toastOptions"
  />
</template>

<style scoped>
.welcome-boot-mask {
  position: fixed;
  inset: 0;
  z-index: 9998;
  background:
    radial-gradient(circle at 18% 18%, rgba(118, 164, 255, 0.18), transparent 28%),
    radial-gradient(circle at 82% 24%, rgba(255, 159, 237, 0.14), transparent 24%),
    linear-gradient(180deg, rgba(8, 10, 18, 0.98), rgba(8, 10, 18, 0.92));
}
</style>

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
