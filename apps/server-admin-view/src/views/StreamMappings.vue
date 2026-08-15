<template>
  <div class="space-y-6">
    <Card>
      <CardHeader>
        <CardTitle
          class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between"
        >
          <div class="flex flex-wrap items-center gap-2">
            <span>{{ t("admin.streamMappings.title") }}</span>
            <Badge
              v-if="protocolMappingEnabled && scheduleState"
              :variant="scheduleState === 'open' ? 'default' : 'secondary'"
              class="gap-1.5"
            >
              <Clock3 class="h-3.5 w-3.5" />
              {{
                scheduleState === "open"
                  ? t("admin.streamMappings.scheduleOpen", {
                      window: scheduleWindow,
                    })
                  : t("admin.streamMappings.scheduleClosed", {
                      window: scheduleWindow,
                    })
              }}
            </Badge>
          </div>
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
                  <DropdownMenuItem @click="openAvailabilityDialog">
                    <Clock3 class="mr-2 h-4 w-4" />
                    {{ t("admin.streamMappings.scheduleAvailability") }}
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
          v-else-if="scheduleState === 'closed'"
          class="items-start rounded-xl border-amber-300 bg-amber-50/80 text-amber-950 shadow-none"
        >
          <Clock3 class="mt-0.5 h-4 w-4 shrink-0" />
          <div class="space-y-1">
            <AlertTitle>{{
              t("admin.streamMappings.scheduleClosedTitle")
            }}</AlertTitle>
            <AlertDescription class="text-sm leading-6 text-amber-900">
              {{
                t("admin.streamMappings.scheduleClosedDescription", {
                  window: scheduleWindow,
                })
              }}
            </AlertDescription>
          </div>
        </Alert>
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
        <StreamMappingTable
          :mappings="allMappings"
          :removing-mapping-key="removingMappingKey"
          :probing-mapping-key="probingMappingKey"
          :on-remove="removeMapping"
          :on-save-comment="updateComment"
          @edit="openEditDialog"
          @probe="probeMapping"
          @policy="openBypassPolicy"
          @service="openServiceProfile"
        />
      </CardContent>
    </Card>

    <StreamMappingEditorDialog
      v-model:open="isDialogOpen"
      :existing-mappings="allMappings"
      :mapping="editingMapping"
      :saving="isSaving"
      @save="saveMapping"
    />
    <StreamMappingAvailabilityDialog
      :open="isAvailabilityDialogOpen"
      :enabled="availabilityFormEnabled"
      :start-time="availabilityFormStartTime"
      :end-time="availabilityFormEndTime"
      :loading="isSavingAvailability"
      :validation-message="availabilityValidationMessage"
      @update:open="handleAvailabilityDialogOpenChange"
      @update:enabled="availabilityFormEnabled = $event"
      @update:start-time="availabilityFormStartTime = $event"
      @update:end-time="availabilityFormEndTime = $event"
      @cancel="closeAvailabilityDialog"
      @save="saveAvailability"
    />
    <StreamServiceProfileDialog
      :open="isServiceProfileOpen"
      :loading="isSavingServiceProfile"
      :mapping="serviceProfileMapping"
      :catalog="serviceCatalog"
      :initial-service-id="serviceProfileInitialServiceId"
      @update:open="setServiceProfileOpen"
      @clear="clearServiceProfile"
      @confirm="confirmServiceProfile"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { ChevronDown, Clock3, Info, Plus, RefreshCw } from "lucide-vue-next";
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
import { extractErrorMessage } from "@admin-shared/composables/useAsyncAction";
import { toast } from "@admin-shared/utils/toast";
import { ConfigAPI } from "@/lib/api/config";
import { useConfigStore } from "../store/config";
import type { StreamMapping } from "../types";
import StreamMappingDisabledAlert from "./stream-mappings/StreamMappingDisabledAlert.vue";
import StreamMappingAvailabilityDialog from "./stream-mappings/StreamMappingAvailabilityDialog.vue";
import StreamMappingEditorDialog from "./stream-mappings/StreamMappingEditorDialog.vue";
import StreamMappingTable from "./stream-mappings/StreamMappingTable.vue";
import StreamServiceProfileDialog from "./stream-mappings/StreamServiceProfileDialog.vue";
import {
  applyStreamMappingSubmission,
  compareStreamMappings,
  formatMappingLabel,
  getMappingKey,
  normalizeStreamMapping,
  removeStreamMapping,
  type StreamMappingEditorSubmission,
  updateStreamMappingComment,
} from "./stream-mappings/streamMappingModel";
import { useStreamMappingAvailability } from "./stream-mappings/useStreamMappingAvailability";
import { useStreamMappingNavigation } from "./stream-mappings/useStreamMappingNavigation";
import { useStreamMappingSecurity } from "./stream-mappings/useStreamMappingSecurity";

const configStore = useConfigStore();
const { t } = useI18n();
const isDialogOpen = ref(false);
const isSaving = ref(false);
const isSyncing = ref(false);
const editingMapping = ref<StreamMapping | null>(null);
const removingMappingKey = ref<string | null>(null);
const { openBypassPolicy } = useStreamMappingNavigation();
const {
  clearServiceProfile,
  confirmServiceProfile,
  isSavingServiceProfile,
  isServiceProfileOpen,
  openServiceProfile,
  probeMapping,
  probingMappingKey,
  setServiceProfileOpen,
  serviceCatalog,
  serviceProfileInitialServiceId,
  serviceProfileMapping,
} = useStreamMappingSecurity();
const {
  availabilityFormEnabled,
  availabilityFormEndTime,
  availabilityFormStartTime,
  availabilityValidationMessage,
  closeAvailabilityDialog,
  handleAvailabilityDialogOpenChange,
  isAvailabilityDialogOpen,
  isSavingAvailability,
  openAvailabilityDialog,
  saveAvailability,
  scheduleState,
  scheduleWindow,
} = useStreamMappingAvailability();
const allMappings = computed(() =>
  [...(configStore.config?.stream_mappings ?? [])]
    .map(normalizeStreamMapping)
    .sort(compareStreamMappings),
);
const protocolMappingEnabled = computed(
  () => configStore.config?.protocol_mapping_feature?.enabled === true,
);

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
async function removeMapping(mapping: StreamMapping): Promise<boolean> {
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
    return true;
  } catch (error: any) {
    const titleKey = protocolMappingEnabled.value
      ? "admin.streamMappings.deleteFailed"
      : "admin.streamMappings.deleteFailedWhileDisabled";
    toast.error(t(titleKey), {
      description: extractErrorMessage(error, t("common.tryLater")),
    });
    return false;
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
