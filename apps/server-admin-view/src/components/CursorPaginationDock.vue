<script setup lang="ts">
import { ChevronLeft, ChevronRight, ChevronsLeft } from "lucide-vue-next";
import FloatingActionDock from "@admin-shared/components/common/FloatingActionDock.vue";
import { Button } from "@/components/ui/button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import type { CursorPaginationLabels } from "./cursor-pagination-contract";

defineProps<{
  canLoadNewer: boolean;
  canLoadOlder: boolean;
  cursorPageLabel: string;
  handleLimitChange: (value: unknown) => Promise<void> | void;
  handleLoadFirst: () => Promise<void> | void;
  handleLoadNewer: () => Promise<void> | void;
  handleLoadOlder: () => Promise<void> | void;
  labels: CursorPaginationLabels;
  limit: string;
  limitOptions: readonly string[];
  loading: boolean;
  shouldFloat: boolean;
}>();
</script>

<template>
  <FloatingActionDock
    :active="shouldFloat"
    :keep-visible="loading && shouldFloat"
    :keep-visible-release-delay="600"
    align="center"
    variant="surface"
    :visible-threshold="0.4"
    :aria-label="labels.ariaLabel"
    floating-class="min-w-0 max-w-[calc(100vw-2rem)] rounded-[1.25rem] p-2"
  >
    <template #inline>
      <div class="border-t px-3 py-3 sm:px-4">
        <div
          class="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between"
        >
          <div
            class="flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-muted-foreground"
          >
            <span>{{ cursorPageLabel }}</span>
            <span>{{ canLoadOlder ? labels.canLoadOlder : labels.lastPage }}</span>
          </div>

          <div class="flex flex-wrap items-center justify-end gap-2">
            <Button
              variant="outline"
              class="h-8 px-2.5 sm:px-3"
              :aria-label="labels.firstPage"
              :disabled="loading || !canLoadNewer"
              @click="handleLoadFirst"
            >
              <ChevronsLeft class="h-4 w-4 sm:mr-1.5" />
              <span class="hidden sm:inline">{{ labels.firstPage }}</span>
            </Button>
            <Button
              variant="outline"
              class="h-8 px-2.5 sm:px-3"
              :aria-label="labels.previousPage"
              :disabled="loading || !canLoadNewer"
              @click="handleLoadNewer"
            >
              <ChevronLeft class="h-4 w-4 sm:mr-1.5" />
              <span class="hidden sm:inline">{{ labels.previousPage }}</span>
            </Button>
            <Button
              class="h-8 px-3"
              :disabled="loading || !canLoadOlder"
              @click="handleLoadOlder"
            >
              {{ labels.nextPage }}
              <ChevronRight class="ml-1.5 h-4 w-4" />
            </Button>

            <div
              class="flex items-center gap-2 text-xs text-muted-foreground sm:ml-1"
            >
              <span>{{ labels.pageSize }}</span>
              <Select :model-value="limit" @update:model-value="handleLimitChange">
                <div class="w-[96px]">
                  <SelectTrigger :aria-label="labels.pageSize">
                    <SelectValue />
                  </SelectTrigger>
                </div>
                <SelectContent>
                  <SelectItem
                    v-for="option in limitOptions"
                    :key="option"
                    :value="option"
                  >
                    {{ labels.pageSizeOption(option) }}
                  </SelectItem>
                </SelectContent>
              </Select>
            </div>
          </div>
        </div>
      </div>
    </template>

    <template #floating>
      <div class="floating-cursor-pagination">
        <div class="floating-cursor-pagination__controls">
          <Button
            variant="ghost"
            class="floating-cursor-pagination__button"
            :aria-label="labels.firstPage"
            :disabled="loading || !canLoadNewer"
            @click="handleLoadFirst"
          >
            <ChevronsLeft class="h-4 w-4" />
            <span class="hidden sm:inline">{{ labels.firstPage }}</span>
          </Button>
          <Button
            variant="ghost"
            class="floating-cursor-pagination__button"
            :aria-label="labels.previousPage"
            :disabled="loading || !canLoadNewer"
            @click="handleLoadNewer"
          >
            <ChevronLeft class="h-4 w-4" />
            <span class="hidden sm:inline">{{ labels.previousPage }}</span>
          </Button>
          <Button
            variant="ghost"
            class="floating-cursor-pagination__button is-primary"
            :disabled="loading || !canLoadOlder"
            @click="handleLoadOlder"
          >
            <span>{{ labels.nextPage }}</span>
            <ChevronRight class="h-4 w-4" />
          </Button>

          <Select :model-value="limit" @update:model-value="handleLimitChange">
            <SelectTrigger
              :aria-label="labels.pageSize"
              class="h-9 w-[84px] rounded-xl border-white/10 bg-white/10 text-white shadow-none hover:bg-white/15 focus-visible:border-white/30 focus-visible:ring-white/20 [&_svg]:text-white"
            >
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem
                v-for="option in limitOptions"
                :key="option"
                :value="option"
              >
                {{ labels.pageSizeOption(option) }}
              </SelectItem>
            </SelectContent>
          </Select>
        </div>
      </div>
    </template>
  </FloatingActionDock>
</template>

<style scoped>
.floating-cursor-pagination {
  display: flex;
  max-width: calc(100vw - 3rem);
  flex-wrap: wrap;
  align-items: center;
  justify-content: center;
  gap: 0.6rem 0.8rem;
}

.floating-cursor-pagination__controls {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: center;
  gap: 0.35rem;
}

:deep(.floating-cursor-pagination__button) {
  height: 2.25rem;
  min-width: 2.25rem;
  border-color: transparent;
  border-radius: 0.8rem;
  background: transparent;
  padding-inline: 0.7rem;
  color: rgb(255 255 255 / 82%);
  box-shadow: none;
}

:deep(.floating-cursor-pagination__button:hover) {
  background: rgb(255 255 255 / 12%);
  color: #fff;
}

:deep(.floating-cursor-pagination__button.is-primary) {
  background: #fff;
  color: #09090b;
}

:deep(.floating-cursor-pagination__button.is-primary:hover) {
  background: rgb(255 255 255 / 92%);
  color: #09090b;
}

:deep(.floating-cursor-pagination__button:disabled) {
  background: transparent;
  color: rgb(255 255 255 / 28%);
}

:deep(.floating-cursor-pagination__button.is-primary:disabled) {
  background: rgb(255 255 255 / 18%);
  color: rgb(255 255 255 / 38%);
}
</style>
