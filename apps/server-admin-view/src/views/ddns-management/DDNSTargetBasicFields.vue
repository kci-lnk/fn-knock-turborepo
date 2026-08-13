<script setup lang="ts">
import { useId } from "vue";
import { useI18n } from "vue-i18n";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import type { Provider, TargetDialogState } from "./model";

defineProps<{
  providers: Provider[];
  state: TargetDialogState;
}>();
const emit = defineEmits<{ "update:provider": [value: string] }>();
const a11yId = useId();
const { t } = useI18n();
</script>

<template>
  <div
    class="grid items-start gap-3 p-4 transition-colors hover:bg-muted/10 sm:grid-cols-[180px_1fr] sm:p-5 md:grid-cols-[220px_1fr]"
  >
    <div class="space-y-1 sm:mt-1.5">
      <Label :for="`${a11yId}-enabled`" class="text-sm font-medium">
        {{ t("admin.ddns.targetEnabledLabel") }}
      </Label>
      <p class="hidden pr-4 text-xs text-muted-foreground sm:block">
        {{ t("admin.ddns.targetEnabledHint") }}
      </p>
    </div>
    <div class="w-full max-w-md space-y-2 sm:justify-self-end">
      <div
        class="flex min-h-10 w-full items-center justify-start gap-3 sm:justify-end sm:px-3"
      >
        <Switch :id="`${a11yId}-enabled`" v-model="state.enabled" />
        <span class="text-sm text-muted-foreground">
          {{
            state.enabled
              ? t("admin.ddns.activeLabel")
              : t("admin.ddns.stoppedLabel")
          }}
        </span>
      </div>
      <p class="text-[11px] text-muted-foreground sm:hidden">
        {{ t("admin.ddns.targetEnabledHint") }}
      </p>
    </div>
  </div>

  <div
    class="grid items-start gap-2 p-4 transition-colors hover:bg-muted/10 sm:grid-cols-[180px_1fr] sm:p-5 md:grid-cols-[220px_1fr]"
  >
    <div class="mt-1.5 space-y-1">
      <Label for="ddns-target-name" class="text-sm font-medium">
        {{ t("admin.ddns.name") }}
      </Label>
      <p class="hidden pr-4 text-xs text-muted-foreground sm:block">
        {{ t("admin.ddns.targetNameHint") }}
      </p>
    </div>
    <div class="w-full max-w-md space-y-2">
      <Input
        id="ddns-target-name"
        v-model="state.name"
        :placeholder="t('admin.ddns.targetNamePlaceholder')"
      />
      <p class="text-[11px] text-muted-foreground sm:hidden">
        {{ t("admin.ddns.targetNameHint") }}
      </p>
    </div>
  </div>

  <div
    class="grid items-start gap-2 p-4 transition-colors hover:bg-muted/10 sm:grid-cols-[180px_1fr] sm:p-5 md:grid-cols-[220px_1fr]"
  >
    <div class="mt-1.5 space-y-1">
      <Label for="ddns-target-provider" class="text-sm font-medium">
        {{ t("admin.ddns.providerLabel") }}
      </Label>
      <p class="hidden pr-4 text-xs text-muted-foreground sm:block">
        {{ t("admin.ddns.targetProviderHint") }}
      </p>
    </div>
    <div class="w-full max-w-md space-y-2">
      <Select
        :model-value="state.provider"
        @update:model-value="emit('update:provider', String($event ?? ''))"
      >
        <SelectTrigger id="ddns-target-provider">
          <SelectValue :placeholder="t('admin.ddns.selectProvider')" />
        </SelectTrigger>
        <SelectContent>
          <SelectItem
            v-for="provider in providers"
            :key="provider.name"
            :value="provider.name"
          >
            {{ provider.label }}
          </SelectItem>
        </SelectContent>
      </Select>
      <p class="text-[11px] text-muted-foreground sm:hidden">
        {{ t("admin.ddns.targetProviderHint") }}
      </p>
    </div>
  </div>
</template>
