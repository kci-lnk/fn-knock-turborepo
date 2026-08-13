<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { SOURCE_DOMAIN_KEY, STATIC_IPV4_KEY, STATIC_IPV6_KEY } from "./model";
import type { DDNSAddressSourceFieldsProps } from "./ddns-address-source-fields-contract";

defineProps<{ model: DDNSAddressSourceFieldsProps }>();
const { t } = useI18n();
</script>

<template>
  <div
    v-if="model.showStaticIPv4Input"
    class="grid items-start gap-2 p-4 transition-colors hover:bg-muted/10 sm:grid-cols-[200px_1fr] sm:p-6 md:grid-cols-[240px_1fr]"
  >
    <div class="mt-1.5 space-y-1">
      <Label for="ddns-static-ipv4" class="text-sm font-medium">
        {{ t("admin.ddns.staticIpv4Label") }}
      </Label>
      <p class="hidden pr-4 text-xs text-muted-foreground sm:block">
        {{ t("admin.ddns.staticIpv4Hint") }}
      </p>
    </div>
    <div class="w-full max-w-md space-y-2">
      <Input
        id="ddns-static-ipv4"
        :model-value="model.providerConfig[STATIC_IPV4_KEY] || ''"
        placeholder="203.0.113.10"
        inputmode="decimal"
        autocomplete="off"
        @update:model-value="
          model.setFieldValue(STATIC_IPV4_KEY, String($event))
        "
      />
      <p class="mt-1.5 text-[11px] text-muted-foreground sm:hidden">
        {{ t("admin.ddns.staticIpv4Hint") }}
      </p>
    </div>
  </div>

  <div
    v-if="model.showStaticIPv6Input"
    class="grid items-start gap-2 p-4 transition-colors hover:bg-muted/10 sm:grid-cols-[200px_1fr] sm:p-6 md:grid-cols-[240px_1fr]"
  >
    <div class="mt-1.5 space-y-1">
      <Label for="ddns-static-ipv6" class="text-sm font-medium">
        {{ t("admin.ddns.staticIpv6Label") }}
      </Label>
      <p class="hidden pr-4 text-xs text-muted-foreground sm:block">
        {{ t("admin.ddns.staticIpv6Hint") }}
      </p>
    </div>
    <div class="w-full max-w-md space-y-2">
      <Input
        id="ddns-static-ipv6"
        :model-value="model.providerConfig[STATIC_IPV6_KEY] || ''"
        placeholder="2001:db8::10"
        autocomplete="off"
        @update:model-value="
          model.setFieldValue(STATIC_IPV6_KEY, String($event))
        "
      />
      <p class="mt-1.5 text-[11px] text-muted-foreground sm:hidden">
        {{ t("admin.ddns.staticIpv6Hint") }}
      </p>
    </div>
  </div>

  <div
    v-if="model.showSourceDomainBlock"
    class="grid items-start gap-2 p-4 transition-colors hover:bg-muted/10 sm:grid-cols-[200px_1fr] sm:p-6 md:grid-cols-[240px_1fr]"
  >
    <div class="mt-1.5 space-y-1">
      <Label for="ddns-source-domain" class="text-sm font-medium">
        {{ t("admin.ddns.sourceDomainLabel") }}
      </Label>
      <p class="hidden pr-4 text-xs text-muted-foreground sm:block">
        {{ t("admin.ddns.sourceDomainHint") }}
      </p>
    </div>
    <div class="w-full max-w-md space-y-2">
      <Input
        id="ddns-source-domain"
        :model-value="model.providerConfig[SOURCE_DOMAIN_KEY] || ''"
        placeholder="origin.example.com"
        autocomplete="off"
        @update:model-value="
          model.setFieldValue(SOURCE_DOMAIN_KEY, String($event))
        "
      />
      <p class="mt-1.5 text-[11px] text-muted-foreground sm:hidden">
        {{ t("admin.ddns.sourceDomainHint") }}
      </p>
    </div>
  </div>
</template>
