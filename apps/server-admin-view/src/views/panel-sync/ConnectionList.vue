<script setup lang="ts">
import { ServerCog } from "lucide-vue-next";
import { useI18n } from "vue-i18n";
import type { PanelConnection } from "@/lib/api/panel-sync-api";
import ConnectionCard from "./ConnectionCard.vue";

defineProps<{
  connections: PanelConnection[];
  deletingIds: Set<string>;
  loading: boolean;
  previewingId: string;
  testingIds: Set<string>;
}>();
const emit = defineEmits<{
  delete: [connection: PanelConnection];
  edit: [connection: PanelConnection];
  history: [connection: PanelConnection];
  preview: [connection: PanelConnection];
  test: [connection: PanelConnection];
  "toggle-auto": [connection: PanelConnection, value: boolean];
}>();
const { t } = useI18n();
</script>

<template>
  <div v-if="loading" class="py-12 text-center text-sm text-muted-foreground">
    {{ t("admin.panelSync.loadingConnections") }}
  </div>
  <div
    v-else-if="connections.length === 0"
    class="rounded-xl border border-dashed py-14 text-center"
  >
    <ServerCog class="mx-auto h-9 w-9 text-muted-foreground" />
    <p class="mt-3 font-medium">{{ t("admin.panelSync.empty") }}</p>
    <p class="mt-1 text-sm text-muted-foreground">
      {{ t("admin.panelSync.emptyDescription") }}
    </p>
  </div>
  <div v-else class="grid gap-4 xl:grid-cols-2">
    <ConnectionCard
      v-for="connection in connections"
      :key="connection.id"
      :connection="connection"
      :deleting="deletingIds.has(connection.id)"
      :previewing="previewingId === connection.id"
      :testing="testingIds.has(connection.id)"
      @delete="emit('delete', connection)"
      @edit="emit('edit', connection)"
      @history="emit('history', connection)"
      @preview="emit('preview', connection)"
      @test="emit('test', connection)"
      @toggle-auto="emit('toggle-auto', connection, $event)"
    />
  </div>
</template>
