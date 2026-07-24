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
                  :aria-label="t('common.moreActions')"
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

  <ReverseProxyDiscoverDialog
    ref="discoverTargetsSettingsRef"
    v-model:selected-services="selectedServices"
    :discovered-data="discoveredData"
    :is-all-selected="isAllSelected"
    :is-discovering="isDiscovering"
    :is-saving="isSaving"
    :is-selection-valid="isDiscoverSelectionValid"
    :is-settings-open="isDiscoverSettingsOpen"
    :open="isDiscoverDialogOpen"
    :resolve-service-host="resolveDiscoveredServiceHost"
    :show-host-column="showDiscoverHostColumn"
    @cancel="dismissDiscoverDialog"
    @save="saveDiscoveredServices"
    @scan="triggerScan"
    @stop-scan="stopDiscoverScan"
    @toggle-all="onToggleAllDiscoverSelect"
    @toggle-settings="toggleDiscoverSettings"
    @update:open="handleDiscoverDialogOpenChange"
  />

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
import ScanDiscoveryIntensityDialog from "@/components/ScanDiscoveryIntensityDialog.vue";
import DocsLinkButton from "@/components/DocsLinkButton.vue";
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
} from "lucide-vue-next";
import { useConfigStore } from "../store/config";
import { ConfigAPI } from "../lib/api";
import type { ProxyMapping } from "../types";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import PagedTableFooter from "@admin-shared/components/list/PagedTableFooter.vue";
import ReverseProxyDefaultRouteDialog from "./reverse-proxy/ReverseProxyDefaultRouteDialog.vue";
import ReverseProxyDiscoverDialog from "./reverse-proxy/ReverseProxyDiscoverDialog.vue";
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
  typeof ReverseProxyDiscoverDialog
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
