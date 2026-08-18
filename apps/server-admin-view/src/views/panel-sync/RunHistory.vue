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
import type { PanelConnection, PanelSyncRun } from "@/lib/api/panel-sync-api";

defineProps<{
  connection: PanelConnection | null;
  loading: boolean;
  open: boolean;
  runs: PanelSyncRun[];
}>();
const emit = defineEmits<{ "update:open": [value: boolean] }>();
const { t, locale } = useI18n();
const formatTime = (value: string) =>
  new Intl.DateTimeFormat(locale.value, {
    dateStyle: "medium",
    timeStyle: "medium",
  }).format(new Date(value));
</script>

<template>
  <Dialog :open="open" @update:open="emit('update:open', $event)">
    <DialogContent class="max-h-[85vh] overflow-y-auto sm:max-w-2xl">
      <DialogHeader
        ><DialogTitle>{{
          t("admin.panelSync.historyTitle", { name: connection?.name ?? "" })
        }}</DialogTitle
        ><DialogDescription>{{
          t("admin.panelSync.historyDescription")
        }}</DialogDescription></DialogHeader
      >
      <p v-if="loading" class="py-8 text-center text-sm text-muted-foreground">
        {{ t("admin.panelSync.loadingHistory") }}
      </p>
      <p
        v-else-if="runs.length === 0"
        class="py-8 text-center text-sm text-muted-foreground"
      >
        {{ t("admin.panelSync.noHistory") }}
      </p>
      <div v-else class="space-y-2">
        <div v-for="run in runs" :key="run.id" class="rounded-lg border p-3">
          <div class="flex items-center justify-between gap-3">
            <div class="text-sm font-medium">
              {{ formatTime(run.started_at) }}
            </div>
            <Badge variant="outline">{{
              t(`admin.panelSync.runStatus.${run.status}`)
            }}</Badge>
          </div>
          <div class="mt-1 text-xs text-muted-foreground">
            {{ t(`admin.panelSync.triggers.${run.trigger}`) }} · +{{
              run.counts.create
            }}
            / ~{{ run.counts.update }} / −{{ run.counts.delete }}
          </div>
          <p v-if="run.message" class="mt-2 text-sm">{{ run.message }}</p>
        </div>
      </div>
    </DialogContent>
  </Dialog>
</template>
