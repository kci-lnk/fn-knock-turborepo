<script setup lang="ts">
import { computed, reactive, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { Loader2, Radar, Settings2 } from "lucide-vue-next";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
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
import { createRandomTargetName } from "@/lib/wolTargetName";
import type {
  WOLDiscoveredDevice,
  WOLDiscoveryProgress,
  WOLDiscoveryResult,
} from "@/lib/api";

type DiscoveredSelection = WOLDiscoveredDevice & {
  name: string;
};

const props = defineProps<{
  open: boolean;
  result: WOLDiscoveryResult | null;
  progress: WOLDiscoveryProgress | null;
  existingMacs: string[];
  scanning: boolean;
  adding: boolean;
}>();

const emit = defineEmits<{
  "update:open": [value: boolean];
  scan: [targetCidrs: string[]];
  add: [devices: DiscoveredSelection[]];
}>();

const { t } = useI18n();
const customCidrs = ref("");
const showSettings = ref(false);
const selected = ref(new Set<string>());
const names = reactive<Record<string, string>>({});
const existing = computed(() => new Set(props.existingMacs));
const selectableDevices = computed(
  () =>
    props.result?.devices.filter((device) => !existing.value.has(device.mac)) ??
    [],
);
const selectedDevices = computed(() =>
  selectableDevices.value
    .filter((device) => selected.value.has(device.mac))
    .map((device) => ({
      ...device,
      name: names[device.mac]?.trim() ?? "",
    })),
);
const selectAllState = computed<boolean | "indeterminate">(() => {
  if (!selectedDevices.value.length) return false;
  return selectedDevices.value.length === selectableDevices.value.length
    ? true
    : "indeterminate";
});
const progressPercent = computed(() => {
  const progress = props.progress;
  if (!progress?.totalHosts) return 0;
  return Math.min(100, (progress.scannedHosts / progress.totalHosts) * 100);
});

watch(
  () => props.result?.devices ?? [],
  (devices) => {
    if (!props.result) {
      selected.value = new Set();
      for (const key of Object.keys(names)) delete names[key];
      return;
    }
    const macs = devices.map((device) => device.mac);
    const available = new Set(macs);
    selected.value = new Set(
      [...selected.value].filter(
        (mac) => available.has(mac) && !existing.value.has(mac),
      ),
    );
    for (const device of devices) {
      names[device.mac] ??= createRandomTargetName(
        t("admin.wol.targetDialog.generatedNamePrefix"),
      );
    }
  },
);

watch(
  () => props.open,
  (open) => {
    if (!open) {
      customCidrs.value = "";
      showSettings.value = false;
    }
  },
);

const toggle = (mac: string, checked: boolean | "indeterminate") => {
  const next = new Set(selected.value);
  if (checked === true) next.add(mac);
  else next.delete(mac);
  selected.value = next;
};

const toggleAll = (checked: boolean | "indeterminate") => {
  selected.value =
    checked === true
      ? new Set(selectableDevices.value.map((device) => device.mac))
      : new Set();
};

const scan = () => {
  const targetCidrs = customCidrs.value
    .split(/[\s,，;；]+/u)
    .map((value) => value.trim())
    .filter(Boolean);
  showSettings.value = false;
  emit("scan", targetCidrs);
};
</script>

<template>
  <Dialog :open="open" @update:open="emit('update:open', $event)">
    <DialogContent class="sm:max-w-3xl">
      <DialogHeader class="pr-8">
        <div class="flex items-start justify-between gap-3">
          <div class="space-y-1.5">
            <DialogTitle>{{ t("admin.wol.discovery.title") }}</DialogTitle>
            <DialogDescription>
              {{ t("admin.wol.discovery.description") }}
            </DialogDescription>
          </div>
          <Button
            type="button"
            size="icon"
            :variant="showSettings ? 'secondary' : 'ghost'"
            :aria-label="t('admin.wol.discovery.settings')"
            :title="t('admin.wol.discovery.settings')"
            @click="showSettings = !showSettings"
          >
            <Settings2 class="h-4 w-4" />
          </Button>
        </div>
      </DialogHeader>

      <div
        v-if="showSettings"
        class="space-y-2 rounded-xl border bg-muted/20 p-3"
      >
        <Label for="wol-discovery-cidrs">{{
          t("admin.wol.discovery.customCidr")
        }}</Label>
        <Input
          id="wol-discovery-cidrs"
          v-model="customCidrs"
          :disabled="scanning"
          autocomplete="off"
          spellcheck="false"
          :placeholder="t('admin.wol.discovery.customCidrPlaceholder')"
          @keydown.enter.prevent="scan"
        />
        <p class="text-xs text-muted-foreground">
          {{ t("admin.wol.discovery.customCidrHint") }}
        </p>
      </div>

      <div v-if="scanning && progress" class="space-y-2 rounded-xl border p-3">
        <div class="flex items-center justify-between gap-3 text-xs">
          <span class="flex min-w-0 items-center gap-2 text-muted-foreground">
            <Loader2 class="h-4 w-4 shrink-0 animate-spin" />
            <span class="truncate">
              {{
                t("admin.wol.discovery.progress", {
                  scanned: progress.scannedHosts,
                  total: progress.totalHosts,
                  found: progress.foundDevices,
                })
              }}
            </span>
          </span>
          <span class="font-medium">{{ Math.round(progressPercent) }}%</span>
        </div>
        <div class="h-1.5 overflow-hidden rounded-full bg-muted">
          <div
            class="h-full rounded-full bg-primary transition-[width] duration-300"
            :style="{ width: `${progressPercent}%` }"
          />
        </div>
      </div>

      <div v-if="result" class="space-y-3">
        <div class="flex flex-wrap items-center justify-between gap-2 text-xs">
          <div class="flex flex-wrap gap-1.5">
            <Badge
              v-for="network in result.networks"
              :key="`${network.interfaceName}-${network.scanCidr}`"
              variant="outline"
            >
              {{ network.interfaceName }} · {{ network.scanCidr }}
            </Badge>
          </div>
          <span v-if="!scanning" class="text-muted-foreground">
            {{
              t("admin.wol.discovery.resultSummary", {
                count: result.devices.length,
                duration: result.durationMs,
              })
            }}
          </span>
        </div>

        <div
          v-if="!result.devices.length"
          class="rounded-xl border border-dashed px-5 py-10 text-center text-sm text-muted-foreground"
        >
          {{
            scanning
              ? t("admin.wol.discovery.waitingForDevices")
              : t("admin.wol.discovery.empty")
          }}
        </div>
        <template v-else>
          <div
            class="flex flex-wrap items-center justify-between gap-2 rounded-lg border bg-muted/20 px-3 py-2"
          >
            <div class="flex items-center gap-2">
              <Checkbox
                id="wol-discovery-select-all"
                :model-value="selectAllState"
                :disabled="!selectableDevices.length"
                @update:model-value="toggleAll"
              />
              <Label for="wol-discovery-select-all" class="cursor-pointer">
                {{
                  t("admin.wol.discovery.selectAll", {
                    count: selectableDevices.length,
                  })
                }}
              </Label>
            </div>
            <span class="text-xs text-muted-foreground">
              {{
                t("admin.wol.discovery.selectedCount", {
                  count: selectedDevices.length,
                })
              }}
            </span>
          </div>
          <div class="max-h-[23rem] space-y-2 overflow-y-auto pr-1">
            <div
              v-for="device in result.devices"
              :key="device.mac"
              class="flex items-start gap-3 rounded-lg border p-3"
              :class="existing.has(device.mac) && 'opacity-60'"
            >
              <Checkbox
                class="mt-1"
                :aria-label="
                  t('admin.wol.discovery.selectDevice', { ip: device.ip })
                "
                :model-value="selected.has(device.mac)"
                :disabled="existing.has(device.mac)"
                @update:model-value="toggle(device.mac, $event)"
              />
              <div class="min-w-0 flex-1 space-y-2">
                <div>
                  <div class="flex flex-wrap items-center gap-2">
                    <span class="font-medium">{{
                      names[device.mac]?.trim() ||
                      t("admin.wol.discovery.autoNamePending")
                    }}</span>
                    <Badge v-if="existing.has(device.mac)" variant="secondary">
                      {{ t("admin.wol.discovery.added") }}
                    </Badge>
                  </div>
                  <p class="mt-0.5 font-mono text-xs text-muted-foreground">
                    {{ device.ip }} · {{ device.mac }} ·
                    {{ device.interfaceName }} ·
                    {{ device.broadcastAddress }}
                  </p>
                </div>
                <div v-if="!existing.has(device.mac)" class="max-w-sm">
                  <div class="space-y-1">
                    <Label
                      :for="`wol-discovery-name-${device.mac}`"
                      class="text-xs"
                    >
                      {{ t("admin.wol.name") }}
                    </Label>
                    <Input
                      :id="`wol-discovery-name-${device.mac}`"
                      v-model="names[device.mac]"
                      class="h-8"
                      maxlength="64"
                      :aria-label="t('admin.wol.discovery.namePlaceholder')"
                      :placeholder="t('admin.wol.discovery.namePlaceholder')"
                    />
                  </div>
                </div>
              </div>
            </div>
          </div>
        </template>
      </div>
      <div
        v-else
        class="flex min-h-40 items-center justify-center rounded-xl border border-dashed text-sm text-muted-foreground"
      >
        <span class="flex items-center gap-2">
          <Loader2 v-if="scanning" class="h-5 w-5 animate-spin" />
          {{
            scanning
              ? t("admin.wol.discovery.scanning")
              : t("admin.wol.discovery.ready")
          }}
        </span>
      </div>

      <DialogFooter class="gap-2 sm:justify-between">
        <Button variant="outline" :disabled="scanning || adding" @click="scan">
          <Loader2 v-if="scanning" class="mr-1.5 h-4 w-4 animate-spin" />
          <Radar v-else class="mr-1.5 h-4 w-4" />
          {{ t("admin.wol.discovery.rescan") }}
        </Button>
        <div class="flex justify-end gap-2">
          <Button variant="outline" @click="emit('update:open', false)">
            {{ t("common.cancel") }}
          </Button>
          <Button
            :disabled="adding || scanning || !selectedDevices.length"
            @click="emit('add', selectedDevices)"
          >
            <Loader2 v-if="adding" class="mr-1.5 h-4 w-4 animate-spin" />
            {{
              t("admin.wol.discovery.addSelected", {
                count: selectedDevices.length,
              })
            }}
          </Button>
        </div>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
