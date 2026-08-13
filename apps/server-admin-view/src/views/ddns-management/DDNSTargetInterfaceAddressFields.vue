<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import type { DDNSTargetAddressFieldsProps } from "./ddns-target-dialog-contract";
import DDNSInterfaceSelectorEditor from "./DDNSInterfaceSelectorEditor.vue";
import {
  ALLOW_PRIVATE_ADDRESSES_KEY,
  INTERFACE_IPV4_INDEX_KEY,
  INTERFACE_IPV4_SELECTOR_KEY,
  INTERFACE_IPV6_INDEX_KEY,
  INTERFACE_IPV6_SELECTOR_KEY,
  NETWORK_INTERFACE_KEY,
} from "./model";

const props = defineProps<{ model: DDNSTargetAddressFieldsProps }>();
const { t } = useI18n();
const allowPrivateAddresses = computed(
  () => props.model.state.config[ALLOW_PRIVATE_ADDRESSES_KEY] === "true",
);
const selectedNetworkInterface = computed(
  () =>
    props.model.resolvedNetworkInterfaces.find(
      (item) => item.name === props.model.state.config[NETWORK_INTERFACE_KEY],
    ) || null,
);
const updateSelector = (family: "ipv4" | "ipv6", value: string) => {
  const selectorKey =
    family === "ipv4"
      ? INTERFACE_IPV4_SELECTOR_KEY
      : INTERFACE_IPV6_SELECTOR_KEY;
  const indexKey =
    family === "ipv4" ? INTERFACE_IPV4_INDEX_KEY : INTERFACE_IPV6_INDEX_KEY;
  props.model.state.config[selectorKey] = value;
  props.model.state.config[indexKey] = "";
};
</script>

<template>
  <template v-if="model.shouldShowInterfaceBlock">
    <div
      class="grid items-start gap-2 p-4 transition-colors hover:bg-muted/10 sm:grid-cols-[180px_1fr] sm:p-5 md:grid-cols-[220px_1fr]"
    >
      <div class="mt-1.5 space-y-1">
        <Label
          for="ddns-target-allow-private-addresses"
          class="text-sm font-medium"
        >
          {{ t("admin.ddns.allowPrivateAddresses") }}
        </Label>
        <p class="hidden pr-4 text-xs text-muted-foreground sm:block">
          {{ t("admin.ddns.allowPrivateAddressesHint") }}
        </p>
      </div>
      <div class="w-full max-w-md space-y-2">
        <div class="flex min-h-10 items-center gap-3">
          <Switch
            id="ddns-target-allow-private-addresses"
            :model-value="allowPrivateAddresses"
            @update:model-value="
              model.state.config[ALLOW_PRIVATE_ADDRESSES_KEY] = $event
                ? 'true'
                : 'false'
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
      class="grid items-start gap-2 p-4 transition-colors hover:bg-muted/10 sm:grid-cols-[180px_1fr] sm:p-5 md:grid-cols-[220px_1fr]"
    >
      <div class="mt-1.5 space-y-1">
        <div class="text-sm font-medium">
          {{ t("admin.ddns.interfaceAddressHelpTitle") }}
        </div>
        <p class="hidden pr-4 text-xs text-muted-foreground sm:block">
          {{ t("admin.ddns.interfaceAddressHelp") }}
        </p>
      </div>
      <div
        class="w-full max-w-md space-y-2 text-[11px] leading-5 text-muted-foreground"
      >
        <p>{{ t("admin.ddns.addressOrderHelp") }}</p>
        <p>
          {{
            t(
              allowPrivateAddresses
                ? "admin.ddns.privateAddressHelp"
                : "admin.ddns.filteredAddressHelp",
            )
          }}
        </p>
      </div>
    </div>
  </template>

  <div
    v-if="model.updateScope !== 'ipv6_only' && model.shouldShowInterfaceBlock"
    class="grid items-start gap-2 p-4 transition-colors hover:bg-muted/10 sm:grid-cols-[180px_1fr] sm:p-5 md:grid-cols-[220px_1fr]"
  >
    <div class="mt-1.5 space-y-1">
      <Label for="ddns-target-ipv4" class="text-sm font-medium">
        {{ t("admin.ddns.selectIpv4Label") }}
      </Label>
      <p class="hidden pr-4 text-xs text-muted-foreground sm:block">
        {{ t("admin.ddns.selectIpv4Hint") }}
      </p>
    </div>
    <DDNSInterfaceSelectorEditor
      :allow-private-addresses="allowPrivateAddresses"
      :current-address="
        model.state.selectionAnchor?.ipv4 || model.state.lastIP?.ipv4
      "
      family="ipv4"
      id-prefix="ddns-target-ipv4"
      :legacy-index="model.state.config[INTERFACE_IPV4_INDEX_KEY]"
      :model-value="model.state.config[INTERFACE_IPV4_SELECTOR_KEY]"
      :network-interface="selectedNetworkInterface"
      @update:model-value="updateSelector('ipv4', $event)"
    />
  </div>

  <div
    v-if="model.updateScope !== 'ipv4_only' && model.shouldShowInterfaceBlock"
    class="grid items-start gap-2 p-4 transition-colors hover:bg-muted/10 sm:grid-cols-[180px_1fr] sm:p-5 md:grid-cols-[220px_1fr]"
  >
    <div class="mt-1.5 space-y-1">
      <Label for="ddns-target-ipv6" class="text-sm font-medium">
        {{ t("admin.ddns.selectIpv6Label") }}
      </Label>
      <p class="hidden pr-4 text-xs text-muted-foreground sm:block">
        {{ t("admin.ddns.selectIpv6Hint") }}
      </p>
    </div>
    <DDNSInterfaceSelectorEditor
      :allow-private-addresses="allowPrivateAddresses"
      :current-address="
        model.state.selectionAnchor?.ipv6 || model.state.lastIP?.ipv6
      "
      family="ipv6"
      id-prefix="ddns-target-ipv6"
      :legacy-index="model.state.config[INTERFACE_IPV6_INDEX_KEY]"
      :model-value="model.state.config[INTERFACE_IPV6_SELECTOR_KEY]"
      :network-interface="selectedNetworkInterface"
      @update:model-value="updateSelector('ipv6', $event)"
    />
  </div>
</template>
