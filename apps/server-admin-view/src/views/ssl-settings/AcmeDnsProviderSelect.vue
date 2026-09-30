<script setup lang="ts">
import { useI18n } from "vue-i18n";
import type { AcmeDnsProvider } from "@/lib/api/acme";
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectLabel,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

defineProps<{
  id: string;
  activeDnsType: string;
  pending?: boolean;
  groupedProviders: {
    group: string;
    groupKey: string;
    items: AcmeDnsProvider[];
  }[];
}>();
const dnsType = defineModel<string>({ required: true });
const { t } = useI18n();
</script>

<template>
  <div class="grid gap-2">
    <div class="flex items-center justify-between gap-3">
      <label :for="id" class="text-sm text-muted-foreground">
        {{ t("admin.acmeApplicationDialog.dnsProvider") }}
      </label>
      <span
        v-if="activeDnsType"
        class="text-xs font-mono text-muted-foreground"
      >
        {{ activeDnsType }}
      </span>
    </div>
    <Select v-model="dnsType" :disabled="pending">
      <SelectTrigger :id="id" class="w-full">
        <SelectValue
          :placeholder="t('admin.acmeApplicationDialog.selectDnsProvider')"
        />
      </SelectTrigger>
      <SelectContent class="max-h-[320px]">
        <SelectGroup v-for="group in groupedProviders" :key="group.groupKey">
          <SelectLabel>{{ group.group }}</SelectLabel>
          <SelectItem
            v-for="provider in group.items"
            :key="provider.dnsType"
            :value="provider.dnsType"
          >
            <div class="flex w-full items-center justify-between gap-3">
              <span class="truncate">{{ provider.label }}</span>
              <span class="shrink-0 font-mono text-xs text-muted-foreground">
                {{ provider.dnsType }}
              </span>
            </div>
          </SelectItem>
        </SelectGroup>
      </SelectContent>
    </Select>
  </div>
</template>
