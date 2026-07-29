<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { RefreshCw, SlidersHorizontal, X } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import RefreshButton from "@/components/RefreshButton.vue";
import ScanDiscoveryTargetsSettings from "@/components/ScanDiscoveryTargetsSettings.vue";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import type { DiscoveredServiceInfo, ScanDiscoverResponse } from "@/lib/api";

const props = defineProps<{
  discoveredData: ScanDiscoverResponse | null;
  isAllSelected: boolean;
  isDiscovering: boolean;
  isSaving: boolean;
  isSelectionValid: boolean;
  isSettingsOpen: boolean;
  open: boolean;
  resolveServiceHost: (service: DiscoveredServiceInfo) => string;
  selectedServices: DiscoveredServiceInfo[];
  showHostColumn: boolean;
}>();

const emit = defineEmits<{
  cancel: [];
  save: [];
  scan: [];
  stopScan: [];
  toggleAll: [event: Event];
  toggleSettings: [];
  "update:open": [open: boolean];
  "update:selectedServices": [services: DiscoveredServiceInfo[]];
}>();
const { t } = useI18n();
const settingsRef = ref<InstanceType<
  typeof ScanDiscoveryTargetsSettings
> | null>(null);
const selectedServicesModel = computed({
  get: () => props.selectedServices,
  set: (services: DiscoveredServiceInfo[]) => {
    emit("update:selectedServices", services);
  },
});

defineExpose({
  ensureSaved: () => settingsRef.value?.ensureSaved(),
  loadTargets: () => settingsRef.value?.loadTargets(),
});
</script>

