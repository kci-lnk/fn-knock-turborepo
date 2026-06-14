<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import {
  Pagination,
  PaginationContent,
  PaginationEllipsis,
  PaginationFirst,
  PaginationItem,
  PaginationNext,
  PaginationPrevious,
} from "@/components/ui/pagination";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

const props = withDefaults(
  defineProps<{
    total: number;
    page: number;
    limit: string;
    itemsPerPage: number;
    pageSizeOptions?: string[];
    totalText?: string;
  }>(),
  {
    pageSizeOptions: () => ["10", "20", "50", "100"],
  },
);

const { t } = useI18n();

const emit = defineEmits<{
  "update:page": [value: number];
  "update:limit": [value: string];
}>();

const currentLimit = computed({
  get: () => props.limit,
  set: (value: string) => {
    emit("update:limit", value);
  },
});

const handlePageUpdate = (value: number) => {
  emit("update:page", value);
};

const totalLabel = computed(() =>
  t("shared.pagedTableFooter.total", {
    total: props.total,
    itemText: props.totalText ?? t("shared.pagedTableFooter.records"),
  }),
);
</script>

<template>
  <div
    class="p-4 border-t flex items-center justify-between flex-shrink-0 bg-background"
  >
    <div class="text-sm text-muted-foreground">
      {{ totalLabel }}
    </div>
    <div class="flex items-center gap-6">
      <div class="flex items-center gap-2 text-sm">
        <Select v-model="currentLimit">
          <SelectTrigger class="w-[80px]">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem
              v-for="option in props.pageSizeOptions"
              :key="option"
              :value="option"
            >
              {{ option }}
            </SelectItem>
          </SelectContent>
        </Select>
      </div>

      <Pagination
        v-slot="{ page: currentPage }"
        :total="props.total"
        :sibling-count="1"
        show-edges
        :default-page="1"
        :items-per-page="props.itemsPerPage"
        :page="props.page"
        @update:page="handlePageUpdate"
      >
        <PaginationContent v-slot="{ items }" class="flex items-center gap-1">
          <PaginationFirst />
          <PaginationPrevious />
          <template v-for="(item, index) in items" :key="index">
            <PaginationItem
              v-if="item.type === 'page'"
              :value="item.value"
              :isActive="item.value === currentPage"
              as-child
            >
              {{ item.value }}
            </PaginationItem>
            <PaginationEllipsis v-else :index="index" />
          </template>
          <PaginationNext />
        </PaginationContent>
      </Pagination>
    </div>
  </div>
</template>
