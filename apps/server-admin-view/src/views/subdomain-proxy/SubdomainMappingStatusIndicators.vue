<template>
  <div
    class="flex min-w-max flex-nowrap items-center gap-2 text-xs text-muted-foreground"
  >
    <Badge
      v-if="availabilityState === 'disabled'"
      variant="outline"
      class="gap-1 border-muted-foreground/30 px-1.5 text-muted-foreground"
    >
      <CircleOff class="h-3 w-3" />
      {{ t("admin.subdomainProxy.disabledBadge") }}
    </Badge>

    <template v-else>
      <TooltipProvider v-if="availabilityState === 'scheduled_closed'">
        <Tooltip>
          <TooltipTrigger as-child>
            <Badge
              variant="outline"
              class="inline-flex h-6 w-6 cursor-help items-center justify-center rounded-full border-amber-500/35 bg-amber-500/5 p-0 text-amber-700 transition-colors hover:bg-amber-500/10 dark:text-amber-300"
              :aria-label="t('admin.subdomainProxy.unavailableBadge')"
            >
              <Clock class="h-3 w-3" />
            </Badge>
          </TooltipTrigger>
          <TooltipContent side="top" align="center">
            <p>
              {{
                t("admin.subdomainProxy.scheduleClosedTooltip", {
                  window: availabilityWindow,
                })
              }}
            </p>
          </TooltipContent>
        </Tooltip>
      </TooltipProvider>

      <TooltipProvider v-else-if="availabilityState === 'scheduled_open'">
        <Tooltip>
          <TooltipTrigger as-child>
            <span
              class="inline-flex h-5 w-5 shrink-0 cursor-help items-center justify-center rounded-md text-muted-foreground"
              :aria-label="t('admin.subdomainProxy.scheduleOpenAria')"
            >
              <Clock class="h-3.5 w-3.5" />
            </span>
          </TooltipTrigger>
          <TooltipContent side="top" align="center">
            <p>
              {{
                t("admin.subdomainProxy.scheduleOpenTooltip", {
                  window: availabilityWindow,
                })
              }}
            </p>
          </TooltipContent>
        </Tooltip>
      </TooltipProvider>

      <Badge v-if="isAuthService" variant="default">
        {{ t("admin.subdomainProxy.authServiceBadge") }}
      </Badge>

      <TooltipProvider v-if="mapping.is_default">
        <Tooltip>
          <TooltipTrigger as-child>
            <span
              class="inline-flex h-5 w-5 shrink-0 items-center justify-center rounded-md text-muted-foreground"
              :aria-label="
                t('admin.subdomainProxy.defaultDomainAria', {
                  host: formatHost(mapping.host),
                })
              "
            >
              <Star class="h-3.5 w-3.5" />
            </span>
          </TooltipTrigger>
          <TooltipContent side="top" align="center">
            <p>{{ t("admin.subdomainProxy.defaultDomain") }}</p>
          </TooltipContent>
        </Tooltip>
      </TooltipProvider>

      <ShieldCheck v-if="mapping.use_auth" class="h-3.5 w-3.5 shrink-0" />
      <Badge v-else variant="secondary">
        {{ t("admin.subdomainProxy.publicAccess") }}
      </Badge>

      <PanelsTopLeft
        v-if="shouldShowToolbarIndicator"
        class="h-3.5 w-3.5 shrink-0"
      />

      <TooltipProvider v-if="locationRulesCount > 0">
        <Tooltip
          :open="isLocationRulesTooltipOpen(mapping.host)"
          @update:open="
            (nextOpen) =>
              handleLocationRulesTooltipOpenChange(mapping.host, nextOpen)
          "
        >
          <TooltipTrigger as-child>
            <button
              type="button"
              class="inline-flex h-5 w-5 shrink-0 cursor-help items-center justify-center rounded-md transition-colors hover:bg-muted hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/40"
              :aria-label="
                t('admin.subdomainProxy.locationRulesAria', {
                  host: formatHost(mapping.host),
                  count: locationRulesCount,
                })
              "
              @click="handleLocationRulesTooltipTriggerClick(mapping.host)"
            >
              <RouteIcon class="h-3.5 w-3.5" />
            </button>
          </TooltipTrigger>
          <TooltipContent side="top" align="center">
            <p>
              {{
                t("admin.subdomainProxy.locationRulesCount", {
                  count: locationRulesCount,
                })
              }}
            </p>
          </TooltipContent>
        </Tooltip>
      </TooltipProvider>
    </template>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import {
  CircleOff,
  Clock,
  PanelsTopLeft,
  Route as RouteIcon,
  ShieldCheck,
  Star,
} from "lucide-vue-next";
import { Badge } from "@/components/ui/badge";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { isWebSocketProxyTargetUrl } from "@admin-shared/utils/proxyTargetInput";
import type { HostMapping } from "@/types";
import {
  getLocationRulesCount,
  type HostMappingAvailabilityState,
} from "./model";

const props = defineProps<{
  availabilityState: HostMappingAvailabilityState;
  availabilityWindow: string;
  formatHost: (host: string) => string;
  handleLocationRulesTooltipOpenChange: (host: string, open: boolean) => void;
  handleLocationRulesTooltipTriggerClick: (host: string) => void;
  isAuthService: boolean;
  isGatewayPortalEnabled: boolean;
  isLocationRulesTooltipOpen: (host: string) => boolean;
  mapping: HostMapping;
}>();

const { t } = useI18n();

const locationRulesCount = computed(() => getLocationRulesCount(props.mapping));
const shouldShowToolbarIndicator = computed(
  () =>
    props.isGatewayPortalEnabled &&
    props.mapping.use_auth &&
    !props.mapping.suppress_toolbar &&
    !isWebSocketProxyTargetUrl(props.mapping.target),
);
</script>
