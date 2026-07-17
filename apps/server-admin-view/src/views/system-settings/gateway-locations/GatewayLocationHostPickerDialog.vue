<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { Badge } from "@/components/ui/badge";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import type { HostMapping } from "@/types";

defineProps<{
  mappings: HostMapping[];
  open: boolean;
  selectedHost: string;
  selectedMapping: HostMapping | null;
}>();

const emit = defineEmits<{
  select: [host: string];
  "update:open": [open: boolean];
}>();

const { t } = useI18n();

const getMappingTitle = (mapping?: HostMapping | null) =>
  mapping?.title_override.trim() || mapping?.title.trim() || "-";
</script>

<template>
  <Dialog :open="open" @update:open="emit('update:open', $event)">
    <DialogContent class="sm:max-w-[760px]">
      <DialogHeader>
        <DialogTitle>
          {{ t("admin.gatewayLocationsSettings.chooseHost") }}
        </DialogTitle>
        <DialogDescription class="leading-6">
          {{ t("admin.gatewayLocationsSettings.chooseHostDescription") }}
          <span class="font-medium text-foreground">
            {{
              selectedMapping?.host ||
              t("admin.gatewayLocationsSettings.notSelected")
            }}
          </span>
          <template v-if="selectedMapping">
            · {{ getMappingTitle(selectedMapping) }}
          </template>
        </DialogDescription>
      </DialogHeader>

      <div class="grid max-h-[60vh] gap-2 overflow-y-auto pr-1">
        <button
          v-for="mapping in mappings"
          :key="mapping.host"
          type="button"
          class="w-full rounded-md border px-4 py-3 text-left transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring/40"
          :class="
            mapping.host === selectedHost
              ? 'border-border bg-muted/40'
              : 'border-border/60 bg-background hover:border-primary/30 hover:bg-muted/20'
          "
          @click="emit('select', mapping.host)"
        >
          <span
            class="grid min-w-0 gap-3 sm:grid-cols-[minmax(0,1.2fr)_minmax(0,1fr)] sm:items-center"
          >
            <span class="min-w-0 space-y-1">
              <span class="flex min-w-0 flex-wrap items-center gap-2">
                <span class="truncate text-sm font-semibold">
                  {{ mapping.host }}
                </span>
                <Badge v-if="mapping.host === selectedHost" variant="secondary">
                  {{ t("admin.gatewayLocationsSettings.current") }}
                </Badge>
                <span class="text-xs text-muted-foreground">
                  {{ mapping.locations?.length ?? 0 }}
                </span>
              </span>
              <span class="block truncate text-sm text-muted-foreground">
                {{
                  mapping.target ||
                  t("admin.gatewayLocationsSettings.notSelected")
                }}
              </span>
            </span>

            <span class="min-w-0 space-y-1">
              <span class="text-xs font-medium text-muted-foreground">
                {{ t("admin.gatewayLocationsSettings.siteTitle") }}
              </span>
              <span class="block truncate text-sm font-medium">
                {{ getMappingTitle(mapping) }}
              </span>
            </span>
          </span>
        </button>
      </div>
    </DialogContent>
  </Dialog>
</template>
