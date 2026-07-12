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
        <div
          v-if="
            configStore.canSyncSystemClock &&
            systemClockStore.shouldShowBanner &&
            systemClockStore.status
          "
          :class="[
            'mx-auto mt-3 mb-6 w-full max-w-7xl rounded-lg border px-4 py-3',
            systemClockStore.status.timeMismatch
              ? 'border-destructive/35 bg-destructive/10 text-destructive'
              : 'border-amber-500/35 bg-amber-500/10 text-amber-900 dark:text-amber-200',
          ]"
        >
          <div
            class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between"
          >
            <div class="space-y-1">
              <p class="text-sm font-semibold">{{ systemClockBannerTitle }}</p>
              <p class="text-xs leading-5">
                {{ systemClockBannerDescription }}
              </p>
              <p
                v-if="systemClockBannerMeta"
                class="text-[11px] leading-5 opacity-85"
              >
                {{ systemClockBannerMeta }}
              </p>
            </div>
            <div class="flex items-center gap-2">
              <Button
                variant="outline"
                size="sm"
                class="bg-background/80"
                :disabled="
                  systemClockStore.isRefreshing || systemClockStore.isSyncing
                "
                @click="refreshSystemClockStatus"
              >
                {{ t("common.refreshStatus") }}
              </Button>
              <Button
                v-if="configStore.canSyncSystemClock"
                size="sm"
                :variant="
                  systemClockStore.status.timeMismatch
                    ? 'destructive'
                    : 'default'
                "
                :disabled="systemClockStore.isSyncing"
                @click="syncSystemClock"
              >
                {{ t("common.syncNow") }}
              </Button>
            </div>
          </div>
        </div>
        <div
          v-if="updateStore.shouldShowBanner && updateStore.status"
          :class="[
            'mx-auto mt-3 mb-6 w-full max-w-7xl rounded-lg border px-4 py-3',
            updateStore.isForceUpdate
              ? 'border-destructive/35 bg-destructive/10 text-destructive'
              : 'border-amber-500/35 bg-amber-500/10 text-amber-900 dark:text-amber-200',
          ]"
        >
          <div
            class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between"
          >
            <div class="space-y-1">
              <p class="text-sm font-semibold">
                {{
                  t("admin.banner.updateFound", {
                    latest: updateStore.status.latest?.version || "",
                    current: updateStore.status.localVersion,
                  })
                }}
              </p>
              <p class="text-xs">
                {{ updateBannerDescription }}
              </p>
            </div>
            <div class="flex items-center gap-2">
              <Button
                variant="outline"
                size="sm"
                class="bg-background/80"
                @click="goToAbout"
              >
                {{ t("common.viewDetails") }}
              </Button>
              <Button
                v-if="configStore.canSelfUpdate"
                size="sm"
                :variant="updateStore.isForceUpdate ? 'destructive' : 'default'"
                @click="startUpdateFromBanner"
              >
                {{ t("common.updateNow") }}
              </Button>
            </div>
          </div>
        </div>
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

    <Dialog v-model:open="isLocaleDialogOpen">
      <DialogContent class="gap-0 overflow-hidden p-0 sm:max-w-[420px]">
        <DialogHeader class="border-b px-5 py-4 text-left">
          <DialogTitle>{{ t("locale.label") }}</DialogTitle>
        </DialogHeader>
        <div class="divide-y">
          <template v-for="option in localeOptions" :key="option.value">
            <button
              type="button"
              :class="[
                'flex h-14 w-full items-center gap-3 px-5 text-left transition-colors',
                selectedLocale === option.value
                  ? 'bg-muted/90'
                  : 'hover:bg-muted/55',
                isSavingLocale ? 'cursor-not-allowed opacity-60' : '',
              ]"
              :disabled="isSavingLocale"
              :aria-current="
                selectedLocale === option.value ? 'true' : undefined
              "
              @click="handleLocaleSelect(option.value)"
            >
              <span
                :class="[
                  'grid h-5 w-5 shrink-0 place-items-center rounded-full border transition-colors',
                  selectedLocale === option.value
                    ? 'border-emerald-500 bg-emerald-500 text-white'
                    : 'border-muted-foreground/35',
                ]"
              >
                <Check
                  v-if="selectedLocale === option.value"
                  class="h-3.5 w-3.5"
                />
              </span>
              <span class="min-w-0 flex-1 truncate text-sm font-medium">
                {{ option.label }}
              </span>
              <span
                class="grid h-6 w-8 shrink-0 place-items-center overflow-hidden rounded-[5px] bg-white shadow-sm ring-1 ring-black/10"
                aria-hidden="true"
              >
                <svg
                  v-if="option.value === 'zh-CN'"
                  viewBox="0 0 32 24"
                  class="h-6 w-8"
                >
                  <defs>
                    <polygon
                      id="locale-flag-cn-star"
                      points="0,-1 0.24,-0.32 0.96,-0.31 0.38,0.12 0.59,0.82 0,0.4 -0.59,0.82 -0.38,0.12 -0.96,-0.31 -0.24,-0.32"
                    />
                  </defs>
                  <rect width="32" height="24" fill="#f23b2f" />
                  <g fill="#ffde45">
                    <use
                      href="#locale-flag-cn-star"
                      transform="translate(6.2 6.3) scale(3)"
                    />
                    <use
                      href="#locale-flag-cn-star"
                      transform="translate(12.6 3.6) scale(0.95)"
                    />
                    <use
                      href="#locale-flag-cn-star"
                      transform="translate(14.5 6.1) scale(0.95)"
                    />
                    <use
                      href="#locale-flag-cn-star"
                      transform="translate(14.2 9.2) scale(0.95)"
                    />
                    <use
                      href="#locale-flag-cn-star"
                      transform="translate(12.1 11.3) scale(0.95)"
                    />
                  </g>
                </svg>
                <svg
                  v-else-if="option.value === 'zh-Hant'"
                  viewBox="0 0 32 24"
                  class="h-6 w-8"
                >
                  <defs>
                    <path
                      id="locale-flag-hk-petal"
                      d="M0,-0.65 C-1.55,-3.25 -0.25,-5.95 2.35,-6.25 C4,-4 3.05,-1.45 0.8,0.45 C0.55,0.2 0.25,-0.15 0,-0.65Z"
                    />
                  </defs>
                  <rect width="32" height="24" fill="#f43b2f" />
                  <g fill="#fff" transform="translate(16 12)">
                    <use href="#locale-flag-hk-petal" transform="rotate(0)" />
                    <use href="#locale-flag-hk-petal" transform="rotate(72)" />
                    <use href="#locale-flag-hk-petal" transform="rotate(144)" />
                    <use href="#locale-flag-hk-petal" transform="rotate(216)" />
                    <use href="#locale-flag-hk-petal" transform="rotate(288)" />
                    <circle r="0.85" />
                  </g>
                </svg>
                <svg
                  v-else-if="option.value === 'ko-KR'"
                  viewBox="-72 -48 144 96"
                  class="h-6 w-8"
                >
                  <path fill="#fff" d="M-72 -48h144v96H-72z" />
                  <g fill="none" stroke="#000" stroke-width="4">
                    <path
                      transform="rotate(33.69006752598)"
                      d="M-50 -12v24m6 0v-24m6 0v24m76 0V1m0 -2v-11m6 0v11m0 2v11m6 0V1m0 -2v-11"
                    />
                    <path
                      transform="rotate(-33.69006752598)"
                      d="M-50 -12v24m6 0V1m0 -2v-11m6 0v24m76 0V1m0 -2v-11m6 0v24m6 0V1m0 -2v-11"
                    />
                  </g>
                  <g transform="rotate(33.69006752598)">
                    <path
                      fill="#cd2e3a"
                      d="M12 0a18 18 0 1 1 -36 0 24 24 0 1 1 48 0"
                    />
                    <path
                      fill="#0047a0"
                      d="M0 0a12 12 0 1 1 24 0 24 24 0 1 1 -48 0 12 12 0 1 0 24 0"
                    />
                  </g>
                </svg>
                <svg
                  v-else-if="option.value === 'ja-JP'"
                  viewBox="0 0 32 24"
                  class="h-6 w-8"
                >
                  <rect width="32" height="24" fill="#fff" />
                  <circle cx="16" cy="12" r="5.4" fill="#bc002d" />
                </svg>
                <svg v-else viewBox="0 0 32 24" class="h-6 w-8">
                  <rect width="32" height="24" fill="#f8f8f8" />
                  <g fill="#d62d2d">
                    <rect y="0" width="32" height="2.3" />
                    <rect y="4.3" width="32" height="2.3" />
                    <rect y="8.6" width="32" height="2.3" />
                    <rect y="12.9" width="32" height="2.3" />
                    <rect y="17.2" width="32" height="2.3" />
                    <rect y="21.5" width="32" height="2.5" />
                  </g>
                  <rect width="14" height="12.4" fill="#4b5fb8" />
                  <g fill="#fff">
                    <circle cx="2.3" cy="2.1" r="0.5" />
                    <circle cx="5" cy="2.1" r="0.5" />
                    <circle cx="7.7" cy="2.1" r="0.5" />
                    <circle cx="10.4" cy="2.1" r="0.5" />
                    <circle cx="3.65" cy="4.6" r="0.5" />
                    <circle cx="6.35" cy="4.6" r="0.5" />
                    <circle cx="9.05" cy="4.6" r="0.5" />
                    <circle cx="11.75" cy="4.6" r="0.5" />
                    <circle cx="2.3" cy="7.1" r="0.5" />
                    <circle cx="5" cy="7.1" r="0.5" />
                    <circle cx="7.7" cy="7.1" r="0.5" />
                    <circle cx="10.4" cy="7.1" r="0.5" />
                    <circle cx="3.65" cy="9.6" r="0.5" />
                    <circle cx="6.35" cy="9.6" r="0.5" />
                    <circle cx="9.05" cy="9.6" r="0.5" />
                    <circle cx="11.75" cy="9.6" r="0.5" />
                  </g>
                </svg>
              </span>
            </button>
          </template>
        </div>
      </DialogContent>
    </Dialog>
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
import {
  isAnySubdomainRoutingMode,
  isReverseProxySubdomainMode,
} from "../lib/reverse-proxy-submode";
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
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  Sheet,
  SheetContent,
  SheetHeader,
  SheetTitle,
} from "@/components/ui/sheet";
const APP_GITHUB_URL = "https://github.com/kci-lnk/fn-knock-turborepo";
import {
  BellRing,
  Check,
  FileKey2,
  FileSearch,
  Fingerprint,
  Globe2,
  LayoutDashboard,
  ShieldCheck,
  Route as RouteIcon,
  RadioTower,
  Github,
  Settings2,
  ShieldBan,
  SquareTerminal,
  UsersRound,
  Menu,
  Languages,
  LogOut,
  Network,
  ServerCog,
  ShieldAlert,
} from "lucide-vue-next";

