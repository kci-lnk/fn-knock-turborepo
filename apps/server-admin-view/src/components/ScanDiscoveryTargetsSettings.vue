<template>
  <div class="rounded-md border bg-muted/20 p-3">
    <div
      class="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between"
    >
      <div class="space-y-1">
        <p class="text-sm font-medium">{{ t("admin.scanTargets.title") }}</p>
        <p class="text-xs text-muted-foreground">
          {{
            t("admin.scanTargets.description", {
              maxCidrs: limits.maxCidrs,
              maxHosts: limits.maxHosts,
            })
          }}
        </p>
      </div>
      <div class="flex flex-wrap gap-2">
        <Button
          variant="outline"
          size="sm"
          :aria-label="t('common.refreshStatus')"
          :disabled="isLoading || isSaving"
          @click="loadTargets(true)"
        >
          <RefreshCw
            class="mr-2 h-4 w-4"
            :class="{ 'animate-spin': isLoading }"
          />
        </Button>
        <Button
          variant="outline"
          size="sm"
          :disabled="isLoading || isSaving || automaticTargets.length === 0"
          @click="resetToAutomatic"
        >
          <RotateCcw class="mr-2 h-4 w-4" />
          {{ t("admin.scanTargets.resetAutomatic") }}
        </Button>
        <Button
          size="sm"
          :disabled="
            isLoading || isSaving || !isDirty || selectedCidrs.length === 0
          "
          @click="saveTargets()"
        >
          <Save class="mr-2 h-4 w-4" :class="{ 'animate-pulse': isSaving }" />
          {{ t("common.save") }}
        </Button>
      </div>
    </div>

    <div class="mt-3 flex gap-2">
      <Input
        :aria-label="t('admin.scanTargets.placeholder')"
        v-model="customInput"
        :disabled="isLoading || isSaving"
        :placeholder="t('admin.scanTargets.placeholder')"
        @keyup.enter="addCustomCidrs"
      />
      <Button
        variant="outline"
        :disabled="isLoading || isSaving || !customInput.trim()"
        @click="addCustomCidrs"
      >
        <Plus class="mr-2 h-4 w-4" />
        {{ t("admin.scanTargets.add") }}
      </Button>
    </div>

    <div
      v-if="isLoading"
      class="py-6 text-center text-sm text-muted-foreground"
    >
      {{ t("admin.scanTargets.loading") }}
    </div>

    <div
      v-else-if="allTargets.length === 0"
      class="py-6 text-center text-sm text-muted-foreground"
    >
      {{ t("admin.scanTargets.empty") }}
    </div>

    <div v-else class="mt-3 max-h-56 space-y-2 overflow-auto pr-1">
      <div
        v-for="target in allTargets"
        :key="target.cidr"
        class="flex items-start gap-3 rounded-md border bg-background p-3 transition-colors hover:bg-muted/40"
      >
        <input
          :id="`scan-target-${target.cidr}`"
          type="checkbox"
          class="mt-0.5 h-4 w-4 cursor-pointer"
          :checked="selectedSet.has(target.cidr)"
          :disabled="isLoading || isSaving"
          @change="toggleCidr(target.cidr, $event)"
        />
        <label
          :for="`scan-target-${target.cidr}`"
          class="min-w-0 flex-1 cursor-pointer space-y-1"
        >
          <div class="flex flex-wrap items-center gap-2">
            <span class="font-mono text-sm">{{ target.cidr }}</span>
            <Badge variant="secondary">{{
              getSourceLabel(target.source)
            }}</Badge>
            <Badge v-if="target.isAutomatic" variant="outline">
              {{ t("admin.scanTargets.automatic") }}
            </Badge>
          </div>
          <p class="truncate text-xs text-muted-foreground">
            {{ target.label }}
          </p>
        </label>
        <div class="flex shrink-0 items-center gap-2">
          <span class="text-xs text-muted-foreground">
            {{
              target.hostCount > 0
                ? t("admin.scanTargets.hostCount", {
                    count: target.hostCount,
                  })
                : t("admin.scanTargets.pendingSave")
            }}
          </span>
          <Button
            v-if="customCidrs.includes(target.cidr)"
            type="button"
            variant="ghost"
            size="icon"
            :aria-label="t('common.confirmDelete')"
            class="h-7 w-7"
            :disabled="isLoading || isSaving"
            @click.prevent="removeCustomCidr(target.cidr)"
          >
            <Trash2 class="h-4 w-4" />
          </Button>
        </div>
      </div>
    </div>

    <div
      class="mt-3 flex flex-col gap-1 text-xs text-muted-foreground sm:flex-row sm:items-center sm:justify-between"
    >
      <span>
        {{
          t("admin.scanTargets.selectedCidrs", {
            count: selectedCidrs.length,
          })
        }}
        <template v-if="selectedHostCount !== null">
          {{
            t("admin.scanTargets.selectedHosts", {
              count: selectedHostCount,
            })
          }}
        </template>
      </span>
      <span v-if="isDirty" class="text-amber-600">
        {{ t("admin.scanTargets.dirty") }}
      </span>
      <span v-else-if="isAutomaticSelection">
        {{ t("admin.scanTargets.automaticSelection") }}
      </span>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Plus, RefreshCw, RotateCcw, Save, Trash2 } from "lucide-vue-next";
