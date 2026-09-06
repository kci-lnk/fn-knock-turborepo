<script setup lang="ts">
import { ref } from "vue";
import { useI18n } from "vue-i18n";
import {
  Activity,
  Copy,
  Download,
  Loader2,
  RefreshCw,
  Trash2,
} from "lucide-vue-next";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import RuntimeComponentCard from "./RuntimeComponentCard.vue";
import GatewayMemoryDialog from "./GatewayMemoryDialog.vue";
import RuntimeDebugDialog from "./RuntimeDebugDialog.vue";
import {
  formatRuntimeBytes as formatBytes,
  formatRuntimeDate as formatDate,
  formatRuntimeLogLine as formatLogLine,
  getRuntimeEventComponent as eventComponent,
  runtimeStatusClass as statusClass,
} from "./runtimePresentation";
import { useRuntimeHealth } from "./useRuntimeHealth";

const props = withDefaults(defineProps<{ active?: boolean }>(), {
  active: true,
});
const { t } = useI18n();
const debugDialogOpen = ref(false);
const {
  clearRuntimeLogs,
  copying,
  copyDiagnostics,
  exporting,
  exportDiagnostics,
  fetchRuntime,
  gatewayMemoryDialogOpen,
  loadRuntimeLogs,
  loading,
  logDialogOpen,
  logEntries,
  logGeneratedAt,
  logsClearing,
  logsLoading,
  openGatewayMemoryDialog,
  openRuntimeLogs,
  processComponents,
  recentEvents,
  selectedLogComponentName,
  serviceComponents,
  snapshot,
} = useRuntimeHealth({ active: () => props.active });
</script>

