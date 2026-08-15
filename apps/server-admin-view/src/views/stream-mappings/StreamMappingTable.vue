<template>
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
    <Table class="min-w-[70rem] table-fixed">
      <colgroup>
        <col class="w-[8%]" />
        <col class="w-[10%]" />
        <col class="w-[16%]" />
        <col class="w-[18%]" />
        <col class="w-[22%]" />
        <col class="w-[14%]" />
        <col class="w-[12%]" />
      </colgroup>
      <TableHeader>
        <TableRow>
          <TableHead>{{ t("admin.streamMappings.protocol") }}</TableHead>
          <TableHead>{{ t("admin.streamMappings.listenPort") }}</TableHead>
          <TableHead>{{ t("admin.streamMappings.comment") }}</TableHead>
          <TableHead>{{ t("admin.streamMappings.target") }}</TableHead>
          <TableHead>{{ t("admin.streamMappings.serviceProfile") }}</TableHead>
          <TableHead>{{ t("admin.streamMappings.authStatus") }}</TableHead>
          <TableHead class="text-right">{{
            t("admin.sessions.table.actions")
          }}</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        <TableRow v-if="filteredMappings.length === 0">
          <TableCell colspan="7" class="py-8 text-center text-muted-foreground">
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
              :removing-mapping-key="removingMappingKey"
              :on-remove="onRemove"
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
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import InlineCommentEditor from "@admin-shared/components/InlineCommentEditor.vue";
import SearchInput from "@admin-shared/components/SearchInput.vue";
import { Badge } from "@/components/ui/badge";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import type { StreamMapping } from "../../types";
import StreamMappingRowActions from "./StreamMappingRowActions.vue";
import { formatProtocolLabel, getMappingKey } from "./streamMappingModel";

const props = defineProps<{
  mappings: StreamMapping[];
  removingMappingKey: string | null;
  probingMappingKey: string | null;
  onRemove: (mapping: StreamMapping) => Promise<boolean>;
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
</script>
