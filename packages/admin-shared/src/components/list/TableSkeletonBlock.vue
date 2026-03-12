<script setup lang="ts">
import { Skeleton } from '@/components/ui/skeleton';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table';

const props = withDefaults(
  defineProps<{
    headerWidths: string[];
    rowWidths: string[];
    rowCount?: number;
    showToolbar?: boolean;
    toolbarLeftWidth?: string;
    toolbarRightWidths?: string[];
    actionColumn?: boolean;
    actionWidth?: string;
  }>(),
  {
    rowCount: 8,
    showToolbar: true,
    toolbarLeftWidth: 'w-60',
    toolbarRightWidths: () => ['w-24', 'w-24'],
    actionColumn: true,
    actionWidth: 'w-16',
  },
);
</script>

<template>
  <div class="p-4">
    <div v-if="props.showToolbar" class="flex items-center justify-between mb-3">
      <Skeleton :class="['h-9', props.toolbarLeftWidth]" />
      <div class="flex items-center gap-2">
        <Skeleton v-for="(width, idx) in props.toolbarRightWidths" :key="idx" :class="['h-9', width]" />
      </div>
    </div>

    <Table>
      <TableHeader class="sticky top-0 bg-background z-10 shadow-sm">
        <TableRow>
          <TableHead v-for="(width, idx) in props.headerWidths" :key="idx" :class="idx === props.headerWidths.length - 1 ? 'text-right pr-6' : ''">
            <Skeleton :class="['h-4', width, idx === props.headerWidths.length - 1 ? 'ml-auto' : '']" />
          </TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        <TableRow v-for="n in props.rowCount" :key="n">
          <TableCell
            v-for="(width, idx) in props.rowWidths"
            :key="idx"
            :class="props.actionColumn && idx === props.rowWidths.length - 1 ? 'text-right' : ''"
          >
            <Skeleton
              :class="[
                props.actionColumn && idx === props.rowWidths.length - 1
                  ? `h-8 ${props.actionWidth} rounded-md ml-auto`
                  : `h-4 ${width}`,
              ]"
            />
          </TableCell>
        </TableRow>
      </TableBody>
    </Table>
  </div>
</template>
