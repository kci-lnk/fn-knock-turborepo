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
    <Table>
      <TableHeader>
        <TableRow>
          <TableHead>{{ t("admin.streamMappings.protocol") }}</TableHead>
          <TableHead>{{ t("admin.streamMappings.listenPort") }}</TableHead>
          <TableHead>{{ t("admin.streamMappings.comment") }}</TableHead>
          <TableHead>{{ t("admin.streamMappings.target") }}</TableHead>
          <TableHead>{{ t("admin.streamMappings.authStatus") }}</TableHead>
          <TableHead class="text-right">{{
            t("admin.sessions.table.actions")
          }}</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        <TableRow v-if="filteredMappings.length === 0">
          <TableCell colspan="6" class="py-8 text-center text-muted-foreground">
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
              :save="(value) => onSaveComment(mapping, value)"
            />
          </TableCell>
          <TableCell class="font-mono text-sm">{{ mapping.target }}</TableCell>
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
                variant="outline"
                size="sm"
                @click="emit('edit', mapping)"
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
                :on-confirm="() => onRemove(mapping)"
                content-class="w-72 text-left"
              >
                <template #trigger>
                  <Button
                    variant="destructive-outline"
                    size="sm"
                    :disabled="removingMappingKey === getMappingKey(mapping)"
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
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import InlineCommentEditor from "@admin-shared/components/InlineCommentEditor.vue";
import SearchInput from "@admin-shared/components/SearchInput.vue";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import type { StreamMapping } from "../../types";
import {
  formatMappingLabel,
  formatProtocolLabel,
  getMappingKey,
} from "./streamMappingModel";

const props = defineProps<{
  mappings: StreamMapping[];
  removingMappingKey: string | null;
  onRemove: (mapping: StreamMapping) => Promise<void>;
  onSaveComment: (mapping: StreamMapping, comment: string) => Promise<void>;
}>();
const emit = defineEmits<{ edit: [mapping: StreamMapping] }>();
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
      authStatus.toLowerCase().includes(query)
    );
  });
});
</script>
