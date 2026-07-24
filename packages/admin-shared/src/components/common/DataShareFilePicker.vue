<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { cn } from "@/lib/utils";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetFooter,
  SheetHeader,
  SheetTitle,
} from "@/components/ui/sheet";
import {
  CheckCircle2,
  FileText,
  FolderOpen,
  Info,
  RefreshCw,
} from "lucide-vue-next";
import { useMediaQueryMatch } from "@admin-shared/composables/useMediaQueryMatch";

interface SharedDataFileEntry {
  name: string;
  relativePath: string;
  extension: string;
  size: number;
  modifiedAt: string;
}

const props = withDefaults(
  defineProps<{
    open: boolean;
    title?: string;
    description?: string;
    directoryLabel?: string;
    shareName?: string;
    files?: SharedDataFileEntry[];
    supportedFileTypes?: string[];
    available?: boolean;
    loading?: boolean;
    selecting?: boolean;
    errorMessage?: string;
    alertTitle?: string;
    availableDescription?: string;
    unavailableDescription?: string;
    emptyTitle?: string;
    emptyDescription?: string;
    confirmText?: string;
  }>(),
  {
    shareName: "fn-knock",
    files: () => [],
    supportedFileTypes: () => [],
    available: false,
    loading: false,
    selecting: false,
    errorMessage: "",
    emptyTitle: "",
    emptyDescription: "",
  },
);

const { locale, t } = useI18n();

const emit = defineEmits<{
  "update:open": [value: boolean];
  refresh: [];
  select: [file: SharedDataFileEntry];
}>();

const searchQuery = ref("");
const selectedRelativePath = ref("");
const isMobileViewport = useMediaQueryMatch("(max-width: 768px)");
const titleText = computed(
  () => props.title ?? t("shared.dataShareFilePicker.title"),
);
const descriptionText = computed(
  () => props.description ?? t("shared.dataShareFilePicker.description"),
);
const directoryLabelText = computed(
  () => props.directoryLabel ?? t("shared.dataShareFilePicker.directoryLabel"),
);
const alertTitleText = computed(
  () => props.alertTitle ?? t("shared.dataShareFilePicker.alertTitle"),
);
const unavailableDescriptionText = computed(
  () =>
    props.unavailableDescription ??
    t("shared.dataShareFilePicker.unavailableDescription"),
);
const confirmTextLabel = computed(
  () => props.confirmText ?? t("shared.dataShareFilePicker.confirmText"),
);

const normalizedSupportedTypes = computed(() =>
  props.supportedFileTypes
    .map((value) => normalizeExtension(value))
    .filter(Boolean),
);

const selectableFiles = computed(() =>
  props.files.filter((file) => isSelectable(file)),
);

const filteredFiles = computed(() => {
  const keyword = searchQuery.value.trim().toLowerCase();
  if (!keyword) {
    return selectableFiles.value;
  }

  return selectableFiles.value.filter((file) =>
    `${file.name} ${file.relativePath}`.toLowerCase().includes(keyword),
  );
});

const selectedFile = computed(
  () =>
    filteredFiles.value.find(
      (file) => file.relativePath === selectedRelativePath.value,
    ) ?? null,
);
const directoryStatusDescription = computed(() => {
  if (props.available) {
    return (
      props.availableDescription ||
      t("shared.dataShareFilePicker.availableDescription", {
        count: selectableFiles.value.length,
      })
    );
  }

  return unavailableDescriptionText.value;
});

watch(
  () => props.open,
  (open) => {
    if (!open) {
      searchQuery.value = "";
      selectedRelativePath.value = "";
      return;
    }

    selectFirstAvailableFile();
  },
);

watch(filteredFiles, () => {
  if (!props.open) {
    return;
  }

  if (!selectedFile.value) {
    selectFirstAvailableFile();
  }
});

function normalizeExtension(value: string) {
  const normalized = value.trim().toLowerCase();
  if (!normalized) {
    return "";
  }

  return normalized.startsWith(".") ? normalized : `.${normalized}`;
}

function isSelectable(file: SharedDataFileEntry) {
  if (!normalizedSupportedTypes.value.length) {
    return true;
  }

  return normalizedSupportedTypes.value.includes(
    normalizeExtension(file.extension || file.name),
  );
}

function selectFirstAvailableFile() {
  selectedRelativePath.value = filteredFiles.value[0]?.relativePath ?? "";
}

function formatFileSize(size: number) {
  if (size < 1024) {
    return `${size} B`;
  }
  if (size < 1024 * 1024) {
    return `${(size / 1024).toFixed(size >= 10 * 1024 ? 0 : 1)} KB`;
  }
  return `${(size / (1024 * 1024)).toFixed(1)} MB`;
}

