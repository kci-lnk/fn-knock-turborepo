<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import DataShareFilePicker from "@admin-shared/components/common/DataShareFilePicker.vue";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import {
  ChevronDown,
  Download,
  FolderTree,
  Laptop,
  Loader2,
  Trash2,
  Upload,
} from "lucide-vue-next";
import { toast } from "@admin-shared/utils/toast";
import {
  buildKnockBackupFilename,
  KNOCK_BACKUP_EXTENSION,
} from "@admin-shared/utils/maintenanceBackup";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { MaintenanceAPI } from "../../lib/api";
import { supportsSharedBackupForRuntime } from "../../lib/maintenance-runtime";
import type {
  BackupDirectoryFilesPayload,
  FnKnockBackupImportResult,
  SharedDataFileEntry,
} from "../../types";
import { useConfigStore } from "../../store/config";

type BackupSelectionSource = "local" | "fnos";

const configStore = useConfigStore();
const { t } = useI18n();
const fileInputRef = ref<HTMLInputElement | null>(null);
const selectedLocalFile = ref<File | null>(null);
const selectedFnosFile = ref<SharedDataFileEntry | null>(null);
const selectedSource = ref<BackupSelectionSource | null>(null);
const isImportDialogOpen = ref(false);
const isClearDataDialogOpen = ref(false);
const clearDataConfirmation = ref("");
const isBackupPickerOpen = ref(false);
const backupFilesError = ref("");
const hasLoadedBackupFiles = ref(false);
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

const defaultBackupFiles: BackupDirectoryFilesPayload = {
  shareName: "fn-knock / backup",
  available: false,
  files: [],
};
const backupFiles = ref<BackupDirectoryFilesPayload>(defaultBackupFiles);

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

const { isPending: isClearingData, run: runClearData } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.maintenanceSettings.clearAllDataFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.maintenanceSettings.clearAllDataFailedDescription"),
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

const isBusy = computed(() => isExporting.value || isImporting.value);
const expectedClearDataConfirmation = computed(() =>
  t("admin.maintenanceSettings.clearAllDataConfirmationPhrase"),
);
const canClearAllData = computed(
  () =>
    !isClearingData.value &&
    clearDataConfirmation.value === expectedClearDataConfirmation.value,
);
const hasSelectedBackup = computed(() => {
  if (selectedSource.value === "local") {
    return selectedLocalFile.value !== null;
  }
  if (selectedSource.value === "fnos") {
    return selectedFnosFile.value !== null;
  }
  return false;
});

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

  return null;
});

function formatFileSize(size: number): string {
  if (!Number.isFinite(size) || size < 1024) {
    return `${Math.max(0, Math.floor(size || 0))} B`;
  }
  if (size < 1024 * 1024) {
    return `${(size / 1024).toFixed(1)} KB`;
  }
  return `${(size / (1024 * 1024)).toFixed(2)} MB`;
}

function buildDownloadFilename(): string {
  return buildKnockBackupFilename();
}

function resetSelectedBackup() {
  selectedLocalFile.value = null;
  selectedFnosFile.value = null;
  selectedSource.value = null;
  if (fileInputRef.value) {
    fileInputRef.value.value = "";
  }
}

function triggerLocalFilePicker() {
  if (isBusy.value) return;
  if (fileInputRef.value) {
    fileInputRef.value.value = "";
  }
  fileInputRef.value?.click();
}

async function handleFileChange(event: Event) {
  const input = event.target as HTMLInputElement | null;
  const file = input?.files?.[0] ?? null;

  if (!file) {
    return;
  }

  if (!file.name.toLowerCase().endsWith(KNOCK_BACKUP_EXTENSION)) {
    resetSelectedBackup();
    toast.error(t("admin.maintenanceSettings.invalidBackupFile"), {
      description: t("admin.maintenanceSettings.invalidBackupFileDescription", {
        extension: KNOCK_BACKUP_EXTENSION,
      }),
    });
    return;
  }

  selectedLocalFile.value = file;
  selectedFnosFile.value = null;
  selectedSource.value = "local";
}

async function loadBackupFiles(force = false) {
  if (hasLoadedBackupFiles.value && !force) return;

  backupFilesError.value = "";
  const nextFiles = await runLoadBackupFiles(async () =>
    MaintenanceAPI.getBackupDirectoryFiles(),
  );
  if (!nextFiles) return;

  backupFiles.value = nextFiles;
  hasLoadedBackupFiles.value = true;
}

