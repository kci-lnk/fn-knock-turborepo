<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { AlertTriangle, Loader2 } from "lucide-vue-next";
import { Alert, AlertDescription } from "@/components/ui/alert";
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
import type {
  PanelConnection,
  PanelSyncPreview,
} from "@/lib/api/panel-sync-api";

defineProps<{
  connection: PanelConnection | null;
  open: boolean;
  preview: PanelSyncPreview | null;
  syncing: boolean;
}>();
const emit = defineEmits<{ confirm: []; "update:open": [value: boolean] }>();
const { t } = useI18n();
const actionDetail = (action: PanelSyncPreview["actions"][number]) => {
  const object = action.object_type === "group" ? "Group" : "Link";
  const key = `admin.panelSync.actionDetails.${action.kind}${object}`;
  const translated = t(key);
  return translated === key ? action.detail : translated;
};
const warningText = (warning: string) =>
  warning.includes("Sun-Panel") && warning.includes("残留")
    ? t("admin.panelSync.warnings.sunResidual")
    : warning;
</script>

<template>
  <Dialog :open="open" @update:open="emit('update:open', $event)">
    <DialogContent class="max-h-[90vh] overflow-y-auto sm:max-w-3xl">
      <DialogHeader>
        <DialogTitle>{{
          t("admin.panelSync.previewTitle", { name: connection?.name ?? "" })
        }}</DialogTitle>
        <DialogDescription>{{
          t("admin.panelSync.previewDescription")
        }}</DialogDescription>
      </DialogHeader>
      <template v-if="preview">
        <div class="grid grid-cols-3 gap-2 sm:grid-cols-6">
          <div
            v-for="key in [
              'create',
              'update',
              'delete',
              'unchanged',
              'residual',
              'conflict',
            ] as const"
            :key="key"
            class="rounded-lg border p-2 text-center"
          >
            <div class="text-lg font-semibold">{{ preview.counts[key] }}</div>
            <div class="text-xs text-muted-foreground">
              {{ t(`admin.panelSync.actions.${key}`) }}
            </div>
          </div>
        </div>
        <Alert
          v-for="warning in preview.warnings"
          :key="warning"
          variant="destructive"
        >
          <AlertTriangle class="h-4 w-4" /><AlertDescription>{{
            warningText(warning)
          }}</AlertDescription>
        </Alert>
        <div class="max-h-72 space-y-2 overflow-y-auto pr-1">
          <div
            v-for="(action, index) in preview.actions"
            :key="`${action.source_id}-${index}`"
            class="flex items-start gap-3 rounded-lg border p-3"
          >
            <Badge variant="outline">{{
              t(`admin.panelSync.actions.${action.kind}`)
            }}</Badge>
            <div class="min-w-0">
              <div class="truncate text-sm font-medium">{{ action.title }}</div>
              <div class="text-xs text-muted-foreground">
                {{ actionDetail(action) }}
              </div>
            </div>
          </div>
        </div>
      </template>
      <DialogFooter>
        <Button variant="outline" @click="emit('update:open', false)">{{
          t("common.cancel")
        }}</Button>
        <Button
          :disabled="!preview?.can_apply || syncing"
          @click="emit('confirm')"
        >
          <Loader2 v-if="syncing" class="mr-2 h-4 w-4 animate-spin" />
          {{
            syncing
              ? t("admin.panelSync.syncing")
              : t("admin.panelSync.confirmSync")
          }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