function formatDateTime(dateValue: string) {
  const date = new Date(dateValue);
  if (Number.isNaN(date.getTime())) {
    return dateValue;
  }

  return date.toLocaleString(locale.value, {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function confirmSelection() {
  if (!selectedFile.value) {
    return;
  }

  emit("select", selectedFile.value);
}

function setOpen(value: boolean) {
  emit("update:open", value);
}
</script>

<template>
  <component
    :is="isMobileViewport ? Sheet : Dialog"
    :open="open"
    @update:open="setOpen"
  >
    <component
      :is="isMobileViewport ? SheetContent : DialogContent"
      v-bind="isMobileViewport ? { side: 'bottom' } : {}"
      :class="
        isMobileViewport
          ? 'flex max-h-[88vh] flex-col gap-0 rounded-t-[10px] border-x-0 border-b-0 bg-background/98 px-0 pb-0'
          : 'border-border/60 bg-background/98 sm:max-w-[700px]'
      "
    >
      <component
        :is="isMobileViewport ? SheetHeader : DialogHeader"
        class="gap-2 px-6 pb-0 pt-6"
      >
        <component :is="isMobileViewport ? SheetTitle : DialogTitle">{{
          titleText
        }}</component>
        <component
          :is="isMobileViewport ? SheetDescription : DialogDescription"
        >
          {{ descriptionText }}
        </component>
      </component>

      <div class="grid min-h-0 flex-1 gap-4 px-6">
        <Alert v-if="errorMessage" variant="destructive" class="rounded-[10px]">
          <Info />
          <AlertTitle>{{ alertTitleText }}</AlertTitle>
          <AlertDescription>{{ errorMessage }}</AlertDescription>
        </Alert>

        <div
          class="flex mt-3 flex-wrap items-center justify-between gap-3 rounded-[10px] border border-border/60 bg-muted/10 px-4 py-3"
        >
          <div class="min-w-0">
            <div class="flex items-center gap-2">
              <FolderOpen class="h-4 w-4 text-muted-foreground" />
              <p class="truncate text-sm font-medium">
                {{ directoryLabelText }}
              </p>
            </div>
            <p class="mt-1 text-xs leading-5 text-muted-foreground">
              {{ directoryStatusDescription }}
            </p>
          </div>
          <Button
            type="button"
            variant="outline"
            size="sm"
            class="rounded-full border-border/70 bg-background/80 shadow-none"
            :disabled="loading || selecting"
            @click="emit('refresh')"
          >
            <RefreshCw :class="cn('mr-2 h-4 w-4', loading && 'animate-spin')" />
            {{ t("common.refreshStatus") }}
          </Button>
        </div>

        <div
          class="min-h-0 rounded-[10px] border border-border/60 bg-background/85"
        >
          <div v-if="loading" class="grid gap-3 p-4">
            <div
              v-for="index in 4"
              :key="index"
              class="h-[82px] animate-pulse rounded-[22px] border border-border/50 bg-muted/20"
            />
          </div>

          <div
            v-else-if="!filteredFiles.length"
            class="grid min-h-[220px] place-items-center px-6 py-10 text-center"
          >
            <div class="grid max-w-sm gap-2">
              <div
                class="mx-auto flex h-11 w-11 items-center justify-center rounded-full border border-border/60 bg-muted/15"
              >
                <FileText class="h-5 w-5 text-muted-foreground" />
              </div>
              <p class="text-sm font-medium">
                {{
                  selectableFiles.length
                    ? t("shared.dataShareFilePicker.noMatchedFiles")
                    : emptyTitle || t("shared.dataShareFilePicker.emptyTitle")
                }}
              </p>
              <p class="text-sm leading-6 text-muted-foreground">
                {{
                  selectableFiles.length
                    ? t("shared.dataShareFilePicker.noMatchedDescription")
                    : emptyDescription ||
                      t("shared.dataShareFilePicker.emptyDescription")
                }}
              </p>
            </div>
          </div>

          <div v-else class="grid max-h-[38vh] gap-3 overflow-y-auto p-4">
            <button
              v-for="file in filteredFiles"
              :key="file.relativePath"
              type="button"
              :disabled="selecting"
              :class="
                cn(
                  'w-full rounded-[22px] border px-4 py-3 text-left transition-colors',
                  selectedRelativePath === file.relativePath
                    ? 'border-foreground/15 bg-muted/20'
                    : 'border-border/60 bg-background hover:bg-muted/10',
                )
              "
              @click="selectedRelativePath = file.relativePath"
            >
              <div class="flex items-start justify-between gap-3">
                <div class="min-w-0">
                  <div class="flex flex-wrap items-center gap-2">
                    <p class="break-all text-sm font-medium">{{ file.name }}</p>
                    <Badge
                      variant="outline"
                      class="rounded-full text-[10px] uppercase tracking-[0.08em] text-muted-foreground"
                    >
                      {{
                        file.extension ||
                        t("shared.dataShareFilePicker.noExtension")
                      }}
                    </Badge>
                  </div>
                  <p
                    class="mt-1 break-all text-xs leading-5 text-muted-foreground"
                  >
                    {{ file.relativePath }}
                  </p>
                </div>
                <CheckCircle2
                  v-if="selectedRelativePath === file.relativePath"
                  class="mt-0.5 h-4 w-4 shrink-0 text-foreground/80"
                />
              </div>

              <div
                class="mt-3 flex flex-wrap gap-x-3 gap-y-1 text-xs text-muted-foreground"
              >
                <span>{{ formatFileSize(file.size) }}</span>
                <span>{{ formatDateTime(file.modifiedAt) }}</span>
              </div>
            </button>
          </div>
        </div>
      </div>

      <component
        :is="isMobileViewport ? SheetFooter : DialogFooter"
        class="border-t border-border/50 bg-background/95 px-6 py-4"
      >
        <Button
          type="button"
          variant="outline"
          class="rounded-full"
          :disabled="selecting"
          @click="setOpen(false)"
        >
          {{ t("common.cancel") }}
        </Button>
        <Button
          type="button"
          class="rounded-full"
          :disabled="!selectedFile || selecting"
          @click="confirmSelection"
        >
          <RefreshCw v-if="selecting" class="mr-2 h-4 w-4 animate-spin" />
          {{ confirmTextLabel }}
        </Button>
      </component>
    </component>
  </component>
</template>
