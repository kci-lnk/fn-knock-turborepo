<script setup lang="ts">
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
import OverflowTooltipText from "@admin-shared/components/common/OverflowTooltipText.vue";
import type { DDNSNetworkInterfacePayload } from "@/lib/api";
import {
  DEFAULT_DDNS_IP_SOURCE,
  DEFAULT_DDNS_UPDATE_SCOPE,
  INTERFACE_IPV4_INDEX_KEY,
  INTERFACE_IPV6_INDEX_KEY,
  IP_SOURCE_KEY,
  IP_SOURCE_OPTIONS,
  NETWORK_INTERFACE_AUTO_VALUE,
  NETWORK_INTERFACE_KEY,
  SOURCE_DOMAIN_KEY,
  STATIC_IPV4_KEY,
  STATIC_IPV6_KEY,
  UPDATE_SCOPE_KEY,
  UPDATE_SCOPE_OPTIONS,
  normalizeInterfaceAddressIndex,
  normalizeUpdateScope,
  toNetworkInterfaceSelectValue,
  type DDNSIpSource,
  type DDNSUpdateScope,
} from "./model";

type AddressOption = {
  label: string;
  value: string;
};

defineProps<{
  configuredNetworkInterface: string;
  configuredNetworkInterfaceLabel: string;
  formatOptionLabel: (option: { labelKey: string }) => string;
  interfaceIPv4Options: AddressOption[];
  interfaceIPv6Options: AddressOption[];
  isIpSourceOptionDisabled: (
    providerName: string,
    option: DDNSIpSource,
  ) => boolean;
  isUpdateScopeOptionDisabled: (
    providerName: string,
    option: DDNSUpdateScope,
  ) => boolean;
  providerConfig: Record<string, string>;
  resolvedNetworkInterfaces: DDNSNetworkInterfacePayload[];
  selectedNetworkInterfaceDetail: string;
  selectedProvider: string;
  setFieldValue: (key: string, value: string) => void;
  showInterfaceAddressBlock: boolean;
  showInterfaceIPv4Select: boolean;
  showInterfaceIPv6Select: boolean;
  showSourceDomainBlock: boolean;
  showStaticIPv4Input: boolean;
  showStaticIPv6Input: boolean;
  updateNetworkInterface: (value: string) => void;
  updateIpSource: (value: string) => void;
}>();

const { t } = useI18n();
</script>

