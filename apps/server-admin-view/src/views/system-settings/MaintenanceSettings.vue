<script setup lang="ts">
import { computed, ref } from "vue";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Download, Upload } from "lucide-vue-next";
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
import type { FnKnockBackupImportResult } from "../../types";
import { useConfigStore } from "../../store/config";

const configStore = useConfigStore();
const fileInputRef = ref<HTMLInputElement | null>(null);
const selectedFile = ref<File | null>(null);
const isImportDialogOpen = ref(false);

const { isPending: isExporting, run: runExport } = useAsyncAction({
  onError: (error) => {
    toast.error("导出失败", {
      description: extractErrorMessage(error, "无法导出备份文件"),
    });
  },
});

const { isPending: isImporting, run: runImport } = useAsyncAction({
  onError: (error) => {
    toast.error("导入失败", {
      description: extractErrorMessage(error, "无法导入备份文件"),
    });
  },
});

const hasSelectedBackup = computed(() => selectedFile.value !== null);

const selectedSummary = computed(() => {
  if (!selectedFile.value) {
    return null;
  }

  return {
    name: selectedFile.value.name,
    size: formatFileSize(selectedFile.value.size),
  };
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
  selectedFile.value = null;
  if (fileInputRef.value) {
    fileInputRef.value.value = "";
  }
}

function triggerFilePicker() {
  if (isImporting.value) return;
  fileInputRef.value?.click();
}

async function handleFileChange(event: Event) {
  const input = event.target as HTMLInputElement | null;
  const file = input?.files?.[0] ?? null;

  if (!file) {
    resetSelectedBackup();
    return;
  }

  if (!file.name.toLowerCase().endsWith(KNOCK_BACKUP_EXTENSION)) {
    resetSelectedBackup();
    toast.error("备份文件无效", {
      description: `请选择 ${KNOCK_BACKUP_EXTENSION} 备份文件`,
    });
    return;
  }

  selectedFile.value = file;
}

async function exportBackup() {
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
    toast.success("备份文件已开始下载");
  });
}

function openImportDialog() {
  if (!hasSelectedBackup.value) {
    toast.error("请先选择备份文件");
    return;
  }
  isImportDialogOpen.value = true;
}

function buildImportDescription(result: FnKnockBackupImportResult): string {
  if (result.warnings.length === 0) {
    return `已恢复 ${result.imported_keys} 个 Redis 键，并同步 ${result.synced_steps.length} 项运行态。`;
  }

  const preview = result.warnings.slice(0, 2).join("；");
  return result.warnings.length > 2
    ? `${preview}；另有 ${result.warnings.length - 2} 项提示。`
    : preview;
}

async function readFileAsBase64(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();

    reader.onerror = () =>
      reject(reader.error || new Error("读取备份文件失败"));
    reader.onload = () => {
      const result = typeof reader.result === "string" ? reader.result : "";
      const marker = "base64,";
      const markerIndex = result.indexOf(marker);

      if (markerIndex < 0) {
        reject(new Error("无法解析备份文件内容"));
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
      if (!selectedFile.value) {
        throw new Error("请先选择备份文件");
      }

      return MaintenanceAPI.importBackup({
        filename: selectedFile.value.name,
        archive_base64: await readFileAsBase64(selectedFile.value),
      });
    },
    {
      onSuccess: async (result) => {
        isImportDialogOpen.value = false;
        resetSelectedBackup();
        await configStore.loadConfig();

        if (result.warnings.length > 0) {
          toast.info("备份已导入", {
            description: buildImportDescription(result),
          });
        } else {
          toast.success("备份已导入", {
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
</script>

<template>
  <div class="w-full">
    <section class="overflow-hidden rounded-2xl border bg-background">
      <div
        class="flex flex-col gap-2 border-b px-6 py-5 sm:flex-row sm:items-end sm:justify-between sm:px-8"
      >
        <div>
          <h2 class="text-xl font-semibold tracking-tight">维护</h2>
          <p class="mt-1 text-sm text-muted-foreground">导出或恢复系统备份。</p>
        </div>
        <p class="max-w-md text-xs leading-5 text-muted-foreground">
          导入会覆盖当前配置，并在完成后自动刷新页面。
        </p>
      </div>

      <div class="divide-y">
        <div
          class="flex flex-col gap-4 px-6 py-6 sm:px-8 lg:flex-row lg:items-center lg:justify-between"
        >
          <div class="space-y-1">
            <div class="flex items-center gap-2 text-sm font-medium">
              <Download class="h-4 w-4" />
              <span>导出备份</span>
            </div>
            <p class="text-sm text-muted-foreground">
              生成
              <code>{{ KNOCK_BACKUP_EXTENSION }}</code>
              归档，建议在调整配置前保留一份。
            </p>
          </div>

          <Button
            variant="default"
            size="default"
            class="min-w-[168px]"
            :disabled="isExporting || isImporting"
            @click="exportBackup"
          >
            <Download class="mr-2 h-4 w-4" />
            {{ isExporting ? "导出中..." : "导出备份" }}
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
                  <span>导入备份</span>
                </div>
                <p class="text-sm text-muted-foreground">
                  从已有归档恢复系统设置。
                </p>
              </div>

              <div class="flex flex-wrap gap-3 lg:justify-end">
                <Button
                  variant="outline"
                  :disabled="isImporting"
                  @click="triggerFilePicker"
                >
                  选择文件
                </Button>
                <Button
                  variant="default"
                  size="default"
                  class="min-w-[168px]"
                  :disabled="!hasSelectedBackup || isImporting || isExporting"
                  @click="openImportDialog"
                >
                  <Upload class="mr-2 h-4 w-4" />
                  {{ isImporting ? "导入中..." : "开始导入" }}
                </Button>
              </div>
            </div>

            <div class="w-full rounded-xl border bg-muted/[0.12] px-4 py-3 text-sm">
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
                    {{ KNOCK_BACKUP_EXTENSION }}
                  </span>
                </div>
                <p v-else class="w-full text-muted-foreground">
                  未选择备份文件
                </p>
              </div>
            </div>
          </div>
        </div>
      </div>
    </section>

    <Dialog
      :open="isImportDialogOpen"
      @update:open="isImportDialogOpen = $event"
    >
      <DialogContent class="sm:max-w-[420px]">
        <DialogHeader class="space-y-2">
          <DialogTitle class="text-left">确认导入</DialogTitle>
          <DialogDescription class="text-left text-sm leading-6">
            这会先清空当前配置，再用所选备份文件覆盖恢复。
          </DialogDescription>
        </DialogHeader>

        <div
          v-if="selectedSummary"
          class="rounded-xl border border-destructive/25 bg-destructive/5 px-4 py-3 text-sm"
        >
          <p class="font-medium text-foreground">{{ selectedSummary.name }}</p>
          <p class="mt-1 text-muted-foreground">
            {{ selectedSummary.size }} · {{ KNOCK_BACKUP_EXTENSION }}
          </p>
        </div>

        <DialogFooter class="gap-2">
          <Button
            variant="outline"
            :disabled="isImporting"
            @click="isImportDialogOpen = false"
          >
            取消
          </Button>
          <Button
            variant="destructive"
            :disabled="isImporting || !hasSelectedBackup"
            @click="importBackup"
          >
            {{ isImporting ? "正在导入..." : "确认导入" }}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </div>
</template>
