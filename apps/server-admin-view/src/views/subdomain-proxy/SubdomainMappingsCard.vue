<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import {
  CalendarClock,
  ChevronDown,
  ChevronRight,
  CircleAlert,
  Download,
  Eraser,
  FolderInput,
  FolderPlus,
  Folders,
  GripVertical,
  Image,
  ListTree,
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
import { Checkbox } from "@/components/ui/checkbox";
import HostTrafficActivity from "@/components/HostTrafficActivity.vue";
import SearchInput from "@admin-shared/components/SearchInput.vue";
import type { HostTrafficStats, HostMapping, HostMappingGroup } from "@/types";
import {
  getMappingDisplayTitle,
  getMappingFaviconSrc,
  isHttpTargetUrl,
  type HostMappingAvailabilityState,
} from "./model";
import type { MappingStatusTooltip } from "./useSubdomainTouchTooltips";
import SubdomainMappingStatusIndicators from "./SubdomainMappingStatusIndicators.vue";
import SubdomainMappingGroupRows from "./SubdomainMappingGroupRows.vue";
import SubdomainGroupManagerDialog from "./SubdomainGroupManagerDialog.vue";
import {
  buildHostMappingGroupSections,
  type HostMappingGroupSection,
} from "./host-mapping-groups";

const props = defineProps<{
  allMappingsCount: number;
  allRegularMappings: HostMapping[];
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
  groupedView: boolean;
  groups: HostMappingGroup[];
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
  isDefaultDomainAvailable: boolean;
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
  "move-mappings": [hosts: string[], groupId: string | null];
  "open-create": [groupId?: string | null];
  "open-discover": [];
  "open-discover-settings": [];
  "open-availability": [mapping: HostMapping];
  "open-gateway-locations": [host: string];
  "open-advanced-auth": [host: string];
  "open-stale-cleanup": [];
  "refresh-all-titles": [];
  "save-order": [];
  "save-grouped-order": [sections: HostMappingGroupSection[]];
  "save-groups": [
    groups: HostMappingGroup[],
    onComplete: (saved: boolean) => void,
  ];
  "set-default": [mapping: HostMapping];
  "sync-routes": [];
  "toggle-enabled": [mapping: HostMapping];
  "update-grouped-view": [value: boolean];
  "update:draggableMappings": [mappings: HostMapping[]];
  "update:searchQuery": [value: string];
}>();

const { t } = useI18n();

const searchModel = computed({
  get: () => props.searchQuery,
  set: (value: string) => emit("update:searchQuery", value),
});

const mappingSelectionCheckboxClass =
  "size-[18px] rounded-[5px] border-muted-foreground/40 bg-background shadow-none transition-[color,background-color,border-color,opacity] hover:border-primary/70 data-[state=indeterminate]:border-primary data-[state=indeterminate]:bg-primary data-[state=indeterminate]:text-primary-foreground";
const hiddenMappingSelectionCheckboxClass =
  "pointer-events-none opacity-0 group-hover:pointer-events-auto group-hover:opacity-100 group-focus-within:pointer-events-auto group-focus-within:opacity-100 [@media(hover:none)]:pointer-events-auto [@media(hover:none)]:opacity-100";
const isMappingTableScrolled = ref(false);
const isGroupManagerOpen = ref(false);
const groupSections = ref<HostMappingGroupSection[]>([]);
const selectedHosts = ref(new Set<string>());
const collapsedGroupKeys = ref(new Set<string>());
const collapseStorageKey = "fnknock.admin.hostMappingGroups.collapsed";

if (typeof window !== "undefined") {
  try {
    const stored = JSON.parse(
      window.localStorage.getItem(collapseStorageKey) || "[]",
    );
    if (Array.isArray(stored)) {
      collapsedGroupKeys.value = new Set(
        stored.filter((item): item is string => typeof item === "string"),
      );
    }
  } catch {
    collapsedGroupKeys.value = new Set();
  }
}

const hasGroups = computed(() => props.groups.length > 0);
const showGroupedView = computed(() => hasGroups.value && props.groupedView);
const isGroupedViewActive = computed(() => props.groupedView);
const dragDisabled = computed(
  () =>
    props.isSavingMappings ||
    Boolean(props.searchQuery.trim()) ||
    props.filteredMappings.length < 2,
);
const selectedCount = computed(() => selectedHosts.value.size);
const mappingSelectionVisibilityClass = computed(() =>
  selectedCount.value > 0
    ? "pointer-events-auto opacity-100"
    : hiddenMappingSelectionCheckboxClass,
);
const allVisibleSelected = computed(
  () =>
    props.filteredMappings.length > 0 &&
    props.filteredMappings.every((mapping) =>
      selectedHosts.value.has(mapping.host),
    ),
);
const someVisibleSelected = computed(
  () =>
    !allVisibleSelected.value &&
    props.filteredMappings.some((mapping) =>
      selectedHosts.value.has(mapping.host),
    ),
);

const syncGroupSections = () => {
  groupSections.value = buildHostMappingGroupSections(
    props.filteredMappings,
    showGroupedView.value ? props.groups : [],
    t("admin.subdomainProxy.ungrouped"),
    showGroupedView.value && !props.searchQuery.trim(),
  );
  const visibleHosts = new Set(props.filteredMappings.map((item) => item.host));
  selectedHosts.value = new Set(
    [...selectedHosts.value].filter((host) => visibleHosts.has(host)),
  );
};

watch(
  () =>
    [
      props.filteredMappings,
      props.groups,
      props.searchQuery,
      props.isSavingMappings,
      showGroupedView.value,
    ] as const,
  syncGroupSections,
  { deep: true, immediate: true },
);

const updateSectionMappings = (key: string, mappings: HostMapping[]) => {
  const section = groupSections.value.find((item) => item.key === key);
  if (section) section.mappings = mappings;
};

const handleSortEnd = async () => {
  await nextTick();
  if (showGroupedView.value) {
    emit(
      "save-grouped-order",
      groupSections.value.map((section) => ({
        ...section,
        mappings: [...section.mappings],
      })),
    );
    return;
  }
  emit("update:draggableMappings", groupSections.value[0]?.mappings ?? []);
  emit("save-order");
};

const isSectionCollapsed = (section: HostMappingGroupSection) =>
  props.searchQuery.trim() ? false : collapsedGroupKeys.value.has(section.key);

const toggleSectionCollapsed = (section: HostMappingGroupSection) => {
  const next = new Set(collapsedGroupKeys.value);
  if (next.has(section.key)) next.delete(section.key);
  else next.add(section.key);
  collapsedGroupKeys.value = next;
  if (typeof window !== "undefined") {
    window.localStorage.setItem(collapseStorageKey, JSON.stringify([...next]));
  }
};

const isMappingSelected = (host: string) => selectedHosts.value.has(host);
const setMappingSelected = (host: string, selected: boolean) => {
  const next = new Set(selectedHosts.value);
  if (selected) next.add(host);
  else next.delete(host);
  selectedHosts.value = next;
};
const isSectionSelected = (section: HostMappingGroupSection) =>
  section.mappings.length > 0 &&
  section.mappings.every((mapping) => selectedHosts.value.has(mapping.host));
const isSectionPartiallySelected = (section: HostMappingGroupSection) =>
  !isSectionSelected(section) &&
  section.mappings.some((mapping) => selectedHosts.value.has(mapping.host));
const setSectionSelected = (
  section: HostMappingGroupSection,
  selected: boolean,
) => {
  const next = new Set(selectedHosts.value);
  for (const mapping of section.mappings) {
    if (selected) next.add(mapping.host);
    else next.delete(mapping.host);
  }
  selectedHosts.value = next;
};
const setAllVisibleSelected = (selected: boolean) => {
  const next = new Set(selectedHosts.value);
  for (const mapping of props.filteredMappings) {
    if (selected) next.add(mapping.host);
    else next.delete(mapping.host);
  }
  selectedHosts.value = next;
};
const moveSelected = (groupId: string | null) => {
  emit("move-mappings", [...selectedHosts.value], groupId);
  selectedHosts.value = new Set();
};
const moveOne = (mapping: HostMapping, groupId: string | null) => {
  emit("move-mappings", [mapping.host], groupId);
};
const toggleGroupedView = () => {
  emit("update-grouped-view", !isGroupedViewActive.value);
  selectedHosts.value = new Set();
};
const saveGroupsAndCloseOnSuccess = (nextGroups: HostMappingGroup[]) => {
  emit("save-groups", nextGroups, (saved) => {
    if (saved) isGroupManagerOpen.value = false;
  });
};

watch(showGroupedView, () => {
  selectedHosts.value = new Set();
});

const handleMappingTableScroll = (event: Event) => {
  if (!(event.currentTarget instanceof HTMLElement)) return;
  isMappingTableScrolled.value = event.currentTarget.scrollLeft > 0;
};
</script>

<template>
  <Card>
    <CardHeader>
      <div
        class="flex flex-col items-stretch justify-between gap-4 sm:flex-row sm:items-center"
      >
        <CardTitle>{{ t("admin.subdomainProxy.mappingsTitle") }}</CardTitle>
        <div
          class="grid w-full grid-cols-[auto_auto_minmax(0,1fr)] items-center gap-2 sm:flex sm:w-auto sm:flex-wrap sm:justify-end"
        >
          <DocsLinkButton
            :href="docsHref"
            size="default"
            class="w-auto px-2 [&_svg]:hidden sm:px-3 sm:[&_svg]:block"
          />
          <Button
            :variant="isGroupedViewActive ? 'secondary' : 'outline'"
            :disabled="isSavingMappings"
            :aria-pressed="isGroupedViewActive"
            class="min-w-0 px-2 sm:w-auto sm:px-3"
            @click="toggleGroupedView"
          >
            <ListTree class="hidden h-4 w-4 sm:block" />
            <span class="truncate">{{
              t("admin.subdomainProxy.groupedView")
            }}</span>
          </Button>
          <Button
            v-if="isGroupedViewActive"
            variant="outline"
            :disabled="isSavingMappings"
            class="hidden sm:inline-flex"
            @click="isGroupManagerOpen = true"
          >
            <Folders class="mr-2 h-4 w-4" />
            {{ t("admin.subdomainProxy.manageGroups") }}
          </Button>
          <Button
            v-if="!authServiceMapping"
            :disabled="!canManageNewMappings || isSavingMappings"
            variant="default"
            class="col-span-3 w-full sm:w-auto"
            @click="emit('add-auth-service')"
          >
            <ShieldCheck class="mr-2 h-4 w-4" />
            {{ t("admin.subdomainProxy.addAuthService") }}
          </Button>
          <div
            v-if="authServiceMapping"
            class="flex min-w-0 w-full items-center sm:w-auto"
          >
            <Button
              :variant="discoverButtonVariant"
              :disabled="
                !canManageNewMappings || isDiscovering || isSavingMappings
              "
              class="min-w-0 flex-1 rounded-r-none px-2 text-xs sm:flex-none sm:px-3 sm:text-sm"
              @click="emit('open-discover')"
            >
              <Search class="hidden h-4 w-4 sm:block" />
              <span class="truncate">
                {{
                  isDiscovering
                    ? t("admin.subdomainProxy.discovering")
                    : t("admin.subdomainProxy.discover")
                }}
              </span>
            </Button>
            <DropdownMenu>
              <DropdownMenuTrigger as-child>
                <Button
                  data-testid="subdomain-discover-menu-trigger"
                  :variant="discoverButtonVariant"
                  size="icon"
                  :aria-label="t('common.moreActions')"
                  :disabled="isSavingMappings"
                  :class="[
                    'h-9 w-8 rounded-l-none border-l px-1 sm:w-9 sm:px-2',
                    discoverButtonDividerClass,
                  ]"
                >
                  <ChevronDown class="h-4 w-4" />
                </Button>
              </DropdownMenuTrigger>
              <DropdownMenuContent align="end">
                <DropdownMenuItem
                  v-if="isGroupedViewActive"
                  data-testid="mobile-manage-groups-menu-item"
                  class="sm:hidden"
                  :disabled="isSavingMappings"
                  @select="isGroupManagerOpen = true"
                >
                  <Folders class="mr-2 h-4 w-4" />
                  {{ t("admin.subdomainProxy.manageGroups") }}
                </DropdownMenuItem>
                <DropdownMenuSeparator
                  v-if="isGroupedViewActive"
                  class="sm:hidden"
                />
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
      </div>
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
      <div
        v-if="showGroupedView && selectedCount > 0"
        class="flex flex-wrap items-center gap-3 rounded-md border bg-muted/35 px-3 py-2"
        role="toolbar"
        :aria-label="t('admin.subdomainProxy.batchActions')"
      >
        <span class="text-sm font-medium">
          {{
            t("admin.subdomainProxy.selectedMappingsCount", {
              count: selectedCount,
            })
          }}
        </span>
        <DropdownMenu>
          <DropdownMenuTrigger as-child>
            <Button size="sm" variant="outline" :disabled="isSavingMappings">
              <FolderInput class="mr-2 h-4 w-4" />
              {{ t("admin.subdomainProxy.moveToGroup") }}
              <ChevronDown class="ml-2 h-4 w-4" />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="start">
            <DropdownMenuItem
              v-for="group in groups"
              :key="group.id"
              @select="moveSelected(group.id)"
            >
              {{ group.name }}
            </DropdownMenuItem>
            <DropdownMenuSeparator />
            <DropdownMenuItem @select="moveSelected(null)">
              {{ t("admin.subdomainProxy.ungrouped") }}
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
        <Button size="sm" variant="ghost" @click="selectedHosts = new Set()">
          {{ t("admin.subdomainProxy.clearSelection") }}
        </Button>
      </div>
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
            {
              'mapping-table-scroll--grouped': showGroupedView,
              'mapping-table-scroll--scrolled': isMappingTableScrolled,
            },
          ]"
          @scroll.passive="handleMappingTableScroll"
        >
          <TableHeader>
            <TableRow class="group">
              <TableHead
                class="mapping-sticky-cell mapping-order-cell mapping-icon-cell"
              >
                <div class="flex items-center pl-3">
                  <Checkbox
                    v-if="showGroupedView"
                    :class="[
                      mappingSelectionCheckboxClass,
                      mappingSelectionVisibilityClass,
                    ]"
                    :model-value="
                      someVisibleSelected ? 'indeterminate' : allVisibleSelected
                    "
                    :aria-label="t('admin.subdomainProxy.selectAllMappings')"
                    @update:model-value="
                      (value) => setAllVisibleSelected(value === true)
                    "
                  />
                </div>
              </TableHead>
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
          <tbody v-if="groupSections.length === 0">
            <TableRow>
              <TableCell
                colspan="8"
                class="py-8 text-center text-muted-foreground"
              >
                {{ t("admin.subdomainProxy.emptyMappings") }}
              </TableCell>
            </TableRow>
          </tbody>
          <SubdomainMappingGroupRows
            v-for="section in groupSections"
            :key="section.key"
            :mappings="section.mappings"
            :collapsed="showGroupedView && isSectionCollapsed(section)"
            :disabled="dragDisabled"
            :empty-label="t('admin.subdomainProxy.emptyGroup')"
            :show-header="showGroupedView"
            @update:mappings="
              (mappings) => updateSectionMappings(section.key, mappings)
            "
            @end="handleSortEnd"
          >
            <template #header>
              <TableRow class="mapping-group-header-row group">
                <TableCell colspan="8" class="p-0">
                  <div
                    class="mapping-group-header-sticky flex min-h-11 items-center gap-2 px-3 py-2"
                  >
                    <Checkbox
                      :class="[
                        mappingSelectionCheckboxClass,
                        mappingSelectionVisibilityClass,
                      ]"
                      :model-value="
                        isSectionPartiallySelected(section)
                          ? 'indeterminate'
                          : isSectionSelected(section)
                      "
                      :aria-label="
                        t('admin.subdomainProxy.selectGroupMappings', {
                          group: section.name,
                        })
                      "
                      @update:model-value="
                        (value) => setSectionSelected(section, value === true)
                      "
                    />
                    <button
                      type="button"
                      class="inline-flex min-w-0 flex-1 items-center gap-2 rounded-sm text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                      :aria-expanded="!isSectionCollapsed(section)"
                      @click="toggleSectionCollapsed(section)"
                    >
                      <ChevronRight
                        class="h-4 w-4 shrink-0 transition-transform duration-200 ease-out motion-reduce:transition-none"
                        :class="{
                          'rotate-90': !isSectionCollapsed(section),
                        }"
                      />
                      <span class="truncate font-medium">{{
                        section.name
                      }}</span>
                      <span
                        class="rounded-full bg-background px-2 py-0.5 text-xs text-muted-foreground"
                      >
                        {{ section.mappings.length }}
                      </span>
                    </button>
                    <DropdownMenu>
                      <DropdownMenuTrigger as-child>
                        <Button
                          variant="ghost"
                          size="icon"
                          :aria-label="t('common.moreActions')"
                        >
                          <MoreHorizontal class="h-4 w-4" />
                        </Button>
                      </DropdownMenuTrigger>
                      <DropdownMenuContent align="end">
                        <DropdownMenuItem
                          :disabled="isSavingMappings"
                          @select="emit('open-create', section.groupId)"
                        >
                          <FolderPlus class="mr-2 h-4 w-4" />
                          {{ t("admin.subdomainProxy.addMappingToGroup") }}
                        </DropdownMenuItem>
                        <DropdownMenuItem @select="isGroupManagerOpen = true">
                          <Folders class="mr-2 h-4 w-4" />
                          {{ t("admin.subdomainProxy.manageGroups") }}
                        </DropdownMenuItem>
                      </DropdownMenuContent>
                    </DropdownMenu>
                  </div>
                </TableCell>
              </TableRow>
            </template>
            <template #default="{ mapping }">
              <TableRow
                class="mapping-row"
                :class="[
                  'group',
                  isMappingUnavailable(mapping) ? 'text-muted-foreground' : '',
                ]"
              >
                <TableCell
                  class="mapping-sticky-cell mapping-order-cell mapping-icon-cell"
                >
                  <div
                    class="flex items-center"
                    :class="{ 'gap-1 pl-7': showGroupedView }"
                  >
                    <Checkbox
                      v-if="showGroupedView"
                      :class="[
                        mappingSelectionCheckboxClass,
                        mappingSelectionVisibilityClass,
                        'shrink-0',
                      ]"
                      :model-value="isMappingSelected(mapping.host)"
                      :aria-label="
                        t('admin.subdomainProxy.selectMapping', {
                          host: formatHost(mapping.host),
                        })
                      "
                      @update:model-value="
                        (value) =>
                          setMappingSelected(mapping.host, value === true)
                      "
                    />
                    <button
                      type="button"
                      class="mapping-drag-handle inline-flex h-7 items-center justify-center rounded-md text-muted-foreground transition hover:bg-muted hover:text-foreground disabled:pointer-events-none disabled:opacity-40"
                      :class="showGroupedView ? 'w-5' : '-ml-1 w-7'"
                      :disabled="dragDisabled"
                      :aria-label="t('admin.subdomainProxy.dragSortAria')"
                    >
                      <GripVertical class="h-4 w-4" />
                    </button>
                  </div>
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
                          @focus="openProtocolHeadersWarning(mapping.host)"
                          @blur="
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
                        @focusin="openProtocolHeadersWarning(mapping.host)"
                        @focusout="
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
                    :is-default-domain-available="isDefaultDomainAvailable"
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
                          :aria-label="t('common.moreActions')"
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
                            !isDefaultDomainAvailable
                          "
                          disabled
                        >
                          <StarOff class="mr-2 h-4 w-4" />
                          {{
                            t("admin.subdomainProxy.defaultDomainUnavailable")
                          }}
                        </DropdownMenuItem>
                        <DropdownMenuItem
                          v-else-if="
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
                          v-if="
                            hasGroups && !isAuthServiceTarget(mapping.target)
                          "
                        >
                          <DropdownMenuSubTrigger :disabled="isSavingMappings">
                            <FolderInput class="mr-2 h-4 w-4" />
                            {{ t("admin.subdomainProxy.moveToGroup") }}
                          </DropdownMenuSubTrigger>
                          <DropdownMenuSubContent class="w-48">
                            <DropdownMenuItem
                              v-for="group in groups"
                              :key="group.id"
                              :disabled="mapping.group_id === group.id"
                              @select="moveOne(mapping, group.id)"
                            >
                              {{ group.name }}
                            </DropdownMenuItem>
                            <DropdownMenuSeparator />
                            <DropdownMenuItem
                              :disabled="!mapping.group_id"
                              @select="moveOne(mapping, null)"
                            >
                              {{ t("admin.subdomainProxy.ungrouped") }}
                            </DropdownMenuItem>
                          </DropdownMenuSubContent>
                        </DropdownMenuSub>
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
                              {{
                                t("admin.subdomainProxy.scheduleAvailability")
                              }}
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
            </template>
          </SubdomainMappingGroupRows>
        </Table>
      </div>
    </CardContent>
  </Card>
  <SubdomainGroupManagerDialog
    v-model:open="isGroupManagerOpen"
    :groups="groups"
    :mappings="allRegularMappings"
    :saving="isSavingMappings"
    @save="saveGroupsAndCloseOnSuccess"
  />
</template>
