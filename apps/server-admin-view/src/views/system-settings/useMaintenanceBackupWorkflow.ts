import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { toast } from "@admin-shared/utils/toast";
import { downloadBlob } from "@admin-shared/utils/downloadBlob";
import {
  buildKnockBackupFilename,
  KNOCK_BACKUP_EXTENSION,
  MAX_KNOCK_BACKUP_ARCHIVE_SIZE,
} from "@admin-shared/utils/maintenanceBackup";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { MaintenanceAPI } from "@/lib/api";
import { supportsSharedBackupForRuntime } from "@/lib/maintenance-runtime";
import {
  automaticBackupSourceIsAvailable,
  backupSourceMenuIsRequired,
  buildAutomaticBackupSelectionSummary,
} from "@/lib/automatic-backup";
import type {
  BackupDirectoryFilesPayload,
  AutomaticBackupFilesPayload,
  FnKnockBackupImportResult,
  SharedDataFileEntry,
} from "@/types";
import { useConfigStore } from "@/store/config";

type BackupSelectionSource = "local" | "fnos" | "automatic";

const defaultBackupFiles: BackupDirectoryFilesPayload = {
  shareName: "fn-knock / backup",
  available: false,
  files: [],
};

const defaultAutomaticBackupFiles: AutomaticBackupFilesPayload = {
  directoryPath: "",
  available: true,
  files: [],
};

