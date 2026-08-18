<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";

type BlacklistHitRow = {
  key: string | number;
  time: string;
  path: string;
  interval: string;
};

const props = withDefaults(
  defineProps<{
    rows: BlacklistHitRow[];
    emptyText?: string;
    actionHeader?: string;
  }>(),
  {},
);

const { t } = useI18n();
const slots = defineSlots<{
  action?: (props: { row: BlacklistHitRow }) => unknown;
}>();
const hasActions = computed(() => Boolean(slots.action));
const resolvedEmptyText = computed(
  () => props.emptyText || t("admin.components.blacklistHitsTable.empty"),
);
</script>

<template>
  <div class="border rounded-md overflow-hidden">
    <Table class="w-max min-w-full">
      <TableHeader>
        <TableRow>
          <TableHead class="w-[220px]">{{
            t("admin.components.blacklistHitsTable.visitedAt")
          }}</TableHead>
          <TableHead>{{
            t("admin.components.blacklistHitsTable.path")
          }}</TableHead>
          <TableHead class="w-[160px]">{{
            t("admin.components.blacklistHitsTable.interval")
          }}</TableHead>
          <TableHead v-if="hasActions" class="w-[240px] text-right">
            {{
              props.actionHeader ||
              t("admin.components.blacklistHitsTable.actions")
            }}
          </TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        <TableRow v-if="props.rows.length === 0">
          <TableCell
            :colspan="hasActions ? 4 : 3"
            class="text-center text-muted-foreground py-6"
            >{{ resolvedEmptyText }}</TableCell
          >
        </TableRow>
        <TableRow v-else v-for="row in props.rows" :key="row.key">
          <TableCell class="whitespace-nowrap">{{ row.time }}</TableCell>
          <TableCell class="font-mono text-xs">{{ row.path }}</TableCell>
          <TableCell class="whitespace-nowrap text-muted-foreground">{{
            row.interval
          }}</TableCell>
          <TableCell v-if="hasActions" class="text-right">
            <slot name="action" :row="row" />
          </TableCell>
        </TableRow>
      </TableBody>
    </Table>
  </div>
</template>
