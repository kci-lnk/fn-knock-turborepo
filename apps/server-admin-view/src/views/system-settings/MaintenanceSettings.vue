<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
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
import { KNOCK_BACKUP_EXTENSION } from "@admin-shared/utils/maintenanceBackup";
import { useMaintenanceBackupWorkflow } from "./useMaintenanceBackupWorkflow";
import { useMaintenanceClearData } from "./useMaintenanceClearData";
import AutomaticBackupPicker from "./AutomaticBackupPicker.vue";
import AutomaticBackupSettings from "./AutomaticBackupSettings.vue";
import FnosBackupPicker from "./FnosBackupPicker.vue";

const { t } = useI18n();
const {
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
} = useMaintenanceBackupWorkflow();
const {
  canClearAllData,
  clearAllData,
  clearDataConfirmation,
  expectedClearDataConfirmation,
  handleClearDataDialogOpenChange,
  handleClearDataEnter,
  isClearDataDialogOpen,
  isClearingData,
  openClearDataDialog,
} = useMaintenanceClearData();

// Vue assigns this string template ref at runtime.
void fileInputRef;
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
                <DropdownMenu v-if="hasMultipleBackupSources">
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
                      v-if="hasAutomaticBackups"
                      :disabled="isBusy"
                      @select="openAutomaticBackupPicker"
                    >
                      <FolderTree class="mr-2 h-4 w-4" />
                      {{ t("admin.maintenanceSettings.importFromAutomatic") }}
                    </DropdownMenuItem>
                    <DropdownMenuItem
                      v-if="supportsSharedBackup"
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
      <AutomaticBackupSettings @files-changed="refreshAutomaticBackupFiles" />
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

    <FnosBackupPicker
      v-if="supportsSharedBackup"
      v-model:open="isBackupPickerOpen"
      :files="backupFiles"
      :loading="isLoadingBackupFiles"
      :selecting="isImporting"
      :error-message="backupFilesError"
      @refresh="refreshBackupFiles"
      @select="handleFnosFileSelect"
    />

    <AutomaticBackupPicker
      v-model:open="isAutomaticBackupPickerOpen"
      :files="automaticBackupFiles"
      :loading="isLoadingAutomaticBackupFiles"
      :selecting="isImporting"
      :error-message="automaticBackupFilesError"
      @refresh="refreshAutomaticBackupFiles"
      @select="handleAutomaticFileSelect"
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
