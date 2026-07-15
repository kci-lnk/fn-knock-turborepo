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
        <Tooltip
          :open="isMappingStatusTooltipOpen(mapping.host, 'availability')"
          @update:open="
            (nextOpen) =>
              handleMappingStatusTooltipOpenChange(
                mapping.host,
                'availability',
                nextOpen,
              )
          "
        >
          <TooltipTrigger as-child>
            <Badge
              as="button"
              type="button"
              variant="outline"
              class="inline-flex h-6 w-6 cursor-help items-center justify-center rounded-full border-amber-500/35 bg-amber-500/5 p-0 text-amber-700 transition-colors hover:bg-amber-500/10 dark:text-amber-300"
              :aria-label="t('admin.subdomainProxy.unavailableBadge')"
              @click="
                handleMappingStatusTooltipTriggerClick(
                  mapping.host,
                  'availability',
                )
              "
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
        <Tooltip
          :open="isMappingStatusTooltipOpen(mapping.host, 'availability')"
          @update:open="
            (nextOpen) =>
              handleMappingStatusTooltipOpenChange(
                mapping.host,
                'availability',
                nextOpen,
              )
          "
        >
          <TooltipTrigger as-child>
            <button
              type="button"
              class="inline-flex h-5 w-5 shrink-0 cursor-help items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-muted hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/40"
              :aria-label="t('admin.subdomainProxy.scheduleOpenAria')"
              @click="
                handleMappingStatusTooltipTriggerClick(
                  mapping.host,
                  'availability',
                )
              "
            >
              <Clock class="h-3.5 w-3.5" />
            </button>
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
        <Tooltip
          :open="isMappingStatusTooltipOpen(mapping.host, 'default-domain')"
          @update:open="
            (nextOpen) =>
              handleMappingStatusTooltipOpenChange(
                mapping.host,
                'default-domain',
                nextOpen,
              )
          "
        >
          <TooltipTrigger as-child>
            <button
              type="button"
              class="inline-flex h-5 w-5 shrink-0 cursor-help items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-muted hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/40"
              :aria-label="
                t('admin.subdomainProxy.defaultDomainAria', {
                  host: formatHost(mapping.host),
                })
              "
              @click="
                handleMappingStatusTooltipTriggerClick(
                  mapping.host,
                  'default-domain',
                )
              "
            >
              <Star class="h-3.5 w-3.5" />
            </button>
          </TooltipTrigger>
          <TooltipContent side="top" align="center">
            <p>{{ t("admin.subdomainProxy.defaultDomain") }}</p>
          </TooltipContent>
        </Tooltip>
      </TooltipProvider>

      <TooltipProvider v-if="mapping.use_auth">
        <Tooltip
          :open="isMappingStatusTooltipOpen(mapping.host, 'authentication')"
          @update:open="
            (nextOpen) =>
              handleMappingStatusTooltipOpenChange(
                mapping.host,
                'authentication',
                nextOpen,
              )
          "
        >
          <TooltipTrigger as-child>
            <button
              type="button"
              class="inline-flex h-5 w-5 shrink-0 cursor-help items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-muted hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/40"
              :aria-label="
                t('admin.subdomainProxy.statusAuthRequiredAria', {
                  host: formatHost(mapping.host),
                })
              "
              @click="
                handleMappingStatusTooltipTriggerClick(
                  mapping.host,
                  'authentication',
                )
              "
            >
              <ShieldCheck class="h-3.5 w-3.5" />
            </button>
          </TooltipTrigger>
          <TooltipContent side="top" align="center">
            <p>{{ t("admin.subdomainProxy.statusAuthRequiredTooltip") }}</p>
          </TooltipContent>
        </Tooltip>
      </TooltipProvider>
      <Badge v-else variant="secondary">
        {{ t("admin.subdomainProxy.publicAccess") }}
      </Badge>

      <TooltipProvider v-if="securityIndicators.waf">
        <Tooltip
          :open="isMappingStatusTooltipOpen(mapping.host, 'waf')"
          @update:open="
            (nextOpen) =>
              handleMappingStatusTooltipOpenChange(
                mapping.host,
                'waf',
                nextOpen,
              )
          "
        >
          <TooltipTrigger as-child>
            <button
              type="button"
              class="inline-flex h-5 w-5 shrink-0 cursor-help items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-muted hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/40"
              :aria-label="
                t('admin.subdomainProxy.statusWafEnabledAria', {
                  host: formatHost(mapping.host),
                })
              "
              @click="
                handleMappingStatusTooltipTriggerClick(mapping.host, 'waf')
              "
            >
              <BrickWall class="h-3.5 w-3.5" />
            </button>
          </TooltipTrigger>
          <TooltipContent side="top" align="center">
            <p>{{ t("admin.subdomainProxy.statusWafEnabledTooltip") }}</p>
          </TooltipContent>
        </Tooltip>
      </TooltipProvider>

      <TooltipProvider v-if="securityIndicators.visibility">
        <Tooltip
          :open="isMappingStatusTooltipOpen(mapping.host, 'visibility')"
          @update:open="
            (nextOpen) =>
              handleMappingStatusTooltipOpenChange(
                mapping.host,
                'visibility',
                nextOpen,
              )
          "
        >
          <TooltipTrigger as-child>
            <button
              type="button"
              class="inline-flex h-5 w-5 shrink-0 cursor-help items-center justify-center rounded-md transition-colors hover:bg-muted focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/40"
              :class="
                securityIndicators.visibility === 'custom'
                  ? 'text-primary hover:text-primary'
                  : 'text-muted-foreground hover:text-foreground'
              "
              :aria-label="
                securityIndicators.visibility === 'custom'
                  ? t('admin.subdomainProxy.statusVisibilityCustomAria', {
                      host: formatHost(mapping.host),
                      regions: securityIndicators.regionCount,
                      cidrs: securityIndicators.customCidrCount,
                    })
                  : t('admin.subdomainProxy.statusVisibilityInheritAria', {
                      host: formatHost(mapping.host),
                    })
              "
              @click="
                handleMappingStatusTooltipTriggerClick(
                  mapping.host,
                  'visibility',
                )
              "
            >
              <ScanEye
                v-if="securityIndicators.visibility === 'custom'"
                class="h-3.5 w-3.5"
              />
              <Eye v-else class="h-3.5 w-3.5" />
            </button>
          </TooltipTrigger>
          <TooltipContent side="top" align="center">
            <p v-if="securityIndicators.visibility === 'custom'">
              {{
                t("admin.subdomainProxy.statusVisibilityCustomTooltip", {
                  regions: securityIndicators.regionCount,
                  cidrs: securityIndicators.customCidrCount,
                })
              }}
            </p>
            <p v-else>
              {{ t("admin.subdomainProxy.statusVisibilityInheritTooltip") }}
            </p>
          </TooltipContent>
        </Tooltip>
      </TooltipProvider>

      <TooltipProvider v-if="shouldShowToolbarIndicator">
        <Tooltip
          :open="isMappingStatusTooltipOpen(mapping.host, 'toolbar')"
          @update:open="
            (nextOpen) =>
              handleMappingStatusTooltipOpenChange(
                mapping.host,
                'toolbar',
                nextOpen,
              )
          "
        >
          <TooltipTrigger as-child>
            <button
              type="button"
              class="inline-flex h-5 w-5 shrink-0 cursor-help items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-muted hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/40"
              :aria-label="
                t('admin.subdomainProxy.statusToolbarEnabledAria', {
                  host: formatHost(mapping.host),
                })
              "
              @click="
                handleMappingStatusTooltipTriggerClick(mapping.host, 'toolbar')
              "
            >
              <PanelsTopLeft class="h-3.5 w-3.5" />
            </button>
          </TooltipTrigger>
          <TooltipContent side="top" align="center">
            <p>{{ t("admin.subdomainProxy.statusToolbarEnabledTooltip") }}</p>
          </TooltipContent>
        </Tooltip>
      </TooltipProvider>

      <TooltipProvider v-if="locationRulesCount > 0">
        <Tooltip
          :open="isMappingStatusTooltipOpen(mapping.host, 'location-rules')"
          @update:open="
            (nextOpen) =>
              handleMappingStatusTooltipOpenChange(
                mapping.host,
                'location-rules',
                nextOpen,
              )
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
              @click="
                handleMappingStatusTooltipTriggerClick(
                  mapping.host,
                  'location-rules',
                )
              "
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
  BrickWall,
  CircleOff,
  Clock,
  Eye,
  PanelsTopLeft,
  Route as RouteIcon,
  ScanEye,
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
  getMappingSecurityIndicatorState,
  type HostMappingAvailabilityState,
} from "./model";
import type { MappingStatusTooltip } from "./useSubdomainTouchTooltips";

