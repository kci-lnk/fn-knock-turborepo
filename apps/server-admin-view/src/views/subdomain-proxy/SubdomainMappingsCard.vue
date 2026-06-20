<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import {
  ChevronDown,
  CircleAlert,
  Download,
  Eraser,
  GripVertical,
  Image,
  PanelsTopLeft,
  Plus,
  RefreshCw,
  Route as RouteIcon,
  Search,
  ShieldCheck,
  Trash2,
} from "lucide-vue-next";
import { Badge } from "@/components/ui/badge";
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
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import {
  Popover,
  PopoverAnchor,
  PopoverContent,
} from "@/components/ui/popover";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import {
  Table,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { VueDraggable } from "vue-draggable-plus";
import HostTrafficActivity from "@/components/HostTrafficActivity.vue";
import InlineCommentEditor from "@admin-shared/components/InlineCommentEditor.vue";
import SearchInput from "@admin-shared/components/SearchInput.vue";
import { isWebSocketProxyTargetUrl } from "@admin-shared/utils/proxyTargetInput";
import type { HostTrafficStats, HostMapping } from "@/types";
import {
  getLocationRulesCount,
  getMappingDisplayTitle,
  getMappingFaviconSrc,
} from "./model";

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
  getHostTrafficSample: (host: string) => HostTrafficStats | null;
  getMappingTitleForDisplay: (mapping: HostMapping) => string;
  handleLocationRulesTooltipOpenChange: (
    host: string,
    open: boolean,
  ) => void;
  handleLocationRulesTooltipTriggerClick: (host: string) => void;
  handleProtocolHeadersWarningOpenChange: (
    host: string,
    open: boolean,
  ) => void;
  hasRegularHostMappings: boolean;
  isClearingAllSubdomainConfig: boolean;
  isConfigLoading: boolean;
  isDiscovering: boolean;
  isExportingBookmarks: boolean;
  isFaviconBroken: (mapping: HostMapping) => boolean;
  isGatewayPortalEnabled: boolean;
  isLocationRulesTooltipOpen: (host: string) => boolean;
  isProtocolHeadersWarningOpen: (host: string) => boolean;
  isRefreshingTitles: boolean;
  isRootDomainPendingSave: boolean;
  isSavingMappings: boolean;
  isSyncing: boolean;
  isAuthServiceTarget: (target: string) => boolean;
  markFaviconBroken: (mapping: HostMapping) => void;
  openProtocolHeadersWarning: (host: string) => void;
  saveMappingTitleOverride: (
    mapping: HostMapping,
    value: string,
  ) => Promise<void>;
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
  "copy-host": [mapping: HostMapping];
  "delete": [host: string];
  edit: [mapping: HostMapping];
  "export-bookmarks": [];
  "open-clear-all-config": [];
  "open-create": [];
  "open-discover": [];
  "open-gateway-locations": [host: string];
  "open-stale-cleanup": [];
  "refresh-all-titles": [];
  "save-order": [];
  "sync-routes": [];
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
        v-if="!savedRootDomain || isRootDomainPendingSave"
        class="text-xs text-amber-600"
      >
        {{
          !savedRootDomain
            ? t("admin.subdomainProxy.rootDomainRequired")
            : t("admin.subdomainProxy.rootDomainDirty")
        }}
      </p>

      <div class="overflow-hidden rounded-md border">
        <Table container-class="mapping-table-scroll">
          <TableHeader>
            <TableRow>
              <TableHead
                class="mapping-sticky-cell mapping-sticky-cell-1"
              ></TableHead>
              <TableHead
                class="mapping-sticky-cell mapping-sticky-cell-2 mapping-icon-cell"
              >
                <span class="sr-only">Icon</span>
              </TableHead>
              <TableHead
                class="mapping-sticky-cell mapping-sticky-cell-3 mapping-title-cell"
              >
                {{ t("admin.subdomainProxy.columns.title") }}
              </TableHead>
              <TableHead>{{ t("admin.subdomainProxy.columns.domain") }}</TableHead>
              <TableHead>{{ t("admin.subdomainProxy.columns.target") }}</TableHead>
              <TableHead class="w-[7rem] min-w-[7rem] max-w-[7rem]">
                {{ t("admin.subdomainProxy.columns.traffic") }}
              </TableHead>
              <TableHead class="w-[5.5rem] min-w-[5.5rem]">
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
              class="group"
            >
              <TableCell
                class="mapping-sticky-cell mapping-sticky-cell-1 mapping-icon-cell"
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
                class="mapping-sticky-cell mapping-sticky-cell-2 mapping-icon-cell"
              >
                <img
                  v-if="getMappingFaviconSrc(mapping) && !isFaviconBroken(mapping)"
                  :src="getMappingFaviconSrc(mapping)"
                  :alt="`${getMappingTitleForDisplay(mapping)} favicon`"
                  class="h-4 w-4 object-contain"
                  @error="markFaviconBroken(mapping)"
                />
              </TableCell>
              <TableCell
                class="mapping-sticky-cell mapping-sticky-cell-3 mapping-title-cell text-sm"
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
                          {{ t("admin.subdomainProxy.goDisableProtocolHeaders") }}
                        </a>
                      </div>
                    </PopoverContent>
                  </Popover>
                  <div class="min-w-0 flex-1">
                    <InlineCommentEditor
                      :text="getMappingDisplayTitle(mapping)"
                      :placeholder="t('admin.subdomainProxy.titlePlaceholder')"
                      :empty-text="t('admin.subdomainProxy.notFetched')"
                      :save="(value) => saveMappingTitleOverride(mapping, value)"
                    />
                  </div>
                </div>
              </TableCell>
              <TableCell class="break-all font-medium">
                <button
                  type="button"
                  class="break-all rounded-sm text-left transition-colors hover:text-primary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
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
              <TableCell>{{ mapping.target }}</TableCell>
              <TableCell class="w-[7rem] min-w-[7rem] max-w-[7rem]">
                <HostTrafficActivity
                  :host="mapping.host"
                  :title="getMappingTitleForDisplay(mapping)"
                  :sample="getHostTrafficSample(mapping.host)"
                  :timestamp="trafficTimestamp ?? null"
                />
              </TableCell>
              <TableCell class="w-[5.5rem] min-w-[5.5rem]">
                <div
                  class="flex min-w-max flex-nowrap items-center gap-2 text-xs text-muted-foreground"
                >
                  <Badge
                    v-if="isAuthServiceTarget(mapping.target)"
                    variant="default"
                  >
                    {{ t("admin.subdomainProxy.authServiceBadge") }}
                  </Badge>
                  <ShieldCheck
                    v-if="mapping.use_auth"
                    class="h-3.5 w-3.5 shrink-0"
                  />
                  <Badge v-else variant="secondary">
                    {{ t("admin.subdomainProxy.publicAccess") }}
                  </Badge>
                  <PanelsTopLeft
                    v-if="
                      isGatewayPortalEnabled &&
                      mapping.use_auth &&
                      !mapping.suppress_toolbar &&
                      !isWebSocketProxyTargetUrl(mapping.target)
                    "
                    class="h-3.5 w-3.5 shrink-0"
                  />
                  <TooltipProvider v-if="getLocationRulesCount(mapping) > 0">
                    <Tooltip
                      :open="isLocationRulesTooltipOpen(mapping.host)"
                      @update:open="
                        (nextOpen) =>
                          handleLocationRulesTooltipOpenChange(
                            mapping.host,
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
                              count: getLocationRulesCount(mapping),
                            })
                          "
                          @click="
                            handleLocationRulesTooltipTriggerClick(mapping.host)
                          "
                        >
                          <RouteIcon class="h-3.5 w-3.5" />
                        </button>
                      </TooltipTrigger>
                      <TooltipContent side="top" align="center">
                        <p>
                          {{
                            t("admin.subdomainProxy.locationRulesCount", {
                              count: getLocationRulesCount(mapping),
                            })
                          }}
                        </p>
                      </TooltipContent>
                    </Tooltip>
                  </TooltipProvider>
                </div>
              </TableCell>
              <TableCell class="text-right">
                <div class="flex justify-end gap-2">
                  <Button
                    variant="ghost"
                    size="sm"
                    @click="emit('edit', mapping)"
                  >
                    {{ t("admin.subdomainProxy.edit") }}
                  </Button>
                  <Button
                    v-if="!isAuthServiceTarget(mapping.target)"
                    variant="ghost"
                    size="sm"
                    @click="emit('open-gateway-locations', mapping.host)"
                  >
                    {{ t("admin.subdomainProxy.paths") }}
                  </Button>
                  <Button
                    variant="ghost"
                    size="sm"
                    class="text-destructive hover:bg-destructive/10 hover:text-destructive"
                    :disabled="isSavingMappings"
                    @click="emit('delete', mapping.host)"
                  >
                    {{ t("admin.subdomainProxy.delete") }}
                  </Button>
                </div>
              </TableCell>
            </TableRow>
          </VueDraggable>
        </Table>
      </div>
    </CardContent>
  </Card>
</template>
