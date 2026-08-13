<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import {
  ALLOW_PRIVATE_ADDRESSES_KEY,
  INTERFACE_IPV4_INDEX_KEY,
  INTERFACE_IPV4_SELECTOR_KEY,
  INTERFACE_IPV6_INDEX_KEY,
  INTERFACE_IPV6_SELECTOR_KEY,
} from "./model";
import DDNSInterfaceSelectorEditor from "./DDNSInterfaceSelectorEditor.vue";
import type { DDNSAddressSourceFieldsProps } from "./ddns-address-source-fields-contract";

const props = defineProps<{ model: DDNSAddressSourceFieldsProps }>();
const { t } = useI18n();
const allowPrivateAddresses = computed(
  () => props.model.providerConfig[ALLOW_PRIVATE_ADDRESSES_KEY] === "true",
);
const selectedNetworkInterface = computed(
  () =>
    props.model.resolvedNetworkInterfaces.find(
      (item) => item.name === props.model.configuredNetworkInterface,
    ) || null,
);
const updateSelector = (family: "ipv4" | "ipv6", value: string) => {
  const selectorKey =
    family === "ipv4"
      ? INTERFACE_IPV4_SELECTOR_KEY
      : INTERFACE_IPV6_SELECTOR_KEY;
  const indexKey =
    family === "ipv4" ? INTERFACE_IPV4_INDEX_KEY : INTERFACE_IPV6_INDEX_KEY;
  props.model.setFieldValue(selectorKey, value);
  props.model.setFieldValue(indexKey, "");
};
</script>

<template>
  <template v-if="model.showInterfaceAddressBlock">
    <div
      class="grid items-start gap-2 p-4 transition-colors hover:bg-muted/10 sm:grid-cols-[200px_1fr] sm:p-6 md:grid-cols-[240px_1fr]"
    >
      <div class="mt-1.5 space-y-1">
        <Label for="ddns-allow-private-addresses" class="text-sm font-medium">
          {{ t("admin.ddns.allowPrivateAddresses") }}
        </Label>
        <p class="hidden pr-4 text-xs text-muted-foreground sm:block">
          {{ t("admin.ddns.allowPrivateAddressesHint") }}
        </p>
      </div>
      <div class="w-full max-w-md space-y-2">
        <div class="flex min-h-10 items-center gap-3">
          <Switch
            id="ddns-allow-private-addresses"
            :model-value="allowPrivateAddresses"
            @update:model-value="
              model.setFieldValue(
                ALLOW_PRIVATE_ADDRESSES_KEY,
                $event ? 'true' : 'false',
              )
            "
          />
          <span class="text-sm text-muted-foreground">
            {{
              t(
                allowPrivateAddresses
                  ? "admin.ddns.allowedLabel"
                  : "admin.ddns.filteredLabel",
              )
            }}
          </span>
        </div>
        <p
          v-if="allowPrivateAddresses"
          class="rounded-md border border-amber-500/30 bg-amber-500/5 px-3 py-2 text-[11px] leading-5 text-amber-700 dark:text-amber-300"
        >
          {{ t("admin.ddns.allowPrivateAddressesWarning") }}
        </p>
      </div>
    </div>

    <div
      class="grid items-start gap-2 p-4 transition-colors hover:bg-muted/10 sm:grid-cols-[200px_1fr] sm:p-6 md:grid-cols-[240px_1fr]"
    >
      <div class="mt-1.5 space-y-1">
        <div class="text-sm font-medium">
          {{ t("admin.ddns.interfaceAddressHelpTitle") }}
        </div>
        <p class="hidden pr-4 text-xs text-muted-foreground sm:block">
          {{ t("admin.ddns.interfaceAddressHelp") }}
        </p>
      </div>
      <div class="w-full max-w-md space-y-2">
        <p
          v-if="!model.configuredNetworkInterface"
          class="text-sm text-muted-foreground"
        >
          {{ t("admin.ddns.chooseInterfaceFirst") }}
        </p>
        <template v-else>
          <p class="text-[11px] leading-5 text-muted-foreground">
            {{ t("admin.ddns.addressOrderHelp") }}
          </p>
          <p class="text-[11px] leading-5 text-muted-foreground">
            {{
              t(
                allowPrivateAddresses
                  ? "admin.ddns.privateAddressHelp"
                  : "admin.ddns.filteredAddressHelp",
              )
            }}
          </p>
        </template>
        <p class="mt-1.5 text-[11px] text-muted-foreground sm:hidden">
          {{ t("admin.ddns.interfaceAddressHelp") }}
        </p>
      </div>
    </div>
  </template>

  <div
    v-if="model.showInterfaceIPv4Select"
    class="grid items-start gap-2 p-4 transition-colors hover:bg-muted/10 sm:grid-cols-[200px_1fr] sm:p-6 md:grid-cols-[240px_1fr]"
  >
    <div class="mt-1.5 space-y-1">
      <Label for="ddns-interface-ipv4" class="text-sm font-medium">
        {{ t("admin.ddns.selectIpv4Label") }}
      </Label>
      <p class="hidden pr-4 text-xs text-muted-foreground sm:block">
        {{ t("admin.ddns.selectIpv4Hint") }}
      </p>
    </div>
    <DDNSInterfaceSelectorEditor
      :allow-private-addresses="allowPrivateAddresses"
      :current-address="model.selectionAnchor?.ipv4 || model.lastIp?.ipv4"
      family="ipv4"
      id-prefix="ddns-interface-ipv4"
      :legacy-index="model.providerConfig[INTERFACE_IPV4_INDEX_KEY]"
      :model-value="model.providerConfig[INTERFACE_IPV4_SELECTOR_KEY]"
      :network-interface="selectedNetworkInterface"
      @update:model-value="updateSelector('ipv4', $event)"
    />
  </div>

  <div
    v-if="model.showInterfaceIPv6Select"
    class="grid items-start gap-2 p-4 transition-colors hover:bg-muted/10 sm:grid-cols-[200px_1fr] sm:p-6 md:grid-cols-[240px_1fr]"
  >
    <div class="mt-1.5 space-y-1">
      <Label for="ddns-interface-ipv6" class="text-sm font-medium">
        {{ t("admin.ddns.selectIpv6Label") }}
      </Label>
      <p class="hidden pr-4 text-xs text-muted-foreground sm:block">
        {{ t("admin.ddns.selectIpv6Hint") }}
      </p>
    </div>
    <DDNSInterfaceSelectorEditor
      :allow-private-addresses="allowPrivateAddresses"
      :current-address="model.selectionAnchor?.ipv6 || model.lastIp?.ipv6"
      family="ipv6"
      id-prefix="ddns-interface-ipv6"
      :legacy-index="model.providerConfig[INTERFACE_IPV6_INDEX_KEY]"
      :model-value="model.providerConfig[INTERFACE_IPV6_SELECTOR_KEY]"
      :network-interface="selectedNetworkInterface"
      @update:model-value="updateSelector('ipv6', $event)"
    />
  </div>
</template>