import { toast } from "@admin-shared/utils/toast";
import { isValidCIDR } from "@admin-shared/utils/cidr";
import {
  ScanAPI,
  type ScanDiscoveryTarget,
  type ScanDiscoveryTargetSource,
  type ScanDiscoveryTargetsResponse,
} from "../lib/api";

const { t } = useI18n();

const targets = ref<ScanDiscoveryTargetsResponse | null>(null);
const selectedCidrs = ref<string[]>([]);
const customCidrs = ref<string[]>([]);
const customInput = ref("");
const isLoading = ref(false);
const isSaving = ref(false);
const savedSignature = ref("");
const isAutomaticSelection = ref(false);

const limits = computed(
  () =>
    targets.value?.limits || {
      maxCidrs: 16,
      maxHosts: 1024,
    },
);

const normalizeCidrs = (values: Iterable<string>): string[] => {
  const result: string[] = [];
  const seen = new Set<string>();
  for (const value of values) {
    const normalized = value.trim();
    if (!normalized) continue;
    const key = normalized.toLowerCase();
    if (seen.has(key)) continue;
    seen.add(key);
    result.push(normalized);
  }
  return result;
};

const buildSignature = () =>
  JSON.stringify({
    customCidrs: normalizeCidrs(customCidrs.value),
    selectedCidrs: isAutomaticSelection.value
      ? []
      : normalizeCidrs(selectedCidrs.value),
  });

const automaticTargets = computed(() => targets.value?.automaticTargets || []);
const selectedSet = computed(() => new Set(selectedCidrs.value));
const isDirty = computed(() => savedSignature.value !== buildSignature());

const allTargets = computed<ScanDiscoveryTarget[]>(() => {
  const map = new Map<string, ScanDiscoveryTarget>();
  const push = (target: ScanDiscoveryTarget) => {
    if (!map.has(target.cidr)) {
      map.set(target.cidr, target);
    }
  };

  for (const target of targets.value?.automaticTargets || []) push(target);
  for (const target of targets.value?.customTargets || []) push(target);
  for (const target of targets.value?.selectedTargets || []) push(target);

  for (const cidr of customCidrs.value) {
    if (!map.has(cidr)) {
      map.set(cidr, {
        cidr,
        label: t("admin.scanTargets.customLabel", { cidr }),
        source: "custom",
        hostCount: 0,
        isAutomatic: false,
      });
    }
  }

  for (const cidr of selectedCidrs.value) {
    if (!map.has(cidr)) {
      map.set(cidr, {
        cidr,
        label: t("admin.scanTargets.savedLabel", { cidr }),
        source: "saved",
        hostCount: 0,
        isAutomatic: false,
      });
    }
  }

  return [...map.values()];
});

const selectedHostCount = computed(() => {
  let total = 0;
  for (const cidr of selectedCidrs.value) {
    const target = allTargets.value.find((item) => item.cidr === cidr);
    if (!target || target.hostCount <= 0) return null;
    total += target.hostCount;
  }
  return total;
});

