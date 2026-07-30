<template>
  <div class="space-y-6">
    <Card>
      <CardHeader>
        <CardTitle
          class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between"
        >
          <span>{{ t("admin.streamMappings.title") }}</span>
          <div class="flex flex-wrap items-center gap-2">
            <div class="flex">
              <Button class="rounded-r-none" @click="openCreateDialog">
                <Plus class="mr-2 h-4 w-4" />
                {{ t("admin.streamMappings.addMapping") }}
              </Button>
              <DropdownMenu>
                <DropdownMenuTrigger as-child>
                  <Button
                    variant="default"
                    size="icon"
                    :aria-label="t('common.moreActions')"
                    class="rounded-l-none border-l border-primary-foreground/20 px-2"
                  >
                    <ChevronDown class="h-4 w-4" />
                  </Button>
                </DropdownMenuTrigger>
                <DropdownMenuContent align="end">
                  <DropdownMenuItem
                    @click="syncRoutes"
                    :disabled="isSyncing || !protocolMappingEnabled"
                  >
                    <RefreshCw
                      class="mr-2 h-4 w-4"
                      :class="{ 'animate-spin': isSyncing }"
                    />
                    {{
                      isSyncing
                        ? t("admin.streamMappings.syncing")
                        : t("admin.streamMappings.syncGateway")
                    }}
                  </DropdownMenuItem>
                </DropdownMenuContent>
              </DropdownMenu>
            </div>
          </div>
        </CardTitle>
        <CardDescription>
          {{ t("admin.streamMappings.description") }}
        </CardDescription>
      </CardHeader>

      <CardContent class="space-y-4">
        <StreamMappingDisabledAlert v-if="!protocolMappingEnabled" />
        <Alert
          class="items-start rounded-xl border-zinc-200 bg-zinc-50/70 text-zinc-900 shadow-none"
        >
          <Info class="mt-0.5 h-4 w-4 shrink-0" />
          <div class="space-y-1">
            <AlertTitle>{{ t("admin.streamMappings.accessTitle") }}</AlertTitle>
            <AlertDescription class="text-sm leading-6 text-zinc-700">
              {{ t("admin.streamMappings.accessDescription") }}
            </AlertDescription>
          </div>
        </Alert>
        <div
          class="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between"
        >
          <SearchInput
            v-model="searchQuery"
            :placeholder="t('admin.streamMappings.searchPlaceholder')"
            class="max-w-xs"
          />
        </div>
        <div class="overflow-hidden rounded-md border">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>{{ t("admin.streamMappings.protocol") }}</TableHead>
                <TableHead>{{
                  t("admin.streamMappings.listenPort")
                }}</TableHead>
                <TableHead>{{ t("admin.streamMappings.comment") }}</TableHead>
                <TableHead>{{ t("admin.streamMappings.target") }}</TableHead>
                <TableHead>{{
                  t("admin.streamMappings.authStatus")
                }}</TableHead>
                <TableHead class="text-right">{{
                  t("admin.sessions.table.actions")
                }}</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              <TableRow v-if="filteredMappings.length === 0">
                <TableCell
                  colspan="6"
                  class="py-8 text-center text-muted-foreground"
                >
                  {{ t("admin.streamMappings.empty") }}
                </TableCell>
              </TableRow>
              <TableRow
                v-for="mapping in filteredMappings"
                :key="getMappingKey(mapping)"
                class="group"
              >
                <TableCell>
                  <Badge
                    variant="outline"
                    class="font-mono uppercase tracking-[0.16em]"
                  >
                    {{ mapping.protocol }}
                  </Badge>
                </TableCell>
                <TableCell class="font-medium">
                  <div
                    class="inline-flex items-center gap-2 rounded-full border px-3 py-1 text-sm"
                  >
                    <span>{{ mapping.listen_port }}</span>
                  </div>
                </TableCell>
                <TableCell class="min-w-[180px]">
                  <InlineCommentEditor
                    :text="mapping.comment"
                    :save="(value) => updateComment(mapping, value)"
                  />
                </TableCell>
                <TableCell class="font-mono text-sm">{{
                  mapping.target
                }}</TableCell>
                <TableCell class="min-w-[15rem]">
                  <div
                    class="flex flex-wrap items-center gap-2 text-xs text-muted-foreground"
                  >
                    <Badge v-if="mapping.use_auth" variant="default">
                      {{ t("admin.streamMappings.authRequired") }}
                    </Badge>
                    <Badge v-else variant="secondary">{{
                      t("admin.streamMappings.publicAccess")
                    }}</Badge>
                  </div>
                </TableCell>
                <TableCell class="text-right">
                  <div class="flex justify-end gap-2">
                    <Button
                      variant="ghost"
                      size="sm"
                      @click="openEditDialog(mapping)"
                    >
                      {{ t("admin.streamMappings.edit") }}
                    </Button>
                    <ConfirmDangerPopover
                      :title="
                        t('admin.streamMappings.deleteTitle', {
                          protocol: formatProtocolLabel(mapping.protocol),
                        })
                      "
                      :description="
                        t('admin.streamMappings.deleteDescription', {
                          mapping: formatMappingLabel(mapping),
                          target: mapping.target,
                        })
                      "
                      :loading="removingMappingKey === getMappingKey(mapping)"
                      :disabled="removingMappingKey === getMappingKey(mapping)"
                      :on-confirm="() => removeMapping(mapping)"
                      content-class="w-72 text-left"
                    >
                      <template #trigger>
                        <Button
                          variant="ghost"
                          size="sm"
                          class="text-destructive hover:bg-destructive/10 hover:text-destructive"
                          :disabled="
                            removingMappingKey === getMappingKey(mapping)
                          "
                        >
                          {{ t("admin.streamMappings.delete") }}
                        </Button>
                      </template>
                    </ConfirmDangerPopover>
                  </div>
                </TableCell>
              </TableRow>
            </TableBody>
          </Table>
        </div>
      </CardContent>
    </Card>

    <StreamMappingEditorDialog
      v-model:open="isDialogOpen"
      :existing-mappings="allMappings"
      :mapping="editingMapping"
      :saving="isSaving"
      @save="saveMapping"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { ChevronDown, Info, Plus, RefreshCw } from "lucide-vue-next";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import InlineCommentEditor from "@admin-shared/components/InlineCommentEditor.vue";
