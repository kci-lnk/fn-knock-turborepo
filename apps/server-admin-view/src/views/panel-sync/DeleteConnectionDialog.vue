<script setup lang="ts">
import { ref, watch } from "vue";
import { useI18n } from "vue-i18n";
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
import type {
  PanelConnection,
  PanelSyncPreview,
} from "@/lib/api/panel-sync-api";

const props = defineProps<{
  cleanupPreview: PanelSyncPreview | null;
  connection: PanelConnection | null;
  deleting: boolean;
  open: boolean;
  previewingCleanup: boolean;
}>();
const emit = defineEmits<{
  confirm: [cleanupRemote: boolean];
  "preview-cleanup": [];
  "update:open": [value: boolean];
}>();
const { t } = useI18n();
const cleanupRemote = ref(false);

watch(
  () => [props.open, props.connection?.id] as const,
  () => {
    cleanupRemote.value = false;
  },
);

const updateCleanup = (value: boolean | "indeterminate") => {
  cleanupRemote.value = value === true;
  if (cleanupRemote.value) emit("preview-cleanup");
};
</script>

<template>
  <Dialog :open="open" @update:open="emit('update:open', $event)">
    <DialogContent>
      <DialogHeader>
        <DialogTitle>{{ t("admin.panelSync.deleteTitle") }}</DialogTitle>
        <DialogDescription>
          {{
            t("admin.panelSync.deleteConfirm", {
              name: connection?.name ?? "",
            })
          }}
        </DialogDescription>
      </DialogHeader>
      <div
        v-if="connection?.provider !== 'sun_panel'"
        class="space-y-3 rounded-md border p-3"
      >
        <div class="flex items-start gap-2">
          <Checkbox
            id="panel-sync-cleanup-remote"
            :model-value="cleanupRemote"
            :disabled="deleting || previewingCleanup"
            @update:model-value="updateCleanup"
          />
          <label
            for="panel-sync-cleanup-remote"
            class="cursor-pointer text-sm leading-5"
          >
            {{ t("admin.panelSync.cleanupRemote") }}
          </label>
        </div>
        <p class="text-xs text-muted-foreground">
          {{ t("admin.panelSync.cleanupRemoteDescription") }}
        </p>
        <p v-if="previewingCleanup" class="text-sm text-muted-foreground">
          {{ t("admin.panelSync.cleanupPreviewing") }}
        </p>
        <div
          v-else-if="cleanupRemote && cleanupPreview"
          class="space-y-2 rounded-md bg-muted/60 p-3 text-sm"
        >
          <p class="font-medium">
            {{ t("admin.panelSync.cleanupPreviewReady") }}
          </p>
          <p class="text-muted-foreground">
            {{
              t("admin.panelSync.cleanupPreviewCounts", {
                groups: cleanupPreview.actions.filter(
                  (item) =>
                    item.kind === "delete" && item.object_type === "group",
                ).length,
                links: cleanupPreview.actions.filter(
                  (item) =>
                    item.kind === "delete" && item.object_type === "link",
                ).length,
              })
            }}
          </p>
          <ul class="max-h-32 space-y-1 overflow-auto text-xs">
            <li
              v-for="action in cleanupPreview.actions.filter(
                (item) => item.kind === 'delete',
              )"
              :key="`${action.object_type}-${action.source_id}`"
            >
              {{ action.title }}
            </li>
          </ul>
        </div>
      </div>
      <p
        v-else
        class="rounded-md border border-amber-500/40 bg-amber-500/10 p-3 text-sm"
      >
        {{ t("admin.panelSync.sunPanelDetachOnly") }}
      </p>
      <DialogFooter>
        <Button variant="outline" @click="emit('update:open', false)">
          {{ t("common.cancel") }}
        </Button>
        <Button
          variant="destructive"
          :disabled="
            deleting ||
            previewingCleanup ||
            (cleanupRemote && cleanupPreview === null)
          "
          @click="emit('confirm', cleanupRemote)"
        >
          {{ t("admin.panelSync.actions.delete") }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
