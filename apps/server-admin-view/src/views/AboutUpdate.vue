<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { Card, CardContent } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Progress } from "@/components/ui/progress";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import ReleaseNotesMarkdown from "../components/ReleaseNotesMarkdown.vue";
import { useUpdateStore } from "../store/update";
import { useConfigStore } from "../store/config";
import {
  Github,
  RefreshCw,
  Rocket,
  ArrowRight,
  CheckCircle2,
  Sparkles,
  Terminal,
  MonitorUp,
  AlertCircle,
  BookOpen,
  ExternalLink,
  Globe2,
} from "lucide-vue-next";
import {
  FULL_VERSION_WEBSITE_URL,
  OFFICIAL_DOCUMENTATION_URL,
  OFFICIAL_WEBSITE_URL,
  shouldShowOneClickUpdate,
} from "../lib/update-presentation";

const updateStore = useUpdateStore();
const configStore = useConfigStore();
const { t } = useI18n();
const showInstallingOverlay = ref(false);

const status = computed(() => updateStore.status);
const canSelfUpdate = computed(() => configStore.canSelfUpdate);
const desktopUpdateManaged = computed(() => configStore.isDesktopUpdateManaged);
const isSynologyDeployment = computed(() => configStore.isSynologyDeployment);
const isFpkLiteDeployment = computed(() => configStore.isFpkLiteDeployment);
const nonSelfUpdateTarget = computed(() => {
  if (isFpkLiteDeployment.value) return "Lite";
  if (desktopUpdateManaged.value) return "Desktop";
  if (isSynologyDeployment.value) return "Synology";
  if (configStore.isOpenWrtDeployment) return "OpenWrt";
  if (configStore.isDockerDeployment) return "Docker";
  return "Generic";
});
const updateSubtitleKey = computed(() =>
  canSelfUpdate.value
    ? "admin.aboutUpdate.subtitleSelfUpdate"
    : `admin.aboutUpdate.subtitle${nonSelfUpdateTarget.value}`,
);
const unsupportedDescriptionKey = computed(
  () =>
    `admin.aboutUpdate.selfUpdateUnsupportedDescription${nonSelfUpdateTarget.value}`,
);
const nonSelfUpdateVersionMessageKey = computed(
  () => `admin.aboutUpdate.newVersion${nonSelfUpdateTarget.value}`,
);
const nonSelfUpdateVersionHintKey = computed(
  () => `admin.aboutUpdate.newVersion${nonSelfUpdateTarget.value}Hint`,
);
const versionCheckHintKey = computed(() =>
  canSelfUpdate.value
    ? "admin.aboutUpdate.latestHint"
    : `admin.aboutUpdate.versionCheckHint${nonSelfUpdateTarget.value}`,
);
const versionStatusMessage = computed(() => {
  if (status.value?.hasUpdate) {
    return canSelfUpdate.value
      ? t("admin.aboutUpdate.newVersionSelfUpdate")
      : t(nonSelfUpdateVersionMessageKey.value);
  }
  if (canSelfUpdate.value || desktopUpdateManaged.value) {
    return status.value?.updateEnabled
      ? t("admin.aboutUpdate.alreadyLatest")
      : t("admin.aboutUpdate.updateDisabled");
  }
  return t("admin.aboutUpdate.versionCheckOnly");
});
const downloadState = computed(() => status.value?.download.status ?? "idle");

const progressValue = computed(() => status.value?.download.percent ?? 0);
const progressText = computed(() => {
  const current = status.value;
  if (!current) return "";
  const bytes = current.download.downloadedBytes;
  const total = current.download.totalBytes;
  if (!total || total <= 0) return `${bytes} B`;
  const toMB = (v: number) => (v / (1024 * 1024)).toFixed(2);
  return `${toMB(bytes)} MB / ${toMB(total)} MB`;
});

const canTriggerOneClick = computed(() => {
  return (
    !updateStore.isChecking &&
    !updateStore.isTriggeringDownload &&
    !updateStore.isTriggeringInstall &&
    downloadState.value !== "downloading" &&
    downloadState.value !== "verifying" &&
    downloadState.value !== "installing"
  );
});

const oneClickLabel = computed(() => {
  if (
    downloadState.value === "downloading" ||
    downloadState.value === "verifying" ||
    downloadState.value === "installing"
  ) {
    return t("admin.aboutUpdate.updateInProgress");
  }
  if (updateStore.canInstall) return t("admin.aboutUpdate.oneClickInstall");
  return t("admin.aboutUpdate.oneClickUpdate");
});