const getSourceLabel = (source: ScanDiscoveryTargetSource): string => {
  if (source === "docker") return "Docker";
  if (source === "loopback") return t("admin.scanTargets.sourceLoopback");
  if (source === "interface") return t("admin.scanTargets.sourceInterface");
  if (source === "mapping") return t("admin.scanTargets.sourceMapping");
  if (source === "custom") return t("admin.scanTargets.sourceCustom");
  return t("admin.scanTargets.sourceSaved");
};

const applyTargets = (payload: ScanDiscoveryTargetsResponse) => {
  targets.value = payload;
  customCidrs.value = normalizeCidrs(
    payload.customTargets.map((target) => target.cidr),
  );
  selectedCidrs.value = normalizeCidrs(
    payload.selectedCidrs.length > 0
      ? payload.selectedCidrs
      : payload.effectiveCidrs,
  );
  isAutomaticSelection.value = payload.selectionMode !== "custom";
  savedSignature.value = buildSignature();
};

async function loadTargets(force = false) {
  if (targets.value && !force) return;
  isLoading.value = true;
  try {
    applyTargets(await ScanAPI.getDiscoverTargets());
  } catch (error) {
    toast.error(t("admin.scanTargets.loadFailed"), {
      description:
        error instanceof Error
          ? error.message
          : t("admin.scanTargets.loadFallback"),
    });
    throw error;
  } finally {
    isLoading.value = false;
  }
}

const addCustomCidrs = () => {
  const values = normalizeCidrs(customInput.value.split(/[,\s，；;]+/u));
  if (values.length === 0) return;

  const invalid = values.filter((value) => !isValidCIDR(value));
  if (invalid.length > 0) {
    toast.error(t("admin.scanTargets.invalidCidr"), {
      description: t("admin.scanTargets.invalidCidrDescription", {
        values: invalid.slice(0, 3).join("、"),
      }),
    });
    return;
  }

  customCidrs.value = normalizeCidrs([...customCidrs.value, ...values]);
  selectedCidrs.value = normalizeCidrs([...selectedCidrs.value, ...values]);
  isAutomaticSelection.value = false;
  customInput.value = "";
};

const removeCustomCidr = (cidr: string) => {
  customCidrs.value = customCidrs.value.filter((item) => item !== cidr);
  selectedCidrs.value = selectedCidrs.value.filter((item) => item !== cidr);
};

const toggleCidr = (cidr: string, event: Event) => {
  const checked = (event.target as HTMLInputElement).checked;
  isAutomaticSelection.value = false;
  selectedCidrs.value = checked
    ? normalizeCidrs([...selectedCidrs.value, cidr])
    : selectedCidrs.value.filter((item) => item !== cidr);
};

const resetToAutomatic = () => {
  isAutomaticSelection.value = true;
  selectedCidrs.value = automaticTargets.value.map((target) => target.cidr);
};

async function saveTargets(silent = false): Promise<string[]> {
  if (selectedCidrs.value.length === 0) {
    const message = t("admin.scanTargets.selectRequired");
    toast.error(message);
    throw new Error(message);
  }

  isSaving.value = true;
  try {
    const payload = await ScanAPI.saveDiscoverTargets({
      custom_cidrs: customCidrs.value,
      selected_cidrs: isAutomaticSelection.value ? [] : selectedCidrs.value,
    });
    applyTargets(payload);
    if (!silent) {
      toast.success(t("admin.scanTargets.saveSuccess"));
    }
    return [...selectedCidrs.value];
  } catch (error) {
    toast.error(t("admin.scanTargets.saveFailed"), {
      description:
        error instanceof Error
          ? error.message
          : t("admin.scanTargets.saveFallback"),
    });
    throw error;
  } finally {
    isSaving.value = false;
  }
}

async function ensureSaved(): Promise<string[]> {
  await loadTargets();
  if (isDirty.value) {
    return saveTargets(true);
  }
  if (isAutomaticSelection.value) {
    await loadTargets(true);
  }
  if (selectedCidrs.value.length === 0) {
    const message = t("admin.scanTargets.selectRequired");
    toast.error(message);
    throw new Error(message);
  }
  return [...selectedCidrs.value];
}

defineExpose({
  loadTargets,
  ensureSaved,
  getSelectedCidrs: () => [...selectedCidrs.value],
});
</script>