<template>
  <template v-if="selectedProvider">
    <div
      class="p-4 sm:p-6 grid gap-2 sm:grid-cols-[200px_1fr] md:grid-cols-[240px_1fr] items-start transition-colors hover:bg-muted/10"
    >
      <div class="space-y-1 mt-1.5">
        <Label for="ddns-network-interface" class="text-sm font-medium">
          {{ t("admin.ddns.outboundInterface") }}
        </Label>
        <p class="text-xs text-muted-foreground hidden sm:block pr-4">
          {{ t("admin.ddns.interfaceHint") }}
        </p>
      </div>
      <div class="w-full max-w-md space-y-2">
        <Select
          :modelValue="
            toNetworkInterfaceSelectValue(providerConfig[NETWORK_INTERFACE_KEY])
          "
          @update:modelValue="
            (val: any) =>
              updateNetworkInterface(
                val === NETWORK_INTERFACE_AUTO_VALUE ? '' : String(val ?? ''),
              )
          "
        >
          <SelectTrigger
            class="w-full overflow-hidden"
            id="ddns-network-interface"
          >
            <SelectValue :placeholder="t('admin.ddns.autoSelect')">
              <span class="block min-w-0 max-w-full truncate">
                {{ configuredNetworkInterfaceLabel }}
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
              v-for="networkInterface in resolvedNetworkInterfaces"
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
          v-if="selectedNetworkInterfaceDetail"
          class="text-[11px] leading-5 text-muted-foreground break-all"
        >
          {{ selectedNetworkInterfaceDetail }}
        </p>

        <p class="text-[11px] text-muted-foreground sm:hidden mt-1.5">
          {{ t("admin.ddns.interfaceHint") }}
        </p>
      </div>
    </div>

    <div
      class="p-4 sm:p-6 grid gap-2 sm:grid-cols-[200px_1fr] md:grid-cols-[240px_1fr] items-start transition-colors hover:bg-muted/10"
    >
      <div class="space-y-1 mt-1.5">
        <Label for="ddns-ip-source" class="text-sm font-medium">
          {{ t("admin.ddns.ipSourceLabel") }}
        </Label>
        <p class="text-xs text-muted-foreground hidden sm:block pr-4">
          {{ t("admin.ddns.ipSourceHint") }}
        </p>
      </div>
      <div class="w-full max-w-md space-y-2">
        <Select
          :modelValue="providerConfig[IP_SOURCE_KEY] || DEFAULT_DDNS_IP_SOURCE"
          @update:modelValue="(val: any) => updateIpSource(String(val ?? ''))"
        >
          <SelectTrigger class="w-full" id="ddns-ip-source">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem
              v-for="option in IP_SOURCE_OPTIONS"
              :key="option.value"
              :value="option.value"
              :disabled="isIpSourceOptionDisabled(selectedProvider, option.value)"
            >
              {{ formatOptionLabel(option) }}
            </SelectItem>
          </SelectContent>
        </Select>

        <p class="text-[11px] text-muted-foreground">
          {{ t("admin.ddns.interfaceOnlyFiltered") }}
        </p>

        <p class="text-[11px] text-muted-foreground sm:hidden mt-1.5">
          {{ t("admin.ddns.ipSourceHint") }}
        </p>
      </div>
    </div>

    <div
      class="p-4 sm:p-6 grid gap-2 sm:grid-cols-[200px_1fr] md:grid-cols-[240px_1fr] items-start transition-colors hover:bg-muted/10"
    >
      <div class="space-y-1 mt-1.5">
        <Label for="ddns-update-scope" class="text-sm font-medium">
          {{ t("admin.ddns.updateScopeLabel") }}
        </Label>
        <p class="text-xs text-muted-foreground hidden sm:block pr-4">
          {{ t("admin.ddns.updateScopeHint") }}
        </p>
      </div>
      <div class="w-full max-w-md space-y-2">
        <Select
          :modelValue="
            providerConfig[UPDATE_SCOPE_KEY] || DEFAULT_DDNS_UPDATE_SCOPE
          "
          @update:modelValue="
            (val: any) =>
              setFieldValue(
                UPDATE_SCOPE_KEY,
                normalizeUpdateScope(String(val ?? '')),
              )
          "
        >
          <SelectTrigger class="w-full" id="ddns-update-scope">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem
              v-for="option in UPDATE_SCOPE_OPTIONS"
              :key="option.value"
              :value="option.value"
              :disabled="
                isUpdateScopeOptionDisabled(selectedProvider, option.value)
              "
            >
              {{ formatOptionLabel(option) }}
            </SelectItem>
          </SelectContent>
        </Select>

        <p class="text-[11px] text-muted-foreground sm:hidden mt-1.5">
          {{ t("admin.ddns.updateScopeHint") }}
        </p>
      </div>
    </div>

    <div
      v-if="showStaticIPv4Input"
      class="p-4 sm:p-6 grid gap-2 sm:grid-cols-[200px_1fr] md:grid-cols-[240px_1fr] items-start transition-colors hover:bg-muted/10"
    >
      <div class="space-y-1 mt-1.5">
        <Label for="ddns-static-ipv4" class="text-sm font-medium">
          {{ t("admin.ddns.staticIpv4Label") }}
        </Label>
        <p class="text-xs text-muted-foreground hidden sm:block pr-4">
          {{ t("admin.ddns.staticIpv4Hint") }}
        </p>
      </div>
      <div class="w-full max-w-md space-y-2">
        <Input
          id="ddns-static-ipv4"
          :model-value="providerConfig[STATIC_IPV4_KEY] || ''"
          placeholder="203.0.113.10"
          inputmode="decimal"
          autocomplete="off"
          @update:model-value="
            (value: string | number) =>
              setFieldValue(STATIC_IPV4_KEY, String(value))
          "
        />
        <p class="text-[11px] text-muted-foreground sm:hidden mt-1.5">
          {{ t("admin.ddns.staticIpv4Hint") }}
        </p>
      </div>
    </div>

    <div
      v-if="showStaticIPv6Input"
      class="p-4 sm:p-6 grid gap-2 sm:grid-cols-[200px_1fr] md:grid-cols-[240px_1fr] items-start transition-colors hover:bg-muted/10"
    >
      <div class="space-y-1 mt-1.5">
        <Label for="ddns-static-ipv6" class="text-sm font-medium">
          {{ t("admin.ddns.staticIpv6Label") }}
        </Label>
        <p class="text-xs text-muted-foreground hidden sm:block pr-4">
          {{ t("admin.ddns.staticIpv6Hint") }}
        </p>
      </div>
      <div class="w-full max-w-md space-y-2">
        <Input
          id="ddns-static-ipv6"
          :model-value="providerConfig[STATIC_IPV6_KEY] || ''"
          placeholder="2001:db8::10"
          autocomplete="off"
          @update:model-value="
            (value: string | number) =>
              setFieldValue(STATIC_IPV6_KEY, String(value))
          "
        />
        <p class="text-[11px] text-muted-foreground sm:hidden mt-1.5">
          {{ t("admin.ddns.staticIpv6Hint") }}
        </p>
      </div>
    </div>

    <div
      v-if="showSourceDomainBlock"
      class="p-4 sm:p-6 grid gap-2 sm:grid-cols-[200px_1fr] md:grid-cols-[240px_1fr] items-start transition-colors hover:bg-muted/10"
    >
      <div class="space-y-1 mt-1.5">
        <Label for="ddns-source-domain" class="text-sm font-medium">
          {{ t("admin.ddns.sourceDomainLabel") }}
        </Label>
        <p class="text-xs text-muted-foreground hidden sm:block pr-4">
          {{ t("admin.ddns.sourceDomainHint") }}
        </p>
      </div>
      <div class="w-full max-w-md space-y-2">
        <Input
          id="ddns-source-domain"
          :model-value="providerConfig[SOURCE_DOMAIN_KEY] || ''"
          placeholder="origin.example.com"
          autocomplete="off"
          @update:model-value="
            (value: string | number) =>
              setFieldValue(SOURCE_DOMAIN_KEY, String(value))
          "
        />
        <p class="text-[11px] text-muted-foreground sm:hidden mt-1.5">
          {{ t("admin.ddns.sourceDomainHint") }}
        </p>
      </div>
    </div>

    <div
      v-if="showInterfaceAddressBlock"
      class="p-4 sm:p-6 grid gap-2 sm:grid-cols-[200px_1fr] md:grid-cols-[240px_1fr] items-start transition-colors hover:bg-muted/10"
    >
      <div class="space-y-1 mt-1.5">
        <Label class="text-sm font-medium">
          {{ t("admin.ddns.interfaceAddressHelpTitle") }}
        </Label>
        <p class="text-xs text-muted-foreground hidden sm:block pr-4">
          {{ t("admin.ddns.interfaceAddressHelp") }}
        </p>
      </div>
      <div class="w-full max-w-md space-y-2">
        <p v-if="!configuredNetworkInterface" class="text-sm text-muted-foreground">
          {{ t("admin.ddns.chooseInterfaceFirst") }}
        </p>
        <template v-else>
          <p class="text-[11px] leading-5 text-muted-foreground">
            {{ t("admin.ddns.addressOrderHelp") }}
          </p>
          <p class="text-[11px] leading-5 text-muted-foreground">
            {{ t("admin.ddns.filteredAddressHelp") }}
          </p>
        </template>

        <p class="text-[11px] text-muted-foreground sm:hidden mt-1.5">
          {{ t("admin.ddns.interfaceAddressHelp") }}
        </p>
      </div>
    </div>

    <div
      v-if="showInterfaceIPv4Select"
      class="p-4 sm:p-6 grid gap-2 sm:grid-cols-[200px_1fr] md:grid-cols-[240px_1fr] items-start transition-colors hover:bg-muted/10"
    >
      <div class="space-y-1 mt-1.5">
        <Label for="ddns-interface-ipv4" class="text-sm font-medium">
          {{ t("admin.ddns.selectIpv4Label") }}
        </Label>
        <p class="text-xs text-muted-foreground hidden sm:block pr-4">
          {{ t("admin.ddns.selectIpv4Hint") }}
        </p>
      </div>
      <div class="w-full max-w-md space-y-2">
        <Select
          :modelValue="
            normalizeInterfaceAddressIndex(
              providerConfig[INTERFACE_IPV4_INDEX_KEY],
            ) || undefined
          "
          :disabled="
            !configuredNetworkInterface || interfaceIPv4Options.length === 0
          "
          @update:modelValue="
            (val: any) =>
              setFieldValue(
                INTERFACE_IPV4_INDEX_KEY,
                normalizeInterfaceAddressIndex(String(val ?? '')),
              )
          "
        >
          <SelectTrigger class="w-full" id="ddns-interface-ipv4">
            <SelectValue :placeholder="t('admin.ddns.selectIpv4Placeholder')" />
          </SelectTrigger>
          <SelectContent>
            <SelectItem
              v-for="option in interfaceIPv4Options"
              :key="option.value"
              :value="option.value"
            >
              {{ option.label }}
            </SelectItem>
            <div
              v-if="interfaceIPv4Options.length === 0"
              class="px-2 py-1.5 text-sm text-muted-foreground"
            >
              {{ t("admin.ddns.noIpv4Address") }}
            </div>
          </SelectContent>
        </Select>

        <p class="text-[11px] text-muted-foreground sm:hidden mt-1.5">
          {{ t("admin.ddns.selectIpv4Hint") }}
        </p>
      </div>
    </div>

    <div
      v-if="showInterfaceIPv6Select"
      class="p-4 sm:p-6 grid gap-2 sm:grid-cols-[200px_1fr] md:grid-cols-[240px_1fr] items-start transition-colors hover:bg-muted/10"
    >
      <div class="space-y-1 mt-1.5">
        <Label for="ddns-interface-ipv6" class="text-sm font-medium">
          {{ t("admin.ddns.selectIpv6Label") }}
        </Label>
        <p class="text-xs text-muted-foreground hidden sm:block pr-4">
          {{ t("admin.ddns.selectIpv6Hint") }}
        </p>
      </div>
      <div class="w-full max-w-md space-y-2">
        <Select
          :modelValue="
            normalizeInterfaceAddressIndex(
              providerConfig[INTERFACE_IPV6_INDEX_KEY],
            ) || undefined
          "
          :disabled="
            !configuredNetworkInterface || interfaceIPv6Options.length === 0
          "
          @update:modelValue="
            (val: any) =>
              setFieldValue(
                INTERFACE_IPV6_INDEX_KEY,
                normalizeInterfaceAddressIndex(String(val ?? '')),
              )
          "
        >
          <SelectTrigger class="w-full" id="ddns-interface-ipv6">
            <SelectValue :placeholder="t('admin.ddns.selectIpv6Placeholder')" />
          </SelectTrigger>
          <SelectContent>
            <SelectItem
              v-for="option in interfaceIPv6Options"
              :key="option.value"
              :value="option.value"
            >
              {{ option.label }}
            </SelectItem>
            <div
              v-if="interfaceIPv6Options.length === 0"
              class="px-2 py-1.5 text-sm text-muted-foreground"
            >
              {{ t("admin.ddns.noIpv6Address") }}
            </div>
          </SelectContent>
        </Select>

        <p class="text-[11px] text-muted-foreground sm:hidden mt-1.5">
          {{ t("admin.ddns.selectIpv6Hint") }}
        </p>
      </div>
    </div>
  </template>
</template>
