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
              class="w-auto max-w-[calc(100vw-7rem)] min-w-0 !shrink justify-center overflow-hidden [&>span]:min-w-0 [&>span]:truncate"
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
import { ref, computed, nextTick, onMounted, onUnmounted } from "vue";
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
import { toast } from "@admin-shared/utils/toast";
import { useConfigStore } from "../store/config";
import { ConfigAPI, ScanAPI, SystemAPI } from "../lib/api";
import type { ProxyMapping } from "../types";
import type {
  DiscoveredServiceInfo,
  ScanDiscoverPollEvent,
  ScanDiscoverProgress,
  ScanDiscoverResponse,
} from "../lib/api";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import PagedTableFooter from "@admin-shared/components/list/PagedTableFooter.vue";
import ReverseProxyDefaultRouteDialog from "./reverse-proxy/ReverseProxyDefaultRouteDialog.vue";
import ReverseProxyMappingDialog from "./reverse-proxy/ReverseProxyMappingDialog.vue";
import { useAsyncAction } from "@admin-shared/composables/useAsyncAction";
import { useDiscoverServicesSelection } from "@admin-shared/composables/useDiscoverServicesSelection";
import { useDefaultRouteConfirm } from "@admin-shared/composables/useDefaultRouteConfirm";
import { useProxyMappingDialogForm } from "@admin-shared/composables/useProxyMappingDialogForm";
import { useLocalPagedList } from "@admin-shared/composables/useLocalPagedList";
import { docsUrls } from "../lib/docs";
import {
  needsClearDefaultRouteConfirm,
  needsSetDefaultRouteConfirm,
} from "@admin-shared/utils/defaultRouteGuard";
import { extractPortFromTarget } from "@admin-shared/utils/extractPortFromTarget";
import { persistProxyMappings } from "@admin-shared/utils/persistProxyMappings";
import { isWebSocketProxyTargetUrl } from "@admin-shared/utils/proxyTargetInput";
import {
  buildProxyMapping,
  DEFAULT_PROXY_MAPPING_FLAGS,
} from "@admin-shared/utils/proxyMapping";
import {
  createReverseProxyMessages,
  showReverseProxyActionError,
  showReverseProxyBooleanResultToast,
  showReverseProxyDuplicateItemsError,
} from "@admin-shared/utils/reverseProxyFeedback";
import {
  validateBatchMappingDuplicates,
  validateSingleMappingDuplicates,
} from "@admin-shared/utils/validateProxyMappingDuplicates";

const currentHostname = window.location.hostname;
const { t } = useI18n();
const reverseProxyMessages = createReverseProxyMessages(t);
const discoverTargetsSettingsRef = ref<InstanceType<
  typeof ScanDiscoveryTargetsSettings
> | null>(null);
const isDiscoverSettingsOpen = ref(false);

const isDefaultRoute = (path: string) => {
  return configStore.config?.default_route === path;
};

const DEFAULT_SYSTEM_PORT = 5666;

const configStore = useConfigStore();
const accessEntryPort = ref("7999");

const removingPath = ref<string | null>(null);
const { run: runRemoveMapping } = useAsyncAction({
  onError: (error) => {
    showReverseProxyActionError(
      reverseProxyMessages.deleteFailed,
      error,
      reverseProxyMessages.unknownError,
    );
  },
});

const { isPending: isSaving, run: runSaveAction } = useAsyncAction({
  onError: (error) => {
    showReverseProxyActionError(
      reverseProxyMessages.saveFailed,
      error,
      reverseProxyMessages.unknownError,
    );
  },
});
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

onMounted(() => {
  void loadAccessEntryPort();
});

onUnmounted(() => {
  stopDiscoverScan();
});

async function loadAccessEntryPort() {
  try {
    const info = await SystemAPI.getAccessEntry();
    accessEntryPort.value = info.port;
  } catch (error) {
    console.warn("load access entry port failed:", error);
  }
}

