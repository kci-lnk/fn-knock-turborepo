<template>
  <Card class="mb-6">
    <CardHeader>
      <CardTitle class="flex justify-between items-center">
        <span>{{ t("admin.reverseProxy.title") }}</span>
        <div class="flex items-center gap-2">
          <DocsLinkButton :href="docsUrls.guides.reverseProxy" />
          <div class="flex">
            <Button @click="openDiscoverDialog" class="rounded-r-none">
              <Search class="mr-2 w-4 h-4" />
              {{ t("admin.reverseProxy.discover") }}
            </Button>
            <DropdownMenu>
              <DropdownMenuTrigger as-child>
                <Button
                  variant="default"
                  size="icon"
                  class="rounded-l-none border-l border-primary-foreground/20 px-2"
                >
                  <ChevronDown class="h-4 w-4" />
                </Button>
              </DropdownMenuTrigger>
              <DropdownMenuContent align="end">
                <DropdownMenuItem
                  :disabled="isDiscovering"
                  @select="isScanIntensityDialogOpen = true"
                >
                  <SlidersHorizontal class="mr-2 h-4 w-4" />
                  {{ t("admin.scanIntensity.title") }}
                </DropdownMenuItem>
                <DropdownMenuItem @click="openAddDialog">
                  <Plus class="mr-2 h-4 w-4" />
                  {{ t("admin.reverseProxy.addMapping") }}
                </DropdownMenuItem>
                <DropdownMenuItem @click="syncRoutes" :disabled="isSyncing">
                  <RefreshCw
                    class="mr-2 h-4 w-4"
                    :class="{ 'animate-spin': isSyncing }"
                  />
                  {{
                    isSyncing
                      ? t("admin.reverseProxy.syncing")
                      : t("admin.reverseProxy.syncRoutes")
                  }}
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenu>
          </div>
        </div>
      </CardTitle>
      <CardDescription>{{
        t("admin.reverseProxy.description", { port: accessEntryPort })
      }}</CardDescription>
    </CardHeader>
    <CardContent>
      <div class="flex items-center mb-4 space-x-2">
        <SearchInput
          v-model="searchQuery"
          :placeholder="t('admin.reverseProxy.searchPlaceholder')"
          class="max-w-xs"
        />
      </div>

      <div class="border rounded-md overflow-x-auto">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>{{ t("admin.reverseProxy.columns.path") }}</TableHead>
              <TableHead>{{
                t("admin.reverseProxy.columns.target")
              }}</TableHead>
              <TableHead>{{
                t("admin.reverseProxy.columns.options")
              }}</TableHead>
              <TableHead class="text-right">{{
                t("admin.reverseProxy.columns.actions")
              }}</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            <TableRow v-if="paginatedMappings.length === 0">
              <TableCell
                colspan="4"
                class="text-center text-muted-foreground py-6"
              >
                {{ t("admin.reverseProxy.empty") }}
              </TableCell>
            </TableRow>
            <TableRow
              v-for="(mapping, index) in paginatedMappings"
              :key="index"
              class="group transition-colors"
            >
              <TableCell class="font-medium">{{ mapping.path }}</TableCell>
              <TableCell>{{ mapping.target }}</TableCell>
              <TableCell>
                <div
                  class="flex flex-wrap gap-2 text-xs text-muted-foreground whitespace-normal"
                >
                  <Badge
                    v-if="isDefaultRoute(mapping.path)"
                    variant="secondary"
                    class="border border-emerald-500/30 bg-emerald-500/10 text-emerald-700"
                  >
                    {{ t("admin.reverseProxy.defaultRoute") }}
                  </Badge>
                  <span
                    v-if="
                      mapping.rewrite_html &&
                      !isWebSocketProxyTargetUrl(mapping.target)
                    "
                    class="px-2 py-0.5 bg-muted rounded"
                    >{{ t("admin.reverseProxy.rewriteHtml") }}</span
                  >
                  <span
                    v-if="mapping.use_auth"
                    class="px-2 py-0.5 bg-muted rounded"
                    >{{ t("admin.reverseProxy.authRequiredShort") }}</span
                  >
                  <span
                    v-if="
                      mapping.use_root_mode &&
                      !isWebSocketProxyTargetUrl(mapping.target)
                    "
                    class="px-2 py-0.5 bg-muted rounded"
                    >{{ t("admin.reverseProxy.rootMode") }}</span
                  >
                  <span
                    v-if="mapping.strip_path"
                    class="px-2 py-0.5 bg-muted rounded"
                    >{{ t("admin.reverseProxy.stripPath") }}</span
                  >
                </div>
              </TableCell>
              <TableCell class="text-right">
                <div class="flex justify-end gap-1">
                  <Button
                    v-if="isDefaultRoute(mapping.path)"
                    variant="outline"
                    size="sm"
                    class="border-border text-muted-foreground hover:text-foreground opacity-0 group-hover:opacity-100 transition-opacity mr-2"
                    @click="requestClearDefaultRoute(mapping)"
                  >
                    {{ t("admin.reverseProxy.clearDefaultRoute") }}
                  </Button>
                  <Button
                    v-else
                    variant="outline"
                    size="sm"
                    class="opacity-0 group-hover:opacity-100 transition-opacity mr-2"
                    @click="requestSetDefaultRoute(mapping)"
                  >
                    {{ t("admin.reverseProxy.setDefaultRoute") }}
                  </Button>

                  <Button
                    variant="ghost"
                    size="sm"
                    @click="openEditDialog(mapping)"
                  >
                    {{ t("admin.reverseProxy.edit") }}
                  </Button>

                  <ConfirmDangerPopover
                    :title="t('admin.reverseProxy.deleteConfirmTitle')"
                    :description="
                      t('admin.reverseProxy.deleteDescription', {
                        path: mapping.path,
                      })
                    "
                    :loading="removingPath === mapping.path"
                    :disabled="removingPath === mapping.path"
                    :on-confirm="() => removeMapping(mapping)"
                    content-class="w-60 text-left"
                  >
                    <template #trigger>
                      <Button
                        variant="ghost"
                        size="sm"
                        class="text-destructive hover:bg-destructive/10 hover:text-destructive"
                        :disabled="removingPath === mapping.path"
                      >
                        {{ t("admin.reverseProxy.delete") }}
                      </Button>
                    </template>
                  </ConfirmDangerPopover>
                </div>
              </TableCell>
            </TableRow>
          </TableBody>
        </Table>
      </div>

      <PagedTableFooter
        class="mt-4 border rounded-md"
        :total="filteredMappings.length"
        :page="currentPage"
        :limit="limit"
        :items-per-page="parsedLimit"
        @update:page="handlePageChange"
        @update:limit="handleLimitChange"
      />
    </CardContent>
  </Card>

  <ScanDiscoveryIntensityDialog
    v-model:open="isScanIntensityDialogOpen"
    :disabled="isDiscovering"
  />

  <ReverseProxyMappingDialog
    :open="isMappingDialogOpen"
    :form="newMapping"
    :is-editing="isEditing"
    :is-saving="isSaving"
    :is-valid="isValid"
    :is-web-socket-target="isNewMappingWebSocketTarget"
    @update:open="handleMappingDialogOpenChange"
    @update-form="updateMappingDraft"
    @close="closeMappingDialog(true)"
    @save="saveMapping"
  />

  <Dialog
    :open="isDiscoverDialogOpen"
    @update:open="handleDiscoverDialogOpenChange"
  >
    <DialogContent
      class="max-w-[calc(100vw-2rem)] sm:max-w-[800px] max-h-[85vh] flex flex-col overflow-hidden"
    >
      <DialogHeader class="shrink-0">
        <div
          class="flex min-w-0 flex-col gap-3 sm:flex-row sm:items-center sm:justify-between"
        >
          <DialogTitle>{{ t("admin.reverseProxy.discoverTitle") }}</DialogTitle>
          <div
            class="flex w-fit max-w-full min-w-0 self-center items-center gap-2 sm:self-auto"
          >
            <Button
              variant="outline"
              size="icon"
              class="h-11 w-11 sm:h-9 sm:w-9"
              :disabled="isDiscovering"
              @click="toggleDiscoverSettings"
            >
              <SlidersHorizontal class="h-4 w-4" />
            </Button>
            <RefreshButton
              class="h-11 w-auto max-w-[calc(100vw-7rem)] min-w-0 !shrink justify-center overflow-hidden sm:h-9 [&>span]:min-w-0 [&>span]:truncate"
              :label="t('admin.reverseProxy.refreshServices')"
              :loading="isDiscovering"
              :disabled="isDiscovering"
              @click="triggerScan"
            />
            <Button
              v-if="isDiscovering"
              class="h-11 sm:h-9"
              variant="outline"
              @click="stopDiscoverScan"
            >
              <X class="mr-2 h-4 w-4" />
              {{ t("admin.reverseProxy.cancel") }}
            </Button>
          </div>
        </div>
        <DialogDescription>
          {{ t("admin.reverseProxy.discoverDescription") }}
        </DialogDescription>
        <ScanDiscoveryTargetsSettings
          ref="discoverTargetsSettingsRef"
          v-show="isDiscoverSettingsOpen"
          class="mt-3"
        />
      </DialogHeader>

      <div class="flex-1 min-h-0 overflow-auto">
        <div class="py-2">
          <div
            v-if="
              isDiscovering &&
              (!discoveredData || discoveredData.services.length === 0)
            "
            class="flex flex-col items-center justify-center py-16 space-y-4"
          >
            <RefreshCw class="h-8 w-8 animate-spin text-muted-foreground" />
            <p class="text-sm text-muted-foreground">
              {{ t("admin.reverseProxy.probing") }}
            </p>
          </div>

          <div
            v-else-if="
              !isDiscovering &&
              discoveredData &&
              discoveredData.services.length === 0
            "
            class="text-center py-16 text-muted-foreground"
          >
            {{ t("admin.reverseProxy.discoverEmpty") }}
          </div>

          <div
            v-else-if="discoveredData && discoveredData.services.length > 0"
            class="border rounded-md bg-background"
          >
            <Table container-class="overflow-visible">
              <TableHeader
                class="sticky top-0 z-10 bg-background shadow-sm [&_th]:sticky [&_th]:top-0 [&_th]:z-10 [&_th]:bg-background"
              >
                <TableRow>
                  <TableHead class="w-[50px] text-center">
                    <input
                      type="checkbox"
                      class="h-4 w-4 rounded border-gray-300 text-primary cursor-pointer"
                      :checked="isAllSelected"
                      @change="onToggleAllDiscoverSelect"
                    />
                  </TableHead>
                  <TableHead v-if="showDiscoverHostColumn" class="w-[140px]">
                    {{ t("admin.reverseProxy.discoverColumns.host") }}
                  </TableHead>
                  <TableHead class="w-[80px]">{{
                    t("admin.reverseProxy.discoverColumns.port")
                  }}</TableHead>
                  <TableHead class="w-[100px]">{{
                    t("admin.reverseProxy.discoverColumns.status")
                  }}</TableHead>
                  <TableHead>{{
                    t("admin.reverseProxy.discoverColumns.serviceId")
                  }}</TableHead>
                  <TableHead class="w-[200px]">{{
                    t("admin.reverseProxy.discoverColumns.suggestedPath")
                  }}</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                <TableRow
                  v-for="(svc, index) in discoveredData.services"
                  :key="index"
                >
                  <TableCell class="text-center">
                    <input
                      type="checkbox"
                      class="h-4 w-4 rounded border-gray-300 text-primary cursor-pointer"
                      :value="svc"
                      v-model="selectedServices"
                    />
                  </TableCell>
                  <TableCell
                    v-if="showDiscoverHostColumn"
                    class="font-mono text-xs text-muted-foreground"
                  >
                    {{ resolveDiscoveredServiceHost(svc) }}
                  </TableCell>
                  <TableCell class="font-medium">
                    <a
                      :href="`http://${resolveDiscoveredServiceHost(svc)}:${svc.port}`"
                      target="_blank"
                      class="text-primary hover:underline hover:text-primary/80 transition-colors"
                      :title="t('admin.reverseProxy.openNewWindow')"
                    >
                      {{ svc.port }}
                    </a>
                  </TableCell>
                  <TableCell>
                    <span
                      v-if="svc.httpStatus === 401"
                      class="text-amber-600 bg-amber-500/10 text-xs px-2 py-0.5 rounded"
                      >{{ t("admin.reverseProxy.authRequiredShort") }}</span
                    >
                    <span
                      v-else
                      class="text-green-600 bg-green-500/10 text-xs px-2 py-0.5 rounded"
                      >{{ svc.httpStatus }}</span
                    >
                  </TableCell>
                  <TableCell>
                    <span v-if="svc.detail.label" class="text-sm">{{
                      svc.detail.label
                    }}</span>
                    <span v-else class="text-red-500 text-sm font-medium">{{
                      t("admin.reverseProxy.unknownService")
                    }}</span>
                  </TableCell>
                  <TableCell>
                    <Input
                      v-model="svc.detail.rule.path"
                      :placeholder="
                        t('admin.reverseProxy.requiredPathPlaceholder')
                      "
                      class="h-8 text-sm"
                      :class="{
                        'border-destructive focus-visible:ring-destructive':
                          selectedServices.includes(svc) &&
                          !svc.detail.rule.path.trim(),
                      }"
                    />
                  </TableCell>
                </TableRow>
              </TableBody>
            </Table>
          </div>
        </div>
      </div>

      <DialogFooter class="mt-2 shrink-0 items-center sm:justify-between">
        <span class="text-sm text-muted-foreground">
          <template v-if="discoveredData">
            <template v-if="isDiscovering">
              {{ t("admin.reverseProxy.probing") }}
            </template>
            <template v-else>
              {{
                t("admin.reverseProxy.scannedPorts", {
                  count: discoverFooterScannedPorts,
                })
              }}
            </template>
            <template v-if="discoveredData.intensityLevel">
              ，{{
                t("admin.scanIntensity.resultSummary", {
                  level: t(
                    `admin.scanIntensity.levels.${discoveredData.intensityLevel}`,
                  ),
                  count: discoveredData.effectiveConcurrency || 0,
                })
              }}
            </template>
            ，{{
              t("admin.reverseProxy.selectedItems", {
                count: `${selectedServices.length} / ${discoveredData.services.length}`,
              })
            }}
            <template v-if="discoveredData.scanCidrs?.length">
              ，{{
                t("admin.reverseProxy.coveredCidrsHosts", {
                  cidrs: discoveredData.scanCidrs.length,
                  hosts:
                    discoveredData.scanHostCount ||
                    discoveredData.scannedHosts ||
                    0,
                })
              }}
            </template>
            <template
              v-if="
                !discoveredData.scanCidrs?.length &&
                discoveredData.scannedHosts &&
                discoveredData.scannedHosts > 1
              "
            >
              {{
                t("admin.reverseProxy.coveredHosts", {
                  hosts:
                    discoveredData.scanScope || discoveredData.scannedHosts,
                })
              }}
            </template>
          </template>
        </span>
        <div class="space-x-2">
          <Button variant="outline" @click="dismissDiscoverDialog">
            {{ t("admin.reverseProxy.cancel") }}
          </Button>
          <Button
            @click="saveDiscoveredServices"
            :disabled="
              selectedServices.length === 0 ||
              !isDiscoverSelectionValid ||
              isSaving
            "
          >
            {{ t("admin.reverseProxy.addSelected") }}
          </Button>
        </div>
      </DialogFooter>
    </DialogContent>
  </Dialog>

  <ReverseProxyDefaultRouteDialog
    :open="isDefaultRouteConfirmOpen"
    :title="defaultRouteDialogTitle"
    :description="defaultRouteDialogDescription"
    :show-fnos-hint="showDefaultRouteFnosHint"
    :saving="isSavingDefaultRoute"
    @update:open="handleDefaultRouteConfirmOpenChange"
    @cancel="closeDefaultRouteConfirm"
    @confirm="confirmDefaultRouteChange"
  />
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from "vue";
import { useI18n } from "vue-i18n";
import {
  Card,
  CardHeader,
  CardTitle,
  CardDescription,
  CardContent,
} from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import RefreshButton from "@/components/RefreshButton.vue";
import ScanDiscoveryTargetsSettings from "@/components/ScanDiscoveryTargetsSettings.vue";
import ScanDiscoveryIntensityDialog from "@/components/ScanDiscoveryIntensityDialog.vue";
import DocsLinkButton from "@/components/DocsLinkButton.vue";
import { Input } from "@/components/ui/input";
import SearchInput from "@admin-shared/components/SearchInput.vue";
import {
  Table,
  TableHeader,
  TableRow,
  TableHead,
  TableBody,
  TableCell,
} from "@/components/ui/table";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from "@/components/ui/dialog";
import {
  DropdownMenu,
  DropdownMenuTrigger,
  DropdownMenuContent,
  DropdownMenuItem,
} from "@/components/ui/dropdown-menu";
import {
  ChevronDown,
  RefreshCw,
  Plus,
  Search,
  SlidersHorizontal,
  X,
} from "lucide-vue-next";
import { useConfigStore } from "../store/config";
import { ConfigAPI } from "../lib/api";
import type { ProxyMapping } from "../types";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import PagedTableFooter from "@admin-shared/components/list/PagedTableFooter.vue";
import ReverseProxyDefaultRouteDialog from "./reverse-proxy/ReverseProxyDefaultRouteDialog.vue";
import ReverseProxyMappingDialog from "./reverse-proxy/ReverseProxyMappingDialog.vue";
import { useAsyncAction } from "@admin-shared/composables/useAsyncAction";
import { useAccessEntryPort } from "@/composables/useAccessEntryPort";
import { useDefaultRouteConfirm } from "@admin-shared/composables/useDefaultRouteConfirm";
import { useProxyMappingDialogForm } from "@admin-shared/composables/useProxyMappingDialogForm";
import { useReverseProxyDiscoverFlow } from "./reverse-proxy/useReverseProxyDiscoverFlow";
import { useReverseProxyMappingActions } from "./reverse-proxy/useReverseProxyMappingActions";
import { useLocalPagedList } from "@admin-shared/composables/useLocalPagedList";
import { docsUrls } from "../lib/docs";
import {
  needsClearDefaultRouteConfirm,
  needsSetDefaultRouteConfirm,
} from "@admin-shared/utils/defaultRouteGuard";
import { extractPortFromTarget } from "@admin-shared/utils/extractPortFromTarget";
import { isWebSocketProxyTargetUrl } from "@admin-shared/utils/proxyTargetInput";
import { DEFAULT_PROXY_MAPPING_FLAGS } from "@admin-shared/utils/proxyMapping";
import {
  createReverseProxyMessages,
  showReverseProxyActionError,
  showReverseProxyBooleanResultToast,
} from "@admin-shared/utils/reverseProxyFeedback";

