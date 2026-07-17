<template>
  <div class="flex min-h-dvh w-full flex-col bg-muted/40">
    <div
      v-if="configStore.isLoading"
      class="ml-4 text-sm text-muted-foreground animate-pulse"
    >
      {{ t("common.loadingConfig") }}
    </div>
    <div v-if="configStore.isError" class="ml-4 text-sm text-destructive">
      {{ t("common.loadConfigFailed") }}
    </div>

    <div
      class="sticky top-0 z-20 border-b bg-background/95 backdrop-blur sm:hidden"
    >
      <div class="mx-auto flex h-14 max-w-[96rem] items-center gap-2 px-4">
        <Button variant="ghost" size="icon" @click="isMobileNavOpen = true">
          <Menu class="h-5 w-5" />
          <span class="sr-only">{{ t("admin.nav.openNavigation") }}</span>
        </Button>
        <p class="min-w-0 flex-1 truncate text-sm font-medium">
          {{ currentNavLabel }}
        </p>
        <ThemeModeToggle />
      </div>
    </div>

    <Sheet v-model:open="isMobileNavOpen">
      <SheetContent side="left" class="w-[66vw] max-w-[240px] p-0">
        <SheetHeader class="sr-only">
          <SheetTitle>{{ t("admin.nav.navigationMenu") }}</SheetTitle>
        </SheetHeader>
        <div class="flex h-full flex-col">
          <div class="border-b px-4 py-3 text-sm font-semibold">
            {{ t("admin.nav.navigationMenu") }}
          </div>
          <nav
            class="flex-1 space-y-2 overflow-y-auto p-3 [scrollbar-width:none] [-ms-overflow-style:none] [&::-webkit-scrollbar]:hidden"
          >
            <Button
              v-for="item in navItems"
              :key="item.path"
              :variant="isNavActive(item.path) ? 'default' : 'ghost'"
              class="w-full justify-start gap-3"
              @click="navigateTo(item.path)"
            >
              <component :is="item.icon" class="h-4 w-4" />
              <span>{{ item.name }}</span>
            </Button>
          </nav>
          <div class="border-t p-3">
            <div class="mb-5 flex justify-center gap-2">
              <ThemeModeToggle />
              <Button
                variant="ghost"
                size="sm"
                class="h-8 max-w-full gap-1.5 rounded-md border border-border/60 bg-background/70 px-2.5 text-xs shadow-none hover:bg-muted"
                :title="t('locale.label')"
                @click="openLocaleDialog"
              >
                <Languages class="h-3.5 w-3.5 shrink-0" />
                <span class="max-w-[5.25rem] truncate">{{
                  selectedLocaleLabel
                }}</span>
              </Button>
              <ConfirmDangerPopover
                v-if="shouldShowPanelLogout"
                :title="t('admin.dockerAdmin.logoutConfirmTitle')"
                :description="t('admin.dockerAdmin.logoutConfirmDescription')"
                :confirm-text="t('admin.dockerAdmin.logoutConfirm')"
                :loading="dockerAdminAuthStore.isSubmitting"
                :disabled="dockerAdminAuthStore.isSubmitting"
                :on-confirm="handlePanelLogout"
                content-class="w-72 text-left"
              >
                <template #trigger>
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    class="h-8 w-8 shrink-0 rounded-md border border-destructive/20 bg-destructive/5 p-0 text-destructive shadow-none hover:bg-destructive/10 hover:text-destructive"
                    :disabled="dockerAdminAuthStore.isSubmitting"
                    :title="t('admin.dockerAdmin.logout')"
                  >
                    <LogOut class="h-3.5 w-3.5" />
                    <span class="sr-only">{{
                      t("admin.dockerAdmin.logout")
                    }}</span>
                  </Button>
                </template>
              </ConfirmDangerPopover>
            </div>
            <p class="mb-2 text-center text-xs font-medium text-primary/70">
              <a
                :href="APP_GITHUB_URL"
                target="_blank"
                rel="noopener noreferrer"
                class="inline-flex max-w-full items-center gap-1.5 rounded-full px-2.5 py-1 leading-none transition-colors hover:text-foreground hover:bg-background/70"
                :title="t('admin.nav.openGithub')"
              >
                <Github class="h-3.5 w-3.5" />
                <span>{{ currentVersionLabel }}</span>
              </a>
            </p>
            <div class="flex justify-center pb-10">
              <Button
                variant="secondary"
                class="w-auto min-w-28 justify-center px-5"
                @click="navigateTo('/about')"
              >
                {{ aboutEntryLabel }}
              </Button>
            </div>
          </div>
        </div>
      </SheetContent>
    </Sheet>

    <div
      class="mx-auto flex w-full max-w-[96rem] min-w-0 flex-1 min-h-0 flex-col gap-4 px-4 py-4 sm:flex-row sm:gap-4 sm:px-6 sm:py-6 lg:gap-5"
    >
      <aside
        class="hidden shrink-0 sm:sticky sm:top-6 sm:block sm:h-[calc(100dvh-3rem)] sm:w-36 md:w-[9.25rem] xl:w-[9.5rem]"
      >
        <div class="flex h-full min-h-0 flex-col gap-3">
          <nav
            class="flex min-h-0 flex-1 flex-col items-stretch gap-1.5 overflow-y-auto [scrollbar-width:none] [-ms-overflow-style:none] [&::-webkit-scrollbar]:hidden"
          >
            <Button
              v-for="item in navItems"
              :key="item.path"
              :variant="isNavActive(item.path) ? 'default' : 'ghost'"
              :class="[
                'min-w-0 w-full justify-start gap-2 overflow-hidden px-2.5 transition-[transform,box-shadow,background-color,color] duration-150',
                isNavActive(item.path)
                  ? 'shadow-sm shadow-primary/15'
                  : 'hover:-translate-y-[1px]',
              ]"
              @click="navigateTo(item.path)"
            >
              <component :is="item.icon" class="h-4 w-4 shrink-0" />
              <span class="min-w-0 truncate">{{ item.name }}</span>
            </Button>
          </nav>
          <div>
            <div class="mb-5 flex justify-center gap-2">
              <ThemeModeToggle />
              <Button
                variant="ghost"
                size="sm"
                class="h-8 max-w-full justify-center gap-1.5 rounded-md border border-border/60 bg-background/70 px-2.5 text-xs shadow-none hover:bg-muted"
                :title="t('locale.label')"
                @click="openLocaleDialog"
              >
                <Languages class="h-3.5 w-3.5 shrink-0" />
              </Button>
              <ConfirmDangerPopover
                v-if="shouldShowPanelLogout"
                :title="t('admin.dockerAdmin.logoutConfirmTitle')"
                :description="t('admin.dockerAdmin.logoutConfirmDescription')"
                :confirm-text="t('admin.dockerAdmin.logoutConfirm')"
                :loading="dockerAdminAuthStore.isSubmitting"
                :disabled="dockerAdminAuthStore.isSubmitting"
                :on-confirm="handlePanelLogout"
                content-class="w-64 text-left"
              >
                <template #trigger>
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    class="h-8 w-8 shrink-0 rounded-md border border-destructive/20 bg-destructive/5 p-0 text-destructive shadow-none hover:bg-destructive/10 hover:text-destructive"
                    :disabled="dockerAdminAuthStore.isSubmitting"
                    :title="t('admin.dockerAdmin.logout')"
                  >
                    <LogOut class="h-3.5 w-3.5" />
                    <span class="sr-only">{{
                      t("admin.dockerAdmin.logout")
                    }}</span>
                  </Button>
                </template>
              </ConfirmDangerPopover>
            </div>
            <p
              class="mb-2 min-w-0 text-center text-xs font-medium text-primary/70"
            >
              <a
                :href="APP_GITHUB_URL"
                target="_blank"
                rel="noopener noreferrer"
                class="inline-flex max-w-full items-center gap-1.5 rounded-full px-2.5 py-1 leading-none transition-colors hover:text-foreground hover:bg-background/70"
                :title="t('admin.nav.openGithub')"
              >
                <Github class="h-3.5 w-3.5" />
                <span>{{ currentVersionLabel }}</span>
              </a>
            </p>
            <div class="flex justify-center">
              <Button
                variant="secondary"
                class="h-8 w-auto min-w-24 justify-center px-3"
                @click="navigateTo('/about')"
              >
                {{ aboutEntryLabel }}
              </Button>
            </div>
          </div>
        </div>
      </aside>

      <main class="flex-1 w-full min-w-0" :aria-busy="isRouteNavigating">
        <LayoutStatusBanners :navigate-to="navigateTo" />
        <div
          v-if="isRouteNavigating"
          class="mx-auto mb-4 flex w-full max-w-7xl justify-end"
        >
          <div
            class="inline-flex items-center gap-2 rounded-full border border-border/70 bg-background/88 px-3 py-1.5 text-xs text-muted-foreground shadow-sm backdrop-blur"
          >
            <span
              class="h-1.5 w-1.5 rounded-full bg-primary animate-pulse"
            ></span>
            <span>{{ t("common.pageSwitching") }}</span>
          </div>
        </div>
        <RouterView v-if="!configStore.isLoading && !configStore.isError" />
        <div
          v-else-if="configStore.isLoading"
          class="flex h-full min-h-[400px] items-center justify-center"
        >
          <div
            class="h-8 w-8 animate-spin rounded-full border-b-2 border-primary"
          ></div>
        </div>
      </main>
    </div>

    <LayoutLocaleDialog
      v-model:open="isLocaleDialogOpen"
      :is-saving="isSavingLocale"
      :options="localeOptions"
      :selected-locale="selectedLocale"
      @select="handleLocaleSelect"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { isNavigationFailure, useRoute, useRouter } from "vue-router";