async function removeMapping(mapping: ProxyMapping) {
  removingPath.value = mapping.path;
  await runRemoveMapping(
    async () => {
      const newList = allMappings.value.filter((item) => item !== mapping);
      await configStore.saveProxyMappings(newList);

      if (isDefaultRoute(mapping.path)) {
        await configStore.saveDefaultRoute("/__select__");
      }

      if (paginatedMappings.value.length === 1 && currentPage.value > 1) {
        currentPage.value--;
      }

      toast.success(reverseProxyMessages.deleteSuccess);
    },
    {
      onFinally: () => {
        removingPath.value = null;
      },
    },
  );
}

async function saveMapping() {
  if (!isValid.value) return;
  const isWebSocketTarget = isWebSocketProxyTargetUrl(newMapping.target);
  const normalizedMapping = buildProxyMapping({
    ...newMapping,
    rewrite_html: isWebSocketTarget ? false : newMapping.rewrite_html,
    use_root_mode: isWebSocketTarget ? false : newMapping.use_root_mode,
  });
  const { path: trimmedPath, target: trimmedTarget } = normalizedMapping;
  const ignorePath =
    isEditing.value && editingOriginalMapping.value
      ? editingOriginalMapping.value.path.trim()
      : null;
  const ignoreTarget =
    isEditing.value && editingOriginalMapping.value
      ? editingOriginalMapping.value.target.trim()
      : null;
  const { duplicatePath, duplicateTarget } = validateSingleMappingDuplicates(
    allMappings.value,
    { path: trimmedPath, target: trimmedTarget },
    { ignorePath, ignoreTarget },
  );

  if (duplicatePath) {
    toast.error(reverseProxyMessages.duplicatePath(trimmedPath));
    return;
  }
  if (duplicateTarget) {
    toast.error(reverseProxyMessages.duplicateTarget(trimmedTarget));
    return;
  }

  const isCreate = !isEditing.value;
  await runSaveAction(async () => {
    const newList = [...allMappings.value];
    if (isEditing.value && editingOriginalMapping.value) {
      const index = newList.indexOf(editingOriginalMapping.value);
      if (index !== -1) {
        newList[index] = normalizedMapping;
      }
    } else {
      newList.push(normalizedMapping);
    }

    await persistProxyMappings(
      newList,
      {
        saveMappings: (list) => configStore.saveProxyMappings(list),
        saveDefaultRoute: (path) => configStore.saveDefaultRoute(path),
        resetPage: () => {
          currentPage.value = 1;
        },
        resetSearch: () => {
          searchQuery.value = "";
        },
      },
      {
        resetPage: isCreate,
        resetSearch: isCreate,
        onAfterPersist: () => {
          closeMappingDialog(true);
        },
      },
    );

    toast.success(
      isCreate
        ? reverseProxyMessages.createSuccess
        : reverseProxyMessages.updateSuccess,
    );
  });
}

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

const { isPending: isDiscovering, run: runDiscoverServices } = useAsyncAction({
  onError: (error) => {
    if (isDiscoverAbortError(error)) return;
    showReverseProxyActionError(
      reverseProxyMessages.scanFailed,
      error,
      reverseProxyMessages.unknownError,
    );
  },
});
const discoverAbortController = ref<AbortController | null>(null);
const discoverProgress = ref<ScanDiscoverProgress | null>(null);
const {
  open: isDiscoverDialogOpen,
  discoveredData,
  selectedServices,
  isAllSelected,
  isSelectionValid: isDiscoverSelectionValid,
  setAllSelected,
  resetSelection: resetDiscoverSelection,
  setDiscoveredData,
  openDialog: openDiscoverDialogState,
  closeDialog: closeDiscoverDialog,
} = useDiscoverServicesSelection<DiscoveredServiceInfo, ScanDiscoverResponse>({
  getPath: (svc) => svc.detail.rule.path,
});
const showDiscoverHostColumn = computed(() => {
  const hosts = new Set(
    (discoveredData.value?.services || [])
      .map((service) => service.host?.trim())
      .filter(Boolean),
  );
  return hosts.size > 1;
});
const resolveDiscoveredServiceHost = (service: DiscoveredServiceInfo) =>
  service.host?.trim() || discoveredData.value?.host?.trim() || currentHostname;