const currentHostname = window.location.hostname;
const { t } = useI18n();
const reverseProxyMessages = createReverseProxyMessages(t);
const isScanIntensityDialogOpen = ref(false);
const discoverTargetsSettingsRef = ref<InstanceType<
  typeof ScanDiscoveryTargetsSettings
> | null>(null);

const isDefaultRoute = (path: string) => {
  return configStore.config?.default_route === path;
};

const DEFAULT_SYSTEM_PORT = 5666;

const configStore = useConfigStore();
const { accessEntryPort, loadAccessEntryPort } = useAccessEntryPort();

const {
  open: isMappingDialogOpen,
  isEditing,
  editingOriginal: editingOriginalMapping,
  form: newMapping,
  isValid,
  openAdd: openAddDialog,
  openEdit: openEditDialog,
  close: closeMappingDialog,
} = useProxyMappingDialogForm<ProxyMapping>(DEFAULT_PROXY_MAPPING_FLAGS);
const isNewMappingWebSocketTarget = computed(() =>
  isWebSocketProxyTargetUrl(newMapping.target),
);

const handleMappingDialogOpenChange = (nextOpen: boolean) => {
  if (!nextOpen) {
    closeMappingDialog(true);
  }
};

function updateMappingDraft(patch: Partial<ProxyMapping>) {
  Object.assign(newMapping, patch);
}