import { useConfigStore } from "../store/config";
import { useDockerAdminAuthStore } from "../store/dockerAdminAuth";
import { useSystemClockStore } from "../store/systemClock";
import { useUpdateStore } from "../store/update";
import { isRouteNavigating, pendingNavPath } from "../router/navigation-state";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import { Button } from "@/components/ui/button";
import { ThemeModeToggle } from "@/components/ui/theme-toggle";
import { toast } from "@admin-shared/utils/toast";
import {
  LOCALE_DISPLAY_NAMES,
  SUPPORTED_LOCALES,
  type LocaleCode,
  normalizeLocale,
} from "@fn-knock/i18n/core";
import { setFnKnockLocale } from "@fn-knock/i18n/vue/admin";
import {
  Sheet,
  SheetContent,
  SheetHeader,
  SheetTitle,
} from "@/components/ui/sheet";
const APP_GITHUB_URL = "https://github.com/kci-lnk/fn-knock-turborepo";
import { Github, Languages, LogOut, Menu } from "lucide-vue-next";
import LayoutLocaleDialog from "./layout/LayoutLocaleDialog.vue";
import LayoutStatusBanners from "./layout/LayoutStatusBanners.vue";
import { useLayoutNavigation } from "./layout/useLayoutNavigation";

const router = useRouter();
const route = useRoute();
const configStore = useConfigStore();
const dockerAdminAuthStore = useDockerAdminAuthStore();
const systemClockStore = useSystemClockStore();
const updateStore = useUpdateStore();
const {
  aboutEntryLabel,
  currentNavLabel,
  currentVersionLabel,
  isNavActive,
  navItems,
} = useLayoutNavigation();
const isMobileNavOpen = ref(false);
const isLocaleDialogOpen = ref(false);
const isSavingLocale = ref(false);
const i18n = useI18n();
const { t, locale } = i18n;
const selectedLocale = ref<LocaleCode>(
  normalizeLocale(String(locale.value)) ?? "zh-CN",
);

