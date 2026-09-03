<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { Plus, RefreshCw } from "lucide-vue-next";
import { useI18n } from "vue-i18n";
import {
  Breadcrumb,
  BreadcrumbItem,
  BreadcrumbLink,
  BreadcrumbList,
  BreadcrumbPage,
  BreadcrumbSeparator,
} from "@/components/ui/breadcrumb";
import { Button } from "@/components/ui/button";
import type {
  PanelConnection,
  PanelSyncPreview,
} from "@/lib/api/panel-sync-api";
import ConnectionEditor from "./panel-sync/ConnectionEditor.vue";
import ConnectionList from "./panel-sync/ConnectionList.vue";
import DeleteConnectionDialog from "./panel-sync/DeleteConnectionDialog.vue";
import RunHistory from "./panel-sync/RunHistory.vue";
import SyncPreview from "./panel-sync/SyncPreview.vue";
import { usePanelSyncPage } from "./panel-sync/usePanelSyncPage";

const { t } = useI18n();
const page = usePanelSyncPage();
const deleteTarget = ref<PanelConnection | null>(null);
const deletePreview = ref<PanelSyncPreview | null>(null);
const deleteOpen = computed({
  get: () => deleteTarget.value !== null,
  set: (value: boolean) => {
    if (!value) deleteTarget.value = null;
  },
});

watch(deleteTarget, () => {
  deletePreview.value = null;
});

const previewDeleteCleanup = async () => {
  if (!deleteTarget.value) return;
  deletePreview.value = await page.connections.previewCleanup(
    deleteTarget.value,
  );
};

const confirmDelete = async (cleanupRemote: boolean) => {
  if (!deleteTarget.value) return;
  if (cleanupRemote && !deletePreview.value) return;
  await page.connections.remove(
    deleteTarget.value,
    cleanupRemote ? (deletePreview.value ?? undefined) : undefined,
  );
  deleteTarget.value = null;
};

const toggleAuto = async (connection: PanelConnection, enabled: boolean) => {
  await page.connections.update(connection.id, {
    name: connection.name,
    base_url: connection.base_url,
    api_path: connection.api_path,
    allow_invalid_tls: connection.allow_invalid_tls ?? false,
    grouping: {
      mode: connection.grouping?.mode ?? "mirror",
      namespace: connection.grouping?.namespace ?? "fn-knock",
      single_group_name: connection.grouping?.single_group_name ?? "",
    },
    auto_sync: {
      enabled,
      interval_minutes: connection.auto_sync?.interval_minutes ?? 60,
    },
    clear_credential: false,
  });
};
</script>

<template>
  <div class="space-y-6">
    <Breadcrumb>
      <BreadcrumbList>
        <BreadcrumbItem>
          <BreadcrumbLink href="#/mappings?tab=subdomain">
            {{ t("admin.nav.mappingManagement") }}
          </BreadcrumbLink>
        </BreadcrumbItem>
        <BreadcrumbSeparator />
        <BreadcrumbItem>
          <BreadcrumbPage>{{ t("admin.panelSync.title") }}</BreadcrumbPage>
        </BreadcrumbItem>
      </BreadcrumbList>
    </Breadcrumb>
    <div
      class="flex flex-col gap-4 sm:flex-row sm:items-start sm:justify-between"
    >
      <div>
        <h1 class="text-2xl font-semibold tracking-tight">
          {{ t("admin.panelSync.title") }}
        </h1>
        <p class="mt-1 max-w-3xl text-sm text-muted-foreground">
          {{ t("admin.panelSync.description") }}
        </p>
      </div>
      <div class="flex gap-2">
        <Button
          variant="outline"
          :disabled="page.connections.loading.value"
          @click="page.connections.load"
        >
          <RefreshCw class="mr-2 h-4 w-4" />{{ t("common.refreshStatus") }}
        </Button>
        <Button @click="page.editor.openCreate"
          ><Plus class="mr-2 h-4 w-4" />{{
            t("admin.panelSync.addConnection")
          }}</Button
        >
      </div>
    </div>

    <ConnectionList
      :connections="page.connections.connections.value"
      :deleting-ids="page.connections.deletingIds.value"
      :loading="page.connections.loading.value"
      :previewing-id="page.run.previewingId.value"
      :testing-ids="page.connections.testingIds.value"
      @delete="deleteTarget = $event"
      @edit="page.editor.openEdit"
      @history="page.run.openHistory"
      @preview="page.run.openPreview"
      @test="page.connections.testSaved"
      @toggle-auto="toggleAuto"
    />

    <ConnectionEditor
      v-model:open="page.editor.open.value"
      :auto-sync-ready="page.editor.autoSyncReady.value"
      :draft-verified="page.editor.draftVerified.value"
      :form="page.editor.form"
      :is-editing="page.editor.isEditing.value"
      :providers="page.connections.providers.value"
      :saving="page.connections.saving.value"
      :testing="page.editor.testing.value"
      @save="page.editor.save"
      @select-provider="page.editor.selectProvider"
      @test="page.editor.testDraft"
    />
    <SyncPreview
      v-model:open="page.run.previewOpen.value"
      :connection="page.run.previewConnection.value"
      :preview="page.run.preview.value"
      :syncing="page.run.syncing.value"
      @confirm="page.run.confirmSync"
    />
    <RunHistory
      v-model:open="page.run.historyOpen.value"
      :connection="page.run.historyConnection.value"
      :loading="page.run.loadingHistory.value"
      :runs="page.run.history.value"
    />
    <DeleteConnectionDialog
      v-model:open="deleteOpen"
      :connection="deleteTarget"
      :deleting="
        deleteTarget
          ? page.connections.deletingIds.value.has(deleteTarget.id)
          : false
      "
      :cleanup-preview="deletePreview"
      :previewing-cleanup="
        deleteTarget
          ? page.connections.previewingCleanupIds.value.has(deleteTarget.id)
          : false
      "
      @confirm="confirmDelete"
      @preview-cleanup="previewDeleteCleanup"
    />
  </div>
</template>
