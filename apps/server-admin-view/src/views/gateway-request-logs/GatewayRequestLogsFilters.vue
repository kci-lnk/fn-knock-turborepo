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
import {
  LOGIN_FILTER_OPTIONS,
  STATUS_FILTER_OPTIONS,
  WAF_FILTER_OPTIONS,
} from "./model";

const { searchQuery } = defineProps<{
  activeCredentialLabel: string;
  activeLoggedInLabel: string;
  activeStatusLabel: string;
  activeWafStatusLabel: string;
  availableDates: string[];
  credentialOptions: Array<{ label: string; value: string }>;
  cursorPageLabel: string;
  entriesCount: number;
  handleCredentialChange: (value: unknown) => Promise<void> | void;
  handleDateChange: (value: unknown) => Promise<void> | void;
  handleLoggedInChange: (value: unknown) => Promise<void> | void;
  handleSearch: () => Promise<void> | void;
  handleStatusChange: (value: unknown) => Promise<void> | void;
  handleWafStatusChange: (value: unknown) => Promise<void> | void;
  logsDir: string;
  searchQuery: string;
  selectedCredential: string;
  selectedDate: string;
  selectedLoggedIn: string;
  selectedStatus: string;
  selectedWafStatus: string;
}>();
const emit = defineEmits<{ "update:searchQuery": [value: string] }>();
const { t } = useI18n();
</script>

<template>
  <div class="border-b px-3 py-3 sm:px-4">
    <div class="flex flex-col gap-2 lg:flex-row lg:items-start">
      <SearchInput
        :model-value="searchQuery"
        :placeholder="t('admin.gatewayRequestLogs.searchPlaceholder')"
        class="w-full min-w-0 sm:w-[320px] lg:shrink-0"
        @update:model-value="emit('update:searchQuery', $event)"
        @search="handleSearch"
      />

      <div
        class="grid min-w-0 flex-1 grid-cols-2 items-center gap-2 sm:flex sm:flex-wrap sm:justify-end"
      >
        <Select :model-value="selectedDate" @update:model-value="handleDateChange">
          <div class="order-1 w-full min-w-0 sm:order-none sm:w-[148px]">
            <SelectTrigger
              :aria-label="t('admin.gatewayRequestLogs.datePlaceholder')"
              class="w-full min-w-0"
            >
              <SelectValue :placeholder="t('admin.gatewayRequestLogs.datePlaceholder')" />
            </SelectTrigger>
          </div>
          <SelectContent>
            <SelectItem v-for="date in availableDates" :key="date" :value="date">
              {{ date }}
            </SelectItem>
          </SelectContent>
        </Select>

        <Select
          :model-value="selectedStatus"
          @update:model-value="handleStatusChange"
        >
          <div class="order-2 w-full min-w-0 sm:order-none sm:w-[156px]">
            <SelectTrigger
              :aria-label="t('admin.gatewayRequestLogs.statusPlaceholder')"
              class="w-full min-w-0"
            >
              <SelectValue :placeholder="t('admin.gatewayRequestLogs.statusPlaceholder')" />
            </SelectTrigger>
          </div>
          <SelectContent>
            <SelectItem
              v-for="option in STATUS_FILTER_OPTIONS"
              :key="option.value"
              :value="option.value"
            >
              {{ t(option.labelKey) }}
            </SelectItem>
          </SelectContent>
        </Select>

        <Select
          :model-value="selectedLoggedIn"
          @update:model-value="handleLoggedInChange"
        >
          <div class="order-3 w-full min-w-0 sm:order-none sm:w-[168px]">
            <SelectTrigger
              :aria-label="t('admin.gatewayRequestLogs.loginPlaceholder')"
              class="w-full min-w-0"
            >
              <SelectValue :placeholder="t('admin.gatewayRequestLogs.loginPlaceholder')" />
            </SelectTrigger>
          </div>
          <SelectContent>
            <SelectItem
              v-for="option in LOGIN_FILTER_OPTIONS"
              :key="option.value"
              :value="option.value"
            >
              {{ t(option.labelKey) }}
            </SelectItem>
          </SelectContent>
        </Select>

        <Select
          :model-value="selectedCredential"
          @update:model-value="handleCredentialChange"
        >
          <div
            class="order-5 col-span-2 w-full min-w-0 sm:order-none sm:col-span-1 sm:w-[220px]"
          >
            <SelectTrigger
              :aria-label="t('admin.gatewayRequestLogs.credentialPlaceholder')"
              class="w-full min-w-0"
            >
              <SelectValue
                :placeholder="t('admin.gatewayRequestLogs.credentialPlaceholder')"
              />
            </SelectTrigger>
          </div>
          <SelectContent class="max-w-[min(28rem,calc(100vw-2rem))]">
            <SelectItem
              v-for="option in credentialOptions"
              :key="option.value"
              :value="option.value"
              class="min-w-0"
            >
              <span class="block max-w-[22rem] truncate" :title="option.label">
                {{ option.label }}
              </span>
            </SelectItem>
          </SelectContent>
        </Select>

        <Select
          :model-value="selectedWafStatus"
          @update:model-value="handleWafStatusChange"
        >
          <div class="order-4 w-full min-w-0 sm:order-none sm:w-[144px]">
            <SelectTrigger
              :aria-label="t('admin.gatewayRequestLogs.wafPlaceholder')"
              class="w-full min-w-0"
            >
              <SelectValue :placeholder="t('admin.gatewayRequestLogs.wafPlaceholder')" />
            </SelectTrigger>
          </div>
          <SelectContent>
            <SelectItem
              v-for="option in WAF_FILTER_OPTIONS"
              :key="option.value"
              :value="option.value"
            >
              {{ t(option.labelKey) }}
            </SelectItem>
          </SelectContent>
        </Select>
      </div>
    </div>

    <div
      class="mt-2 flex flex-wrap items-center gap-x-3 gap-y-1 text-[11px] text-muted-foreground"
    >
      <span>
        {{ cursorPageLabel }} ·
        {{ t("admin.gatewayRequestLogs.rowsCount", { count: entriesCount }) }}
      </span>
      <span>{{ activeStatusLabel }}</span>
      <span>{{ activeLoggedInLabel }}</span>
      <span class="max-w-[220px] truncate" :title="activeCredentialLabel">
        {{ activeCredentialLabel }}
      </span>
      <span>{{ activeWafStatusLabel }}</span>
      <span v-if="searchQuery.trim()">
        {{
          t("admin.gatewayRequestLogs.keywordFilter", {
            keyword: searchQuery.trim(),
          })
        }}
      </span>
      <span class="hidden break-all sm:inline">
        {{
          t("admin.gatewayRequestLogs.directoryLabel", {
            directory: logsDir || "-",
          })
        }}
      </span>
    </div>
  </div>
</template>