const localeOptions = SUPPORTED_LOCALES.map((value) => ({
  value,
  label: LOCALE_DISPLAY_NAMES[value],
}));

const selectedLocaleLabel = computed(
  () => LOCALE_DISPLAY_NAMES[selectedLocale.value],
);
const shouldShowPanelLogout = computed(
  () =>
    dockerAdminAuthStore.isEnabled &&
    dockerAdminAuthStore.isAuthenticated &&
    dockerAdminAuthStore.authSource !== "reauth_session",
);

type WindowWithIdleCallback = Window & {
  requestIdleCallback?: (
    callback: IdleRequestCallback,
    options?: IdleRequestOptions,
  ) => number;
};

const runAfterFirstPaint = (callback: () => void) => {
  window.requestAnimationFrame(() => {
    const requestIdleCallback = (window as WindowWithIdleCallback)
      .requestIdleCallback;
    if (requestIdleCallback) {
      requestIdleCallback(callback, { timeout: 1500 });
      return;
    }
    window.setTimeout(callback, 250);
  });
};

onMounted(() => {
  void configStore.loadConfig().finally(() => {
    runAfterFirstPaint(() => {
      if (configStore.canSyncSystemClock) {
        void systemClockStore.initialize();
      }
      // Every platform polls update status so version and update notices stay
      // current. Installation remains capability-gated in the update page.
      void updateStore.initialize();
    });
  });
});