<template>
  <div class="flex h-full flex-col gap-4 overflow-auto pb-2">
    <div
      class="flex flex-col items-stretch justify-between gap-4 rounded-lg border bg-background p-4 lg:flex-row lg:items-center"
    >
      <div class="space-y-1">
        <div class="flex items-center gap-2">
          <span class="text-base font-semibold">{{
            t("admin.eventCenter.runtime.overall")
          }}</span>
          <Badge
            v-if="snapshot"
            variant="outline"
            :class="statusClass(snapshot.overall_status)"
          >
            {{
              t(`admin.eventCenter.runtime.status.${snapshot.overall_status}`)
            }}
          </Badge>
          <Loader2
            v-else-if="loading"
            class="h-4 w-4 animate-spin text-muted-foreground"
          />
        </div>
        <div class="text-sm text-muted-foreground">
          {{ t("admin.eventCenter.runtime.lastChecked") }}:
          {{ formatDate(snapshot?.last_checked_at) }}
          <span v-if="snapshot">
            · {{ t("admin.eventCenter.runtime.supervisor") }}:
            {{ snapshot.supervisor }}</span
          >
        </div>
      </div>
      <div
        class="grid w-full grid-cols-1 gap-2 sm:grid-cols-2 xl:grid-cols-4 lg:w-auto"
      >
        <Button variant="outline" size="sm" @click="debugDialogOpen = true">
          <Activity class="mr-2 h-4 w-4" />
          {{ t("admin.eventCenter.runtime.debug.open") }}
        </Button>
        <Button
          variant="outline"
          size="sm"
          :disabled="loading"
          @click="fetchRuntime()"
        >
          <RefreshCw
            class="mr-2 h-4 w-4"
            :class="{ 'animate-spin': loading }"
          />
          {{ t("admin.eventCenter.runtime.refresh") }}
        </Button>
        <Button
          variant="outline"
          size="sm"
          :disabled="copying"
          @click="copyDiagnostics"
        >
          <Loader2 v-if="copying" class="mr-2 h-4 w-4 animate-spin" />
          <Copy v-else class="mr-2 h-4 w-4" />
          {{ t("admin.eventCenter.runtime.copy") }}
        </Button>
        <Button size="sm" :disabled="exporting" @click="exportDiagnostics">
          <Loader2 v-if="exporting" class="mr-2 h-4 w-4 animate-spin" />
          <Download v-else class="mr-2 h-4 w-4" />
          {{ t("admin.eventCenter.runtime.export") }}
        </Button>
      </div>
    </div>

    <div class="grid gap-4 xl:grid-cols-3">
      <section
        class="flex flex-col overflow-hidden rounded-lg border xl:col-span-2"
      >
        <div class="border-b bg-muted/20 px-4 py-3">
          <h3 class="font-medium">
            {{ t("admin.eventCenter.runtime.processSection") }}
          </h3>
        </div>
        <div class="grid flex-1 gap-px bg-border md:grid-cols-2">
          <RuntimeComponentCard
            v-for="component in processComponents"
            :key="component.id"
            :component="component"
            variant="process"
            show-log-action
            :show-memory-action="component.id === 'gateway_process'"
            :show-debug-action="component.id === 'management'"
            @view-debug="debugDialogOpen = true"
            @view-logs="openRuntimeLogs"
            @manage-memory="openGatewayMemoryDialog"
          />
        </div>
      </section>

      <section
        class="flex flex-col overflow-hidden rounded-lg border xl:col-span-1"
      >
        <div class="border-b bg-muted/20 px-4 py-3">
          <h3 class="font-medium">
            {{ t("admin.eventCenter.runtime.serviceSection") }}
          </h3>
        </div>
        <div
          class="grid flex-1 gap-px bg-border sm:grid-cols-2 xl:auto-rows-fr xl:grid-cols-1"
        >
          <RuntimeComponentCard
            v-for="component in serviceComponents"
            :key="component.id"
            :component="component"
            variant="service"
          />
        </div>
      </section>
    </div>

    <div v-if="snapshot" class="rounded-lg border bg-background p-4">
      <div class="mb-3 font-medium">
        {{ t("admin.eventCenter.runtime.logs") }}
      </div>
      <div class="grid gap-3 text-sm sm:grid-cols-2 lg:grid-cols-4">
        <div>
          <span class="text-muted-foreground">{{
            t("admin.eventCenter.runtime.coverage")
          }}</span>
          <div>
            {{ formatDate(snapshot.logs.oldest_at) }} —
            {{ formatDate(snapshot.logs.newest_at) }}
          </div>
        </div>
        <div>
          <span class="text-muted-foreground">{{
            t("admin.eventCenter.runtime.diskUsage")
          }}</span>
          <div>{{ formatBytes(snapshot.logs.bytes_used) }} / 6 MiB</div>
        </div>
        <div>
          <span class="text-muted-foreground">{{
            t("admin.eventCenter.runtime.dropped")
          }}</span>
          <div>{{ snapshot.logs.dropped_info }}</div>
        </div>
        <div>
          <span class="text-muted-foreground">{{
            t("admin.eventCenter.runtime.directory")
          }}</span>
          <div>{{ snapshot.logs.directory }}</div>
        </div>
      </div>
    </div>

    <div class="rounded-lg border bg-background">
      <div class="border-b px-4 py-3 font-medium">
        {{ t("admin.eventCenter.runtime.recentEvents") }}
      </div>
      <div v-if="recentEvents.length" class="divide-y">
        <div
          v-for="event in recentEvents"
          :key="event.id"
          class="flex flex-wrap items-center gap-2 px-4 py-3 text-sm"
        >
          <Badge variant="outline">{{
            t(`admin.eventCenter.eventTypes.${event.type}`)
          }}</Badge>
          <span class="font-medium">{{
            t(`admin.eventCenter.runtime.components.${eventComponent(event)}`)
          }}</span>
          <span class="text-muted-foreground">{{
            String(event.payload?.reason_code || "-")
          }}</span>
          <span class="ml-auto text-xs text-muted-foreground">{{
            formatDate(event.happened_at)
          }}</span>
        </div>
      </div>
      <div v-else class="px-4 py-8 text-center text-sm text-muted-foreground">
        {{ t("admin.eventCenter.runtime.noEvents") }}
      </div>
    </div>

    <Dialog v-model:open="logDialogOpen">
      <DialogContent class="flex max-h-[85vh] flex-col sm:max-w-4xl">
        <DialogHeader class="shrink-0 pr-8 text-left">
          <DialogTitle>
            {{
              t("admin.eventCenter.runtime.logDialogTitle", {
                component: selectedLogComponentName,
              })
            }}
          </DialogTitle>
          <DialogDescription>
            {{ t("admin.eventCenter.runtime.logDialogDescription") }}
          </DialogDescription>
        </DialogHeader>

        <div
          class="flex shrink-0 flex-col items-stretch justify-between gap-2 text-xs sm:flex-row sm:items-center"
        >
          <span class="text-muted-foreground">
            {{ t("admin.eventCenter.runtime.logUpdatedAt") }}:
            {{ formatDate(logGeneratedAt) }}
          </span>
          <div class="grid grid-cols-2 gap-2 sm:flex">
            <ConfirmDangerPopover
              :title="t('admin.eventCenter.runtime.clearLogTitle')"
              :description="
                t('admin.eventCenter.runtime.clearLogDescription', {
                  component: selectedLogComponentName,
                })
              "
              :confirm-text="t('admin.eventCenter.runtime.confirmClearLogs')"
              :loading="logsClearing"
              :disabled="logsLoading || logsClearing"
              content-class="w-80 text-left"
              :on-confirm="clearRuntimeLogs"
            >
              <template #trigger>
                <Button
                  variant="outline"
                  size="sm"
                  class="border-destructive/20 text-destructive hover:bg-destructive/5 hover:text-destructive"
                  :disabled="logsLoading || logsClearing"
                >
                  <Trash2 class="mr-2 h-4 w-4" />
                  {{ t("admin.eventCenter.runtime.clearLogs") }}
                </Button>
              </template>
            </ConfirmDangerPopover>
            <Button
              variant="outline"
              size="sm"
              :disabled="logsLoading || logsClearing"
              @click="loadRuntimeLogs"
            >
              <RefreshCw
                class="mr-2 h-4 w-4"
                :class="{ 'animate-spin': logsLoading }"
              />
              {{ t("admin.eventCenter.runtime.refresh") }}
            </Button>
          </div>
        </div>

        <div
          class="min-h-48 flex-1 overflow-auto rounded-md border bg-slate-950 p-3 font-mono text-xs leading-5 text-slate-100"
        >
          <div
            v-if="logsLoading && !logEntries.length"
            class="flex h-48 items-center justify-center text-slate-400"
          >
            <Loader2 class="mr-2 h-4 w-4 animate-spin" />
            {{ t("admin.eventCenter.runtime.loadingLogs") }}
          </div>
          <div
            v-else-if="!logEntries.length"
            class="flex h-48 items-center justify-center text-slate-400"
          >
            {{ t("admin.eventCenter.runtime.noLogs") }}
          </div>
          <div v-else class="space-y-1">
            <div
              v-for="(entry, index) in logEntries"
              :key="`${entry.time}-${entry.event}-${index}`"
              class="whitespace-pre-wrap break-all"
            >
              {{ formatLogLine(entry) }}
            </div>
          </div>
        </div>
      </DialogContent>
    </Dialog>

    <RuntimeDebugDialog v-model:open="debugDialogOpen" :active="props.active" />

    <GatewayMemoryDialog
      v-model:open="gatewayMemoryDialogOpen"
      @updated="fetchRuntime(false)"
    />
  </div>
</template>
