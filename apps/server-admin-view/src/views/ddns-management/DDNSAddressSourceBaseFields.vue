<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import OverflowTooltipText from "@admin-shared/components/common/OverflowTooltipText.vue";
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
  normalizeUpdateScope,
  toNetworkInterfaceSelectValue,
} from "./model";
import type { DDNSAddressSourceFieldsProps } from "./ddns-address-source-fields-contract";

defineProps<{ model: DDNSAddressSourceFieldsProps }>();
const { t } = useI18n();
</script>

<template>
  <div
    class="grid items-start gap-2 p-4 transition-colors hover:bg-muted/10 sm:grid-cols-[200px_1fr] sm:p-6 md:grid-cols-[240px_1fr]"
  >
    <div class="mt-1.5 space-y-1">
      <Label for="ddns-network-interface" class="text-sm font-medium">
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
            model.providerConfig[NETWORK_INTERFACE_KEY],
          )
        "
        @update:model-value="
          (value) =>
            model.updateNetworkInterface(
              value === NETWORK_INTERFACE_AUTO_VALUE
                ? ''
                : String(value ?? ''),
            )
        "
      >
        <SelectTrigger
          id="ddns-network-interface"
          class="w-full min-w-0 max-w-full overflow-hidden"
        >
          <SelectValue
            :placeholder="t('admin.ddns.autoSelect')"
            class="min-w-0 flex-1 overflow-hidden"
          >
            <span class="block w-full min-w-0 max-w-full truncate text-left">
              {{ model.configuredNetworkInterfaceLabel }}
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
      <p
        v-if="model.selectedNetworkInterfaceDetail"
        class="break-all text-[11px] leading-5 text-muted-foreground"
      >
        {{ model.selectedNetworkInterfaceDetail }}
      </p>
      <p class="mt-1.5 text-[11px] text-muted-foreground sm:hidden">
        {{ t("admin.ddns.interfaceHint") }}
      </p>
    </div>
  </div>

  <div
    class="grid items-start gap-2 p-4 transition-colors hover:bg-muted/10 sm:grid-cols-[200px_1fr] sm:p-6 md:grid-cols-[240px_1fr]"
  >
    <div class="mt-1.5 space-y-1">
      <Label for="ddns-ip-source" class="text-sm font-medium">
        {{ t("admin.ddns.ipSourceLabel") }}
      </Label>
      <p class="hidden pr-4 text-xs text-muted-foreground sm:block">
        {{ t("admin.ddns.ipSourceHint") }}
      </p>
    </div>
    <div class="w-full max-w-md space-y-2">
      <Select
        :model-value="
          model.providerConfig[IP_SOURCE_KEY] || DEFAULT_DDNS_IP_SOURCE
        "
        @update:model-value="model.updateIpSource(String($event ?? ''))"
      >
        <SelectTrigger id="ddns-ip-source" class="w-full">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem
            v-for="option in IP_SOURCE_OPTIONS"
            :key="option.value"
            :value="option.value"
            :disabled="
              model.isIpSourceOptionDisabled(
                model.selectedProvider,
                option.value,
              )
            "
          >
            {{ model.formatOptionLabel(option) }}
          </SelectItem>
        </SelectContent>
      </Select>
      <p
        v-if="model.showInterfaceAddressBlock"
        class="text-[11px] text-muted-foreground"
      >
        {{
          t(
            model.providerConfig[ALLOW_PRIVATE_ADDRESSES_KEY] === "true"
              ? "admin.ddns.interfacePrivateFilterOn"
              : "admin.ddns.interfaceOnlyFiltered",
          )
        }}
      </p>
      <p class="mt-1.5 text-[11px] text-muted-foreground sm:hidden">
        {{ t("admin.ddns.ipSourceHint") }}
      </p>
    </div>
  </div>

  <div
    class="grid items-start gap-2 p-4 transition-colors hover:bg-muted/10 sm:grid-cols-[200px_1fr] sm:p-6 md:grid-cols-[240px_1fr]"
  >
    <div class="mt-1.5 space-y-1">
      <Label for="ddns-update-scope" class="text-sm font-medium">
        {{ t("admin.ddns.updateScopeLabel") }}
      </Label>
      <p class="hidden pr-4 text-xs text-muted-foreground sm:block">
        {{ t("admin.ddns.updateScopeHint") }}
      </p>
    </div>
    <div class="w-full max-w-md space-y-2">
      <Select
        :model-value="
          model.providerConfig[UPDATE_SCOPE_KEY] || DEFAULT_DDNS_UPDATE_SCOPE
        "
        @update:model-value="
          model.setFieldValue(
            UPDATE_SCOPE_KEY,
            normalizeUpdateScope(String($event ?? '')),
          )
        "
      >
        <SelectTrigger id="ddns-update-scope" class="w-full">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem
            v-for="option in UPDATE_SCOPE_OPTIONS"
            :key="option.value"
            :value="option.value"
            :disabled="
              model.isUpdateScopeOptionDisabled(
                model.selectedProvider,
                option.value,
              )
            "
          >
            {{ model.formatOptionLabel(option) }}
          </SelectItem>
        </SelectContent>
      </Select>
      <p class="mt-1.5 text-[11px] text-muted-foreground sm:hidden">
        {{ t("admin.ddns.updateScopeHint") }}
      </p>
    </div>
  </div>
</template>