const props = defineProps<{
  availabilityState: HostMappingAvailabilityState;
  availabilityWindow: string;
  formatHost: (host: string) => string;
  handleMappingStatusTooltipOpenChange: (
    host: string,
    tooltip: MappingStatusTooltip,
    open: boolean,
  ) => void;
  handleMappingStatusTooltipTriggerClick: (
    host: string,
    tooltip: MappingStatusTooltip,
  ) => void;
  globalVisibilityEnabled: boolean;
  globalWafEnabled: boolean;
  isAuthService: boolean;
  isGatewayPortalEnabled: boolean;
  isMappingStatusTooltipOpen: (
    host: string,
    tooltip: MappingStatusTooltip,
  ) => boolean;
  mapping: HostMapping;
}>();

const { t } = useI18n();

const locationRulesCount = computed(() => getLocationRulesCount(props.mapping));
const securityIndicators = computed(() =>
  getMappingSecurityIndicatorState({
    globalVisibilityEnabled: props.globalVisibilityEnabled,
    globalWafEnabled: props.globalWafEnabled,
    isAuthService: props.isAuthService,
    mapping: props.mapping,
  }),
);
const shouldShowToolbarIndicator = computed(
  () =>
    props.isGatewayPortalEnabled &&
    props.mapping.use_auth &&
    !props.mapping.suppress_toolbar &&
    !isWebSocketProxyTargetUrl(props.mapping.target),
);
</script>