async function openFnosBackupPicker() {
  if (isBusy.value) return;
  await loadBackupFiles();
  if (backupFilesError.value) return;
  isBackupPickerOpen.value = true;
}

async function refreshBackupFiles() {
  await loadBackupFiles(true);
}

function handleFnosFileSelect(file: SharedDataFileEntry) {
  selectedFnosFile.value = file;
  selectedLocalFile.value = null;
  selectedSource.value = "fnos";
  isBackupPickerOpen.value = false;
  toast.success(
    t("admin.maintenanceSettings.fnosBackupSelected", { name: file.name }),
  );
}

async function exportBackupToLocal() {
  await runExport(async () => {
    const blob = await MaintenanceAPI.downloadBackup();
    const downloadUrl = URL.createObjectURL(blob);
    const anchor = document.createElement("a");
    anchor.href = downloadUrl;
    anchor.download = buildDownloadFilename();
    document.body.appendChild(anchor);
    anchor.click();
    anchor.remove();
    URL.revokeObjectURL(downloadUrl);
    toast.success(t("admin.maintenanceSettings.backupDownloadStarted"));
  });
}

async function exportBackupToFnos() {
  await runExport(async () => {
    const result = await MaintenanceAPI.exportBackupToFnos();
    if (hasLoadedBackupFiles.value) {
      await loadBackupFiles(true);
    }
    toast.success(t("admin.maintenanceSettings.backupExportedToFnos"), {
      description: t("admin.maintenanceSettings.writtenToPath", {
        path: result.relativePath,
      }),
    });
  });
}

function openImportDialog() {
  if (!hasSelectedBackup.value) {
    toast.error(t("admin.maintenanceSettings.chooseBackupFirst"));
    return;
  }
  isImportDialogOpen.value = true;
}

function buildImportDescription(result: FnKnockBackupImportResult): string {
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
}

async function readFileAsBase64(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
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
        reject(new Error(t("admin.maintenanceSettings.parseBackupFileFailed")));
        return;
      }

      resolve(result.slice(markerIndex + marker.length));
    };

    reader.readAsDataURL(file);
  });
}