import SearchInput from "@admin-shared/components/SearchInput.vue";
import { extractErrorMessage } from "@admin-shared/composables/useAsyncAction";
import { toast } from "@admin-shared/utils/toast";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { ConfigAPI } from "../lib/api";
import { useConfigStore } from "../store/config";
import type { StreamMapping } from "../types";
import StreamMappingDisabledAlert from "./stream-mappings/StreamMappingDisabledAlert.vue";
import StreamMappingEditorDialog from "./stream-mappings/StreamMappingEditorDialog.vue";
import {
  applyStreamMappingSubmission,
  compareStreamMappings,
  formatMappingLabel,
  formatProtocolLabel,
  getMappingKey,
  normalizeStreamMapping,
  removeStreamMapping,
  type StreamMappingEditorSubmission,
  updateStreamMappingComment,
} from "./stream-mappings/streamMappingModel";

const configStore = useConfigStore();
const { t } = useI18n();
const searchQuery = ref("");
const isDialogOpen = ref(false);
const isSaving = ref(false);
const isSyncing = ref(false);
const editingMapping = ref<StreamMapping | null>(null);
const removingMappingKey = ref<string | null>(null);
const allMappings = computed(() =>
  [...(configStore.config?.stream_mappings ?? [])]
    .map(normalizeStreamMapping)
    .sort(compareStreamMappings),
);
const protocolMappingEnabled = computed(
  () => configStore.config?.protocol_mapping_feature?.enabled === true,
);
const filteredMappings = computed(() => {
  const query = searchQuery.value.trim().toLowerCase();
  if (!query) return allMappings.value;

  return allMappings.value.filter((mapping) => {
    const authStatus = mapping.use_auth
      ? t("admin.streamMappings.authRequired")
      : t("admin.streamMappings.publicAccess");
    return (
      mapping.protocol.includes(query) ||
      formatProtocolLabel(mapping.protocol).toLowerCase().includes(query) ||
      String(mapping.listen_port).includes(query) ||
      (mapping.comment ?? "").toLowerCase().includes(query) ||
      mapping.target.toLowerCase().includes(query) ||
      authStatus.includes(query)
    );
  });
});