onUnmounted(() => {
  systemClockStore.stopPolling();
  updateStore.stopPolling();
});

const navigateTo = async (path: string) => {
  isMobileNavOpen.value = false;
  if (route.path === path) return;
  pendingNavPath.value = path;
  try {
    const failure = await router.push(path);
    if (isNavigationFailure(failure)) {
      pendingNavPath.value = null;
    }
  } catch (error) {
    pendingNavPath.value = null;
    const message =
      error instanceof Error ? error.message : t("admin.route.loadFailedRetry");
    toast.error(t("admin.route.navigationFailed"), { description: message });
  }
};

const applySystemLocale = async (value: string | null | undefined) => {
  const next = await setFnKnockLocale(i18n, value);
  selectedLocale.value = next;
  return next;
};

const openLocaleDialog = () => {
  isLocaleDialogOpen.value = true;
};

const handleLocaleSelect = async (value: LocaleCode) => {
  const next = normalizeLocale(value) ?? "zh-CN";
  if (
    selectedLocale.value === next &&
    configStore.config?.locale?.default_locale === next
  ) {
    isLocaleDialogOpen.value = false;
    return;
  }

  isSavingLocale.value = true;
  try {
    const saved = await configStore.saveLocaleConfig({ default_locale: next });
    await applySystemLocale(saved.default_locale);
    isLocaleDialogOpen.value = false;
    toast.success(t("locale.saved"));
  } catch (error) {
    const message =
      error instanceof Error ? error.message : t("locale.saveFailed");
    toast.error(t("locale.saveFailed"), { description: message });
  } finally {
    isSavingLocale.value = false;
  }
};

const handlePanelLogout = async () => {
  try {
    await dockerAdminAuthStore.logout();
    isMobileNavOpen.value = false;
  } catch (error) {
    const message =
      error instanceof Error ? error.message : t("common.tryLater");
    toast.error(t("admin.dockerAdmin.logoutFailed"), { description: message });
  }
};

watch(
  () => route.path,
  () => {
    isMobileNavOpen.value = false;
  },
);

watch(
  () => locale.value,
  (next) => {
    selectedLocale.value = normalizeLocale(String(next)) ?? "zh-CN";
  },
);

watch(
  () => configStore.config?.locale?.default_locale,
  (next) => {
    if (!next) return;
    void applySystemLocale(next).catch((error) => {
      console.error("Failed to apply system locale", error);
    });
  },
  { immediate: true },
);
</script>