const { isPending: isSyncing, run: runSyncRoutes } = useAsyncAction({
  onError: (error) => {
    showReverseProxyActionError(
      reverseProxyMessages.syncFailed,
      error,
      reverseProxyMessages.networkError,
    );
  },
});
const { isPending: isSavingDefaultRoute, run: runSaveDefaultRoute } =
  useAsyncAction({
    onError: (error) => {
      showReverseProxyActionError(
        reverseProxyMessages.defaultRouteUpdateFailed,
        error,
        reverseProxyMessages.unknownError,
      );
    },
  });

const allMappings = computed(() => configStore.config?.proxy_mappings || []);

const {
  open: isDefaultRouteConfirmOpen,
  pendingPath: pendingDefaultRoutePath,
  showDefaultRouteFnosHint,
  dialogTitle: defaultRouteDialogTitle,
  dialogDescription: defaultRouteDialogDescription,
  queue: queueDefaultRouteAction,
  reset: closeDefaultRouteConfirm,
} = useDefaultRouteConfirm(DEFAULT_SYSTEM_PORT);

const handleDefaultRouteConfirmOpenChange = (nextOpen: boolean) => {
  if (!nextOpen) {
    closeDefaultRouteConfirm();
  }
};

const currentDefaultRouteMapping = computed(() => {
  const currentDefaultPath = configStore.config?.default_route;
  if (!currentDefaultPath || currentDefaultPath === "/__select__") return null;
  return (
    allMappings.value.find((mapping) => mapping.path === currentDefaultPath) ??
    null
  );
});
const currentDefaultRoutePort = computed(() => {
  if (!currentDefaultRouteMapping.value) return null;
  return extractPortFromTarget(currentDefaultRouteMapping.value.target);
});
function requestClearDefaultRoute(mapping: ProxyMapping) {
  const targetPort = extractPortFromTarget(mapping.target);
  if (needsClearDefaultRouteConfirm(targetPort, DEFAULT_SYSTEM_PORT)) {
    queueDefaultRouteAction("/__select__", "clear", targetPort);
    return;
  }
  void applyDefaultRoute("/__select__");
}