async function importBackup() {
  await runImport(
    async () => {
      if (selectedSource.value === "fnos" && selectedFnosFile.value) {
        return MaintenanceAPI.importBackupFromFnos(
          selectedFnosFile.value.relativePath,
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

        if (result.warnings.length > 0) {
          toast.info(t("admin.maintenanceSettings.backupImported"), {
            description: buildImportDescription(result),
          });
        } else {
          toast.success(t("admin.maintenanceSettings.backupImported"), {
            description: buildImportDescription(result),
          });
        }

        if (typeof window !== "undefined") {
          window.setTimeout(() => {
            window.location.reload();
          }, 1200);
        }
      },
    },
  );
}

function openClearDataDialog() {
  if (isClearingData.value) return;
  clearDataConfirmation.value = "";
  isClearDataDialogOpen.value = true;
}

function handleClearDataDialogOpenChange(open: boolean) {
  if (isClearingData.value) return;
  isClearDataDialogOpen.value = open;
  if (!open) {
    clearDataConfirmation.value = "";
  }
}

async function clearAllData() {
  if (!canClearAllData.value) return;

  await runClearData(
    () => MaintenanceAPI.clearAllData(clearDataConfirmation.value),
    {
      onSuccess: () => {
        if (typeof window === "undefined") return;
        window.localStorage.clear();
        window.location.reload();
      },
    },
  );
}

function handleClearDataEnter() {
  if (canClearAllData.value) {
    void clearAllData();
  }
}
</script>

<template>
  <div class="w-full">
    <section class="overflow-hidden rounded-2xl border bg-background">
      <div
        class="flex flex-col gap-2 border-b px-6 py-5 sm:flex-row sm:items-end sm:justify-between sm:px-8"
      >
        <div>
          <h2 class="text-xl font-semibold tracking-tight">
            {{ t("admin.maintenanceSettings.title") }}
          </h2>
          <p class="mt-1 text-sm text-muted-foreground">
            {{ t("admin.maintenanceSettings.description") }}
          </p>
        </div>
        <p class="max-w-md text-xs leading-5 text-muted-foreground">
          {{ t("admin.maintenanceSettings.importWarning") }}
        </p>
      </div>

      <div class="divide-y">
        <div
          class="flex flex-col gap-4 px-6 py-6 sm:px-8 lg:flex-row lg:items-center lg:justify-between"
        >
          <div class="space-y-1">
            <div class="flex items-center gap-2 text-sm font-medium">
              <Download class="h-4 w-4" />
              <span>{{ t("admin.maintenanceSettings.exportBackup") }}</span>
            </div>
            <p class="text-sm text-muted-foreground">
              {{ t("admin.maintenanceSettings.exportHintBefore") }}
              <code>{{ KNOCK_BACKUP_EXTENSION }}</code>
              {{ t("admin.maintenanceSettings.exportHintAfter") }}
            </p>
          </div>

          <DropdownMenu v-if="supportsSharedBackup">
            <DropdownMenuTrigger as-child>
              <Button
                variant="default"
                size="default"
                class="min-w-[168px]"
                :disabled="isBusy"
              >
                <Download class="mr-2 h-4 w-4" />
                {{
                  isExporting
                    ? t("admin.maintenanceSettings.exporting")
                    : t("admin.maintenanceSettings.exportBackup")
                }}
                <ChevronDown class="ml-2 h-4 w-4" />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end">
              <DropdownMenuItem :disabled="isBusy" @select="exportBackupToFnos">
                <FolderTree class="mr-2 h-4 w-4" />
                {{ t("admin.maintenanceSettings.exportToFnos") }}
              </DropdownMenuItem>
              <DropdownMenuItem
                :disabled="isBusy"
                @select="exportBackupToLocal"
              >
                <Laptop class="mr-2 h-4 w-4" />
                {{ t("admin.maintenanceSettings.exportToLocal") }}
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
          <Button
            v-else
            variant="default"
            size="default"
            class="min-w-[168px]"
            :disabled="isBusy"
            @click="exportBackupToLocal"
          >
            <Download class="mr-2 h-4 w-4" />
            {{
              isExporting
                ? t("admin.maintenanceSettings.exporting")
                : t("admin.maintenanceSettings.downloadBackup")
            }}
          </Button>
        </div>

        <div class="px-6 py-6 sm:px-8">
          <input
            ref="fileInputRef"
            type="file"
            accept=".knock,application/octet-stream,application/zip"
            class="hidden"
            @change="handleFileChange"
          />

          <div class="flex flex-col gap-4">
            <div
              class="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between"
            >
              <div class="min-w-0 flex-1 space-y-1">
                <div class="flex items-center gap-2 text-sm font-medium">
                  <Upload class="h-4 w-4" />
                  <span>{{ t("admin.maintenanceSettings.importBackup") }}</span>
                </div>
                <p class="text-sm text-muted-foreground">
                  {{ t("admin.maintenanceSettings.importDescription") }}
                </p>
                <p class="text-xs leading-5 text-muted-foreground">
                  <template v-if="supportsSharedBackup">
                    {{ t("admin.maintenanceSettings.sharedImportHintBefore") }}
                    <code>backup</code>
                    {{ t("admin.maintenanceSettings.sharedImportHintBetween") }}
                    <code>{{ KNOCK_BACKUP_EXTENSION }}</code>
                    {{ t("admin.maintenanceSettings.sharedImportHintAfter") }}
                  </template>
                  <template v-else>
                    {{ t(localImportHintBeforeKey) }}
                    <code>{{ KNOCK_BACKUP_EXTENSION }}</code>
                    {{ t(localImportHintAfterKey) }}
                  </template>
                </p>
              </div>

              <div class="flex flex-wrap gap-3 lg:justify-end">
                <DropdownMenu v-if="supportsSharedBackup">
                  <DropdownMenuTrigger as-child>
                    <Button variant="outline" :disabled="isBusy">
                      <Upload class="mr-2 h-4 w-4" />
                      {{
                        selectedSummary
                          ? t("admin.maintenanceSettings.reselectSource")
                          : t("admin.maintenanceSettings.importBackup")
                      }}
                      <ChevronDown class="ml-2 h-4 w-4" />
                    </Button>
                  </DropdownMenuTrigger>
                  <DropdownMenuContent align="end">
                    <DropdownMenuItem
                      :disabled="isBusy"
                      @select="openFnosBackupPicker"
                    >
                      <FolderTree class="mr-2 h-4 w-4" />
                      {{ t("admin.maintenanceSettings.importFromFnos") }}
                    </DropdownMenuItem>
                    <DropdownMenuItem
                      :disabled="isBusy"
                      @select="triggerLocalFilePicker"
                    >
                      <Laptop class="mr-2 h-4 w-4" />
                      {{ t("admin.maintenanceSettings.chooseFromLocal") }}
                    </DropdownMenuItem>
                  </DropdownMenuContent>
                </DropdownMenu>
                <Button
                  v-else
                  variant="outline"
                  :disabled="isBusy"
                  @click="triggerLocalFilePicker"
                >
                  <Upload class="mr-2 h-4 w-4" />
                  {{
                    selectedSummary
                      ? t("admin.maintenanceSettings.reselectFile")
                      : t("admin.maintenanceSettings.chooseBackupFile")
                  }}
                </Button>
                <Button
                  variant="default"
                  size="default"
                  class="min-w-[168px]"
                  :disabled="!hasSelectedBackup || isBusy"
                  @click="openImportDialog"
                >
                  <Upload class="mr-2 h-4 w-4" />
                  {{
                    isImporting
                      ? t("admin.maintenanceSettings.importing")
                      : t("admin.maintenanceSettings.startImport")
                  }}
                </Button>
              </div>
            </div>

            <div
              class="w-full rounded-xl border bg-muted/[0.12] px-4 py-3 text-sm"
            >
              <div class="space-y-1">
                <div
                  v-if="selectedSummary"
                  class="flex flex-wrap items-center gap-x-3 gap-y-1"
                >
                  <span class="min-w-0 truncate font-medium text-foreground">
                    {{ selectedSummary.name }}
                  </span>
                  <span class="text-muted-foreground">
                    {{ selectedSummary.size }}
                  </span>
                  <span class="text-muted-foreground">
                    {{ selectedSummary.sourceLabel }}
                  </span>
                </div>
                <p
                  v-if="selectedSummary?.location"
                  class="break-all text-xs text-muted-foreground"
                >
                  {{ selectedSummary.location }}
                </p>
                <p v-else class="w-full text-muted-foreground">
                  {{ t("admin.maintenanceSettings.noBackupSelected") }}
                </p>
              </div>
            </div>
          </div>
        </div>
      </div>
    </section>

    <section class="mt-6 overflow-hidden rounded-2xl border bg-background">
      <div class="border-b px-6 py-5 sm:px-8">
        <h2 class="text-xl font-semibold tracking-tight">
          {{ t("admin.maintenanceSettings.dangerZoneTitle") }}
        </h2>
        <p class="mt-1 text-sm text-muted-foreground">
          {{ t("admin.maintenanceSettings.dangerZoneDescription") }}
        </p>
      </div>

      <div
        class="flex flex-col gap-4 px-6 py-5 sm:px-8 lg:flex-row lg:items-center lg:justify-between"
      >
        <div class="space-y-1">
          <p class="text-sm font-medium">
            {{ t("admin.maintenanceSettings.clearAllDataTitle") }}
          </p>
          <p class="max-w-3xl text-sm leading-6 text-muted-foreground">
            {{ t("admin.maintenanceSettings.clearAllDataDescription") }}
          </p>
        </div>

        <Button
          variant="outline"
          class="shrink-0 border-destructive/40 text-destructive hover:bg-destructive/5 hover:text-destructive focus-visible:ring-destructive/20 lg:min-w-[168px]"
          :disabled="isClearingData"
          @click="openClearDataDialog"
        >
          <Trash2 class="mr-2 h-4 w-4" />
          {{ t("admin.maintenanceSettings.clearAllDataAction") }}
        </Button>
      </div>
    </section>

    <DataShareFilePicker
      v-if="supportsSharedBackup"
      v-model:open="isBackupPickerOpen"
      :title="t('admin.maintenanceSettings.pickerTitle')"
      :description="t('admin.maintenanceSettings.pickerDescription')"
      :directory-label="t('admin.maintenanceSettings.pickerDirectoryLabel')"
      :share-name="backupFiles.shareName"
      :files="backupFiles.files"
      :supported-file-types="[KNOCK_BACKUP_EXTENSION]"
      :available="backupFiles.available"
      :loading="isLoadingBackupFiles"
      :selecting="isImporting"
      :error-message="backupFilesError"
      :alert-title="t('admin.maintenanceSettings.pickerAlertTitle')"
      :available-description="
        t('admin.maintenanceSettings.pickerAvailableDescription')
      "
      :unavailable-description="
        t('admin.maintenanceSettings.pickerUnavailableDescription')
      "
      :empty-title="t('admin.maintenanceSettings.pickerEmptyTitle')"
      :empty-description="t('admin.maintenanceSettings.pickerEmptyDescription')"
      :confirm-text="t('admin.maintenanceSettings.pickerConfirmText')"
      @refresh="refreshBackupFiles"
      @select="handleFnosFileSelect"
    />

    <Dialog
      :open="isImportDialogOpen"
      @update:open="isImportDialogOpen = $event"
    >
      <DialogContent class="sm:max-w-[420px]">
        <DialogHeader class="space-y-2">
          <DialogTitle class="text-left">{{
            t("admin.maintenanceSettings.confirmImportTitle")
          }}</DialogTitle>
          <DialogDescription class="text-left text-sm leading-6">
            {{ t("admin.maintenanceSettings.confirmImportDescription") }}
          </DialogDescription>
        </DialogHeader>

        <div
          v-if="selectedSummary"
          class="rounded-xl border border-destructive/25 bg-destructive/5 px-4 py-3 text-sm"
        >
          <p class="font-medium text-foreground">{{ selectedSummary.name }}</p>
          <p class="mt-1 text-muted-foreground">
            {{ selectedSummary.size }} · {{ selectedSummary.sourceLabel }}
          </p>
          <p
            v-if="selectedSummary.location"
            class="mt-1 break-all text-xs text-muted-foreground"
          >
            {{ selectedSummary.location }}
          </p>
        </div>

        <DialogFooter class="gap-2">
          <Button
            variant="outline"
            :disabled="isImporting"
            @click="isImportDialogOpen = false"
          >
            {{ t("common.cancel") }}
          </Button>
          <Button
            variant="destructive"
            :disabled="isImporting || !hasSelectedBackup"
            @click="importBackup"
          >
            {{
              isImporting
                ? t("admin.maintenanceSettings.importingNow")
                : t("admin.maintenanceSettings.confirmImport")
            }}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>

    <Dialog
      :open="isClearDataDialogOpen"
      @update:open="handleClearDataDialogOpenChange"
    >
      <DialogContent
        class="sm:max-w-[420px]"
        :show-close-button="!isClearingData"
      >
        <DialogHeader>
          <DialogTitle class="text-left">
            {{ t("admin.maintenanceSettings.clearAllDataDialogTitle") }}
          </DialogTitle>
          <DialogDescription class="text-left text-sm leading-6">
            {{ t("admin.maintenanceSettings.clearAllDataDialogDescription") }}
          </DialogDescription>
        </DialogHeader>

        <p class="text-sm leading-6 text-destructive">
          {{ t("admin.maintenanceSettings.clearAllDataWarning") }}
        </p>

        <div class="space-y-2">
          <label for="clear-all-data-confirmation" class="text-sm font-medium">
            {{
              t("admin.maintenanceSettings.clearAllDataTypePrompt", {
                phrase: expectedClearDataConfirmation,
              })
            }}
          </label>
          <Input
            id="clear-all-data-confirmation"
            v-model="clearDataConfirmation"
            :placeholder="expectedClearDataConfirmation"
            :disabled="isClearingData"
            :aria-invalid="
              clearDataConfirmation.length > 0 &&
              clearDataConfirmation !== expectedClearDataConfirmation
                ? 'true'
                : undefined
            "
            @keydown.enter="handleClearDataEnter"
          />
        </div>

        <DialogFooter class="mt-1 gap-2">
          <Button
            variant="outline"
            :disabled="isClearingData"
            @click="handleClearDataDialogOpenChange(false)"
          >
            {{ t("common.cancel") }}
          </Button>
          <Button
            variant="destructive"
            :disabled="!canClearAllData"
            @click="clearAllData"
          >
            <Loader2 v-if="isClearingData" class="mr-2 h-4 w-4 animate-spin" />
            {{
              isClearingData
                ? t("admin.maintenanceSettings.clearingAllData")
                : t("admin.maintenanceSettings.confirmClearAllData")
            }}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </div>
</template>
