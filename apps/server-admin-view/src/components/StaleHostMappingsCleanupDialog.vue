<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { RefreshCw, Trash2 } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import type { HostMapping } from "@/types";
import StaleHostMappingsResults from "./stale-host-mappings/StaleHostMappingsResults.vue";
import { useStaleHostMappingsCleanupDialog } from "./stale-host-mappings/useStaleHostMappingsCleanupDialog";

const props = defineProps<{
  mappings: HostMapping[];
  saveMappings: (mappings: HostMapping[]) => Promise<unknown>;
  isAuthServiceTarget: (target: string) => boolean;
}>();
const emit = defineEmits<{ cleaned: [count: number] }>();
const { t } = useI18n();
const model = useStaleHostMappingsCleanupDialog({
  mappings: () => props.mappings,
  saveMappings: (mappings) => props.saveMappings(mappings),
  isAuthServiceTarget: (target) => props.isAuthServiceTarget(target),
  onCleaned: (count) => emit("cleaned", count),
});

defineExpose({ open: model.openCleanupDialog });
</script>

<template>
  <Dialog :open="model.isOpen" @update:open="model.handleOpenChange">
    <DialogContent
      class="flex max-h-[88vh] flex-col overflow-hidden max-sm:max-h-[92dvh] max-sm:p-4 sm:max-w-[900px]"
    >
      <DialogHeader class="shrink-0">
        <div
          class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between"
        >
          <div class="space-y-1">
            <DialogTitle>
              {{ t("admin.subdomainProxy.staleCleanupTitle") }}
            </DialogTitle>
            <DialogDescription>
              {{ t("admin.subdomainProxy.staleCleanupDescription") }}
            </DialogDescription>
          </div>
          <Button
            class="w-full sm:w-auto"
            variant="outline"
            :disabled="model.isProbing || model.isCleaning"
            @click="model.handleProbe"
          >
            <RefreshCw
              class="mr-2 h-4 w-4"
              :class="{ 'animate-spin': model.isProbing }"
            />
            {{
              t(
                model.isProbing
                  ? "admin.subdomainProxy.staleCleanupChecking"
                  : "admin.subdomainProxy.staleCleanupRefresh",
              )
            }}
          </Button>
        </div>
      </DialogHeader>

      <div class="min-h-0 flex-1 overflow-x-hidden overflow-y-auto">
        <div class="py-2">
          <div
            v-if="model.isProbing"
            class="flex flex-col items-center justify-center space-y-4 py-16"
          >
            <RefreshCw class="h-8 w-8 animate-spin text-muted-foreground" />
            <p class="text-sm text-muted-foreground">
              {{ t("admin.subdomainProxy.staleCleanupProbing") }}
            </p>
          </div>
          <div
            v-else-if="model.probeableMappings.length === 0"
            class="py-16 text-center text-sm text-muted-foreground"
          >
            {{ t("admin.subdomainProxy.staleCleanupEmpty") }}
          </div>
          <div
            v-else-if="model.results.length === 0"
            class="py-16 text-center text-sm text-muted-foreground"
          >
            {{ t("admin.subdomainProxy.staleCleanupNoResults") }}
          </div>
          <div
            v-else-if="model.visibleResults.length === 0"
            class="py-16 text-center text-sm text-muted-foreground"
          >
            {{ t("admin.subdomainProxy.staleCleanupNoStale") }}
          </div>
          <StaleHostMappingsResults v-else :model="model" />
        </div>
      </div>

      <DialogFooter
        class="mt-2 shrink-0 items-stretch gap-3 sm:items-center sm:justify-between"
      >
        <span class="text-sm text-muted-foreground">
          <template v-if="model.visibleResults.length > 0">
            {{
              t("admin.subdomainProxy.staleCleanupSelected", {
                selected: model.selectedCount,
                total: model.staleResults.length,
              })
            }}
          </template>
        </span>
        <div class="flex flex-col-reverse gap-2 sm:flex-row sm:justify-end">
          <Button
            variant="outline"
            class="w-full sm:w-auto"
            :disabled="model.isCleaning"
            @click="model.closeDialog"
          >
            {{ t("admin.subdomainProxy.cancel") }}
          </Button>
          <Button
            class="w-full sm:w-auto"
            variant="destructive"
            :disabled="
              model.selectedCount === 0 || model.isProbing || model.isCleaning
            "
            @click="model.handleCleanSelected"
          >
            <Trash2 v-if="!model.isCleaning" class="mr-2 h-4 w-4" />
            <RefreshCw v-else class="mr-2 h-4 w-4 animate-spin" />
            {{
              t(
                model.isCleaning
                  ? "admin.subdomainProxy.staleCleanupCleaning"
                  : "admin.subdomainProxy.staleCleanupCleanSelected",
              )
            }}
          </Button>
        </div>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