function requestSetDefaultRoute(mapping: ProxyMapping) {
  if (
    needsSetDefaultRouteConfirm(
      currentDefaultRoutePort.value,
      currentDefaultRouteMapping.value?.path,
      mapping.path,
      DEFAULT_SYSTEM_PORT,
    )
  ) {
    queueDefaultRouteAction(mapping.path, "set", currentDefaultRoutePort.value);
    return;
  }
  void applyDefaultRoute(mapping.path);
}

async function applyDefaultRoute(path: string) {
  await runSaveDefaultRoute(async () => {
    await configStore.saveDefaultRoute(path);
  });
}

async function confirmDefaultRouteChange() {
  if (!pendingDefaultRoutePath.value) return;

  await applyDefaultRoute(pendingDefaultRoutePath.value);
  closeDefaultRouteConfirm();
}

const {
  searchQuery,
  currentPage,
  limit,
  parsedLimit,
  filteredItems: filteredMappings,
  pagedItems: paginatedMappings,
  handlePageChange,
  handleLimitChange,
} = useLocalPagedList<ProxyMapping>({
  items: allMappings,
  normalizeQuery: (q) => q.toLowerCase(),
  filter: (mapping, query) =>
    mapping.path.toLowerCase().includes(query) ||
    mapping.target.toLowerCase().includes(query),
});

