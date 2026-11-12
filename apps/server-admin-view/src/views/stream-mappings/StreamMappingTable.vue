<template>
  <div
    class="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between"
  >
    <SearchInput
      v-model="searchQuery"
      :placeholder="t('admin.streamMappings.searchPlaceholder')"
      class="max-w-xs"
    />
    <Button
      variant="outline"
      size="sm"
      class="min-h-11 sm:min-h-8"
      :aria-pressed="selectionMode"
      :disabled="saving || (!selectionMode && !mappings.length)"
      @click="toggleSelectionMode"
    >
      <ListChecks class="mr-2 h-4 w-4" />
      {{
        selectionMode
          ? t("admin.subdomainProxy.exitSelectionMode")
          : t("admin.subdomainProxy.selectionMode")
      }}
    </Button>
  </div>
  <div
    v-if="selectionMode"
    role="toolbar"
    :aria-label="t('admin.subdomainProxy.batchActions')"
    class="flex flex-wrap items-center gap-2 rounded-md border bg-muted/35 px-3 py-2"
  >
    <span class="mr-auto text-sm font-medium" aria-live="polite">{{
      t("admin.streamMappings.selectedCount", { count: selected.size })
    }}</span>
    <Button
      size="sm"
      class="min-h-11 sm:min-h-8"
      variant="outline"
      :disabled="saving || !selected.size"
      @click="clearSelection"
    >
      {{ t("admin.subdomainProxy.clearSelection") }}
    </Button>
    <Button
      v-for="action in ruleActions"
      :key="action"
      size="sm"
      class="min-h-11 sm:min-h-8"
      :variant="action === 'delete' ? 'destructive' : 'outline'"
      :disabled="
        saving || !selectedRules.some((item) => affected(item, action))
      "
      @click="requestAction(action)"
    >
      {{ t(`admin.streamMappings.actions.${action}.label`) }}
    </Button>
  </div>
  <div class="overflow-hidden rounded-md border">
    <Table class="min-w-[70rem] table-fixed">
      <colgroup>
        <col v-if="selectionMode" class="w-12" />
        <col class="w-[8%]" />
        <col class="w-[10%]" />
        <col class="w-[14%]" />
        <col class="w-[16%]" />
        <col class="w-[18%]" />
        <col class="w-[10%]" />
        <col class="w-[12%]" />
        <col class="w-[12%]" />
      </colgroup>
      <TableHeader>
        <TableRow>
          <TableHead v-if="selectionMode">
            <Checkbox
              class="relative after:absolute after:-inset-3.5 sm:after:hidden"
              :model-value="allChecked"
              :disabled="saving || !filteredMappings.length"
              :aria-label="t('admin.streamMappings.selectVisible')"
              @update:model-value="setAllVisible($event === true)"
            />
          </TableHead>
          <TableHead>{{ t("admin.streamMappings.protocol") }}</TableHead>
          <TableHead>{{ t("admin.streamMappings.listenPort") }}</TableHead>
          <TableHead>{{ t("admin.streamMappings.comment") }}</TableHead>
          <TableHead>{{ t("admin.streamMappings.target") }}</TableHead>
          <TableHead>{{ t("admin.streamMappings.serviceProfile") }}</TableHead>
          <TableHead>{{ t("admin.streamMappings.traffic") }}</TableHead>
          <TableHead>{{ t("admin.streamMappings.authStatus") }}</TableHead>
          <TableHead class="text-right">{{
            t("admin.sessions.table.actions")
          }}</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        <TableRow v-if="filteredMappings.length === 0">
          <TableCell
            :colspan="selectionMode ? 9 : 8"
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
          <TableCell v-if="selectionMode">
            <Checkbox
              class="relative after:absolute after:-inset-3.5 sm:after:hidden"
              :model-value="selected.has(getMappingKey(mapping))"
              :disabled="saving"
              :aria-label="
                t('admin.streamMappings.selectRule', {
                  mapping: formatMappingLabel(mapping),
                })
              "
              @update:model-value="
                setSelected(getMappingKey(mapping), $event === true)
              "
            />
          </TableCell>
          <TableCell>
            <div class="flex flex-col items-start gap-1.5">
              <Badge
                variant="outline"
                class="font-mono uppercase tracking-[0.16em]"
              >
                {{ mapping.protocol }}
              </Badge>
              <Badge
                :variant="mapping.manually_disabled ? 'secondary' : 'outline'"
              >
                {{
                  mapping.manually_disabled
                    ? t("admin.streamMappings.ruleDisabled")
                    : t("admin.streamMappings.ruleEnabled")
                }}
              </Badge>
            </div>
          </TableCell>
          <TableCell class="font-medium">
            <div
              class="inline-flex items-center gap-2 rounded-full border px-3 py-1 text-sm"
            >
              <span>{{ mapping.listen_port }}</span>
            </div>
          </TableCell>
          <TableCell class="min-w-0">
            <InlineCommentEditor
              :text="mapping.comment"
              :save="(value) => onSaveComment(mapping, value)"
            />
          </TableCell>
          <TableCell class="min-w-0 font-mono text-sm">
            <span class="block truncate" :title="mapping.target">
              {{ mapping.target }}
            </span>
          </TableCell>
          <TableCell class="whitespace-normal">
            <div class="flex flex-col gap-1.5 text-xs">
              <div class="flex flex-wrap items-center gap-1.5">
                <Badge
                  v-if="mapping.disabled && !mapping.manually_disabled"
                  variant="secondary"
                >
                  {{ t("admin.streamMappings.gatewayRuleDisabled") }}
                </Badge>
                <Badge :variant="mapping.disabled ? 'secondary' : 'outline'">
                  {{
                    mapping.service_profile?.service_id ||
                    t("admin.streamMappings.serviceUnknown")
                  }}
                </Badge>
                <Badge variant="secondary">{{
                  mapping.probe_status || "legacy"
                }}</Badge>
              </div>
              <span
                v-if="mapping.service_profile?.device_role"
                class="text-muted-foreground"
              >
                {{ mapping.service_profile.device_role }} ·
                {{ mapping.service_profile.role_confidence || "unknown" }}
              </span>
              <span class="text-muted-foreground">
                {{
                  mapping.validation_mode === "strict"
                    ? t("admin.streamMappings.validationStrict")
                    : t("admin.streamMappings.validationOff")
                }}
              </span>
            </div>
          </TableCell>
          <TableCell class="w-[7rem] min-w-[7rem] max-w-[7rem]">
            <StreamTrafficActivity
              :stream-key="streamTrafficKey(mapping)"
              :title="formatMappingLabel(mapping)"
              :sample="getStreamTrafficSample(mapping)"
              :timestamp="trafficTimestamp ?? null"
            />
          </TableCell>
          <TableCell class="whitespace-normal">
            <div
              class="flex flex-wrap items-center gap-2 text-xs text-muted-foreground"
            >
              <Badge v-if="mapping.use_auth" variant="default">
                {{ t("admin.streamMappings.authRequired") }}
              </Badge>
              <Badge v-else variant="secondary">{{
                t("admin.streamMappings.publicAccess")
              }}</Badge>
              <Badge
                v-if="mapping.bypass_policy?.enabled"
                variant="outline"
                class="border-emerald-300 text-emerald-700 dark:text-emerald-300"
              >
                {{ t("admin.streamMappings.policyActive") }}
              </Badge>
              <Badge
                v-else-if="mapping.bypass_policy?.groups.length"
                variant="secondary"
              >
                {{ t("admin.streamMappings.policyDraft") }}
              </Badge>
            </div>
          </TableCell>
          <TableCell class="text-right">
            <StreamMappingRowActions
              :mapping="mapping"
              :probing-mapping-key="probingMappingKey"
              :saving="saving"
              @action="requestAction"
              @edit="emit('edit', $event)"
              @probe="emit('probe', $event)"
              @policy="emit('policy', $event)"
              @service="emit('service', $event)"
            />
          </TableCell>
        </TableRow>
      </TableBody>
    </Table>
  </div>
  <SubdomainActionConfirmDialog
    :open="!!pending"
    :loading="saving"
    :title="
      pending ? t(`admin.streamMappings.actions.${pending.action}.label`) : ''
    "
    :description="confirmationDescription"
    :cancel-label="t('common.cancel')"
    :confirm-label="
      pending ? t(`admin.streamMappings.actions.${pending.action}.label`) : ''
    "
    :confirm-variant="pending?.action === 'enable' ? 'default' : 'destructive'"
    @update:open="!$event && cancelAction()"
    @cancel="cancelAction"
    @confirm="confirmAction"
  />
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import InlineCommentEditor from "@admin-shared/components/InlineCommentEditor.vue";
import SearchInput from "@admin-shared/components/SearchInput.vue";
import { ListChecks } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import SubdomainActionConfirmDialog from "../subdomain-proxy/SubdomainActionConfirmDialog.vue";
import { useStreamRuleActions } from "./useStreamRuleActions";
import { Badge } from "@/components/ui/badge";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import type { StreamMapping, StreamTrafficStats } from "../../types";
import StreamTrafficActivity from "@/components/StreamTrafficActivity.vue";
import StreamMappingRowActions from "./StreamMappingRowActions.vue";
import {
  formatMappingLabel,
  formatProtocolLabel,
  getMappingKey,
  streamTrafficKey,
  type StreamRuleAction,
} from "./streamMappingModel";

