<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { Button } from "@/components/ui/button";
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
  Upload,
} from "lucide-vue-next";
import { KNOCK_BACKUP_EXTENSION } from "@admin-shared/utils/maintenanceBackup";
import type { MaintenanceBackupController } from "./maintenance-settings-contract";

const props = defineProps<{ controller: MaintenanceBackupController }>();
const { t } = useI18n();
const {
  exportBackupToFnos,
  exportBackupToLocal,
  fileInputRef,
  handleFileChange,
  hasAutomaticBackups,
  hasMultipleBackupSources,
  hasSelectedBackup,
  isBusy,
  isExporting,
  isImporting,
  localImportHintAfterKey,
  localImportHintBeforeKey,
  openAutomaticBackupPicker,
  openFnosBackupPicker,
  openImportDialog,
  selectedSummary,
  supportsSharedBackup,
  triggerLocalFilePicker,
} = props.controller;

// Vue assigns this string template ref at runtime.
void fileInputRef;
</script>

<template>
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
</template>