const discoverFooterScannedPorts = computed(() => {
  return discoveredData.value?.totalPortsScanned || 0;
});

const isDiscoverAbortError = (error: unknown): boolean =>
  error instanceof DOMException
    ? error.name === "AbortError"
    : error instanceof Error && error.name === "AbortError";

const createEmptyDiscoverResponse = (
  patch: Partial<ScanDiscoverResponse> = {},
): ScanDiscoverResponse => ({
  host: patch.host || "",
  totalPortsScanned: patch.totalPortsScanned || 0,
  foundServices: patch.foundServices || 0,
  scannedHosts: patch.scannedHosts,
  scanHostCount: patch.scanHostCount,
  scanScope: patch.scanScope,
  scanCidrs: patch.scanCidrs,
  services: [],
});

const cloneDiscoveredService = (
  service: DiscoveredServiceInfo,
): DiscoveredServiceInfo => ({
  ...service,
  detail: {
    ...service.detail,
    rule: { ...service.detail.rule },
  },
});

const upsertDiscoveredService = (service: DiscoveredServiceInfo) => {
  const current = discoveredData.value || createEmptyDiscoverResponse();
  const nextService = cloneDiscoveredService(service);
  const serviceKey =
    nextService.serviceKey ||
    `${nextService.host?.trim() || current.host}:${nextService.port}`;
  const nextServices = [...current.services];
  const existingIndex = nextServices.findIndex((item) => {
    const itemKey =
      item.serviceKey || `${item.host?.trim() || current.host}:${item.port}`;
    return itemKey === serviceKey;
  });

  if (existingIndex >= 0) {
    const previous = nextServices[existingIndex]!;
    nextServices[existingIndex] = nextService;
    const selectedIndex = selectedServices.value.indexOf(previous);
    if (selectedIndex >= 0) {
      selectedServices.value[selectedIndex] = nextService;
    }
  } else {
    nextServices.push(nextService);
    if (nextService.detail.rule.path?.trim()) {
      selectedServices.value.push(nextService);
    }
  }

  setDiscoveredData({
    ...current,
    foundServices: nextServices.length,
    services: nextServices,
  });
};

const applyDiscoverPollEvent = (event: ScanDiscoverPollEvent) => {
  if (event.type === "meta") {
    setDiscoveredData(createEmptyDiscoverResponse(event.data));
    return;
  }

  if (event.type === "progress") {
    discoverProgress.value = event.data;
    return;
  }

  if (event.type === "service") {
    upsertDiscoveredService(event.data.service);
    return;
  }

  if (event.type === "done") {
    const current = discoveredData.value;
    if (!current) {
      setDiscoveredData(event.data);
      selectedServices.value = event.data.services.filter((svc) =>
        Boolean(svc.detail.rule.path?.trim()),
      );
      return;
    }

    setDiscoveredData({
      ...current,
      ...event.data,
      foundServices: current.services.length,
      services: current.services,
    });
  }
};

const onToggleAllDiscoverSelect = (e: Event) => {
  const checked = (e.target as HTMLInputElement).checked;
  setAllSelected(checked);
};

const handleDiscoverDialogOpenChange = (nextOpen: boolean) => {
  if (!nextOpen) {
    dismissDiscoverDialog();
  }
};

function dismissDiscoverDialog() {
  stopDiscoverScan();
  closeDiscoverDialog(true);
  isDiscoverSettingsOpen.value = false;
}