<template>
  <Dialog :open="open" @update:open="emit('update:open', $event)">
    <DialogContent
      class="flex max-h-[85vh] max-w-[calc(100vw-2rem)] flex-col overflow-hidden sm:max-w-[800px]"
    >
      <DialogHeader class="shrink-0">
        <div
          class="flex min-w-0 flex-col gap-3 sm:flex-row sm:items-center sm:justify-between"
        >
          <DialogTitle>{{ t("admin.reverseProxy.discoverTitle") }}</DialogTitle>
          <div
            class="flex w-fit max-w-full min-w-0 self-center items-center gap-2 sm:self-auto"
          >
            <Button
              variant="outline"
              size="icon"
              :aria-label="t('common.settings')"
              class="h-11 w-11 sm:h-9 sm:w-9"
              :disabled="isDiscovering"
              @click="emit('toggleSettings')"
            >
              <SlidersHorizontal class="h-4 w-4" />
            </Button>
            <RefreshButton
              class="h-11 w-auto max-w-[calc(100vw-7rem)] min-w-0 !shrink justify-center overflow-hidden sm:h-9 [&>span]:min-w-0 [&>span]:truncate"
              :label="t('admin.reverseProxy.refreshServices')"
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
              {{ t("admin.reverseProxy.cancel") }}
            </Button>
          </div>
        </div>
        <DialogDescription>
          {{ t("admin.reverseProxy.discoverDescription") }}
        </DialogDescription>
        <ScanDiscoveryTargetsSettings
          ref="settingsRef"
          v-show="isSettingsOpen"
          class="mt-3"
        />
      </DialogHeader>

      <div class="min-h-0 flex-1 overflow-auto">
        <div class="py-2">
          <div
            v-if="
              isDiscovering &&
              (!discoveredData || discoveredData.services.length === 0)
            "
            class="flex flex-col items-center justify-center space-y-4 py-16"
          >
            <RefreshCw class="h-8 w-8 animate-spin text-muted-foreground" />
            <p class="text-sm text-muted-foreground">
              {{ t("admin.reverseProxy.probing") }}
            </p>
          </div>

          <div
            v-else-if="
              !isDiscovering &&
              discoveredData &&
              discoveredData.services.length === 0
            "
            class="py-16 text-center text-muted-foreground"
          >
            {{ t("admin.reverseProxy.discoverEmpty") }}
          </div>

          <div
            v-else-if="discoveredData && discoveredData.services.length > 0"
            class="rounded-md border bg-background"
          >
            <Table
              class="min-w-[42rem] table-fixed"
              container-class="overflow-visible"
            >
              <TableHeader
                class="sticky top-0 z-10 bg-background shadow-sm [&_th]:sticky [&_th]:top-0 [&_th]:z-10 [&_th]:bg-background"
              >
                <TableRow>
                  <TableHead class="w-11 text-center">
                    <input
                      type="checkbox"
                      :aria-label="t('common.selectAll')"
                      class="h-4 w-4 cursor-pointer rounded border-gray-300 text-primary"
                      :checked="isAllSelected"
                      @change="emit('toggleAll', $event)"
                    />
                  </TableHead>
                  <TableHead v-if="showHostColumn" class="w-[100px]">
                    {{ t("admin.reverseProxy.discoverColumns.host") }}
                  </TableHead>
                  <TableHead class="w-16">
                    {{ t("admin.reverseProxy.discoverColumns.port") }}
                  </TableHead>
                  <TableHead class="w-[88px]">
                    {{ t("admin.reverseProxy.discoverColumns.status") }}
                  </TableHead>
                  <TableHead class="w-[180px]">
                    {{ t("admin.reverseProxy.discoverColumns.serviceId") }}
                  </TableHead>
                  <TableHead class="w-[200px]">
                    {{ t("admin.reverseProxy.discoverColumns.suggestedPath") }}
                  </TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                <TableRow
                  v-for="(service, index) in discoveredData.services"
                  :key="index"
                >
                  <TableCell class="text-center">
                    <input
                      v-model="selectedServicesModel"
                      type="checkbox"
                      :aria-label="
                        t('common.selectItem', {
                          item: `${resolveServiceHost(service)}:${service.port}`,
                        })
                      "
                      class="h-4 w-4 cursor-pointer rounded border-gray-300 text-primary"
                      :value="service"
                    />
                  </TableCell>
                  <TableCell
                    v-if="showHostColumn"
                    class="max-w-[100px] font-mono text-xs text-muted-foreground"
                  >
                    <span
                      class="block max-w-full truncate"
                      :title="resolveServiceHost(service)"
                    >
                      {{ resolveServiceHost(service) }}
                    </span>
                  </TableCell>
                  <TableCell class="font-medium">
                    <a
                      :href="`http://${resolveServiceHost(service)}:${service.port}`"
                      target="_blank"
                      rel="noopener noreferrer"
                      class="text-primary transition-colors hover:text-primary/80 hover:underline"
                      :title="t('admin.reverseProxy.openNewWindow')"
                    >
                      {{ service.port }}
                    </a>
                  </TableCell>
                  <TableCell>
                    <span
                      v-if="service.httpStatus === 401"
                      class="rounded bg-amber-500/10 px-2 py-0.5 text-xs text-amber-600"
                    >
                      {{ t("admin.reverseProxy.authRequiredShort") }}
                    </span>
                    <span
                      v-else
                      class="rounded bg-green-500/10 px-2 py-0.5 text-xs text-green-600"
                    >
                      {{ service.httpStatus }}
                    </span>
                  </TableCell>
                  <TableCell class="max-w-[180px]">
                    <span
                      v-if="service.detail.label"
                      class="block max-w-full truncate text-sm"
                      :title="service.detail.label"
                    >
                      {{ service.detail.label }}
                    </span>
                    <span v-else class="text-sm font-medium text-red-500">
                      {{ t("admin.reverseProxy.unknownService") }}
                    </span>
                  </TableCell>
                  <TableCell>
                    <Input
                      :aria-label="
                        t('admin.reverseProxy.requiredPathPlaceholder')
                      "
                      v-model="service.detail.rule.path"
                      :placeholder="
                        t('admin.reverseProxy.requiredPathPlaceholder')
                      "
                      class="h-8 min-w-0 text-sm"
                      :class="{
                        'border-destructive focus-visible:ring-destructive':
                          selectedServices.includes(service) &&
                          !service.detail.rule.path.trim(),
                      }"
                    />
                  </TableCell>
                </TableRow>
              </TableBody>
            </Table>
          </div>
        </div>
      </div>

      <DialogFooter
        class="mt-2 shrink-0 items-center sm:flex-nowrap sm:justify-between"
      >
        <span class="w-full text-sm text-muted-foreground sm:min-w-0 sm:flex-1">
          <template v-if="discoveredData">
            {{
              t("admin.reverseProxy.selectedItems", {
                count: `${selectedServices.length}/${discoveredData.services.length}`,
              })
            }}
          </template>
        </span>
        <div class="flex shrink-0 items-center gap-2">
          <Button variant="outline" @click="emit('cancel')">
            {{ t("admin.reverseProxy.cancel") }}
          </Button>
          <Button
            :disabled="
              selectedServices.length === 0 || !isSelectionValid || isSaving
            "
            @click="emit('save')"
          >
            {{ t("admin.reverseProxy.addSelected") }}
          </Button>
        </div>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
