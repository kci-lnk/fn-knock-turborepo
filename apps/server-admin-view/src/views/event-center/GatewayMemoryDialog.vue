<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { Loader2, RefreshCw } from "lucide-vue-next";
import { toast } from "vue-sonner";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { RuntimeHealthAPI } from "@/lib/api";

const props = defineProps<{ open: boolean }>();
const emit = defineEmits<{
  "update:open": [value: boolean];
  updated: [];
}>();

const { t } = useI18n();
const loading = ref(false);
const saving = ref(false);
const reclaiming = ref(false);
const loaded = ref(false);
const loadFailed = ref(false);
const draft = ref("100");
let requestId = 0;

const MIN_GC_PERCENT = 25;
const MAX_GC_PERCENT = 500;

const options = computed(() =>
  [50, 100, 200].map((value) => ({
    value: String(value),
    label: t(`admin.eventCenter.runtime.memory.levels.${value}.label`),
    description: t(
      `admin.eventCenter.runtime.memory.levels.${value}.description`,
    ),
  })),
);
const selected = computed(
  () => options.value.find((option) => option.value === draft.value) ?? null,
);
const draftPercent = computed(() => Number(draft.value));
const validDraft = computed(
  () =>
    Number.isInteger(draftPercent.value) &&
    draftPercent.value >= MIN_GC_PERCENT &&
    draftPercent.value <= MAX_GC_PERCENT,
);
const actionBusy = computed(
  () => loading.value || saving.value || reclaiming.value,
);