const props = defineProps<{
  mappings: StreamMapping[];
  probingMappingKey: string | null;
  trafficTimestamp: number | null;
  getStreamTrafficSample: (mapping: StreamMapping) => StreamTrafficStats | null;
  onRuleAction: (
    action: StreamRuleAction,
    rules: readonly StreamMapping[],
  ) => Promise<boolean>;
  onSaveComment: (mapping: StreamMapping, comment: string) => Promise<void>;
}>();
const emit = defineEmits<{
  edit: [mapping: StreamMapping];
  probe: [mapping: StreamMapping];
  policy: [mapping: StreamMapping];
  service: [mapping: StreamMapping];
}>();
const { t } = useI18n();
const searchQuery = ref("");
const filteredMappings = computed(() => {
  const query = searchQuery.value.trim().toLowerCase();
  if (!query) return props.mappings;

  return props.mappings.filter((mapping) => {
    const authStatus = mapping.use_auth
      ? t("admin.streamMappings.authRequired")
      : t("admin.streamMappings.publicAccess");
    return (
      mapping.protocol.includes(query) ||
      formatProtocolLabel(mapping.protocol).toLowerCase().includes(query) ||
      String(mapping.listen_port).includes(query) ||
      (mapping.comment ?? "").toLowerCase().includes(query) ||
      mapping.target.toLowerCase().includes(query) ||
      (mapping.service_profile?.service_id ?? "")
        .toLowerCase()
        .includes(query) ||
      (mapping.service_profile?.device_role ?? "")
        .toLowerCase()
        .includes(query) ||
      authStatus.toLowerCase().includes(query)
    );
  });
});
const ruleActions = ["enable", "disable", "delete"] as const;
const {
  selectionMode,
  selected,
  selectedRules,
  saving,
  allChecked,
  pending,
  clearSelection,
  toggleSelectionMode,
  setSelected,
  setAllVisible,
  affected,
  requestAction,
  cancelAction,
  confirmAction,
} = useStreamRuleActions(filteredMappings, searchQuery, props.onRuleAction);

const confirmationDescription = computed(() => {
  const request = pending.value;
  if (!request) return "";
  const [mapping] = request.rules;
  if (!mapping) return "";
  if (request.batch)
    return t(`admin.streamMappings.actions.${request.action}.batch`, {
      count: request.rules.length,
    });
  if (request.action === "delete")
    return t("admin.streamMappings.deleteDescription", {
      mapping: formatMappingLabel(mapping),
      target: mapping.target,
    });
  return t(`admin.streamMappings.actions.${request.action}.single`, {
    mapping: formatMappingLabel(mapping),
  });
});
</script>
