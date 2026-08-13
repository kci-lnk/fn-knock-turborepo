<script setup lang="ts">
import { useI18n } from "vue-i18n";
import SearchInput from "@admin-shared/components/SearchInput.vue";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

defineProps<{
  availableDates: string[];
  cursorPageLabel: string;
  entryCount: number;
  selectedDate: string;
  traceFilter: string;
}>();
const searchQuery = defineModel<string>("searchQuery", { required: true });
const emit = defineEmits<{
  dateChange: [value: unknown];
  search: [];
}>();
const { t } = useI18n();
</script>

<template>
  <div class="border-b px-4 py-3">
    <div class="flex flex-col gap-2 md:flex-row md:items-center">
      <SearchInput
        v-model="searchQuery"
        :placeholder="t('admin.wafLogs.searchPlaceholder')"
        class="w-full md:w-[320px] md:max-w-[320px]"
        @search="emit('search')"
      />

      <Select :model-value="selectedDate" @update:model-value="emit('dateChange', $event)">
        <div class="w-[148px]">
          <SelectTrigger :aria-label="t('admin.wafLogs.datePlaceholder')">
            <SelectValue :placeholder="t('admin.wafLogs.datePlaceholder')" />
          </SelectTrigger>
        </div>
        <SelectContent>
          <SelectItem v-for="date in availableDates" :key="date" :value="date">
            {{ date }}
          </SelectItem>
        </SelectContent>
      </Select>
    </div>

    <div
      class="mt-2 flex flex-wrap items-center gap-x-3 gap-y-1 text-[11px] text-muted-foreground"
    >
      <span>
        {{ cursorPageLabel }} ·
        {{ t("admin.wafLogs.rowsCount", { count: entryCount }) }}
      </span>
      <span v-if="traceFilter.trim()" class="font-mono">
        {{ t("admin.wafLogs.traceFilter", { trace: traceFilter.trim() }) }}
      </span>
      <span v-if="searchQuery.trim()">
        {{ t("admin.wafLogs.keywordFilter", { keyword: searchQuery.trim() }) }}
      </span>
    </div>
  </div>
</template>