function openDiscoverDialog() {
  openDiscoverDialogState();
  // Trigger the initial scan only when no previous scan data exists.
  if (!discoveredData.value) {
    void nextTick().then(() => triggerScan());
  }
}

async function toggleDiscoverSettings() {
  isDiscoverSettingsOpen.value = !isDiscoverSettingsOpen.value;
  if (isDiscoverSettingsOpen.value) {
    await nextTick();
    void discoverTargetsSettingsRef.value?.loadTargets();
  }
}

async function triggerScan() {
  let targetCidrs: string[];
  try {
    await nextTick();
    const selectedCidrs =
      await discoverTargetsSettingsRef.value?.ensureSaved();
    if (!selectedCidrs || selectedCidrs.length === 0) return;
    targetCidrs = selectedCidrs;
  } catch {
    return;
  }

  resetDiscoverSelection();
  discoverProgress.value = null;
  discoverAbortController.value?.abort();
  const abortController = new AbortController();
  discoverAbortController.value = abortController;
  await runDiscoverServices(
    () =>
      ScanAPI.discoverPolling(
        { target_cidrs: targetCidrs },
        {
          signal: abortController.signal,
          onEvent: applyDiscoverPollEvent,
        },
      ),
    {
      onFinally: () => {
        if (discoverAbortController.value === abortController) {
          discoverAbortController.value = null;
        }
      },
    },
  );
}

function stopDiscoverScan() {
  discoverAbortController.value?.abort();
  discoverAbortController.value = null;
}

async function saveDiscoveredServices() {
  if (!isDiscoverSelectionValid.value || !discoveredData.value) return;
  const candidates = selectedServices.value.map((svc) => ({
    path: svc.detail.rule.path?.trim() || "",
    target: `http://${resolveDiscoveredServiceHost(svc)}:${svc.port}/`.trim(),
  }));
  const { duplicatePaths, duplicateTargets } = validateBatchMappingDuplicates(
    allMappings.value,
    candidates,
  );

  if (duplicatePaths.length > 0) {
    showReverseProxyDuplicateItemsError(
      reverseProxyMessages.duplicateItems(
        t("admin.reverseProxy.duplicatePathLabel"),
        duplicatePaths,
      ),
    );
    return;
  }
  if (duplicateTargets.length > 0) {
    showReverseProxyDuplicateItemsError(
      reverseProxyMessages.duplicateItems(
        t("admin.reverseProxy.duplicateTargetLabel"),
        duplicateTargets,
      ),
    );
    return;
  }

  stopDiscoverScan();
  await runSaveAction(async () => {
    const newList = [...allMappings.value];
    let defaultRouteToSet: string | null = null;
    let addedCount = 0;

    for (const svc of selectedServices.value) {
      const rule = svc.detail.rule;
      const discoveredHost = resolveDiscoveredServiceHost(svc);
      const newMap = buildProxyMapping({
        path: rule.path,
        target: `http://${discoveredHost}:${svc.port}/`,
        rewrite_html: rule.rewrite_html,
        use_auth: rule.use_auth,
        use_root_mode: rule.use_root_mode,
        strip_path: rule.strip_path,
      });

      newList.push(newMap);
      addedCount++;

      if (svc.detail.isDefault) {
        defaultRouteToSet = newMap.path;
      }
    }

    await persistProxyMappings(
      newList,
      {
        saveMappings: (list) => configStore.saveProxyMappings(list),
        saveDefaultRoute: (path) => configStore.saveDefaultRoute(path),
        resetPage: () => {
          currentPage.value = 1;
        },
        resetSearch: () => {
          searchQuery.value = "";
        },
      },
      {
        defaultRoutePath: defaultRouteToSet,
        resetPage: true,
        onAfterPersist: () => {
          toast.success(reverseProxyMessages.discoverSaveSuccess(addedCount));
          dismissDiscoverDialog();
        },
      },
    );
  });
}
</script>