const showOneClickUpdateButton = computed(() =>
  shouldShowOneClickUpdate({
    hasUpdate: Boolean(status.value?.hasUpdate),
    canSelfUpdate: canSelfUpdate.value,
    isFpkLite: isFpkLiteDeployment.value,
  }),
);

const isDownloadingOrVerifying = computed(() => {
  return (
    downloadState.value === "downloading" || downloadState.value === "verifying"
  );
});

const isUpdateModalVisible = computed(() => {
  return (
    isDownloadingOrVerifying.value ||
    showInstallingOverlay.value ||
    downloadState.value === "installing"
  );
});

const modalTitle = computed(() => {
  if (downloadState.value === "downloading") {
    return t("admin.aboutUpdate.modalDownloading");
  }
  if (downloadState.value === "verifying") {
    return t("admin.aboutUpdate.modalVerifying");
  }
  return t("admin.aboutUpdate.modalInstalling");
});

const modalDescription = computed(() => {
  if (isDownloadingOrVerifying.value) {
    return t("admin.aboutUpdate.modalDownloadDescription");
  }
  return t("admin.aboutUpdate.modalInstallDescription");
});

const checkNow = async () => {
  await updateStore.checkNow(true);
};

const openGithub = () => {
  const githubUrl = status.value?.githubUrl;
  if (!githubUrl) return;
  window.open(githubUrl, "_blank", "noopener,noreferrer");
};

const openSynologyWebsite = () => {
  window.open(
    "https://www.fnknock.cn/synology",
    "_blank",
    "noopener,noreferrer",
  );
};

const openOfficialWebsite = () => {
  window.open(FULL_VERSION_WEBSITE_URL, "_blank", "noopener,noreferrer");
};

const startInstallFlow = async () => {
  showInstallingOverlay.value = true;
  await new Promise((resolve) => window.setTimeout(resolve, 250));
  const started = await updateStore.startInstall();
  if (!started) {
    showInstallingOverlay.value = false;
  }
};

const oneClickUpdate = async () => {
  if (!canSelfUpdate.value) {
    return;
  }
  if (updateStore.canInstall) {
    await startInstallFlow();
    return;
  }
  await updateStore.checkAndDownload();
};

onMounted(async () => {
  if (!configStore.config) {
    await configStore.loadConfig();
  }
  await updateStore.initialize();
});
</script>

