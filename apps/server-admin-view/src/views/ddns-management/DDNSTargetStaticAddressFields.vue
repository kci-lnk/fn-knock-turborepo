<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import type { DDNSTargetAddressFieldsProps } from "./ddns-target-dialog-contract";
import { SOURCE_DOMAIN_KEY, STATIC_IPV4_KEY, STATIC_IPV6_KEY } from "./model";

defineProps<{ model: DDNSTargetAddressFieldsProps }>();
const { t } = useI18n();
</script>

<template>
  <div
    v-if="model.shouldShowStaticBlock && model.updateScope !== 'ipv6_only'"
    class="grid items-start gap-2 p-4 transition-colors hover:bg-muted/10 sm:grid-cols-[180px_1fr] sm:p-5 md:grid-cols-[220px_1fr]"
  >
    <div class="mt-1.5 space-y-1">
      <Label for="ddns-target-static-ipv4" class="text-sm font-medium">
        {{ t("admin.ddns.staticIpv4Label") }}
      </Label>
      <p class="hidden pr-4 text-xs text-muted-foreground sm:block">
        {{ t("admin.ddns.staticIpv4Hint") }}
      </p>
    </div>
    <div class="w-full max-w-md space-y-2">
      <Input
        id="ddns-target-static-ipv4"
        v-model="model.state.config[STATIC_IPV4_KEY]"
        placeholder="203.0.113.10"
        inputmode="decimal"
        autocomplete="off"
      />
      <p class="text-[11px] text-muted-foreground sm:hidden">
        {{ t("admin.ddns.staticIpv4Hint") }}
      </p>
    </div>
  </div>

  <div
    v-if="model.shouldShowStaticBlock && model.updateScope !== 'ipv4_only'"
    class="grid items-start gap-2 p-4 transition-colors hover:bg-muted/10 sm:grid-cols-[180px_1fr] sm:p-5 md:grid-cols-[220px_1fr]"
  >
    <div class="mt-1.5 space-y-1">
      <Label for="ddns-target-static-ipv6" class="text-sm font-medium">
        {{ t("admin.ddns.staticIpv6Label") }}
      </Label>
      <p class="hidden pr-4 text-xs text-muted-foreground sm:block">
        {{ t("admin.ddns.staticIpv6Hint") }}
      </p>
    </div>
    <div class="w-full max-w-md space-y-2">
      <Input
        id="ddns-target-static-ipv6"
        v-model="model.state.config[STATIC_IPV6_KEY]"
        placeholder="2001:db8::10"
        autocomplete="off"
      />
      <p class="text-[11px] text-muted-foreground sm:hidden">
        {{ t("admin.ddns.staticIpv6Hint") }}
      </p>
    </div>
  </div>

  <div
    v-if="model.shouldShowDomainBlock"
    class="grid items-start gap-2 p-4 transition-colors hover:bg-muted/10 sm:grid-cols-[180px_1fr] sm:p-5 md:grid-cols-[220px_1fr]"
  >
    <div class="mt-1.5 space-y-1">
      <Label for="ddns-target-source-domain" class="text-sm font-medium">
        {{ t("admin.ddns.sourceDomainLabel") }}
      </Label>
      <p class="hidden pr-4 text-xs text-muted-foreground sm:block">
        {{ t("admin.ddns.sourceDomainHint") }}
      </p>
    </div>
    <div class="w-full max-w-md space-y-2">
      <Input
        id="ddns-target-source-domain"
        v-model="model.state.config[SOURCE_DOMAIN_KEY]"
        placeholder="origin.example.com"
        autocomplete="off"
      />
      <p class="text-[11px] text-muted-foreground sm:hidden">
        {{ t("admin.ddns.sourceDomainHint") }}
      </p>
    </div>
  </div>
</template>
