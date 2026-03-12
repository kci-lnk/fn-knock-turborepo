<template>
  <Card class="mb-6">
    <CardHeader>
      <CardTitle class="flex justify-between items-center">
        <span>映射管理</span>
        <div class="flex">
          <Button @click="openDiscoverDialog" class="rounded-r-none">
            <Search class="mr-2 w-4 h-4" /> 一键发现
          </Button>
          <DropdownMenu>
            <DropdownMenuTrigger as-child>
              <Button variant="default" size="icon" class="rounded-l-none border-l border-primary-foreground/20 px-2">
                <ChevronDown class="h-4 w-4" />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end">
              <DropdownMenuItem @click="openAddDialog">
                <Plus class="mr-2 h-4 w-4" /> 添加映射
              </DropdownMenuItem>
              <DropdownMenuItem @click="syncRoutes" :disabled="isSyncing">
                <RefreshCw class="mr-2 h-4 w-4" :class="{ 'animate-spin': isSyncing }" />
                {{ isSyncing ? '同步中...' : '同步路由' }}
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
        </div>
      </CardTitle>
      <CardDescription>配置路径映射以将请求路由到后端服务。用户访问端口：{{ accessEntryPort }}。</CardDescription>
    </CardHeader>
    <CardContent>
      <div class="flex items-center mb-4 space-x-2">
        <SearchInput v-model="searchQuery" placeholder="搜索路径或目标地址..." class="max-w-xs" />
      </div>

      <div class="border rounded-md overflow-x-auto">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>触发路径</TableHead>
              <TableHead>目标地址</TableHead>
              <TableHead>选项配置</TableHead>
              <TableHead class="text-right">操作</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            <TableRow v-if="paginatedMappings.length === 0">
              <TableCell colspan="4" class="text-center text-muted-foreground py-6">
                未找到反代映射。
              </TableCell>
            </TableRow>
            <TableRow v-for="(mapping, index) in paginatedMappings" :key="index" class="group transition-colors">
              <TableCell class="font-medium">{{ mapping.path }}</TableCell>
              <TableCell>{{ mapping.target }}</TableCell>
              <TableCell>
                <div class="flex flex-wrap gap-2 text-xs text-muted-foreground whitespace-normal">
                  <Badge
                    v-if="isDefaultRoute(mapping.path)"
                    variant="secondary"
                    class="border border-emerald-500/30 bg-emerald-500/10 text-emerald-700"
                  >
                    默认路由
                  </Badge>
                  <span v-if="mapping.rewrite_html" class="px-2 py-0.5 bg-muted rounded">重写HTML</span>
                  <span v-if="mapping.use_auth" class="px-2 py-0.5 bg-muted rounded">需鉴权</span>
                  <span v-if="mapping.use_root_mode" class="px-2 py-0.5 bg-muted rounded">根目录模式</span>
                  <span v-if="mapping.strip_path" class="px-2 py-0.5 bg-muted rounded">去除前缀</span>
                </div>
              </TableCell>
              <TableCell class="text-right">
                <div class="flex justify-end gap-1">
                  <Button v-if="isDefaultRoute(mapping.path)" variant="outline" size="sm"
                    class="border-border text-muted-foreground hover:text-foreground opacity-0 group-hover:opacity-100 transition-opacity mr-2"
                    @click="requestClearDefaultRoute(mapping)">
                    清除默认路由
                  </Button>
                  <Button v-else variant="outline" size="sm"
                    class="opacity-0 group-hover:opacity-100 transition-opacity mr-2"
                    @click="requestSetDefaultRoute(mapping)">
                    设为默认路由
                  </Button>

                  <Button variant="ghost" size="sm" @click="openEditDialog(mapping)">
                    编辑
                  </Button>

                  <ConfirmDangerPopover
                    title="确认删除?"
                    :description="`您即将删除代理路径 ${mapping.path}，此操作不可逆转。`"
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
                        删除
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

  <Dialog :open="isMappingDialogOpen" @update:open="handleMappingDialogOpenChange">
    <DialogContent class="sm:max-w-[425px]">
      <DialogHeader>
        <DialogTitle>{{ isEditing ? '编辑反向代理' : '添加反向代理' }}</DialogTitle>
        <DialogDescription>
          {{ isEditing ? '修改现有的路径映射配置。' : '配置新的路径映射。' }}
        </DialogDescription>
      </DialogHeader>
      <div class="grid gap-4 py-4">
        <div class="grid grid-cols-4 items-center gap-4">
          <Label for="path" class="text-right">触发路径</Label>
          <Input id="path" v-model="newMapping.path" placeholder="例如：/api" class="col-span-3" />
        </div>
        <div class="grid grid-cols-4 items-center gap-4">
          <Label for="target" class="text-right">目标地址</Label>
          <Input id="target" v-model="newMapping.target" placeholder="例如：http://localhost:8080" class="col-span-3" />
        </div>

        <div class="grid grid-cols-4 items-center gap-4">
          <Label class="text-right">配置选项</Label>
          <div class="col-span-3 space-y-2">
            <div class="flex items-center space-x-2">
              <Switch id="rewrite" v-model="newMapping.rewrite_html" />
              <Label for="rewrite">重写 HTML 内容</Label>
            </div>
            <div class="flex items-center space-x-2">
              <Switch id="auth" v-model="newMapping.use_auth" />
              <Label for="auth">要求身份认证 (鉴权)</Label>
            </div>
            <div class="flex items-center space-x-2">
              <Switch id="root" v-model="newMapping.use_root_mode" />
              <Label for="root">使用根目录模式</Label>
            </div>
            <div class="flex items-center space-x-2">
              <Switch id="strip" v-model="newMapping.strip_path" />
              <Label for="strip">去除请求前缀</Label>
            </div>
          </div>
        </div>

      </div>
      <DialogFooter>
        <Button variant="outline" @click="closeMappingDialog">取消</Button>
        <Button @click="saveMapping" :disabled="!isValid || isSaving">保存设置</Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>

  <Dialog :open="isDiscoverDialogOpen" @update:open="handleDiscoverDialogOpenChange">
    <DialogContent class="sm:max-w-[800px] max-h-[85vh] flex flex-col">
      <DialogHeader>
        <div class="flex items-center justify-between">
          <DialogTitle>一键发现本地服务</DialogTitle>
          <RefreshButton
            class="mr-6"
            label="刷新服务"
            :loading="isDiscovering"
            :disabled="isDiscovering"
            @click="triggerScan"
          />
        </div>
        <DialogDescription>
          扫描本地端口的运行服务，快速选择并添加至反向代理映射。
        </DialogDescription>
      </DialogHeader>

      <div class="flex-1 overflow-auto py-2">
        <div v-if="isDiscovering" class="flex flex-col items-center justify-center py-16 space-y-4">
          <RefreshCw class="h-8 w-8 animate-spin text-muted-foreground" />
          <p class="text-sm text-muted-foreground">正在探测端口服务，这可能需要两秒钟...</p>
        </div>

        <div v-else-if="discoveredData && discoveredData.services.length === 0"
          class="text-center py-16 text-muted-foreground">
          未探测到任何可代理的服务。
        </div>

        <div v-else-if="discoveredData" class="border rounded-md overflow-hidden">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead class="w-[50px] text-center">
                  <input type="checkbox" class="h-4 w-4 rounded border-gray-300 text-primary cursor-pointer"
                    :checked="isAllSelected" @change="onToggleAllDiscoverSelect" />
                </TableHead>
                <TableHead class="w-[80px]">端口</TableHead>
                <TableHead class="w-[100px]">状态</TableHead>
                <TableHead>服务标识</TableHead>
                <TableHead class="w-[200px]">建议路径</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              <TableRow v-for="(svc, index) in discoveredData.services" :key="index">
                <TableCell class="text-center">
                  <input type="checkbox" class="h-4 w-4 rounded border-gray-300 text-primary cursor-pointer"
                    :value="svc" v-model="selectedServices" />
                </TableCell>
                <TableCell class="font-medium">
                  <a :href="`http://${currentHostname}:${svc.port}`" target="_blank"
                    class="text-primary hover:underline hover:text-primary/80 transition-colors" title="在新窗口打开">
                    {{ svc.port }}
                  </a>
                </TableCell>
                <TableCell>
                  <span v-if="svc.httpStatus === 401"
                    class="text-amber-600 bg-amber-500/10 text-xs px-2 py-0.5 rounded">需认证</span>
                  <span v-else class="text-green-600 bg-green-500/10 text-xs px-2 py-0.5 rounded">{{ svc.httpStatus
                    }}</span>
                </TableCell>
                <TableCell>
                  <span v-if="svc.detail.label" class="text-sm">{{ svc.detail.label }}</span>
                  <span v-else class="text-red-500 text-sm font-medium">未知服务</span>
                </TableCell>
                <TableCell>
                  <Input v-model="svc.detail.rule.path" placeholder="必填，例如 /app" class="h-8 text-sm"
                    :class="{ 'border-destructive focus-visible:ring-destructive': selectedServices.includes(svc) && !svc.detail.rule.path.trim() }" />
                </TableCell>
              </TableRow>
            </TableBody>
          </Table>
        </div>
      </div>

      <DialogFooter class="sm:justify-between items-center mt-2">
        <span class="text-sm text-muted-foreground">
          <template v-if="discoveredData">
            已扫描 {{ discoveredData.totalPortsScanned }} 个端口，选中 {{ selectedServices.length }} / {{
              discoveredData.services.length }} 项
          </template>
        </span>
        <div class="space-x-2">
          <Button variant="outline" @click="closeDiscoverDialog(true)">取消</Button>
          <Button @click="saveDiscoveredServices"
            :disabled="isDiscovering || selectedServices.length === 0 || !isDiscoverSelectionValid || isSaving">
            添加选中项
          </Button>
        </div>
      </DialogFooter>
    </DialogContent>
  </Dialog>

  <Dialog :open="isDefaultRouteConfirmOpen" @update:open="handleDefaultRouteConfirmOpenChange">
    <DialogContent class="sm:max-w-[520px]">
      <DialogHeader>
        <DialogTitle>{{ defaultRouteDialogTitle }}</DialogTitle>
        <DialogDescription class="space-y-2 text-left">
          <p>{{ defaultRouteDialogDescription }}</p>
          <p v-if="showDefaultRouteFnosHint" class="text-amber-600">
            当前默认路由为飞牛 OS，建议保留 5666 端口对应路由作为默认路由。
          </p>
        </DialogDescription>
      </DialogHeader>
      <DialogFooter>
        <Button variant="outline" :disabled="isSavingDefaultRoute" @click="closeDefaultRouteConfirm">
          取消
        </Button>
        <Button variant="destructive" :disabled="isSavingDefaultRoute" @click="confirmDefaultRouteChange">
          {{ isSavingDefaultRoute ? '处理中...' : '继续操作' }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue';
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import RefreshButton from '@/components/RefreshButton.vue';
import { Input } from '@/components/ui/input';
import SearchInput from '@admin-shared/components/SearchInput.vue';
import { Switch } from '@/components/ui/switch';
import { Table, TableHeader, TableRow, TableHead, TableBody, TableCell } from '@/components/ui/table';
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from '@/components/ui/dialog';
import { DropdownMenu, DropdownMenuTrigger, DropdownMenuContent, DropdownMenuItem } from '@/components/ui/dropdown-menu';
import { Label } from '@/components/ui/label';
import { ChevronDown, RefreshCw, Plus, Search } from 'lucide-vue-next';
import { toast } from '@admin-shared/utils/toast';
import { useConfigStore } from '../store/config';
import { ConfigAPI, ScanAPI, SystemAPI } from '../lib/api';
import type { ProxyMapping } from '../types';
import type { ScanDiscoverResponse, DiscoveredServiceInfo } from '../lib/api';
import ConfirmDangerPopover from '@admin-shared/components/common/ConfirmDangerPopover.vue';
import PagedTableFooter from '@admin-shared/components/list/PagedTableFooter.vue';
import { useAsyncAction } from '@admin-shared/composables/useAsyncAction';
import { useDiscoverServicesSelection } from '@admin-shared/composables/useDiscoverServicesSelection';
import { useDefaultRouteConfirm } from '@admin-shared/composables/useDefaultRouteConfirm';
import { useProxyMappingDialogForm } from '@admin-shared/composables/useProxyMappingDialogForm';
import { useLocalPagedList } from '@admin-shared/composables/useLocalPagedList';
import { needsClearDefaultRouteConfirm, needsSetDefaultRouteConfirm } from '@admin-shared/utils/defaultRouteGuard';
import { extractPortFromTarget } from '@admin-shared/utils/extractPortFromTarget';
import { persistProxyMappings } from '@admin-shared/utils/persistProxyMappings';
import { buildProxyMapping, DEFAULT_PROXY_MAPPING_FLAGS } from '@admin-shared/utils/proxyMapping';
import {
  REVERSE_PROXY_MESSAGES,
  showReverseProxyActionError,
  showReverseProxyBooleanResultToast,
  showReverseProxyDuplicateItemsError,
} from '@admin-shared/utils/reverseProxyFeedback';
import { validateBatchMappingDuplicates, validateSingleMappingDuplicates } from '@admin-shared/utils/validateProxyMappingDuplicates';

const currentHostname = window.location.hostname;

const isDefaultRoute = (path: string) => {
  return configStore.config?.default_route === path;
};

const DEFAULT_SYSTEM_PORT = 5666;

const configStore = useConfigStore();
const accessEntryPort = ref('7999');

const removingPath = ref<string | null>(null);
const { run: runRemoveMapping } = useAsyncAction({
  onError: (error) => {
    showReverseProxyActionError(REVERSE_PROXY_MESSAGES.deleteFailed, error, REVERSE_PROXY_MESSAGES.unknownError);
  },
});

const { isPending: isSaving, run: runSaveAction } = useAsyncAction({
  onError: (error) => {
    showReverseProxyActionError(REVERSE_PROXY_MESSAGES.saveFailed, error, REVERSE_PROXY_MESSAGES.unknownError);
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

const handleMappingDialogOpenChange = (nextOpen: boolean) => {
  if (!nextOpen) {
    closeMappingDialog(true);
  }
};

const { isPending: isSyncing, run: runSyncRoutes } = useAsyncAction({
  onError: (error) => {
    showReverseProxyActionError(REVERSE_PROXY_MESSAGES.syncFailed, error, REVERSE_PROXY_MESSAGES.networkError);
  },
});
const { isPending: isSavingDefaultRoute, run: runSaveDefaultRoute } = useAsyncAction({
  onError: (error) => {
    showReverseProxyActionError(REVERSE_PROXY_MESSAGES.defaultRouteUpdateFailed, error, REVERSE_PROXY_MESSAGES.unknownError);
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
  if (!currentDefaultPath || currentDefaultPath === '/__select__') return null;
  return allMappings.value.find((mapping) => mapping.path === currentDefaultPath) ?? null;
});
const currentDefaultRoutePort = computed(() => {
  if (!currentDefaultRouteMapping.value) return null;
  return extractPortFromTarget(currentDefaultRouteMapping.value.target);
});
function requestClearDefaultRoute(mapping: ProxyMapping) {
  const targetPort = extractPortFromTarget(mapping.target);
  if (needsClearDefaultRouteConfirm(targetPort, DEFAULT_SYSTEM_PORT)) {
    queueDefaultRouteAction('/__select__', 'clear', targetPort);
    return;
  }
  void applyDefaultRoute('/__select__');
}

function requestSetDefaultRoute(mapping: ProxyMapping) {
  if (needsSetDefaultRouteConfirm(
    currentDefaultRoutePort.value,
    currentDefaultRouteMapping.value?.path,
    mapping.path,
    DEFAULT_SYSTEM_PORT,
  )) {
    queueDefaultRouteAction(mapping.path, 'set', currentDefaultRoutePort.value);
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
    mapping.path.toLowerCase().includes(query) || mapping.target.toLowerCase().includes(query),
});

onMounted(() => {
  void loadAccessEntryPort();
});

async function loadAccessEntryPort() {
  try {
    const info = await SystemAPI.getAccessEntry();
    accessEntryPort.value = info.port;
  } catch (error) {
    console.warn('load access entry port failed:', error);
  }
}

async function removeMapping(mapping: ProxyMapping) {
  removingPath.value = mapping.path;
  await runRemoveMapping(async () => {
    const newList = allMappings.value.filter(item => item !== mapping);
    await configStore.saveProxyMappings(newList);

    if (isDefaultRoute(mapping.path)) {
      await configStore.saveDefaultRoute('/__select__');
    }

    if (paginatedMappings.value.length === 1 && currentPage.value > 1) {
      currentPage.value--;
    }

    toast.success(REVERSE_PROXY_MESSAGES.deleteSuccess);
  }, {
    onFinally: () => {
      removingPath.value = null;
    },
  });
}

async function saveMapping() {
  if (!isValid.value) return;
  const normalizedMapping = buildProxyMapping(newMapping);
  const { path: trimmedPath, target: trimmedTarget } = normalizedMapping;
  const ignorePath = isEditing.value && editingOriginalMapping.value ? editingOriginalMapping.value.path.trim() : null;
  const ignoreTarget = isEditing.value && editingOriginalMapping.value ? editingOriginalMapping.value.target.trim() : null;
  const { duplicatePath, duplicateTarget } = validateSingleMappingDuplicates(
    allMappings.value,
    { path: trimmedPath, target: trimmedTarget },
    { ignorePath, ignoreTarget },
  );

  if (duplicatePath) {
    toast.error(REVERSE_PROXY_MESSAGES.duplicatePath(trimmedPath));
    return;
  }
  if (duplicateTarget) {
    toast.error(REVERSE_PROXY_MESSAGES.duplicateTarget(trimmedTarget));
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

    await persistProxyMappings(newList, {
      saveMappings: (list) => configStore.saveProxyMappings(list),
      saveDefaultRoute: (path) => configStore.saveDefaultRoute(path),
      resetPage: () => {
        currentPage.value = 1;
      },
      resetSearch: () => {
        searchQuery.value = '';
      },
    }, {
      resetPage: isCreate,
      resetSearch: isCreate,
      onAfterPersist: () => {
        closeMappingDialog(true);
      },
    });

    toast.success(isCreate ? REVERSE_PROXY_MESSAGES.createSuccess : REVERSE_PROXY_MESSAGES.updateSuccess);
  });
}

async function syncRoutes() {
  await runSyncRoutes(
    () => ConfigAPI.syncRoutes(),
    {
      onSuccess: (result) => {
        showReverseProxyBooleanResultToast(result, {
          successText: REVERSE_PROXY_MESSAGES.syncSuccess(result.data?.synced_rules ?? 0),
          errorText: REVERSE_PROXY_MESSAGES.syncFailed,
        });
      },
    },
  );
}

const { isPending: isDiscovering, run: runDiscoverServices } = useAsyncAction({
  onError: (error) => {
    showReverseProxyActionError(REVERSE_PROXY_MESSAGES.scanFailed, error, REVERSE_PROXY_MESSAGES.unknownError);
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

const onToggleAllDiscoverSelect = (e: Event) => {
  const checked = (e.target as HTMLInputElement).checked;
  setAllSelected(checked);
};

const handleDiscoverDialogOpenChange = (nextOpen: boolean) => {
  if (!nextOpen) {
    closeDiscoverDialog(true);
  }
};

function openDiscoverDialog() {
  openDiscoverDialogState();
  // 仅当首次或者未扫描时主动触发，也可以每次打开都触发
  if (!discoveredData.value) {
    triggerScan();
  }
}

async function triggerScan() {
  resetDiscoverSelection();
  await runDiscoverServices(
    () => ScanAPI.discover(),
    {
      onSuccess: (data) => {
        setDiscoveredData(data);
        selectedServices.value = data.services.filter((svc) => Boolean(svc.detail.rule.path?.trim()));
      },
    },
  );
}

async function saveDiscoveredServices() {
  if (!isDiscoverSelectionValid.value) return;
  const candidates = selectedServices.value.map((svc) => ({
    path: svc.detail.rule.path?.trim() || '',
    target: `http://127.0.0.1:${svc.port}/`.trim(),
  }));
  const { duplicatePaths, duplicateTargets } = validateBatchMappingDuplicates(allMappings.value, candidates);

  if (duplicatePaths.length > 0) {
    showReverseProxyDuplicateItemsError('路径', duplicatePaths);
    return;
  }
  if (duplicateTargets.length > 0) {
    showReverseProxyDuplicateItemsError('目标地址', duplicateTargets);
    return;
  }

  await runSaveAction(async () => {
    const newList = [...allMappings.value];
    let defaultRouteToSet: string | null = null;
    let addedCount = 0;

    for (const svc of selectedServices.value) {
      const rule = svc.detail.rule;
      const newMap = buildProxyMapping({
        path: rule.path,
        target: `http://127.0.0.1:${svc.port}/`,
        rewrite_html: rule.rewrite_html,
        use_auth: rule.use_auth,
        use_root_mode: rule.use_root_mode,
        strip_path: rule.strip_path,
      });

      newList.push(newMap);
      addedCount++;

      // 如果数据中标识该服务应作为默认服务
      if (svc.detail.isDefault) {
        defaultRouteToSet = newMap.path;
      }
    }

    await persistProxyMappings(newList, {
      saveMappings: (list) => configStore.saveProxyMappings(list),
      saveDefaultRoute: (path) => configStore.saveDefaultRoute(path),
      resetPage: () => {
        currentPage.value = 1;
      },
      resetSearch: () => {
        searchQuery.value = '';
      },
    }, {
      defaultRoutePath: defaultRouteToSet,
      resetPage: true,
      onAfterPersist: () => {
        toast.success(REVERSE_PROXY_MESSAGES.discoverSaveSuccess(addedCount));
        closeDiscoverDialog(true);
      },
    });
  });
}
</script>