export const useMaintenanceBackupWorkflow = () => {
  const configStore = useConfigStore();
  const { t } = useI18n();
  const fileInputRef = ref<HTMLInputElement | null>(null);
  const selectedLocalFile = ref<File | null>(null);
  const selectedFnosFile = ref<SharedDataFileEntry | null>(null);
  const selectedAutomaticFile = ref<SharedDataFileEntry | null>(null);
  const selectedSource = ref<BackupSelectionSource | null>(null);
  const isImportDialogOpen = ref(false);
  const isBackupPickerOpen = ref(false);
  const isAutomaticBackupPickerOpen = ref(false);
  const backupFilesError = ref("");
  const hasLoadedBackupFiles = ref(false);
  const backupFiles = ref<BackupDirectoryFilesPayload>(defaultBackupFiles);
  const automaticBackupFilesError = ref("");
  const hasLoadedAutomaticBackupFiles = ref(false);
  const automaticBackupFiles = ref<AutomaticBackupFilesPayload>(
    defaultAutomaticBackupFiles,
  );

  const supportsSharedBackup = computed(() =>
    supportsSharedBackupForRuntime(
      configStore.runtimeProfile,
      configStore.capabilities,
    ),
  );
  const localImportHintBeforeKey = computed(() => {
    if (configStore.isDockerDeployment) {
      return "admin.maintenanceSettings.dockerImportHintBefore";
    }
    if (configStore.isOpenWrtDeployment) {
      return "admin.maintenanceSettings.openWrtImportHintBefore";
    }
    return "admin.maintenanceSettings.localImportHintBefore";
  });
  const localImportHintAfterKey = computed(() => {
    if (configStore.isDockerDeployment) {
      return "admin.maintenanceSettings.dockerImportHintAfter";
    }
    if (configStore.isOpenWrtDeployment) {
      return "admin.maintenanceSettings.openWrtImportHintAfter";
    }
    return "admin.maintenanceSettings.localImportHintAfter";
  });

  const { isPending: isExporting, run: runExport } = useAsyncAction({
    onError: (error) => {
      toast.error(t("admin.maintenanceSettings.exportFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.maintenanceSettings.exportFailedDescription"),
        ),
      });
    },
  });
  const { isPending: isImporting, run: runImport } = useAsyncAction({
    onError: (error) => {
      toast.error(t("admin.maintenanceSettings.importFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.maintenanceSettings.importFailedDescription"),
        ),
      });
    },
  });
  const { isPending: isLoadingBackupFiles, run: runLoadBackupFiles } =
    useAsyncAction({
      onError: (error) => {
        const message = extractErrorMessage(
          error,
          t("admin.maintenanceSettings.loadFnosDirFailedDescription"),
        );
        backupFilesError.value = message;
        toast.error(t("admin.maintenanceSettings.loadFnosDirFailed"), {
          description: message,
        });
      },
    });
  const {
    isPending: isLoadingAutomaticBackupFiles,
    run: runLoadAutomaticBackupFiles,
  } = useAsyncAction({
    onError: (error) => {
      automaticBackupFilesError.value = extractErrorMessage(
        error,
        t("admin.maintenanceSettings.loadAutomaticDirFailedDescription"),
      );
    },
  });

  const isBusy = computed(() => isExporting.value || isImporting.value);
  const hasSelectedBackup = computed(() => {
    if (selectedSource.value === "local") {
      return selectedLocalFile.value !== null;
    }
    if (selectedSource.value === "fnos") {
      return selectedFnosFile.value !== null;
    }
    if (selectedSource.value === "automatic") {
      return selectedAutomaticFile.value !== null;
    }
    return false;
  });
  const hasAutomaticBackups = computed(() =>
    automaticBackupSourceIsAvailable(automaticBackupFiles.value.files.length),
  );
  const hasMultipleBackupSources = computed(() =>
    backupSourceMenuIsRequired(
      supportsSharedBackup.value,
      automaticBackupFiles.value.files.length,
    ),
  );

  const formatFileSize = (size: number) => {
    if (!Number.isFinite(size) || size < 1024) {
      return `${Math.max(0, Math.floor(size || 0))} B`;
    }
    if (size < 1024 * 1024) return `${(size / 1024).toFixed(1)} KB`;
    return `${(size / (1024 * 1024)).toFixed(2)} MB`;
  };

  const selectedSummary = computed(() => {
    if (selectedSource.value === "local" && selectedLocalFile.value) {
      return {
        name: selectedLocalFile.value.name,
        size: formatFileSize(selectedLocalFile.value.size),
        sourceLabel: t("admin.maintenanceSettings.localFile"),
        location: "",
      };
    }
    if (selectedSource.value === "fnos" && selectedFnosFile.value) {
      return {
        name: selectedFnosFile.value.name,
        size: formatFileSize(selectedFnosFile.value.size),
        sourceLabel: t("admin.maintenanceSettings.fnosBackup"),
        location: selectedFnosFile.value.relativePath,
      };
    }
    if (selectedSource.value === "automatic" && selectedAutomaticFile.value) {
      return buildAutomaticBackupSelectionSummary(
        selectedAutomaticFile.value,
        formatFileSize(selectedAutomaticFile.value.size),
        t("admin.maintenanceSettings.automaticBackup"),
      );
    }
    return null;
  });

  const resetSelectedBackup = () => {
    selectedLocalFile.value = null;
    selectedFnosFile.value = null;
    selectedAutomaticFile.value = null;
    selectedSource.value = null;
    if (fileInputRef.value) fileInputRef.value.value = "";
  };

  const triggerLocalFilePicker = () => {
    if (isBusy.value) return;
    if (fileInputRef.value) fileInputRef.value.value = "";
    fileInputRef.value?.click();
  };

  const handleFileChange = (event: Event) => {
    const input = event.target as HTMLInputElement | null;
    const file = input?.files?.[0] ?? null;
    if (!file) return;
    if (!file.name.toLowerCase().endsWith(KNOCK_BACKUP_EXTENSION)) {
      resetSelectedBackup();
      toast.error(t("admin.maintenanceSettings.invalidBackupFile"), {
        description: t(
          "admin.maintenanceSettings.invalidBackupFileDescription",
          { extension: KNOCK_BACKUP_EXTENSION },
        ),
      });
      return;
    }
    if (file.size > MAX_KNOCK_BACKUP_ARCHIVE_SIZE) {
      resetSelectedBackup();
      toast.error(t("admin.maintenanceSettings.backupFileTooLarge"), {
        description: t(
          "admin.maintenanceSettings.backupFileTooLargeDescription",
          { max: formatFileSize(MAX_KNOCK_BACKUP_ARCHIVE_SIZE) },
        ),
      });
      return;
    }
    selectedLocalFile.value = file;
    selectedFnosFile.value = null;
    selectedAutomaticFile.value = null;
    selectedSource.value = "local";
  };

  const loadBackupFiles = async (force = false) => {
    if (hasLoadedBackupFiles.value && !force) return;
    backupFilesError.value = "";
    const nextFiles = await runLoadBackupFiles(() =>
      MaintenanceAPI.getBackupDirectoryFiles(),
    );
    if (!nextFiles) return;
    backupFiles.value = nextFiles;
    hasLoadedBackupFiles.value = true;
  };

  const openFnosBackupPicker = async () => {
    if (isBusy.value) return;
    await loadBackupFiles();
    if (!backupFilesError.value) isBackupPickerOpen.value = true;
  };
  const refreshBackupFiles = () => loadBackupFiles(true);

  const loadAutomaticBackupFiles = async (force = false) => {
    if (hasLoadedAutomaticBackupFiles.value && !force) return;
    automaticBackupFilesError.value = "";
    const nextFiles = await runLoadAutomaticBackupFiles(() =>
      MaintenanceAPI.getAutomaticBackupFiles(),
    );
    if (!nextFiles) return;
    automaticBackupFiles.value = nextFiles;
    hasLoadedAutomaticBackupFiles.value = true;
  };

  const openAutomaticBackupPicker = async () => {
    if (isBusy.value) return;
    await loadAutomaticBackupFiles(true);
    if (!automaticBackupFilesError.value) {
      isAutomaticBackupPickerOpen.value = true;
    }
  };
  const refreshAutomaticBackupFiles = () => loadAutomaticBackupFiles(true);

  const handleFnosFileSelect = (file: SharedDataFileEntry) => {
    selectedFnosFile.value = file;
    selectedLocalFile.value = null;
    selectedAutomaticFile.value = null;
    selectedSource.value = "fnos";
    isBackupPickerOpen.value = false;
    toast.success(
      t("admin.maintenanceSettings.fnosBackupSelected", { name: file.name }),
    );
  };

  const handleAutomaticFileSelect = (file: SharedDataFileEntry) => {
    selectedAutomaticFile.value = file;
    selectedFnosFile.value = null;
    selectedLocalFile.value = null;
    selectedSource.value = "automatic";
    isAutomaticBackupPickerOpen.value = false;
    toast.success(
      t("admin.maintenanceSettings.automaticBackupSelected", {
        name: file.name,
      }),
    );
  };

  const exportBackupToLocal = async () => {
    await runExport(async () => {
      downloadBlob(
        await MaintenanceAPI.downloadBackup(),
        buildKnockBackupFilename(),
      );
      toast.success(t("admin.maintenanceSettings.backupDownloadStarted"));
    });
  };

  const exportBackupToFnos = async () => {
    await runExport(async () => {
      const result = await MaintenanceAPI.exportBackupToFnos();
      if (hasLoadedBackupFiles.value) await loadBackupFiles(true);
      toast.success(t("admin.maintenanceSettings.backupExportedToFnos"), {
        description: t("admin.maintenanceSettings.writtenToPath", {
          path: result.relativePath,
        }),
      });
    });
  };

  const openImportDialog = () => {
    if (!hasSelectedBackup.value) {
      toast.error(t("admin.maintenanceSettings.chooseBackupFirst"));
      return;
    }
    isImportDialogOpen.value = true;
  };

  const buildImportDescription = (result: FnKnockBackupImportResult) => {
    if (result.warnings.length === 0) {
      return t("admin.maintenanceSettings.importSuccessDescription", {
        keys: result.imported_keys,
        steps: result.synced_steps.length,
      });
    }
    const preview = result.warnings.slice(0, 2).join("；");
    return result.warnings.length > 2
      ? t("admin.maintenanceSettings.importWarningsWithMore", {
          preview,
          count: result.warnings.length - 2,
        })
      : preview;
  };

  const readFileAsBase64 = (file: File): Promise<string> =>
    new Promise((resolve, reject) => {
      const reader = new FileReader();
      reader.onerror = () =>
        reject(
          reader.error ||
            new Error(t("admin.maintenanceSettings.readBackupFileFailed")),
        );
      reader.onload = () => {
        const result = typeof reader.result === "string" ? reader.result : "";
        const marker = "base64,";
        const markerIndex = result.indexOf(marker);
        if (markerIndex < 0) {
          reject(
            new Error(t("admin.maintenanceSettings.parseBackupFileFailed")),
          );
          return;
        }
        resolve(result.slice(markerIndex + marker.length));
      };
      reader.readAsDataURL(file);
    });

  const importBackup = async () => {
    await runImport(
      async () => {
        if (selectedSource.value === "fnos" && selectedFnosFile.value) {
          return MaintenanceAPI.importBackupFromFnos(
            selectedFnosFile.value.relativePath,
          );
        }
        if (
          selectedSource.value === "automatic" &&
          selectedAutomaticFile.value
        ) {
          return MaintenanceAPI.importBackupFromAutomatic(
            selectedAutomaticFile.value.relativePath,
          );
        }
        if (selectedSource.value === "local" && selectedLocalFile.value) {
          return MaintenanceAPI.importBackup({
            filename: selectedLocalFile.value.name,
            archive_base64: await readFileAsBase64(selectedLocalFile.value),
          });
        }
        throw new Error(t("admin.maintenanceSettings.chooseBackupFirst"));
      },
      {
        onSuccess: async (result) => {
          isImportDialogOpen.value = false;
          resetSelectedBackup();
          await configStore.loadConfig();
          const notify =
            result.warnings.length > 0 ? toast.info : toast.success;
          notify(t("admin.maintenanceSettings.backupImported"), {
            description: buildImportDescription(result),
          });
          if (typeof window !== "undefined") {
            window.setTimeout(() => window.location.reload(), 1200);
          }
        },
      },
    );
  };

  onMounted(() => {
    void loadAutomaticBackupFiles();
  });

  return {
    automaticBackupFiles,
    automaticBackupFilesError,
    backupFiles,
    backupFilesError,
    exportBackupToFnos,
    exportBackupToLocal,
    fileInputRef,
    handleFileChange,
    handleAutomaticFileSelect,
    handleFnosFileSelect,
    hasSelectedBackup,
    hasAutomaticBackups,
    hasMultipleBackupSources,
    importBackup,
    isBackupPickerOpen,
    isAutomaticBackupPickerOpen,
    isBusy,
    isExporting,
    isImportDialogOpen,
    isImporting,
    isLoadingBackupFiles,
    isLoadingAutomaticBackupFiles,
    localImportHintAfterKey,
    localImportHintBeforeKey,
    openFnosBackupPicker,
    openAutomaticBackupPicker,
    openImportDialog,
    refreshBackupFiles,
    refreshAutomaticBackupFiles,
    selectedSummary,
    supportsSharedBackup,
    triggerLocalFilePicker,
  };
};