const router = useRouter();
const route = useRoute();
const configStore = useConfigStore();
const dockerAdminAuthStore = useDockerAdminAuthStore();
const systemClockStore = useSystemClockStore();
const updateStore = useUpdateStore();
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

const goToAbout = () => {
  void navigateTo("/about");
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

const startUpdateFromBanner = async () => {
  if (!configStore.canSelfUpdate) {
    await navigateTo("/about");
    return;
  }
  await navigateTo("/about");
  await updateStore.checkAndDownload();
};

const refreshSystemClockStatus = async () => {
  await systemClockStore.refresh(true);
};

const syncSystemClock = async () => {
  await systemClockStore.sync();
};

const isNavActive = (path: string) => {
  const activePath = pendingNavPath.value ?? route.path;
  if (activePath === path) return true;
  if (path === "/") return activePath === "/";
  return activePath.startsWith(`${path}/`);
};

const navItems = computed(() => {
  const items = [
    { name: t("admin.nav.ipWhitelist"), path: "/whitelist", icon: ShieldCheck },
    { name: t("admin.nav.sslCert"), path: "/ssl", icon: FileKey2 },
  ];
  if (
    configStore.config?.run_type === 1 ||
    configStore.config?.run_type === 3
  ) {
    items.unshift({
      name: t("admin.nav.dashboard"),
      path: "/",
      icon: LayoutDashboard,
    });
  }
  items.push({ name: t("admin.nav.ddns"), path: "/ddns", icon: Network });
  if (configStore.config?.run_type === 1) {
    items.splice(1, 0, {
      name: isReverseProxySubdomainMode(configStore.config)
        ? t("admin.nav.subdomainMapping")
        : t("admin.nav.pathMapping"),
      path: isReverseProxySubdomainMode(configStore.config)
        ? "/subdomains"
        : "/proxy",
      icon: isReverseProxySubdomainMode(configStore.config)
        ? Globe2
        : RouteIcon,
    });
    const showTunnel = configStore.canUseFrpc || configStore.canUseCloudflared;
    if (showTunnel) {
      items.splice(2, 0, {
        name: t("admin.nav.tunnel"),
        path: "/tunnel",
        icon: RadioTower,
      });
    }
    items.splice(showTunnel ? 3 : 2, 0, {
      name: t("admin.nav.sessions"),
      path: "/sessions",
      icon: UsersRound,
    });
  } else if (isAnySubdomainRoutingMode(configStore.config)) {
    const isProtocolMappingVisible =
      configStore.config?.protocol_mapping_feature?.enabled === true;
    items.splice(1, 0, {
      name: t("admin.nav.subdomainMapping"),
      path: "/subdomains",
      icon: Globe2,
    });
    if (isProtocolMappingVisible) {
      items.splice(2, 0, {
        name: t("admin.nav.protocolMapping"),
        path: "/streams",
        icon: ServerCog,
      });
    }
    items.splice(isProtocolMappingVisible ? 3 : 2, 0, {
      name: t("admin.nav.sessions"),
      path: "/sessions",
      icon: UsersRound,
    });
  }
  items.push({
    name: t("admin.nav.authConfig"),
    path: "/auth",
    icon: Fingerprint,
  });
  if (
    configStore.canUseSshSecurity &&
    configStore.config?.ssh_security?.enabled === true
  ) {
    items.push({
      name: t("admin.nav.sshSecurity"),
      path: "/ssh-security",
      icon: ShieldBan,
    });
  }
  items.push({ name: t("admin.nav.events"), path: "/events", icon: BellRing });
  if (configStore.config?.gateway_logging?.enabled) {
    items.push({
      name: t("admin.nav.requestLogs"),
      path: "/request-logs",
      icon: FileSearch,
    });
  }
  if (configStore.config?.waf?.enabled) {
    items.push({
      name: t("admin.nav.wafLogs"),
      path: "/waf-logs",
      icon: ShieldAlert,
    });
  }
  if (
    configStore.canUseTerminal &&
    configStore.config?.terminal_feature?.enabled
  ) {
    items.push({
      name: t("admin.nav.webTerminal"),
      path: "/terminal",
      icon: SquareTerminal,
    });
  }
  items.push({
    name: t("admin.nav.systemSettings"),
    path: "/system",
    icon: Settings2,
  });
  return items;
});

const currentNavLabel = computed(() => {
  const activeItem = navItems.value.find((item) => isNavActive(item.path));
  return activeItem?.name ?? t("common.managementConsole");
});

const currentVersionLabel = computed(() => {
  const version = updateStore.status?.localVersion?.trim();
  return version ? `v${version}` : "";
});

const aboutEntryLabel = computed(() => t("admin.nav.systemUpdate"));

const systemClockBannerTitle = computed(() => {
  const status = systemClockStore.status;
  if (!status) return "";
  if (status.timezoneMismatch && status.timeMismatch) {
    return t("admin.banner.clockImmediate");
  }
  if (status.timezoneMismatch) {
    return t("admin.banner.timezoneMismatch");
  }
  return t("admin.banner.clockMismatch");
});

const systemClockBannerDescription = computed(() => {
  const status = systemClockStore.status;
  if (!status) return "";
  const messages = status.issues.map((issue) => issue.message);
  if (status.lastCheckError) {
    messages.push(
      t("admin.banner.lastCheckFailed", { error: status.lastCheckError }),
    );
  }
  if (!configStore.canSyncSystemClock) {
    messages.push(t("admin.banner.hostSyncUnsupported"));
  }
  return messages.join(" ");
});

const systemClockBannerMeta = computed(() => {
  const status = systemClockStore.status;
  if (!status) return "";

  const parts: string[] = [];
  if (status.systemBeijingTime) {
    parts.push(
      t("admin.banner.systemBeijingTime", { time: status.systemBeijingTime }),
    );
  }
  if (status.remoteBeijingTime) {
    parts.push(
      t("admin.banner.remoteBeijingTime", { time: status.remoteBeijingTime }),
    );
  }
  if (status.systemTimeZone) {
    parts.push(
      t("admin.banner.systemTimeZone", { timezone: status.systemTimeZone }),
    );
  }
  if (status.networkSource) {
    parts.push(
      t("admin.banner.networkSource", { source: status.networkSource }),
    );
  }
  return parts.join(" · ");
});

const updateBannerDescription = computed(() => {
  if (configStore.canSelfUpdate) {
    return updateStore.isForceUpdate
      ? t("admin.banner.importantUpdate")
      : t("admin.banner.normalUpdate");
  }

  if (configStore.isOpenWrtDeployment) {
    return t("admin.banner.openWrtUpdateInfo");
  }

  if (configStore.isDockerDeployment) {
    return t("admin.banner.dockerUpdateInfo");
  }

  if (configStore.isDesktopUpdateManaged) {
    return t("admin.banner.windowsUpdateInfo");
  }

  return t("admin.banner.genericUpdateInfo");
});
</script>
