<template>
  <div class="rounded-md border bg-muted/20 p-3">
    <div
      class="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between"
    >
      <div class="space-y-1">
        <p class="text-sm font-medium">扫描网段</p>
        <p class="text-xs text-muted-foreground">
          最多选择 {{ limits.maxCidrs }} 个网段，单次最多扫描
          {{ limits.maxHosts }} 台主机。
        </p>
      </div>
      <div class="flex flex-wrap gap-2">
        <Button
          variant="outline"
          size="sm"
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
          恢复自动
        </Button>
        <Button
          size="sm"
          :disabled="
            isLoading || isSaving || !isDirty || selectedCidrs.length === 0
          "
          @click="saveTargets()"
        >
          <Save class="mr-2 h-4 w-4" :class="{ 'animate-pulse': isSaving }" />
          保存
        </Button>
      </div>
    </div>

    <div class="mt-3 flex gap-2">
      <Input
        v-model="customInput"
        :disabled="isLoading || isSaving"
        placeholder="例如 192.168.31.0/24"
        @keyup.enter="addCustomCidrs"
      />
      <Button
        variant="outline"
        :disabled="isLoading || isSaving || !customInput.trim()"
        @click="addCustomCidrs"
      >
        <Plus class="mr-2 h-4 w-4" />
        添加
      </Button>
    </div>

    <div
      v-if="isLoading"
      class="py-6 text-center text-sm text-muted-foreground"
    >
      正在识别可扫描网段...
    </div>

    <div
      v-else-if="allTargets.length === 0"
      class="py-6 text-center text-sm text-muted-foreground"
    >
      暂未识别到可扫描网段，请添加本地 IPv4 CIDR。
    </div>

    <div v-else class="mt-3 max-h-56 space-y-2 overflow-auto pr-1">
      <label
        v-for="target in allTargets"
        :key="target.cidr"
        class="flex cursor-pointer items-start gap-3 rounded-md border bg-background p-3 transition-colors hover:bg-muted/40"
      >
        <input
          type="checkbox"
          class="mt-0.5 h-4 w-4 cursor-pointer"
          :checked="selectedSet.has(target.cidr)"
          :disabled="isLoading || isSaving"
          @change="toggleCidr(target.cidr, $event)"
        />
        <div class="min-w-0 flex-1 space-y-1">
          <div class="flex flex-wrap items-center gap-2">
            <span class="font-mono text-sm">{{ target.cidr }}</span>
            <Badge variant="secondary">{{
              getSourceLabel(target.source)
            }}</Badge>
            <Badge v-if="target.isAutomatic" variant="outline">自动</Badge>
          </div>
          <p class="truncate text-xs text-muted-foreground">
            {{ target.label }}
          </p>
        </div>
        <div class="flex shrink-0 items-center gap-2">
          <span class="text-xs text-muted-foreground">
            {{ target.hostCount > 0 ? `${target.hostCount} 台` : "待保存" }}
          </span>
          <Button
            v-if="customCidrs.includes(target.cidr)"
            type="button"
            variant="ghost"
            size="icon"
            class="h-7 w-7"
            :disabled="isLoading || isSaving"
            @click.prevent="removeCustomCidr(target.cidr)"
          >
            <Trash2 class="h-4 w-4" />
          </Button>
        </div>
      </label>
    </div>

    <div
      class="mt-3 flex flex-col gap-1 text-xs text-muted-foreground sm:flex-row sm:items-center sm:justify-between"
    >
      <span>
        已选择 {{ selectedCidrs.length }} 个网段
        <template v-if="selectedHostCount !== null">
          / {{ selectedHostCount }} 台主机
        </template>
      </span>
      <span v-if="isDirty" class="text-amber-600">设置尚未保存</span>
      <span v-else-if="isAutomaticSelection">自动跟随候选网段</span>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
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
        label: `${cidr}（自定义）`,
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
        label: `${cidr}（已保存）`,
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
  if (source === "loopback") return "本机";
  if (source === "interface") return "网卡";
  if (source === "mapping") return "映射";
  if (source === "custom") return "自定义";
  return "已保存";
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
    toast.error("加载扫描网段失败", {
      description:
        error instanceof Error ? error.message : "无法获取服务发现扫描网段",
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
    toast.error("CIDR 格式不正确", {
      description: `请检查：${invalid.slice(0, 3).join("、")}`,
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
    toast.error("请选择扫描网段");
    throw new Error("请选择扫描网段");
  }

  isSaving.value = true;
  try {
    const payload = await ScanAPI.saveDiscoverTargets({
      custom_cidrs: customCidrs.value,
      selected_cidrs: isAutomaticSelection.value ? [] : selectedCidrs.value,
    });
    applyTargets(payload);
    if (!silent) {
      toast.success("扫描网段已保存");
    }
    return [...selectedCidrs.value];
  } catch (error) {
    toast.error("保存扫描网段失败", {
      description:
        error instanceof Error ? error.message : "无法保存服务发现扫描网段",
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
  if (selectedCidrs.value.length === 0) {
    toast.error("请选择扫描网段");
    throw new Error("请选择扫描网段");
  }
  return [...selectedCidrs.value];
}

defineExpose({
  loadTargets,
  ensureSaved,
  getSelectedCidrs: () => [...selectedCidrs.value],
});
</script>
