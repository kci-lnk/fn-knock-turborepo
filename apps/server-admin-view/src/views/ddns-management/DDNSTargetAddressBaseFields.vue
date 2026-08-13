<script setup lang="ts">
import { useI18n } from "vue-i18n";
import OverflowTooltipText from "@admin-shared/components/common/OverflowTooltipText.vue";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import type { DDNSTargetAddressFieldsProps } from "./ddns-target-dialog-contract";
import {
  ALLOW_PRIVATE_ADDRESSES_KEY,
  DEFAULT_DDNS_IP_SOURCE,
  DEFAULT_DDNS_UPDATE_SCOPE,
  IP_SOURCE_KEY,
  IP_SOURCE_OPTIONS,
  NETWORK_INTERFACE_AUTO_VALUE,
  NETWORK_INTERFACE_KEY,
  UPDATE_SCOPE_KEY,
  UPDATE_SCOPE_OPTIONS,
  normalizeIpSource,
  normalizeUpdateScope,
  toNetworkInterfaceSelectValue,
} from "./model";

defineProps<{ model: DDNSTargetAddressFieldsProps }>();
const emit = defineEmits<{ "update:networkInterface": [value: string] }>();
const { t } = useI18n();
</script>

<template>
  <div
    class="grid items-start gap-2 p-4 transition-colors hover:bg-muted/10 sm:grid-cols-[180px_1fr] sm:p-5 md:grid-cols-[220px_1fr]"
  >
    <div class="mt-1.5 space-y-1">
      <Label for="ddns-target-update-scope" class="text-sm font-medium">
        {{ t("admin.ddns.updateScopeLabel") }}
      </Label>
      <p class="hidden pr-4 text-xs text-muted-foreground sm:block">
        {{ t("admin.ddns.updateScopeHint") }}
      </p>
    </div>
    <div class="w-full max-w-md space-y-2">
      <Select
        :model-value="
          model.state.config[UPDATE_SCOPE_KEY] || DEFAULT_DDNS_UPDATE_SCOPE
        "
        @update:model-value="
          model.state.config[UPDATE_SCOPE_KEY] = normalizeUpdateScope(
            String($event ?? ''),
          )
        "
      >
        <SelectTrigger id="ddns-target-update-scope"><SelectValue /></SelectTrigger>
        <SelectContent>
          <SelectItem
            v-for="option in UPDATE_SCOPE_OPTIONS"
            :key="option.value"
            :value="option.value"
            :disabled="
              model.isUpdateScopeOptionDisabled(
                model.state.provider,
                option.value,
              )
            "
          >
            {{ model.formatOptionLabel(option) }}
          </SelectItem>
        </SelectContent>
      </Select>
      <p class="text-[11px] text-muted-foreground sm:hidden">
        {{ t("admin.ddns.updateScopeHint") }}
      </p>
    </div>
  </div>

  <div
    class="grid items-start gap-2 p-4 transition-colors hover:bg-muted/10 sm:grid-cols-[180px_1fr] sm:p-5 md:grid-cols-[220px_1fr]"
  >
    <div class="mt-1.5 space-y-1">
      <Label for="ddns-target-ip-source" class="text-sm font-medium">
        {{ t("admin.ddns.ipSourceLabel") }}
      </Label>
      <p class="hidden pr-4 text-xs text-muted-foreground sm:block">
        {{ t("admin.ddns.ipSourceHint") }}
      </p>
    </div>
    <div class="w-full max-w-md space-y-2">
      <Select
        :model-value="
          model.state.config[IP_SOURCE_KEY] || DEFAULT_DDNS_IP_SOURCE
        "
        @update:model-value="
          model.state.config[IP_SOURCE_KEY] = normalizeIpSource(
            String($event ?? ''),
          )
        "
      >
        <SelectTrigger id="ddns-target-ip-source"><SelectValue /></SelectTrigger>
        <SelectContent>
          <SelectItem
            v-for="option in IP_SOURCE_OPTIONS"
            :key="option.value"
            :value="option.value"
            :disabled="
              model.isIpSourceOptionDisabled(
                model.state.provider,
                option.value,
              )
            "
          >
            {{ model.formatOptionLabel(option) }}
          </SelectItem>
        </SelectContent>
      </Select>
      <p
        v-if="model.shouldShowInterfaceBlock"
        class="text-[11px] text-muted-foreground"
      >
        {{
          t(
            model.state.config[ALLOW_PRIVATE_ADDRESSES_KEY] === "true"
              ? "admin.ddns.interfacePrivateFilterOn"
              : "admin.ddns.interfaceOnlyFiltered",
          )
        }}
      </p>
    </div>
  </div>

  <div
    class="grid items-start gap-2 p-4 transition-colors hover:bg-muted/10 sm:grid-cols-[180px_1fr] sm:p-5 md:grid-cols-[220px_1fr]"
  >
    <div class="mt-1.5 space-y-1">
      <Label for="ddns-target-network-interface" class="text-sm font-medium">
        {{ t("admin.ddns.outboundInterface") }}
      </Label>
      <p class="hidden pr-4 text-xs text-muted-foreground sm:block">
        {{ t("admin.ddns.interfaceHint") }}
      </p>
    </div>
    <div class="w-full min-w-0 max-w-full space-y-2 sm:max-w-md">
      <Select
        :model-value="
          toNetworkInterfaceSelectValue(
            model.state.config[NETWORK_INTERFACE_KEY],
          )
        "
        @update:model-value="
          emit(
            'update:networkInterface',
            $event === NETWORK_INTERFACE_AUTO_VALUE ? '' : String($event ?? ''),
          )
        "
      >
        <SelectTrigger
          id="ddns-target-network-interface"
          class="w-full min-w-0 max-w-full overflow-hidden"
        >
          <SelectValue
            :placeholder="t('admin.ddns.autoSelect')"
            class="min-w-0 flex-1 overflow-hidden"
          >
            <span class="block w-full min-w-0 max-w-full truncate text-left">
              {{ model.networkInterfaceLabel }}
            </span>
          </SelectValue>
        </SelectTrigger>
        <SelectContent
          class="w-[var(--reka-select-trigger-width)] max-w-[min(32rem,calc(100vw-2rem))]"
        >
          <SelectItem :value="NETWORK_INTERFACE_AUTO_VALUE">
            {{ t("admin.ddns.autoSelect") }}
          </SelectItem>
          <SelectItem
            v-for="networkInterface in model.resolvedNetworkInterfaces"
            :key="networkInterface.name"
            :value="networkInterface.name"
          >
            <div class="min-w-0 flex-1 pr-5">
              <OverflowTooltipText
                :text="networkInterface.label"
                class="text-sm"
                tooltip-align="start"
                tooltip-side="right"
              />
            </div>
          </SelectItem>
        </SelectContent>
      </Select>
      <p class="text-[11px] text-muted-foreground sm:hidden">
        {{ t("admin.ddns.interfaceHint") }}
      </p>
    </div>
  </div>
</template>
