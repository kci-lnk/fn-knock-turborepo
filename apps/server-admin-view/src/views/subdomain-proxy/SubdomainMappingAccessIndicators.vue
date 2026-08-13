<script setup lang="ts">
import { ShieldCheck, ShieldOff, Star } from "lucide-vue-next";
import { useI18n } from "vue-i18n";
import { Badge } from "@/components/ui/badge";
import type { SubdomainMappingStatusIndicatorsProps } from "./subdomain-mapping-status-contract";
import SubdomainMappingStatusTooltip from "./SubdomainMappingStatusTooltip.vue";

defineProps<{ model: SubdomainMappingStatusIndicatorsProps }>();
const { t } = useI18n();

const formatDuration = (seconds: number): string => {
  const totalMinutes = Math.max(1, Math.round(seconds / 60));
  if (totalMinutes % (24 * 60) === 0) return `${totalMinutes / (24 * 60)}d`;
  if (totalMinutes % 60 === 0) return `${totalMinutes / 60}h`;
  return `${totalMinutes}m`;
};
</script>

<template>
  <Badge v-if="model.isAuthService" variant="default">
    {{ t("admin.subdomainProxy.authServiceBadge") }}
  </Badge>

  <SubdomainMappingStatusTooltip
    v-if="model.mapping.is_default"
    :model="model"
    tooltip="default-domain"
  >
    <template #trigger="{ handleClick }">
      <button
        type="button"
        class="inline-flex h-5 w-5 shrink-0 cursor-help items-center justify-center rounded-md transition-colors hover:bg-muted hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/40"
        :class="
          model.isDefaultDomainAvailable
            ? 'text-muted-foreground'
            : 'text-amber-600 dark:text-amber-300'
        "
        :aria-label="
          t('admin.subdomainProxy.defaultDomainAria', {
            host: model.formatHost(model.mapping.host),
          })
        "
        @click="handleClick"
      >
        <Star class="h-3.5 w-3.5" />
      </button>
    </template>
    <p>
      {{
        model.isDefaultDomainAvailable
          ? t("admin.subdomainProxy.defaultDomain")
          : t("admin.subdomainProxy.defaultDomainInactive")
      }}
    </p>
  </SubdomainMappingStatusTooltip>

  <SubdomainMappingStatusTooltip
    v-if="model.mapping.use_auth"
    :model="model"
    tooltip="authentication"
  >
    <template #trigger="{ handleClick }">
      <button
        type="button"
        class="inline-flex h-5 w-5 shrink-0 cursor-help items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-muted hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/40"
        :aria-label="
          t('admin.subdomainProxy.statusAuthRequiredAria', {
            host: model.formatHost(model.mapping.host),
          })
        "
        @click="handleClick"
      >
        <ShieldCheck class="h-3.5 w-3.5" />
      </button>
    </template>
    <p>{{ t("admin.subdomainProxy.statusAuthRequiredTooltip") }}</p>
  </SubdomainMappingStatusTooltip>

  <SubdomainMappingStatusTooltip
    v-if="model.mapping.advanced_auth?.enabled === true"
    :model="model"
    tooltip="advanced-auth"
  >
    <template #trigger="{ handleClick }">
      <button
        type="button"
        class="inline-flex h-5 w-5 shrink-0 cursor-help items-center justify-center rounded-md text-primary transition-colors hover:bg-primary/10 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/40"
        :aria-label="
          t('admin.subdomainProxy.advancedAuthEnabledAria', {
            host: model.formatHost(model.mapping.host),
          })
        "
        @click="handleClick"
      >
        <ShieldOff class="h-3.5 w-3.5" />
      </button>
    </template>
    <p>
      {{
        t("admin.subdomainProxy.advancedAuthEnabledTooltip", {
          groups: model.mapping.advanced_auth.groups.length,
          idle: formatDuration(model.mapping.advanced_auth.idle_ttl_seconds),
        })
      }}
    </p>
  </SubdomainMappingStatusTooltip>

  <Badge v-if="!model.mapping.use_auth" variant="secondary">
    {{ t("admin.subdomainProxy.publicAccess") }}
  </Badge>
</template>
