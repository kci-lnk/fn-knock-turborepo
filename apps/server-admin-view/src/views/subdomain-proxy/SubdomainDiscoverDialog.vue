<template>
  <Dialog :open="open" @update:open="emit('update:open', $event)">
    <DialogContent
      class="flex max-w-[calc(100vw-2rem)] max-h-[85vh] flex-col overflow-hidden sm:max-w-[820px]"
    >
      <DialogHeader class="shrink-0">
        <div
          class="flex min-w-0 flex-col gap-3 sm:flex-row sm:items-center sm:justify-between"
        >
          <div class="min-w-0 space-y-1">
            <DialogTitle>{{
              t("admin.subdomainProxy.discoverTitle")
            }}</DialogTitle>
            <DialogDescription>
              {{
                t("admin.subdomainProxy.discoverDescription", {
                  domain,
                })
              }}
            </DialogDescription>
          </div>
          <div
            class="flex w-fit max-w-full min-w-0 self-center items-center gap-2 sm:self-auto"
          >
            <Button
              variant="outline"
              size="icon"
              class="h-11 w-11 sm:h-9 sm:w-9"
              :disabled="isDiscovering"
              @click="emit('toggleSettings')"
            >
              <SlidersHorizontal class="h-4 w-4" />
            </Button>
            <RefreshButton
              class="h-11 w-auto max-w-[calc(100vw-7rem)] min-w-0 !shrink justify-center overflow-hidden sm:h-9 [&>span]:min-w-0 [&>span]:truncate"
              :label="
                isDiscovering
                  ? t('admin.subdomainProxy.scanning')
                  : t('admin.subdomainProxy.refreshServices')
              "
              :loading="isDiscovering"
              :disabled="isDiscovering"
              @click="emit('scan')"
            />
            <Button
              v-if="isDiscovering"
              class="h-11 sm:h-9"
              variant="outline"
              @click="emit('stopScan')"
            >
              <X class="mr-2 h-4 w-4" />
              {{ t("admin.subdomainProxy.cancel") }}
            </Button>
          </div>
        </div>
        <ScanDiscoveryTargetsSettings
          ref="settingsRef"
          v-show="isSettingsOpen"
          class="mt-3"
        />
      </DialogHeader>

      <div class="flex-1 min-h-0 overflow-auto">
        <div class="py-2">
          <div
            v-if="
              isDiscovering &&
              (!discoveredData || discoveredData.services.length === 0)
            "
            class="flex flex-col items-center justify-center py-16 space-y-4"
          >
            <RefreshCw class="h-8 w-8 animate-spin text-muted-foreground" />
            <p class="text-sm text-muted-foreground">
              {{ t("admin.subdomainProxy.probing") }}
            </p>
          </div>

          <div
            v-else-if="
              !isDiscovering &&
              discoveredData &&
              discoveredData.services.length === 0
            "
            class="text-center py-16 text-muted-foreground"
          >
            {{
              discoveredData.foundServices > 0
                ? t("admin.subdomainProxy.discoverAllAdded")
                : t("admin.subdomainProxy.discoverEmpty")
            }}
          </div>

          <div
            v-else-if="discoveredData && discoveredData.services.length > 0"
            class="rounded-md border bg-background"
          >
            <Table class="min-w-[42rem]" container-class="overflow-visible">
              <TableHeader
                class="sticky top-0 z-10 bg-background shadow-sm [&_th]:sticky [&_th]:top-0 [&_th]:z-10 [&_th]:bg-background"
              >
                <TableRow>
                  <TableHead class="w-[50px] text-center">
                    <input
                      type="checkbox"
                      class="h-4 w-4 cursor-pointer"
                      :checked="isAllSelected"
                      @change="emitToggleAll"
                    />
                  </TableHead>
                  <TableHead v-if="showHostColumn" class="w-[140px]">
                    {{ t("admin.subdomainProxy.discoverColumns.host") }}
                  </TableHead>
                  <TableHead class="w-[80px]">{{
                    t("admin.subdomainProxy.discoverColumns.port")
                  }}</TableHead>
                  <TableHead class="w-[100px]">{{
                    t("admin.subdomainProxy.discoverColumns.status")
                  }}</TableHead>
                  <TableHead class="min-w-[10rem]">{{
                    t("admin.subdomainProxy.discoverColumns.serviceId")
                  }}</TableHead>
                  <TableHead class="w-[260px] min-w-[18rem]">
                    {{
                      t(
                        "admin.subdomainProxy.discoverColumns.suggestedSubdomain",
                      )
                    }}
                  </TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                <TableRow
                  v-for="(svc, index) in discoveredData.services"
                  :key="`${resolveDiscoveredServiceHost(svc, discoveredData.host)}-${svc.port}-${index}`"
                >
                  <TableCell class="text-center">
                    <input
                      v-model="selectedServicesModel"
                      type="checkbox"
                      class="h-4 w-4 cursor-pointer"
                      :value="svc"
                    />
                  </TableCell>
                  <TableCell
                    v-if="showHostColumn"
                    class="font-mono text-xs text-muted-foreground"
                  >
                    {{ resolveDiscoveredServiceHost(svc, discoveredData.host) }}
                  </TableCell>
                  <TableCell class="font-medium">{{ svc.port }}</TableCell>
                  <TableCell>
                    <span
                      v-if="svc.requiresBasicAuth"
                      class="text-amber-600 bg-amber-500/10 text-xs px-2 py-0.5 rounded"
                    >
                      Basic Auth
                    </span>
                    <span
                      v-else-if="svc.httpStatus === 401"
                      class="text-amber-600 bg-amber-500/10 text-xs px-2 py-0.5 rounded"
                    >
                      {{ t("admin.subdomainProxy.authRequiredShort") }}
                    </span>
                    <span
                      v-else
                      class="text-green-600 bg-green-500/10 text-xs px-2 py-0.5 rounded"
                    >
                      {{ svc.httpStatus }}
                    </span>
                  </TableCell>
                  <TableCell class="min-w-[10rem] text-sm">
                    {{
                      svc.detail.label ||
                      svc.detail.name ||
                      t("admin.subdomainProxy.unknownService")
                    }}
                  </TableCell>
                  <TableCell class="min-w-[18rem]">
                    <div
                      class="flex min-w-[18rem] items-stretch rounded-md border"
                    >
                      <Input
                        v-model="svc.suggestedSubdomain"
                        placeholder="service"
                        class="h-8 rounded-none border-0 text-sm shadow-none focus-visible:ring-0"
                        :class="{
                          'border-destructive focus-visible:ring-destructive':
                            selectedServices.includes(svc) &&
                            !svc.suggestedSubdomain.trim(),
                        }"
                      />
                      <div
                        class="flex shrink-0 items-center border-l bg-muted/30 px-3 text-xs text-muted-foreground"
                      >
                        .{{ domain }}
                      </div>
                    </div>
                  </TableCell>
                </TableRow>
              </TableBody>
            </Table>
          </div>
        </div>
      </div>

      <DialogFooter class="mt-2 shrink-0 items-center sm:justify-between">
        <span class="text-sm text-muted-foreground">
          <template v-if="discoveredData">
            <template v-if="isDiscovering">
              {{ t("admin.subdomainProxy.probing") }}
            </template>
            <template v-else>
              {{
                t("admin.subdomainProxy.discoveredScannedPorts", {
                  count: footerScannedPorts,
                })
              }}
            </template>
            ，{{
              t("admin.subdomainProxy.selectedItems", {
                count: `${selectedServices.length} / ${discoveredData.services.length}`,
              })
            }}
            <template v-if="discoveredData.scanCidrs?.length">
              ，{{
                t("admin.subdomainProxy.coveredCidrsHosts", {
                  cidrs: discoveredData.scanCidrs.length,
                  hosts:
                    discoveredData.scanHostCount ||
                    discoveredData.scannedHosts ||
                    0,
                })
              }}
            </template>
            <template
              v-if="
                !discoveredData.scanCidrs?.length &&
                discoveredData.scannedHosts &&
                discoveredData.scannedHosts > 1
              "
            >
              {{
                t("admin.subdomainProxy.coveredHosts", {
                  hosts:
                    discoveredData.scanScope || discoveredData.scannedHosts,
                })
              }}
            </template>
          </template>
        </span>
        <div class="space-x-2">
          <Button variant="outline" @click="emit('cancel')">
            {{ t("admin.subdomainProxy.cancel") }}
          </Button>
          <Button
            :disabled="
              selectedServices.length === 0 ||
              !isSelectionValid ||
              isSavingMappings
            "
            @click="emit('save')"
          >
            {{ t("admin.subdomainProxy.addSelected") }}
          </Button>
        </div>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { RefreshCw, SlidersHorizontal, X } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import RefreshButton from "@/components/RefreshButton.vue";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import ScanDiscoveryTargetsSettings from "@/components/ScanDiscoveryTargetsSettings.vue";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import type { DiscoveredHostResponse, DiscoveredHostService } from "./model";
