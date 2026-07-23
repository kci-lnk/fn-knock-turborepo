<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import {
  CalendarClock,
  ChevronDown,
  CircleAlert,
  Download,
  Eraser,
  GripVertical,
  Image,
  MoreHorizontal,
  Plus,
  Power,
  PowerOff,
  RefreshCw,
  Route as RouteIcon,
  Search,
  SlidersHorizontal,
  ShieldCheck,
  ShieldOff,
  Star,
  StarOff,
  Trash2,
} from "lucide-vue-next";
import { Button, type ButtonVariants } from "@/components/ui/button";
import DocsLinkButton from "@/components/DocsLinkButton.vue";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuSub,
  DropdownMenuSubContent,
  DropdownMenuSubTrigger,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import {
  Popover,
  PopoverAnchor,
  PopoverContent,
} from "@/components/ui/popover";
import {
  Table,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { VueDraggable } from "vue-draggable-plus";
import HostTrafficActivity from "@/components/HostTrafficActivity.vue";
import SearchInput from "@admin-shared/components/SearchInput.vue";
import type { HostTrafficStats, HostMapping } from "@/types";
import {
  getMappingDisplayTitle,
  getMappingFaviconSrc,
  isHttpTargetUrl,
  type HostMappingAvailabilityState,
} from "./model";
import type { MappingStatusTooltip } from "./useSubdomainTouchTooltips";
import SubdomainMappingStatusIndicators from "./SubdomainMappingStatusIndicators.vue";

const props = defineProps<{
  allMappingsCount: number;
  authServiceMapping: HostMapping | null;
  canManageNewMappings: boolean;
  discoverButtonDividerClass: string;
  discoverButtonVariant: ButtonVariants["variant"];
  docsHref: string;
  draggableMappings: HostMapping[];
  filteredMappings: HostMapping[];
  formatHost: (host: string) => string;
  formatAvailabilityWindow: (mapping: HostMapping) => string;
  getAvailabilityState: (mapping: HostMapping) => HostMappingAvailabilityState;
  getHostTrafficSample: (host: string) => HostTrafficStats | null;
  getMappingTitleForDisplay: (mapping: HostMapping) => string;
  globalVisibilityEnabled: boolean;
  globalWafEnabled: boolean;
  handleMappingStatusTooltipOpenChange: (
    host: string,
    tooltip: MappingStatusTooltip,
    open: boolean,
  ) => void;
  handleMappingStatusTooltipTriggerClick: (
    host: string,
    tooltip: MappingStatusTooltip,
  ) => void;
  handleProtocolHeadersWarningOpenChange: (host: string, open: boolean) => void;
  hasRegularHostMappings: boolean;
  isClearingAllSubdomainConfig: boolean;
  isConfigLoading: boolean;
  isDiscovering: boolean;
  isExportingBookmarks: boolean;
  isFaviconBroken: (mapping: HostMapping) => boolean;
  isGatewayPortalEnabled: boolean;
  isMappingUnavailable: (mapping: HostMapping) => boolean;
  isMappingStatusTooltipOpen: (
    host: string,
    tooltip: MappingStatusTooltip,
  ) => boolean;
  isProtocolHeadersWarningOpen: (host: string) => boolean;
  isRefreshingTitles: boolean;
  isRootDomainPendingSave: boolean;
  isSavingMappings: boolean;
  isSyncing: boolean;
  isAuthServiceTarget: (target: string) => boolean;
  markFaviconBroken: (mapping: HostMapping) => void;
  openProtocolHeadersWarning: (host: string) => void;
  rootDomainValidationMessage: string;
  savedRootDomain: string;
  scheduleCloseProtocolHeadersWarning: (host: string) => void;
  searchQuery: string;
  shouldShowProtocolHeadersWarning: (mapping: HostMapping) => boolean;
  toggleProtocolHeadersWarning: (host: string) => void;
  trafficTimestamp: number | null | undefined;
  visibleMappingsCount: number;
}>();

const emit = defineEmits<{
  "add-auth-service": [];
  "clear-default": [mapping: HostMapping];
  "copy-host": [mapping: HostMapping];
  delete: [host: string];
  edit: [mapping: HostMapping];
  "export-bookmarks": [];
  "open-clear-all-config": [];
  "open-create": [];
  "open-discover": [];
  "open-discover-settings": [];
  "open-availability": [mapping: HostMapping];
  "open-gateway-locations": [host: string];
  "open-advanced-auth": [host: string];
  "open-stale-cleanup": [];
  "refresh-all-titles": [];
  "save-order": [];
  "set-default": [mapping: HostMapping];
  "sync-routes": [];
  "toggle-enabled": [mapping: HostMapping];
  "update:draggableMappings": [mappings: HostMapping[]];
  "update:searchQuery": [value: string];
}>();

const { t } = useI18n();

const draggableModel = computed({
  get: () => props.draggableMappings,
  set: (value: HostMapping[]) => emit("update:draggableMappings", value),
});

const searchModel = computed({
  get: () => props.searchQuery,
  set: (value: string) => emit("update:searchQuery", value),
});

const isMappingTableScrolled = ref(false);

const handleMappingTableScroll = (event: Event) => {
  if (!(event.currentTarget instanceof HTMLElement)) return;
  isMappingTableScrolled.value = event.currentTarget.scrollLeft > 0;
};
</script>

<template>
  <Card>
    <CardHeader>
      <CardTitle class="flex items-center justify-between">
        <span>{{ t("admin.subdomainProxy.mappingsTitle") }}</span>
        <div class="flex items-center gap-2">
          <DocsLinkButton :href="docsHref" />
          <Button
            v-if="!authServiceMapping"
            :disabled="!canManageNewMappings || isSavingMappings"
            variant="default"
            @click="emit('add-auth-service')"
          >
            <ShieldCheck class="mr-2 h-4 w-4" />
            {{ t("admin.subdomainProxy.addAuthService") }}
          </Button>
          <div v-if="authServiceMapping" class="flex items-center">
            <Button
              :variant="discoverButtonVariant"
              :disabled="!canManageNewMappings || isDiscovering"
              class="rounded-r-none"
              @click="emit('open-discover')"
            >
              <Search class="mr-2 h-4 w-4" />
              {{
                isDiscovering
                  ? t("admin.subdomainProxy.discovering")
                  : t("admin.subdomainProxy.discover")
              }}
            </Button>
            <DropdownMenu>
              <DropdownMenuTrigger as-child>
                <Button
                  :variant="discoverButtonVariant"
                  size="icon"
                  :class="[
                    'rounded-l-none border-l px-2',
                    discoverButtonDividerClass,
                  ]"
                >
                  <ChevronDown class="h-4 w-4" />
                </Button>
              </DropdownMenuTrigger>
              <DropdownMenuContent align="end">
                <DropdownMenuItem
                  :disabled="isDiscovering"
                  @select="emit('open-discover-settings')"
                >
                  <SlidersHorizontal class="mr-2 h-4 w-4" />
                  {{ t("admin.scanIntensity.title") }}
                </DropdownMenuItem>
                <DropdownMenuItem
                  v-if="authServiceMapping"
                  variant="destructive"
                  :disabled="isSavingMappings || isClearingAllSubdomainConfig"
                  @select="emit('open-clear-all-config')"
                >
                  <Trash2 class="mr-2 h-4 w-4" />
                  {{ t("admin.subdomainProxy.clearAllConfig") }}
                </DropdownMenuItem>
                <DropdownMenuItem
                  :disabled="
                    !hasRegularHostMappings ||
                    isSavingMappings ||
                    isClearingAllSubdomainConfig
                  "
                  @select="emit('open-stale-cleanup')"
                >
                  <Eraser class="mr-2 h-4 w-4" />
                  {{ t("admin.subdomainProxy.cleanupStaleServices") }}
                </DropdownMenuItem>
                <DropdownMenuItem
                  :disabled="isConfigLoading"
                  @click="emit('open-create')"
                >
                  <Plus class="mr-2 h-4 w-4" />
                  {{ t("admin.subdomainProxy.addMapping") }}
                </DropdownMenuItem>
                <DropdownMenuItem
                  :disabled="isSyncing"
                  @click="emit('sync-routes')"
                >
                  <RefreshCw
                    class="mr-2 h-4 w-4"
                    :class="{ 'animate-spin': isSyncing }"
                  />
                  {{
                    isSyncing
                      ? t("admin.subdomainProxy.syncing")
                      : t("admin.subdomainProxy.syncRoutes")
                  }}
                </DropdownMenuItem>
                <DropdownMenuSeparator />
                <DropdownMenuItem
                  :disabled="isRefreshingTitles || allMappingsCount === 0"
                  @select="emit('refresh-all-titles')"
                >
                  <Image
                    class="mr-2 h-4 w-4"
                    :class="{ 'animate-pulse': isRefreshingTitles }"
                  />
                  {{
                    isRefreshingTitles
                      ? t("admin.subdomainProxy.refreshing")
                      : t("admin.subdomainProxy.refreshIconsTitles")
                  }}
                </DropdownMenuItem>
                <DropdownMenuItem
                  :disabled="isExportingBookmarks || visibleMappingsCount === 0"
                  @select="emit('export-bookmarks')"
                >
                  <Download
                    class="mr-2 h-4 w-4"
                    :class="{ 'animate-pulse': isExportingBookmarks }"
                  />
                  {{
                    isExportingBookmarks
                      ? t("admin.subdomainProxy.exporting")
                      : t("admin.subdomainProxy.exportBookmarks")
                  }}
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenu>
          </div>
        </div>
      </CardTitle>
      <CardDescription>
        {{ t("admin.subdomainProxy.mappingsDescription") }}
      </CardDescription>
    </CardHeader>
    <CardContent class="space-y-4">
      <SearchInput
        v-model="searchModel"
        :placeholder="t('admin.subdomainProxy.searchPlaceholder')"
        class="max-w-xs"
      />
      <p v-if="visibleMappingsCount > 1" class="text-xs text-muted-foreground">
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
      <p
        v-if="
          rootDomainValidationMessage ||
          !savedRootDomain ||
          isRootDomainPendingSave
        "
        class="text-xs text-amber-600"
      >
        {{
          rootDomainValidationMessage ||
          (!savedRootDomain
            ? t("admin.subdomainProxy.rootDomainRequired")
            : t("admin.subdomainProxy.rootDomainDirty"))
        }}
      </p>

      <div class="overflow-hidden rounded-md border">
        <Table
          :container-class="[
            'mapping-table-scroll',
            { 'mapping-table-scroll--scrolled': isMappingTableScrolled },
          ]"
          @scroll.passive="handleMappingTableScroll"
        >
          <TableHeader>
            <TableRow>
              <TableHead
                class="mapping-sticky-cell mapping-order-cell mapping-icon-cell"
              ></TableHead>
              <TableHead
                class="mapping-sticky-cell mapping-favicon-cell mapping-icon-cell"
              >
                <span class="sr-only">Icon</span>
              </TableHead>
              <TableHead class="mapping-sticky-cell mapping-title-cell">
                {{ t("admin.subdomainProxy.columns.title") }}
              </TableHead>
              <TableHead>{{
                t("admin.subdomainProxy.columns.domain")
              }}</TableHead>
              <TableHead>{{
                t("admin.subdomainProxy.columns.target")
              }}</TableHead>
              <TableHead class="w-[7rem] min-w-[7rem] max-w-[7rem]">
                {{ t("admin.subdomainProxy.columns.traffic") }}
              </TableHead>
              <TableHead class="w-[8rem] min-w-[8rem]">
                {{ t("admin.subdomainProxy.columns.status") }}
              </TableHead>
              <TableHead class="text-right">
                {{ t("admin.subdomainProxy.columns.actions") }}
              </TableHead>
            </TableRow>
          </TableHeader>
          <VueDraggable
            v-model="draggableModel"
            tag="tbody"
            class="[&_tr:last-child]:border-0"
            handle=".mapping-drag-handle"
            ghost-class="bg-muted/60"
            chosen-class="bg-muted/80"
            :animation="180"
            :disabled="isSavingMappings || filteredMappings.length < 2"
            @end="emit('save-order')"
          >
            <TableRow v-if="filteredMappings.length === 0">
              <TableCell
                colspan="8"
                class="py-8 text-center text-muted-foreground"
              >
                {{ t("admin.subdomainProxy.emptyMappings") }}
              </TableCell>
            </TableRow>
            <TableRow
              v-for="mapping in draggableModel"
              :key="mapping.host"
              :class="[
                'group',
                isMappingUnavailable(mapping) ? 'text-muted-foreground' : '',
              ]"
            >
              <TableCell
                class="mapping-sticky-cell mapping-order-cell mapping-icon-cell"
              >
                <button
                  type="button"
                  class="mapping-drag-handle -ml-1 inline-flex h-7 w-7 items-center justify-center rounded-md text-muted-foreground transition hover:bg-muted hover:text-foreground disabled:pointer-events-none disabled:opacity-40"
                  :disabled="isSavingMappings || filteredMappings.length < 2"
                  :aria-label="t('admin.subdomainProxy.dragSortAria')"
                >
                  <GripVertical class="h-4 w-4" />
                </button>
              </TableCell>
              <TableCell
                class="mapping-sticky-cell mapping-favicon-cell mapping-icon-cell"
              >
                <img
                  v-if="
                    getMappingFaviconSrc(mapping) && !isFaviconBroken(mapping)
                  "
                  :src="getMappingFaviconSrc(mapping)"
                  :alt="`${getMappingTitleForDisplay(mapping)} favicon`"
                  class="h-4 w-4 object-contain transition-opacity"
                  :class="{ 'opacity-45': isMappingUnavailable(mapping) }"
                  @error="markFaviconBroken(mapping)"
                />
              </TableCell>
              <TableCell
                class="mapping-sticky-cell mapping-title-cell text-sm"
                :title="getMappingTitleForDisplay(mapping)"
              >
                <div class="flex min-w-0 items-center gap-2">
                  <Popover
                    v-if="shouldShowProtocolHeadersWarning(mapping)"
                    :open="isProtocolHeadersWarningOpen(mapping.host)"
                    @update:open="
                      (nextOpen) =>
                        handleProtocolHeadersWarningOpenChange(
                          mapping.host,
                          nextOpen,
                        )
                    "
                  >
                    <PopoverAnchor as-child>
                      <button
                        type="button"
                        class="inline-flex h-5 w-5 shrink-0 items-center justify-center rounded-md text-destructive transition-colors hover:bg-destructive/10 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-destructive/30"
                        :class="{
                          'bg-destructive/10': isProtocolHeadersWarningOpen(
                            mapping.host,
                          ),
                        }"
                        :aria-label="
                          t('admin.subdomainProxy.homeAssistantWarningAria', {
                            host: formatHost(mapping.host),
                          })
                        "
                        @mouseenter="openProtocolHeadersWarning(mapping.host)"
                        @mouseleave="
                          scheduleCloseProtocolHeadersWarning(mapping.host)
                        "
                        @click="toggleProtocolHeadersWarning(mapping.host)"
                      >
                        <CircleAlert class="h-3.5 w-3.5" />
                      </button>
                    </PopoverAnchor>
                    <PopoverContent
                      side="top"
                      align="start"
                      class="w-72 border-destructive/20 text-left"
                      @mouseenter="openProtocolHeadersWarning(mapping.host)"
                      @mouseleave="
                        scheduleCloseProtocolHeadersWarning(mapping.host)
                      "
                    >
                      <div class="space-y-3">
                        <div class="space-y-1">
                          <div class="flex items-center gap-2">
                            <CircleAlert class="h-4 w-4 text-destructive" />
                            <p class="text-sm font-medium">
                              {{
                                t(
                                  "admin.subdomainProxy.homeAssistantWarningTitle",
                                )
                              }}
                            </p>
                          </div>
                          <p class="text-xs leading-5 text-muted-foreground">
                            {{
                              t(
                                "admin.subdomainProxy.homeAssistantWarningDescription",
                              )
                            }}
                          </p>
                        </div>
                        <a
                          href="#/system/gateway-proxy-headers"
                          class="inline-flex rounded-md border border-destructive/20 bg-destructive/5 px-2.5 py-1.5 text-xs font-medium text-destructive transition hover:bg-destructive/10"
                        >
                          {{
                            t("admin.subdomainProxy.goDisableProtocolHeaders")
                          }}
                        </a>
                      </div>
                    </PopoverContent>
                  </Popover>
                  <button
                    type="button"
                    class="min-w-0 flex-1 rounded-sm text-left text-sm transition-colors hover:text-primary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
                    :title="t('admin.subdomainProxy.edit')"
                    :aria-label="
                      t('admin.subdomainProxy.editMappingAria', {
                        host: formatHost(mapping.host),
                      })
                    "
                    @click="emit('edit', mapping)"
                  >
                    <span class="block truncate">
                      {{ getMappingDisplayTitle(mapping) }}
                    </span>
                  </button>
                </div>
              </TableCell>
              <TableCell class="break-all font-medium">
                <button
                  type="button"
                  class="break-all rounded-sm text-left transition-colors hover:text-primary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
                  :class="{
                    'text-muted-foreground hover:text-foreground':
                      isMappingUnavailable(mapping),
                  }"
                  :title="
                    t('admin.subdomainProxy.copyHostTitle', {
                      host: formatHost(mapping.host),
                    })
                  "
                  :aria-label="
                    t('admin.subdomainProxy.copyHostAria', {
                      host: formatHost(mapping.host),
                    })
                  "
                  @click="emit('copy-host', mapping)"
                >
                  {{ formatHost(mapping.host) }}
                </button>
              </TableCell>
              <TableCell
                :class="{
                  'text-muted-foreground': isMappingUnavailable(mapping),
                }"
              >
                {{ mapping.target }}
              </TableCell>
              <TableCell class="w-[7rem] min-w-[7rem] max-w-[7rem]">
                <HostTrafficActivity
                  :host="mapping.host"
                  :title="getMappingTitleForDisplay(mapping)"
                  :sample="getHostTrafficSample(mapping.host)"
                  :timestamp="trafficTimestamp ?? null"
                />
              </TableCell>
              <TableCell class="w-[8rem] min-w-[8rem]">
                <SubdomainMappingStatusIndicators
                  :mapping="mapping"
                  :availability-state="getAvailabilityState(mapping)"
                  :availability-window="formatAvailabilityWindow(mapping)"
                  :format-host="formatHost"
                  :global-visibility-enabled="globalVisibilityEnabled"
                  :global-waf-enabled="globalWafEnabled"
                  :is-auth-service="isAuthServiceTarget(mapping.target)"
                  :is-gateway-portal-enabled="isGatewayPortalEnabled"
                  :is-mapping-status-tooltip-open="isMappingStatusTooltipOpen"
                  :handle-mapping-status-tooltip-open-change="
                    handleMappingStatusTooltipOpenChange
                  "
                  :handle-mapping-status-tooltip-trigger-click="
                    handleMappingStatusTooltipTriggerClick
                  "
                />
              </TableCell>
              <TableCell class="text-right">
                <div class="flex justify-end">
                  <Button
                    variant="outline"
                    size="sm"
                    class="rounded-r-none"
                    @click="emit('edit', mapping)"
                  >
                    {{ t("admin.subdomainProxy.edit") }}
                  </Button>
                  <DropdownMenu>
                    <DropdownMenuTrigger as-child>
                      <Button
                        variant="outline"
                        size="icon"
                        class="h-8 w-8 rounded-l-none border-l-0"
                      >
                        <ChevronDown class="h-4 w-4" />
                      </Button>
                    </DropdownMenuTrigger>
                    <DropdownMenuContent align="end" class="w-44">
                      <DropdownMenuItem
                        v-if="!isAuthServiceTarget(mapping.target)"
                        @select="emit('open-gateway-locations', mapping.host)"
                      >
                        <RouteIcon class="mr-2 h-4 w-4" />
                        {{ t("admin.subdomainProxy.paths") }}
                      </DropdownMenuItem>
                      <DropdownMenuItem
                        v-if="
                          !isAuthServiceTarget(mapping.target) &&
                          mapping.use_auth &&
                          isHttpTargetUrl(mapping.target)
                        "
                        @select="emit('open-advanced-auth', mapping.host)"
                      >
                        <ShieldOff class="mr-2 h-4 w-4" />
                        {{ t("admin.subdomainProxy.advancedAuthConfig") }}
                      </DropdownMenuItem>
                      <DropdownMenuItem
                        v-if="
                          !isAuthServiceTarget(mapping.target) &&
                          mapping.is_default
                        "
                        :disabled="isSavingMappings"
                        @select="emit('clear-default', mapping)"
                      >
                        <StarOff class="mr-2 h-4 w-4" />
                        {{ t("admin.subdomainProxy.clearDefaultDomain") }}
                      </DropdownMenuItem>
                      <DropdownMenuItem
                        v-else-if="!isAuthServiceTarget(mapping.target)"
                        :disabled="isSavingMappings"
                        @select="emit('set-default', mapping)"
                      >
                        <Star class="mr-2 h-4 w-4" />
                        {{ t("admin.subdomainProxy.setDefaultDomain") }}
                      </DropdownMenuItem>
                      <DropdownMenuItem
                        v-if="!isAuthServiceTarget(mapping.target)"
                        :disabled="isSavingMappings"
                        @select="emit('toggle-enabled', mapping)"
                      >
                        <Power v-if="mapping.disabled" class="mr-2 h-4 w-4" />
                        <PowerOff v-else class="mr-2 h-4 w-4" />
                        {{
                          mapping.disabled
                            ? t("admin.subdomainProxy.enableMapping")
                            : t("admin.subdomainProxy.disableMapping")
                        }}
                      </DropdownMenuItem>
                      <DropdownMenuSub
                        v-if="!isAuthServiceTarget(mapping.target)"
                      >
                        <DropdownMenuSubTrigger :disabled="isSavingMappings">
                          <MoreHorizontal class="mr-2 h-4 w-4" />
                          {{ t("admin.subdomainProxy.moreActions") }}
                        </DropdownMenuSubTrigger>
                        <DropdownMenuSubContent class="w-48">
                          <DropdownMenuItem
                            :disabled="isSavingMappings"
                            @select="emit('open-availability', mapping)"
                          >
                            <CalendarClock class="mr-2 h-4 w-4" />
                            {{ t("admin.subdomainProxy.scheduleAvailability") }}
                          </DropdownMenuItem>
                        </DropdownMenuSubContent>
                      </DropdownMenuSub>
                      <DropdownMenuSeparator />
                      <DropdownMenuItem
                        variant="destructive"
                        :disabled="isSavingMappings"
                        @select="emit('delete', mapping.host)"
                      >
                        <Trash2 class="mr-2 h-4 w-4" />
                        {{ t("admin.subdomainProxy.delete") }}
                      </DropdownMenuItem>
                    </DropdownMenuContent>
                  </DropdownMenu>
                </div>
              </TableCell>
            </TableRow>
          </VueDraggable>
        </Table>
      </div>
    </CardContent>
  </Card>
</template>
