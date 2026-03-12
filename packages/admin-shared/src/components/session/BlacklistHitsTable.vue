<script setup lang="ts">
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table';

type BlacklistHitRow = {
  key: string | number;
  time: string;
  path: string;
  interval: string;
};

const props = withDefaults(defineProps<{
  rows: BlacklistHitRow[];
  emptyText?: string;
}>(), {
  emptyText: '暂无访问记录',
});
</script>

<template>
  <div class="border rounded-md overflow-hidden">
    <Table class="w-max min-w-full">
      <TableHeader>
        <TableRow>
          <TableHead class="w-[220px]">访问时间</TableHead>
          <TableHead>路径</TableHead>
          <TableHead class="w-[160px]">间隔</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        <TableRow v-if="props.rows.length === 0">
          <TableCell colspan="3" class="text-center text-muted-foreground py-6">{{ props.emptyText }}</TableCell>
        </TableRow>
        <TableRow v-else v-for="row in props.rows" :key="row.key">
          <TableCell class="whitespace-nowrap">{{ row.time }}</TableCell>
          <TableCell class="font-mono text-xs">{{ row.path }}</TableCell>
          <TableCell class="whitespace-nowrap text-muted-foreground">{{ row.interval }}</TableCell>
        </TableRow>
      </TableBody>
    </Table>
  </div>
</template>