import { resolveDiscoveredServiceHost } from "./model";
import type { ScanDiscoverProgress } from "@/lib/api";

const props = defineProps<{
  discoverProgress: ScanDiscoverProgress | null;
  discoveredData: DiscoveredHostResponse | null;
  domain: string;
  isAllSelected: boolean;
  isDiscovering: boolean;
  isSavingMappings: boolean;
  isSelectionValid: boolean;
  isSettingsOpen: boolean;
  open: boolean;
  selectedServices: DiscoveredHostService[];
  showHostColumn: boolean;
}>();

const emit = defineEmits<{
  cancel: [];
  save: [];
  scan: [];
  stopScan: [];
  toggleAll: [checked: boolean];
  toggleSettings: [];
  "update:open": [open: boolean];
  "update:selectedServices": [services: DiscoveredHostService[]];
}>();

const { t } = useI18n();
const settingsRef = ref<InstanceType<
  typeof ScanDiscoveryTargetsSettings
> | null>(null);

const selectedServicesModel = computed({
  get: () => props.selectedServices,
  set: (value: DiscoveredHostService[]) => {
    emit("update:selectedServices", value);
  },
});

const footerScannedPorts = computed(() => {
  return props.discoveredData?.totalPortsScanned || 0;
});

const emitToggleAll = (event: Event) => {
  emit("toggleAll", (event.target as HTMLInputElement).checked);
};

defineExpose({
  ensureSaved: () => settingsRef.value?.ensureSaved(),
  loadTargets: () => settingsRef.value?.loadTargets(),
});
</script>