function openCreateDialog() {
  editingMapping.value = null;
  isDialogOpen.value = true;
}

function openEditDialog(mapping: StreamMapping) {
  editingMapping.value = normalizeStreamMapping(mapping);
  isDialogOpen.value = true;
}

async function saveMapping(submission: StreamMappingEditorSubmission) {
  isSaving.value = true;
  try {
    await configStore.saveStreamMappings((current) =>
      applyStreamMappingSubmission(current, submission),
    );
    toast.success(
      getSaveSuccessMessage(
        submission.mappings.length,
        submission.editingKey !== null,
      ),
    );
    isDialogOpen.value = false;
  } catch (error: any) {
    toast.error(t("admin.streamMappings.saveFailed"), {
      description: extractErrorMessage(error, t("common.tryLater")),
    });
  } finally {
    isSaving.value = false;
  }
}

function getSaveSuccessMessage(savedCount: number, isEditing: boolean): string {
  const action = isEditing
    ? t("admin.streamMappings.actionUpdate")
    : t("admin.streamMappings.actionCreate");
  return savedCount > 1
    ? t("admin.streamMappings.saveMany", { action, count: savedCount })
    : t("admin.streamMappings.saveOne", { action });
}
async function removeMapping(mapping: StreamMapping) {
  removingMappingKey.value = getMappingKey(mapping);
  try {
    const result = await configStore.saveStreamMappings(
      (current) => removeStreamMapping(current, getMappingKey(mapping)),
      { disableFeatureOnLegacyRepairConflict: true },
    );
    const description = result.protocolMappingDisabled
      ? t("admin.streamMappings.disabledForLegacyRepair")
      : undefined;
    const message = t("admin.streamMappings.removeSuccess", {
      mapping: formatMappingLabel(mapping),
    });
    toast.success(message, { description });
  } catch (error: any) {
    const titleKey = protocolMappingEnabled.value
      ? "admin.streamMappings.deleteFailed"
      : "admin.streamMappings.deleteFailedWhileDisabled";
    toast.error(t(titleKey), {
      description: extractErrorMessage(error, t("common.tryLater")),
    });
  } finally {
    removingMappingKey.value = null;
  }
}

async function updateComment(mapping: StreamMapping, comment: string) {
  try {
    const key = getMappingKey(mapping);
    await configStore.saveStreamMappings((current) =>
      updateStreamMappingComment(current, key, comment),
    );
    toast.success(t("admin.streamMappings.commentUpdated"));
  } catch (error: any) {
    throw new Error(
      extractErrorMessage(error, t("admin.streamMappings.commentUpdateFailed")),
      { cause: error },
    );
  }
}
async function syncRoutes() {
  isSyncing.value = true;
  try {
    const result = await ConfigAPI.syncRoutes();
    if (result.success) {
      toast.success(t("admin.streamMappings.syncSuccess"), {
        description: t("admin.streamMappings.syncDescription", {
          pathRules: result.data?.synced_rules ?? 0,
          hostRules: result.data?.synced_host_rules ?? 0,
          streamRules: result.data?.synced_stream_rules ?? 0,
        }),
      });
      return;
    }

    toast.error(t("admin.streamMappings.syncFailed"), {
      description: result.message || t("admin.streamMappings.syncNoSuccess"),
    });
  } catch (error: any) {
    toast.error(t("admin.streamMappings.syncFailed"), {
      description: extractErrorMessage(error, t("common.tryLater")),
    });
  } finally {
    isSyncing.value = false;
  }
}
</script>
