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

  <Dialog
    :open="isMappingDialogOpen"
    @update:open="handleMappingDialogOpenChange"
  >
    <DialogContent class="sm:max-w-[425px]">
      <DialogHeader>
        <DialogTitle>{{
          isEditing
            ? t("admin.reverseProxy.editTitle")
            : t("admin.reverseProxy.addTitle")
        }}</DialogTitle>
        <DialogDescription>
          {{
            isEditing
              ? t("admin.reverseProxy.editDescription")
              : t("admin.reverseProxy.addDescription")
          }}
        </DialogDescription>
      </DialogHeader>
      <div class="grid gap-4 py-4">
        <div class="grid grid-cols-4 items-center gap-4">
          <Label for="path" class="text-right">{{
            t("admin.reverseProxy.pathLabel")
          }}</Label>
          <Input
            id="path"
            v-model="newMapping.path"
            :placeholder="t('admin.reverseProxy.pathPlaceholder')"
            class="col-span-3"
          />
        </div>
        <div class="grid grid-cols-4 items-start gap-4">
          <Label for="target-endpoint" class="pt-2 text-right">{{
            t("admin.reverseProxy.targetLabel")
          }}</Label>
          <ProxyTargetInputField
            v-model="newMapping.target"
            input-id="target-endpoint"
            protocol-id="target-protocol"
            :placeholder="t('admin.reverseProxy.targetPlaceholder')"
            class="col-span-3"
          />
        </div>

        <div class="grid grid-cols-4 items-center gap-4">
          <Label class="text-right">{{
            t("admin.reverseProxy.optionsLabel")
          }}</Label>
          <div class="col-span-3 space-y-2">
            <div
              v-if="!isNewMappingWebSocketTarget"
              class="flex items-center space-x-2"
            >
              <Switch id="rewrite" v-model="newMapping.rewrite_html" />
              <Label for="rewrite">{{
                t("admin.reverseProxy.rewriteHtmlContent")
              }}</Label>
            </div>
            <div class="flex items-center space-x-2">
              <Switch id="auth" v-model="newMapping.use_auth" />
              <Label for="auth">{{
                t("admin.reverseProxy.requireAuth")
              }}</Label>
            </div>
            <div
              v-if="!isNewMappingWebSocketTarget"
              class="flex items-center space-x-2"
            >
              <Switch id="root" v-model="newMapping.use_root_mode" />
              <Label for="root">{{
                t("admin.reverseProxy.useRootMode")
              }}</Label>
            </div>
            <div class="flex items-center space-x-2">
              <Switch id="strip" v-model="newMapping.strip_path" />
              <Label for="strip">{{
                t("admin.reverseProxy.stripRequestPrefix")
              }}</Label>
            </div>
          </div>
        </div>
      </div>
      <DialogFooter>
        <Button variant="outline" @click="closeMappingDialog(true)">
          {{ t("admin.reverseProxy.cancel") }}
        </Button>
        <Button @click="saveMapping" :disabled="!isValid || isSaving">{{
          t("admin.reverseProxy.saveSettings")
        }}</Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>

  <Dialog
    :open="isDiscoverDialogOpen"
    @update:open="handleDiscoverDialogOpenChange"
  >
    <DialogContent
      class="sm:max-w-[800px] max-h-[85vh] flex flex-col overflow-hidden"
    >
      <DialogHeader class="shrink-0">
        <div
          class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between"
        >
          <DialogTitle>{{ t("admin.reverseProxy.discoverTitle") }}</DialogTitle>
          <div class="flex items-center gap-2">
            <Button
              variant="outline"
              size="icon"
              :disabled="isDiscovering"
              @click="toggleDiscoverSettings"
            >
              <SlidersHorizontal class="h-4 w-4" />
            </Button>
            <RefreshButton
              :label="t('admin.reverseProxy.refreshServices')"
              :loading="isDiscovering"
              :disabled="isDiscovering"
              @click="triggerScan"
            />
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
            v-if="isDiscovering"
            class="flex flex-col items-center justify-center py-16 space-y-4"
          >
            <RefreshCw class="h-8 w-8 animate-spin text-muted-foreground" />
            <p class="text-sm text-muted-foreground">
              {{ t("admin.reverseProxy.probing") }}
            </p>
          </div>

          <div
            v-else-if="discoveredData && discoveredData.services.length === 0"
            class="text-center py-16 text-muted-foreground"
          >
            {{ t("admin.reverseProxy.discoverEmpty") }}
          </div>

          <div
            v-else-if="discoveredData"
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
            {{
              t("admin.reverseProxy.scannedPorts", {
                count: discoveredData.totalPortsScanned,
              })
            }}，{{
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
          <Button variant="outline" @click="closeDiscoverDialog(true)">
            {{ t("admin.reverseProxy.cancel") }}
          </Button>
          <Button
            @click="saveDiscoveredServices"
            :disabled="
              isDiscovering ||
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

  <Dialog
    :open="isDefaultRouteConfirmOpen"
    @update:open="handleDefaultRouteConfirmOpenChange"
  >
    <DialogContent class="sm:max-w-[520px]">
      <DialogHeader>
        <DialogTitle>{{ defaultRouteDialogTitle }}</DialogTitle>
        <DialogDescription class="space-y-2 text-left">
          <p>{{ defaultRouteDialogDescription }}</p>
          <p v-if="showDefaultRouteFnosHint" class="text-amber-600">
            {{ t("admin.reverseProxy.fnosDefaultRouteHint") }}
          </p>
        </DialogDescription>
      </DialogHeader>
      <DialogFooter>
        <Button
          variant="outline"
          :disabled="isSavingDefaultRoute"
          @click="closeDefaultRouteConfirm"
        >
          {{ t("admin.reverseProxy.cancel") }}
        </Button>
        <Button
          variant="destructive"
          :disabled="isSavingDefaultRoute"
          @click="confirmDefaultRouteChange"
        >
          {{
            isSavingDefaultRoute
              ? t("admin.reverseProxy.processing")
              : t("admin.reverseProxy.continueAction")
          }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>

<script setup lang="ts">
import { ref, computed, nextTick, onMounted } from "vue";
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
import { Switch } from "@/components/ui/switch";
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
import { Label } from "@/components/ui/label";
import {
  ChevronDown,
  RefreshCw,
  Plus,
  Search,
  SlidersHorizontal,
} from "lucide-vue-next";
import { toast } from "@admin-shared/utils/toast";
import { useConfigStore } from "../store/config";
import { ConfigAPI, ScanAPI, SystemAPI } from "../lib/api";
import type { ProxyMapping } from "../types";
import type { ScanDiscoverResponse, DiscoveredServiceInfo } from "../lib/api";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import ProxyTargetInputField from "@admin-shared/components/common/ProxyTargetInputField.vue";
import PagedTableFooter from "@admin-shared/components/list/PagedTableFooter.vue";
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
    showReverseProxyActionError(
      reverseProxyMessages.scanFailed,
      error,
      reverseProxyMessages.unknownError,
    );
  },
});
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

const onToggleAllDiscoverSelect = (e: Event) => {
  const checked = (e.target as HTMLInputElement).checked;
  setAllSelected(checked);
};

const handleDiscoverDialogOpenChange = (nextOpen: boolean) => {
  if (!nextOpen) {
    closeDiscoverDialog(true);
    isDiscoverSettingsOpen.value = false;
  }
};

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
  let targetCidrs: string[] | undefined;
  try {
    await nextTick();
    targetCidrs = await discoverTargetsSettingsRef.value?.ensureSaved();
  } catch {
    return;
  }

  resetDiscoverSelection();
  await runDiscoverServices(
    () => ScanAPI.discover({ target_cidrs: targetCidrs }),
    {
      onSuccess: (data) => {
        setDiscoveredData(data);
        selectedServices.value = data.services.filter((svc) =>
          Boolean(svc.detail.rule.path?.trim()),
        );
      },
    },
  );
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
          closeDiscoverDialog(true);
        },
      },
    );
  });
}
</script>
