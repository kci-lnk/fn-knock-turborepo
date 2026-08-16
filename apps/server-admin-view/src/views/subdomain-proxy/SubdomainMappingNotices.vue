<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";

const props = defineProps<{
  visibleMappingsCount: number;
  rootDomainValidationMessage: string;
  savedRootDomain: string;
  rootDomainPendingSave: boolean;
  selectionMode: boolean;
}>();

const { t } = useI18n();
const rootDomainNotice = computed(() =>
  props.rootDomainValidationMessage
    ? props.rootDomainValidationMessage
    : !props.savedRootDomain
      ? t("admin.subdomainProxy.rootDomainRequired")
      : props.rootDomainPendingSave
        ? t("admin.subdomainProxy.rootDomainDirty")
        : "",
);
</script>

<template>
  <p
    v-if="visibleMappingsCount > 1 && !selectionMode"
    class="text-xs text-muted-foreground"
  >
    {{ t("admin.subdomainProxy.orderHintPrefix") }}
    <a
      href="#/system/gateway-proxy-headers"
      class="underline underline-offset-2 hover:text-foreground"
    >
      {{ t("admin.subdomainProxy.disableProxyHeaders") }} </a
    >{{ t("admin.subdomainProxy.orderHintMiddle") }}

    <a
      href="#/system/gateway-host-response"
      class="underline underline-offset-2 hover:text-foreground"
    >
      {{ t("admin.subdomainProxy.disableHostHeader") }}
    </a>
  </p>
  <p v-if="rootDomainNotice" class="text-xs text-amber-600">
    {{ rootDomainNotice }}
  </p>
</template>
