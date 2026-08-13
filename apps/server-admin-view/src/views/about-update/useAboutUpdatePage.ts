import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useConfigStore } from "../../store/config";
import { useUpdateStore } from "../../store/update";
import {
  FULL_VERSION_WEBSITE_URL,
  shouldShowOneClickUpdate,
} from "../../lib/update-presentation";

export function useAboutUpdatePage() {
  const updateStore = useUpdateStore();
  const configStore = useConfigStore();
  const { t } = useI18n();
  const showInstallingOverlay = ref(false);
  let disposed = false;

  const status = computed(() => updateStore.status);
  const canSelfUpdate = computed(() => configStore.canSelfUpdate);
  const desktopUpdateManaged = computed(
    () => configStore.isDesktopUpdateManaged,
  );
  const isSynologyDeployment = computed(
    () => configStore.isSynologyDeployment,
  );
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
  const versionStatusHint = computed(() => {
    if (!status.value?.hasUpdate) return t(versionCheckHintKey.value);
    return canSelfUpdate.value
      ? t("admin.aboutUpdate.newVersionSelfUpdateHint")
      : t(nonSelfUpdateVersionHintKey.value);
  });
  const downloadState = computed(
    () => status.value?.download.status ?? "idle",
  );
  const progressValue = computed(() => status.value?.download.percent ?? 0);
  const progressText = computed(() => {
    const current = status.value;
    if (!current) return "";
    const bytes = current.download.downloadedBytes;
    const total = current.download.totalBytes;
    if (!total || total <= 0) return `${bytes} B`;
    const toMegabytes = (value: number) =>
      (value / (1_024 * 1_024)).toFixed(2);
    return `${toMegabytes(bytes)} MB / ${toMegabytes(total)} MB`;
  });

  const isChecking = computed(() => updateStore.isChecking);
  const canInstall = computed(() => updateStore.canInstall);
  const canTriggerOneClick = computed(
    () =>
      !updateStore.isChecking &&
      !updateStore.isTriggeringDownload &&
      !updateStore.isTriggeringInstall &&
      !["downloading", "verifying", "installing"].includes(
        downloadState.value,
      ),
  );
  const oneClickLabel = computed(() => {
    if (["downloading", "verifying", "installing"].includes(downloadState.value)) {
      return t("admin.aboutUpdate.updateInProgress");
    }
    return updateStore.canInstall
      ? t("admin.aboutUpdate.oneClickInstall")
      : t("admin.aboutUpdate.oneClickUpdate");
  });
  const showOneClickUpdateButton = computed(() =>
    shouldShowOneClickUpdate({
      hasUpdate: Boolean(status.value?.hasUpdate),
      canSelfUpdate: canSelfUpdate.value,
      isFpkLite: isFpkLiteDeployment.value,
    }),
  );
  const isDownloadingOrVerifying = computed(() =>
    ["downloading", "verifying"].includes(downloadState.value),
  );
  const isUpdateModalVisible = computed(
    () =>
      isDownloadingOrVerifying.value ||
      showInstallingOverlay.value ||
      downloadState.value === "installing",
  );
  const modalTitle = computed(() => {
    if (downloadState.value === "downloading") {
      return t("admin.aboutUpdate.modalDownloading");
    }
    if (downloadState.value === "verifying") {
      return t("admin.aboutUpdate.modalVerifying");
    }
    return t("admin.aboutUpdate.modalInstalling");
  });
  const modalDescription = computed(() =>
    isDownloadingOrVerifying.value
      ? t("admin.aboutUpdate.modalDownloadDescription")
      : t("admin.aboutUpdate.modalInstallDescription"),
  );

  const checkNow = async () => {
    await updateStore.checkNow(true);
  };
  const openExternal = (url?: string | null) => {
    if (url) window.open(url, "_blank", "noopener,noreferrer");
  };
  const openGithub = () => openExternal(status.value?.githubUrl);
  const openSynologyWebsite = () =>
    openExternal("https://www.fnknock.cn/synology");
  const openOfficialWebsite = () => openExternal(FULL_VERSION_WEBSITE_URL);

  const startInstallFlow = async () => {
    showInstallingOverlay.value = true;
    await new Promise((resolve) => window.setTimeout(resolve, 250));
    if (disposed) return;
    const started = await updateStore.startInstall();
    if (!started && !disposed) showInstallingOverlay.value = false;
  };
  const oneClickUpdate = async () => {
    if (!canSelfUpdate.value) return;
    if (updateStore.canInstall) {
      await startInstallFlow();
      return;
    }
    await updateStore.checkAndDownload();
  };

  onMounted(async () => {
    if (!configStore.config) await configStore.loadConfig();
    if (!disposed) await updateStore.initialize();
  });
  onBeforeUnmount(() => {
    disposed = true;
  });

  return {
    canInstall,
    canSelfUpdate,
    canTriggerOneClick,
    checkNow,
    desktopUpdateManaged,
    isChecking,
    isDownloadingOrVerifying,
    isFpkLiteDeployment,
    isSynologyDeployment,
    isUpdateModalVisible,
    modalDescription,
    modalTitle,
    oneClickLabel,
    oneClickUpdate,
    openGithub,
    openOfficialWebsite,
    openSynologyWebsite,
    progressText,
    progressValue,
    showOneClickUpdateButton,
    status,
    unsupportedDescriptionKey,
    updateSubtitleKey,
    versionStatusHint,
    versionStatusMessage,
  };
}

export type AboutUpdatePageController = ReturnType<typeof useAboutUpdatePage>;
