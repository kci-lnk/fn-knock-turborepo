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
import FloatingActionDock from "../common/FloatingActionDock.vue";

const props = withDefaults(
  defineProps<{
    total: number;
    page: number;
    limit: string;
    itemsPerPage: number;
    pageSizeOptions?: string[];
    totalText?: string;
    floating?: boolean;
    floatingAriaLabel?: string;
  }>(),
  {
    pageSizeOptions: () => ["10", "20", "50", "100"],
    floating: false,
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

const shouldFloatFooter = computed(() => props.floating && props.total > 0);
</script>

<template>
  <FloatingActionDock
    v-if="shouldFloatFooter"
    :active="true"
    align="center"
    variant="surface"
    :visible-threshold="0.4"
    :aria-label="props.floatingAriaLabel ?? totalLabel"
    floating-class="min-w-0 max-w-[calc(100vw-2rem)] rounded-[1.25rem] p-2"
  >
    <template #inline>
      <div
        class="flex flex-shrink-0 items-center justify-between border-t bg-background p-4"
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
            <PaginationContent
              v-slot="{ items }"
              class="flex items-center gap-1"
            >
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

    <template #floating>
      <div class="paged-table-footer-floating">
        <div class="paged-table-footer-floating__controls">
          <Pagination
            :total="props.total"
            :sibling-count="1"
            show-edges
            :default-page="1"
            :items-per-page="props.itemsPerPage"
            :page="props.page"
            @update:page="handlePageUpdate"
          >
            <PaginationContent
              class="paged-table-footer-floating__pagination"
            >
              <PaginationFirst class="floating-page-control" />
              <PaginationPrevious
                class="floating-page-control"
              />
              <PaginationNext class="floating-page-control is-primary" />
            </PaginationContent>
          </Pagination>

          <Select v-model="currentLimit">
            <SelectTrigger
              class="h-9 w-[92px] rounded-xl border-white/10 bg-white/10 text-white shadow-none hover:bg-white/15 focus-visible:border-white/30 focus-visible:ring-white/20 [&_svg]:text-white"
            >
              <SelectValue>
                {{
                  t("shared.pagedTableFooter.pageSizeOption", {
                    count: currentLimit,
                  })
                }}
              </SelectValue>
            </SelectTrigger>
            <SelectContent>
              <SelectItem
                v-for="option in props.pageSizeOptions"
                :key="option"
                :value="option"
              >
                {{
                  t("shared.pagedTableFooter.pageSizeOption", {
                    count: option,
                  })
                }}
              </SelectItem>
            </SelectContent>
          </Select>
        </div>
      </div>
    </template>
  </FloatingActionDock>

  <div
    v-else
    class="flex flex-shrink-0 items-center justify-between border-t bg-background p-4"
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

<style scoped>
.paged-table-footer-floating {
  display: flex;
  max-width: calc(100vw - 3rem);
  flex-wrap: nowrap;
  align-items: center;
  justify-content: center;
  gap: 0.55rem;
  color: rgb(255 255 255 / 88%);
}

.paged-table-footer-floating__controls,
.paged-table-footer-floating__pagination {
  display: flex;
  flex-wrap: nowrap;
  align-items: center;
  justify-content: center;
  gap: 0.35rem;
}

:deep(.floating-page-control) {
  min-width: 5.2rem;
  height: 2.25rem;
  border-color: transparent;
  border-radius: 0.9rem;
  background: transparent;
  color: rgb(255 255 255 / 82%);
  box-shadow: none;
  padding-inline: 0.75rem;
}

:deep(.floating-page-control:hover) {
  background: rgb(255 255 255 / 12%);
  color: #fff;
}

:deep(.floating-page-control.is-primary) {
  min-width: 6rem;
  background: #fff;
  color: #09090b;
}

:deep(.floating-page-control.is-primary:hover) {
  background: rgb(255 255 255 / 92%);
  color: #09090b;
}

:deep(.floating-page-control:disabled),
:deep(.floating-page-control[disabled]),
:deep(.floating-page-control[data-disabled]),
:deep(.floating-page-control[aria-disabled="true"]) {
  opacity: 0.46;
}

@media (max-width: 640px) {
  .paged-table-footer-floating {
    max-width: calc(100vw - 2rem);
    gap: 0.35rem;
  }

  :deep(.floating-page-control) {
    min-width: 2.2rem;
    height: 2.1rem;
    padding-inline: 0;
  }

  :deep(.floating-page-control span) {
    display: none;
  }

  :deep(.floating-page-control.is-primary) {
    min-width: 3.1rem;
  }
}
</style>