const formatBytes = (bytes: number) => {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KiB`;
  return `${(bytes / 1024 / 1024).toFixed(2)} MiB`;
};

const loadConfig = async () => {
  const currentRequest = ++requestId;
  loading.value = true;
  loaded.value = false;
  loadFailed.value = false;
  try {
    const result = await RuntimeHealthAPI.getGatewayMemoryConfig();
    if (currentRequest !== requestId) return;
    draft.value = String(result.data.gc_percent);
    loaded.value = true;
  } catch (error) {
    if (currentRequest !== requestId) return;
    loadFailed.value = true;
    toast.error(t("admin.eventCenter.runtime.memory.loadFailed"), {
      description: error instanceof Error ? error.message : String(error),
    });
  } finally {
    if (currentRequest === requestId) loading.value = false;
  }
};

const save = async () => {
  if (!loaded.value || !validDraft.value || actionBusy.value) return;
  saving.value = true;
  try {
    const result = await RuntimeHealthAPI.updateGatewayMemoryConfig({
      gc_percent: draftPercent.value,
    });
    draft.value = String(result.data.gc_percent);
    toast.success(t("admin.eventCenter.runtime.memory.saveSuccess"));
    emit("updated");
    emit("update:open", false);
  } catch (error) {
    toast.error(t("admin.eventCenter.runtime.memory.saveFailed"), {
      description: error instanceof Error ? error.message : String(error),
    });
  } finally {
    saving.value = false;
  }
};

const reclaim = async () => {
  if (actionBusy.value) return;
  reclaiming.value = true;
  try {
    const result = await RuntimeHealthAPI.reclaimGatewayMemory();
    toast.success(t("admin.eventCenter.runtime.memory.reclaimSuccess"), {
      description: t(
        "admin.eventCenter.runtime.memory.reclaimSuccessDescription",
        {
          heap: formatBytes(result.data.heap_alloc_bytes),
          rss: formatBytes(result.data.rss_bytes),
        },
      ),
    });
    emit("updated");
  } catch (error) {
    toast.error(t("admin.eventCenter.runtime.memory.reclaimFailed"), {
      description: error instanceof Error ? error.message : String(error),
    });
  } finally {
    reclaiming.value = false;
  }
};

const handleOpenChange = (value: boolean) => {
  if (!value && (saving.value || reclaiming.value)) return;
  emit("update:open", value);
};

watch(
  () => props.open,
  (open) => {
    if (open) {
      draft.value = "100";
      loaded.value = false;
      loadFailed.value = false;
      void loadConfig();
    } else {
      ++requestId;
    }
  },
  { immediate: true },
);
</script>

<template>
  <Dialog :open="open" @update:open="handleOpenChange">
    <DialogContent class="sm:max-w-md">
      <DialogHeader>
        <DialogTitle>{{
          t("admin.eventCenter.runtime.memory.title")
        }}</DialogTitle>
        <DialogDescription>
          {{ t("admin.eventCenter.runtime.memory.description") }}
        </DialogDescription>
      </DialogHeader>

      <div class="grid gap-3 py-2">
        <p class="text-sm font-medium">
          {{ t("admin.eventCenter.runtime.memory.strength") }}
        </p>
        <div class="grid grid-cols-3 gap-2">
          <Button
            v-for="option in options"
            :key="option.value"
            type="button"
            size="sm"
            :variant="draft === option.value ? 'default' : 'outline'"
            :disabled="!loaded || actionBusy"
            :aria-pressed="draft === option.value"
            @click="draft = option.value"
          >
            {{ option.label }}
          </Button>
        </div>
        <div class="grid gap-2">
          <Label for="gateway-memory-strength">GOGC</Label>
          <div class="flex items-center gap-2">
            <Loader2 v-if="loading" class="h-4 w-4 shrink-0 animate-spin" />
            <Input
              id="gateway-memory-strength"
              v-model="draft"
              type="number"
              inputmode="numeric"
              :min="MIN_GC_PERCENT"
              :max="MAX_GC_PERCENT"
              step="1"
              class="tabular-nums"
              :disabled="!loaded || actionBusy"
              :aria-invalid="!validDraft"
              aria-describedby="gateway-memory-strength-description"
            />
            <span class="text-sm text-muted-foreground">%</span>
          </div>
        </div>
        <p
          id="gateway-memory-strength-description"
          class="text-xs leading-5"
          :class="validDraft ? 'text-muted-foreground' : 'text-destructive'"
        >
          <template v-if="validDraft">
            {{
              selected?.description ??
              t("admin.eventCenter.runtime.memory.customDescription")
            }}
          </template>
          <template v-else>
            {{ t("admin.eventCenter.runtime.memory.rangeError") }}
          </template>
        </p>
        <div
          v-if="loadFailed"
          role="alert"
          class="flex items-center justify-between gap-3 rounded-md border border-destructive/30 bg-destructive/5 px-3 py-2 text-xs text-destructive"
        >
          <span>{{ t("admin.eventCenter.runtime.memory.loadFailed") }}</span>
          <Button type="button" variant="outline" size="sm" @click="loadConfig">
            {{ t("admin.eventCenter.runtime.memory.retry") }}
          </Button>
        </div>
        <p
          class="rounded-md bg-muted/50 px-3 py-2 text-xs text-muted-foreground"
        >
          {{ t("admin.eventCenter.runtime.memory.hint") }}
        </p>
      </div>

      <DialogFooter class="gap-2 sm:justify-between">
        <Button variant="outline" :disabled="actionBusy" @click="reclaim">
          <Loader2 v-if="reclaiming" class="h-4 w-4 animate-spin" />
          <RefreshCw v-else class="h-4 w-4" />
          {{ t("admin.eventCenter.runtime.memory.reclaim") }}
        </Button>
        <div class="flex flex-col-reverse gap-2 sm:flex-row">
          <Button
            variant="outline"
            :disabled="saving || reclaiming"
            @click="handleOpenChange(false)"
          >
            {{ t("common.cancel") }}
          </Button>
          <Button
            :disabled="!loaded || !validDraft || actionBusy"
            @click="save"
          >
            <Loader2 v-if="saving" class="h-4 w-4 animate-spin" />
            {{ t("common.save") }}
          </Button>
        </div>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