const { isSaving, removeMapping, removingPath, runSaveAction, saveMapping } =
  useReverseProxyMappingActions({
    allMappings,
    closeMappingDialog,
    currentPage,
    editingOriginalMapping,
    form: newMapping,
    isDefaultRoute,
    isEditing,
    isValid,
    messages: reverseProxyMessages,
    paginatedMappings,
    saveDefaultRoute: (path) => configStore.saveDefaultRoute(path),
    saveProxyMappings: (mappings) => configStore.saveProxyMappings(mappings),
    searchQuery,
  });

onMounted(() => {
  void loadAccessEntryPort();
});

onUnmounted(() => {
  stopDiscoverScan();
});

async function syncRoutes() {
  await runSyncRoutes(() => ConfigAPI.syncRoutes(), {
    onSuccess: (result) => {
      showReverseProxyBooleanResultToast(result, {
        successText: reverseProxyMessages.syncSuccess(
          result.data?.synced_rules ?? 0,
        ),
        errorText: reverseProxyMessages.syncFailed,
        unknownErrorText: reverseProxyMessages.unknownError,
      });
    },
  });
}

const {
  discoveredData,
  discoverFooterScannedPorts,
  dismissDiscoverDialog,
  handleDiscoverDialogOpenChange,
  isAllSelected,
  isDiscoverDialogOpen,
  isDiscovering,
  isDiscoverSelectionValid,
  isDiscoverSettingsOpen,
  onToggleAllDiscoverSelect,
  openDiscoverDialog,
  resolveDiscoveredServiceHost,
  saveDiscoveredServices,
  selectedServices,
  showDiscoverHostColumn,
  stopDiscoverScan,
  toggleDiscoverSettings,
  triggerScan,
} = useReverseProxyDiscoverFlow({
  allMappings,
  currentHostname,
  currentPage,
  discoverTargetsSettingsRef,
  messages: reverseProxyMessages,
  runSaveAction,
  saveDefaultRoute: (path) => configStore.saveDefaultRoute(path),
  saveProxyMappings: (mappings) => configStore.saveProxyMappings(mappings),
  searchQuery,
  translate: (key, params) => (params ? t(key, params) : t(key)),
});
</script>