<template>
  <div class="mx-auto space-y-6">
    <Card class="border-border/50 shadow-sm overflow-hidden">
      <CardContent class="space-y-8">
        <div
          class="flex flex-col gap-4 px-1 sm:flex-row sm:items-start sm:justify-between"
        >
          <div>
            <h2 class="text-2xl font-semibold tracking-tight">
              {{ t("admin.aboutUpdate.title") }}
            </h2>
            <p class="text-sm text-muted-foreground mt-1">
              {{ t(updateSubtitleKey) }}
            </p>
          </div>
          <div class="flex flex-wrap items-center gap-2">
            <Button
              as-child
              variant="outline"
              size="sm"
              class="border-border/70 bg-card shadow-none hover:bg-muted/60 dark:bg-muted/20 dark:hover:bg-muted/35"
            >
              <a
                :href="OFFICIAL_WEBSITE_URL"
                target="_blank"
                rel="noopener noreferrer"
              >
                <Globe2 class="h-4 w-4" />
                {{ t("admin.aboutUpdate.officialWebsite") }}
              </a>
            </Button>
            <Button
              as-child
              variant="outline"
              size="sm"
              class="border-border/70 bg-card shadow-none hover:bg-muted/60 dark:bg-muted/20 dark:hover:bg-muted/35"
            >
              <a
                :href="OFFICIAL_DOCUMENTATION_URL"
                target="_blank"
                rel="noopener noreferrer"
              >
                <BookOpen class="h-4 w-4" />
                {{ t("admin.aboutUpdate.officialDocumentation") }}
              </a>
            </Button>
            <Button
              variant="ghost"
              size="icon-sm"
              class="rounded-full hover:bg-muted"
              :disabled="!status?.githubUrl"
              :title="t('admin.aboutUpdate.openGithub')"
              :aria-label="t('admin.aboutUpdate.openGithub')"
              @click="openGithub"
            >
              <Github class="h-5 w-5" />
            </Button>
          </div>
        </div>

        <Alert
          v-if="desktopUpdateManaged"
          class="rounded-xl border-primary/20 bg-primary/5 text-foreground shadow-none"
        >
          <MonitorUp class="h-4 w-4" />
          <AlertTitle>{{
            t("admin.aboutUpdate.desktopManagedTitle")
          }}</AlertTitle>
          <AlertDescription>
            {{ t("admin.aboutUpdate.desktopManagedDescription") }}
          </AlertDescription>
        </Alert>

        <div
          v-if="isSynologyDeployment"
          class="flex flex-col gap-3 rounded-xl border border-border/50 bg-muted/[0.14] px-4 py-3.5 sm:flex-row sm:items-center sm:justify-between"
        >
          <div class="min-w-0 space-y-0.5">
            <p class="text-sm font-medium text-foreground">
              {{ t("admin.aboutUpdate.synologyUpdateTitle") }}
            </p>
            <p class="text-xs leading-5 text-muted-foreground">
              {{ t("admin.aboutUpdate.synologyUpdateDescription") }}
            </p>
          </div>
          <Button
            variant="outline"
            size="sm"
            class="shrink-0 border-border/70 bg-card shadow-none hover:bg-muted/60 dark:bg-muted/20 dark:hover:bg-muted/35"
            @click="openSynologyWebsite"
          >
            {{ t("admin.aboutUpdate.synologyWebsite") }}
            <ExternalLink class="ml-2 h-3.5 w-3.5" />
          </Button>
        </div>

        <div
          v-if="isFpkLiteDeployment"
          class="flex flex-col gap-4 rounded-xl border border-primary/20 bg-primary/5 px-4 py-4 sm:flex-row sm:items-center sm:justify-between"
        >
          <div class="min-w-0 space-y-1">
            <p class="text-sm font-semibold text-foreground">
              {{ t("admin.aboutUpdate.liteUpdateTitle") }}
            </p>
            <p class="text-xs leading-5 text-muted-foreground">
              {{ t("admin.aboutUpdate.liteUpdateDescription") }}
            </p>
            <p class="text-xs leading-5 text-muted-foreground">
              {{ t("admin.aboutUpdate.liteMigrationSteps") }}
            </p>
          </div>
          <Button
            size="sm"
            class="shrink-0 shadow-sm"
            @click="openOfficialWebsite"
          >
            {{ t("admin.aboutUpdate.liteDownloadFull") }}
            <ExternalLink class="ml-2 h-3.5 w-3.5" />
          </Button>
        </div>

        <Alert
          v-if="
            !canSelfUpdate &&
            !desktopUpdateManaged &&
            !isSynologyDeployment &&
            !isFpkLiteDeployment
          "
          class="rounded-xl border-border/70 bg-muted/30 text-foreground shadow-none"
        >
          <AlertCircle class="w-4 h-4" />
          <AlertTitle>{{
            t("admin.aboutUpdate.selfUpdateUnsupportedTitle")
          }}</AlertTitle>
          <AlertDescription>
            {{ t(unsupportedDescriptionKey) }}
          </AlertDescription>
        </Alert>

        <div
          class="flex items-center justify-center rounded-2xl border border-border/50 bg-muted/[0.16] px-4 py-6"
        >
          <div class="flex flex-col items-center flex-1 space-y-1">
            <span class="text-sm font-medium text-muted-foreground">{{
              t("admin.aboutUpdate.currentVersion")
            }}</span>
            <span
              class="text-2xl font-bold font-mono tracking-tight text-foreground"
            >
              {{ status?.localVersion || "..." }}
            </span>
          </div>

          <div class="px-4 md:px-8 text-muted-foreground/40">
            <ArrowRight class="w-6 h-6" />
          </div>

          <div class="flex flex-col items-center flex-1 space-y-1">
            <span class="text-sm font-medium text-muted-foreground">{{
              t("admin.aboutUpdate.latestVersion")
            }}</span>
            <span
              class="text-2xl font-bold font-mono tracking-tight"
              :class="status?.hasUpdate ? 'text-primary' : 'text-foreground'"
            >
              {{ status?.latest?.version || "..." }}
            </span>
          </div>
        </div>

        <div
          class="flex flex-col items-stretch justify-between gap-6 rounded-xl border border-border/50 bg-muted/[0.14] p-4 sm:flex-row sm:items-center"
        >
          <div class="flex items-center gap-3 w-full sm:w-auto">
            <div
              class="flex items-center justify-center w-10 h-10 rounded-full"
              :class="
                status?.hasUpdate
                  ? 'bg-primary/10 text-primary'
                  : 'bg-muted text-muted-foreground'
              "
            >
              <Sparkles v-if="status?.hasUpdate" class="w-5 h-5" />
              <CheckCircle2 v-else-if="status?.updateEnabled" class="w-5 h-5" />
              <AlertCircle v-else class="w-5 h-5" />
            </div>
            <div class="space-y-0.5">
              <p class="text-sm font-medium">
                {{ versionStatusMessage }}
              </p>
              <p class="text-xs text-muted-foreground">
                {{
                  status?.hasUpdate
                    ? canSelfUpdate
                      ? t("admin.aboutUpdate.newVersionSelfUpdateHint")
                      : t(nonSelfUpdateVersionHintKey)
                    : t(versionCheckHintKey)
                }}
              </p>
            </div>
          </div>

          <div class="flex items-center gap-2 sm:gap-3 w-full sm:w-auto">
            <Button
              variant="outline"
              class="flex-1 min-w-0 border-border/70 bg-card shadow-none hover:bg-muted/60 sm:flex-none sm:w-auto dark:bg-muted/20 dark:hover:bg-muted/35"
              :disabled="updateStore.isChecking"
              @click="checkNow"
            >
              <RefreshCw
                class="mr-2 h-4 w-4"
                :class="updateStore.isChecking ? 'animate-spin' : ''"
              />
              {{ t("admin.aboutUpdate.checkUpdate") }}
            </Button>
            <Button
              v-if="showOneClickUpdateButton"
              class="flex-1 min-w-0 sm:flex-none sm:w-auto shadow-sm"
              :disabled="!canTriggerOneClick"
              :variant="updateStore.canInstall ? 'destructive' : 'default'"
              @click="oneClickUpdate"
            >
              <Rocket class="mr-2 h-4 w-4" />
              <span class="sm:hidden">{{
                updateStore.canInstall
                  ? t("admin.aboutUpdate.installRestart")
                  : oneClickLabel
              }}</span>
              <span class="hidden sm:inline">{{ oneClickLabel }}</span>
            </Button>
          </div>
        </div>

        <Alert
          v-if="status?.check.error"
          variant="destructive"
          class="rounded-xl"
        >
          <AlertCircle class="w-4 h-4" />
          <AlertTitle>{{ t("admin.aboutUpdate.checkFailed") }}</AlertTitle>
          <AlertDescription>{{ status.check.error }}</AlertDescription>
        </Alert>

        <Alert
          v-if="status?.download.error"
          variant="destructive"
          class="rounded-xl"
        >
          <AlertCircle class="w-4 h-4" />
          <AlertTitle>{{ t("admin.aboutUpdate.updateFailed") }}</AlertTitle>
          <AlertDescription>{{ status.download.error }}</AlertDescription>
        </Alert>

        <div
          v-if="status?.latest?.release_notes"
          class="pt-4 border-t border-border/40"
        >
          <h3
            class="text-sm font-medium flex items-center gap-2 mb-4 text-foreground"
          >
            <Terminal class="w-4 h-4 text-muted-foreground" />
            {{ t("admin.aboutUpdate.releaseNotes") }}
          </h3>
          <div class="p-5 rounded-2xl bg-muted/30 border border-border/40">
            <ReleaseNotesMarkdown
              :source="status?.latest?.release_notes"
              :fallback="t('admin.aboutUpdate.noReleaseNotes')"
            />
          </div>
        </div>
      </CardContent>
    </Card>

    <div
      v-if="isUpdateModalVisible"
      class="fixed inset-0 z-[120] flex items-center justify-center bg-background/80 backdrop-blur-md px-4 transition-all duration-300"
    >
      <div
        class="w-full max-w-sm rounded-2xl border border-border/50 bg-background/95 p-8 shadow-2xl flex flex-col items-center text-center space-y-6"
      >
        <div
          class="w-16 h-16 rounded-full bg-primary/10 flex items-center justify-center"
        >
          <RefreshCw
            v-if="isDownloadingOrVerifying"
            class="w-8 h-8 text-primary animate-spin"
          />
          <MonitorUp v-else class="w-8 h-8 text-primary animate-pulse" />
        </div>

        <div class="space-y-2">
          <h3 class="text-lg font-semibold tracking-tight">{{ modalTitle }}</h3>
          <p
            class="text-sm text-muted-foreground leading-relaxed"
            v-html="modalDescription"
          ></p>
        </div>

        <div v-if="isDownloadingOrVerifying" class="w-full space-y-3">
          <Progress :model-value="progressValue" class="h-2 w-full" />
          <div
            class="flex justify-between text-xs text-muted-foreground font-mono"
          >
            <span>{{ progressText }}</span>
            <span class="text-primary font-bold">{{ progressValue }}%</span>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>
