<template>
  <div class="space-y-6">
    <ConfigCollapsibleCard
      title="子域模式配置"
      :configured="isSubdomainModeConfigured"
      :ready="!configStore.isLoading"
      edit-label="编辑配置"
      summary-class="text-xs text-muted-foreground truncate max-w-full"
      expanded-content-class="p-0 sm:p-0"
      actions-class="border-t bg-muted/30 px-4 py-4 sm:px-6 flex flex-col-reverse items-stretch gap-2 rounded-b-lg sm:flex-row sm:items-center sm:justify-end"
    >
      <template #summary>
        <template v-if="savedRootDomain">
          根域名 {{ savedRootDomain }}
          <span v-if="authServiceMapping">
            · 鉴权服务 {{ authServiceMapping.host }}
          </span>
          <span v-else> · 鉴权服务未配置 </span>
        </template>
        <template v-else>还未完成根域名配置</template>
      </template>

      <template #default>
        <div class="divide-y divide-border">
          <div class="p-4 sm:p-6">
            <div class="space-y-1">
              <h3 class="text-base font-semibold">子域模式配置</h3>
              <p class="text-sm text-muted-foreground">
                这里只保留子域模式最常用的配置。你通常只需要先填好根域名，再在下方映射里指定一个“鉴权服务”即可。
              </p>
            </div>
          </div>

          <div class="grid gap-4 p-4 sm:p-6">
            <div class="max-w-xs space-y-2">
              <Label for="root-domain">根域名</Label>
              <Input
                id="root-domain"
                v-model="modeForm.root_domain"
                placeholder="example.com"
              />
              <p class="text-xs text-muted-foreground">
                后续新增映射时，你只需要填写子域名前缀，系统会自动拼接到这个根域名下面。
              </p>
            </div>
            <div class="rounded-lg border px-4 py-3">
              <div
                class="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between"
              >
                <div class="space-y-1">
                  <Label>当前鉴权服务</Label>
                </div>
                <Badge :variant="authServiceMapping ? 'secondary' : 'outline'">
                  {{ authServiceMapping ? "已配置" : "未配置" }}
                </Badge>
              </div>
              <div class="mt-3 text-sm">
                <template v-if="authServiceMapping">
                  <div class="break-all font-medium">
                    {{ authServiceMapping.host }}
                  </div>
                  <div class="break-all text-muted-foreground">
                    {{ authServiceMapping.target }}
                  </div>
                  <div class="mt-1 text-xs text-muted-foreground">
                    在尚未登录时，会自动跳转到
                    <code>https://{{ authServiceMapping.host }}</code>
                    完成登录。
                  </div>
                </template>
                <p v-else class="text-muted-foreground">还没有鉴权服务。</p>
              </div>
            </div>
          </div>
        </div>
      </template>

      <template #actions="{ collapse }">
        <Button variant="outline" @click="collapse">折叠</Button>
        <Button
          variant="outline"
          :disabled="isSavingMode || !isModeDirty"
          @click="resetModeForm"
        >
          放弃更改
        </Button>
        <Button
          :disabled="isSavingMode || !isModeValid || !isModeDirty"
          @click="saveMode"
        >
          <span
            v-if="isSavingMode"
            class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
          ></span>
          保存配置
        </Button>
      </template>
    </ConfigCollapsibleCard>

    <Card>
      <CardHeader>
        <CardTitle class="flex items-center justify-between">
          <span>映射管理</span>
          <div class="flex items-center gap-2">
            <Button
              v-if="!authServiceMapping"
              :disabled="!canManageNewMappings || isSavingMappings"
              variant="default"
              @click="addAuthService"
            >
              <ShieldCheck class="mr-2 h-4 w-4" />
              添加鉴权服务
            </Button>
            <div v-if="authServiceMapping" class="flex items-center">
              <Button
                :variant="discoverButtonVariant"
                :disabled="!canManageNewMappings || isDiscovering"
                class="rounded-r-none"
                @click="openDiscoverDialog"
              >
                <Search class="mr-2 h-4 w-4" />
                {{ isDiscovering ? "发现中..." : "一键发现" }}
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
                    :disabled="isSavingMappings"
                    @select="openDeleteAuthServiceDialog"
                  >
                    <Trash2 class="mr-2 h-4 w-4" />
                    删除鉴权服务
                  </DropdownMenuItem>
                  <DropdownMenuItem
                    :disabled="!canManageNewMappings"
                    @click="openCreateDialog"
                  >
                    <Plus class="mr-2 h-4 w-4" />
                    添加映射
                  </DropdownMenuItem>
                  <DropdownMenuItem @click="syncRoutes" :disabled="isSyncing">
                    <RefreshCw
                      class="mr-2 h-4 w-4"
                      :class="{ 'animate-spin': isSyncing }"
                    />
                    {{ isSyncing ? "同步中..." : "同步路由" }}
                  </DropdownMenuItem>
                </DropdownMenuContent>
              </DropdownMenu>
            </div>
          </div>
        </CardTitle>
        <CardDescription>
          每个子域名都会直接映射到一个本地 HTTP
          服务。新增时只需要填写子域名前缀，根域名会自动补齐。
        </CardDescription>
      </CardHeader>
      <CardContent class="space-y-4">
        <SearchInput
          v-model="searchQuery"
          placeholder="搜索子域名或目标地址..."
          class="max-w-xs"
        />
        <p
          v-if="!savedRootDomain || isRootDomainPendingSave"
          class="text-xs text-amber-600"
        >
          {{
            !savedRootDomain
              ? "请先在上方保存根域名，再添加或发现 Host 映射。"
              : "根域名有未保存的修改，请先保存后再添加或发现 Host 映射。"
          }}
        </p>

        <div class="overflow-hidden rounded-md border">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Host</TableHead>
                <TableHead>Target</TableHead>
                <TableHead>状态</TableHead>
                <TableHead class="text-right">操作</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              <TableRow v-if="filteredMappings.length === 0">
                <TableCell
                  colspan="4"
                  class="py-8 text-center text-muted-foreground"
                >
                  还没有配置任何 Host 映射。
                </TableCell>
              </TableRow>
              <TableRow
                v-for="mapping in filteredMappings"
                :key="mapping.host"
                class="group"
              >
                <TableCell class="font-medium">{{ mapping.host }}</TableCell>
                <TableCell>{{ mapping.target }}</TableCell>
                <TableCell>
                  <div
                    class="flex flex-wrap gap-2 text-xs text-muted-foreground"
                  >
                    <Badge
                      v-if="isAuthServiceTarget(mapping.target)"
                      variant="default"
                    >
                      鉴权服务
                    </Badge>
                    <Badge variant="secondary">
                      {{ mapping.use_auth ? "需鉴权" : "公开访问" }}
                    </Badge>
                  </div>
                </TableCell>
                <TableCell class="text-right">
                  <div class="flex justify-end gap-2">
                    <Button
                      variant="ghost"
                      size="sm"
                      @click="openEditDialog(mapping)"
                    >
                      编辑
                    </Button>
                    <Button
                      variant="ghost"
                      size="sm"
                      class="text-destructive hover:bg-destructive/10 hover:text-destructive"
                      :disabled="isSavingMappings"
                      @click="openDeleteMappingDialog(mapping.host)"
                    >
                      删除
                    </Button>
                  </div>
                </TableCell>
              </TableRow>
            </TableBody>
          </Table>
        </div>
      </CardContent>
    </Card>

    <Dialog :open="isDialogOpen" @update:open="handleDialogOpenChange">
      <DialogContent class="sm:max-w-[520px]">
        <DialogHeader>
          <DialogTitle>
            {{ editingHost ? "编辑 Host 映射" : "添加 Host 映射" }}
          </DialogTitle>
          <DialogDescription>
            普通业务域名默认会走统一登录流程；当 Target
            指向鉴权端口时，该域名会自动成为统一登录入口并保持公开可达。
          </DialogDescription>
        </DialogHeader>
        <div class="grid gap-4 py-4">
          <div class="space-y-2">
            <Label for="mapping-subdomain">
              {{ mappingInputMode === "subdomain" ? "子域名" : "Host" }}
            </Label>
            <template v-if="mappingInputMode === 'subdomain'">
              <div class="flex items-stretch rounded-md border">
                <Input
                  id="mapping-subdomain"
                  v-model="mappingSubdomain"
                  placeholder="redis"
                  class="rounded-none border-0 shadow-none focus-visible:ring-0"
                />
                <div
                  class="flex items-center border-l bg-muted/30 px-3 text-sm text-muted-foreground"
                >
                  .{{ savedRootDomain }}
                </div>
              </div>
              <p class="text-xs text-muted-foreground">
                最终地址：{{ composedPreviewHost || "未填写" }}
              </p>
            </template>
            <template v-else>
              <Input
                id="mapping-subdomain"
                v-model="mappingSubdomain"
                placeholder="auth.other-domain.example"
              />
              <p class="text-xs text-amber-600">
                当前映射不在已固定根域名下，编辑时暂按完整 Host 处理。
              </p>
            </template>
          </div>

          <div class="space-y-2">
            <Label for="mapping-target">Target</Label>
            <Input
              id="mapping-target"
              v-model="mappingForm.target"
              placeholder="http://127.0.0.1:5173"
            />
            <p class="text-xs text-muted-foreground">
              如果这里使用的是鉴权端口
              {{ authServicePort }}，系统会自动把它识别为鉴权服务。
            </p>
          </div>

          <div
            class="flex items-center justify-between rounded-lg border px-4 py-3"
          >
            <div class="space-y-1">
              <Label for="mapping-auth">要求认证</Label>
              <p class="text-xs text-muted-foreground">
                未登录时会跳到认证子域，再回到原始业务子域。
              </p>
            </div>
            <Switch
              id="mapping-auth"
              v-model="mappingForm.use_auth"
              :disabled="isMappingAuthService"
            />
          </div>
        </div>
        <DialogFooter>
          <Button variant="outline" @click="closeDialog">取消</Button>
          <Button
            :disabled="!isMappingValid || isSavingMappings"
            @click="saveMapping"
          >
            保存映射
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>

    <Dialog
      :open="isDeleteDialogOpen"
      @update:open="handleDeleteDialogOpenChange"
    >
      <DialogContent class="sm:max-w-[440px]">
        <DialogHeader>
          <DialogTitle>{{ deleteDialogTitle }}</DialogTitle>
          <DialogDescription>
            {{ deleteDialogDescription }}
          </DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button variant="outline" @click="closeDeleteDialog">取消</Button>
          <Button
            variant="destructive"
            :disabled="isSavingMappings"
            @click="confirmDelete"
          >
            <span
              v-if="isSavingMappings"
              class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
            ></span>
            {{ deleteDialogConfirmLabel }}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>

    <Dialog
      :open="isDiscoverDialogOpen"
      @update:open="handleDiscoverDialogOpenChange"
    >
      <DialogContent class="sm:max-w-[820px] max-h-[85vh] flex flex-col">
        <DialogHeader>
          <div class="flex items-center justify-between gap-4">
            <div class="space-y-1">
              <DialogTitle>一键发现本地服务</DialogTitle>
              <DialogDescription>
                扫描本地端口并生成建议子域名，最终会自动拼接到
                <code>.{{ savedRootDomain }}</code> 下。
              </DialogDescription>
            </div>
            <Button
              variant="outline"
              :disabled="isDiscovering"
              @click="triggerScan"
            >
              <RefreshCw
                class="mr-2 h-4 w-4"
                :class="{ 'animate-spin': isDiscovering }"
              />
              {{ isDiscovering ? "扫描中..." : "刷新服务" }}
            </Button>
          </div>
        </DialogHeader>

        <div class="flex-1 overflow-auto py-2">
          <div
            v-if="isDiscovering"
            class="flex flex-col items-center justify-center py-16 space-y-4"
          >
            <RefreshCw class="h-8 w-8 animate-spin text-muted-foreground" />
            <p class="text-sm text-muted-foreground">
              正在探测端口服务，这可能需要几秒钟...
            </p>
          </div>

          <div
            v-else-if="discoveredData && discoveredData.services.length === 0"
            class="text-center py-16 text-muted-foreground"
          >
            {{
              discoveredData.foundServices > 0
                ? "本次扫描到的服务都已添加到 Host 映射中。"
                : "未探测到任何可代理的服务。"
            }}
          </div>

          <div
            v-else-if="discoveredData"
            class="border rounded-md overflow-hidden"
          >
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead class="w-[50px] text-center">
                    <input
                      type="checkbox"
                      class="h-4 w-4 cursor-pointer"
                      :checked="isAllSelected"
                      @change="onToggleAllDiscoverSelect"
                    />
                  </TableHead>
                  <TableHead class="w-[80px]">端口</TableHead>
                  <TableHead class="w-[100px]">状态</TableHead>
                  <TableHead>服务标识</TableHead>
                  <TableHead class="w-[260px]">建议子域名</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                <TableRow
                  v-for="(svc, index) in discoveredData.services"
                  :key="`${svc.port}-${index}`"
                >
                  <TableCell class="text-center">
                    <input
                      type="checkbox"
                      class="h-4 w-4 cursor-pointer"
                      :value="svc"
                      v-model="selectedServices"
                    />
                  </TableCell>
                  <TableCell class="font-medium">{{ svc.port }}</TableCell>
                  <TableCell>
                    <span
                      v-if="svc.httpStatus === 401"
                      class="text-amber-600 bg-amber-500/10 text-xs px-2 py-0.5 rounded"
                    >
                      需认证
                    </span>
                    <span
                      v-else
                      class="text-green-600 bg-green-500/10 text-xs px-2 py-0.5 rounded"
                    >
                      {{ svc.httpStatus }}
                    </span>
                  </TableCell>
                  <TableCell class="text-sm">
                    {{ svc.detail.label || svc.detail.name || "未知服务" }}
                  </TableCell>
                  <TableCell>
                    <div class="flex items-stretch rounded-md border">
                      <Input
                        v-model="svc.suggestedSubdomain"
                        placeholder="service"
                        class="h-8 rounded-none border-0 text-sm shadow-none focus-visible:ring-0"
                        :class="{
                          'border-destructive focus-visible:ring-destructive':
                            selectedServices.includes(svc) &&
                            !svc.suggestedSubdomain.trim(),
                        }"
                      />
                      <div
                        class="flex items-center border-l bg-muted/30 px-3 text-xs text-muted-foreground"
                      >
                        .{{ savedRootDomain }}
                      </div>
                    </div>
                  </TableCell>
                </TableRow>
              </TableBody>
            </Table>
          </div>
        </div>

        <DialogFooter class="sm:justify-between items-center mt-2">
          <span class="text-sm text-muted-foreground">
            <template v-if="discoveredData">
              已扫描 {{ discoveredData.totalPortsScanned }} 个端口，选中
              {{ selectedServices.length }} /
              {{ discoveredData.services.length }}
              项
            </template>
          </span>
          <div class="space-x-2">
            <Button variant="outline" @click="dismissDiscoverDialog">
              取消
            </Button>
            <Button
              :disabled="
                isDiscovering ||
                selectedServices.length === 0 ||
                !isDiscoverSelectionValid ||
                isSavingMappings
              "
              @click="saveDiscoveredServices"
            >
              添加选中项
            </Button>
          </div>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, reactive, ref, watch } from "vue";
import {
  ChevronDown,
  Plus,
  RefreshCw,
  Search,
  ShieldCheck,
  Trash2,
} from "lucide-vue-next";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import ConfigCollapsibleCard from "@admin-shared/components/ConfigCollapsibleCard.vue";
import SearchInput from "@admin-shared/components/SearchInput.vue";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { toast } from "@admin-shared/utils/toast";
import { useDiscoverServicesSelection } from "@admin-shared/composables/useDiscoverServicesSelection";
import { extractPortFromTarget } from "@admin-shared/utils/extractPortFromTarget";
import { useConfigStore } from "../store/config";
import {
  ConfigAPI,
  ScanAPI,
  type DiscoveredServiceInfo,
  type ScanDiscoverResponse,
} from "../lib/api";
import type { HostMapping, SubdomainModeConfig } from "../types";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";

type MappingInputMode = "subdomain" | "full_host";

type DiscoveredHostService = DiscoveredServiceInfo & {
  suggestedSubdomain: string;
};

type DiscoveredHostResponse = Omit<ScanDiscoverResponse, "services"> & {
  services: DiscoveredHostService[];
};

type DeleteDialogState =
  | {
      kind: "auth_service";
      host: string;
    }
  | {
      kind: "mapping";
      host: string;
    };

const configStore = useConfigStore();

const normalizeHostLike = (value: string): string =>
  value
    .trim()
    .toLowerCase()
    .replace(/^[a-z]+:\/\//i, "")
    .replace(/\/.*$/, "")
    .replace(/\.+$/, "");

const normalizeRootDomainValue = (value: string): string =>
  normalizeHostLike(value);

const stripRootDomainSuffix = (value: string, rootDomain: string): string => {
  const normalized = normalizeHostLike(value);
  const normalizedRoot = normalizeRootDomainValue(rootDomain);
  if (!normalizedRoot) return normalized;
  if (normalized === normalizedRoot) return "";
  if (normalized.endsWith(`.${normalizedRoot}`)) {
    return normalized.slice(0, -1 * (normalizedRoot.length + 1));
  }
  return normalized;
};

const composeHostFromSubdomain = (
  subdomain: string,
  rootDomain: string,
): string => {
  const normalizedRoot = normalizeRootDomainValue(rootDomain);
  const normalizedSubdomain = stripRootDomainSuffix(subdomain, normalizedRoot);
  if (!normalizedRoot || !normalizedSubdomain) return "";
  return `${normalizedSubdomain}.${normalizedRoot}`;
};

const buildSuggestedSubdomain = (service: DiscoveredServiceInfo): string => {
  const candidates = [
    service.detail.rule.path,
    service.detail.label,
    service.detail.name,
    `app-${service.port}`,
  ];

  for (const candidate of candidates) {
    const normalized = String(candidate ?? "")
      .trim()
      .replace(/^\/+|\/+$/g, "")
      .replace(/\//g, "-")
      .replace(/\s+/g, "-")
      .replace(/[^a-zA-Z0-9-]+/g, "-")
      .replace(/-+/g, "-")
      .replace(/^-+|-+$/g, "")
      .toLowerCase();

    if (normalized) return normalized;
  }

  return `app-${service.port}`;
};

const parseTargetPort = (target: string): number | null => {
  const normalizedTarget = target.trim();
  if (!normalizedTarget) return null;

  const explicitPort = extractPortFromTarget(normalizedTarget);
  if (
    explicitPort !== null &&
    Number.isFinite(explicitPort) &&
    explicitPort > 0
  ) {
    return explicitPort;
  }

  try {
    const parsed = new URL(normalizedTarget);
    if (parsed.protocol === "https:") return 443;
    if (parsed.protocol === "http:") return 80;
  } catch {
    // ignore
  }

  return null;
};

const createDefaultModeForm = (): SubdomainModeConfig => ({
  root_domain: "",
  auth_host: "",
  auth_target: "http://localhost:7997",
  cookie_domain: "",
  public_auth_base_url: "",
  default_access_mode: "login_first",
  auto_add_whitelist_on_login: true,
  passkey_rp_mode: "auth_host",
  passkey_rp_id: "",
});

const DEFAULT_AUTH_SUBDOMAIN = "auth";
const DEFAULT_ACCESS_MODE: HostMapping["access_mode"] = "login_first";

const createDefaultMapping = (): HostMapping => ({
  host: "",
  target: "",
  use_auth: true,
  access_mode: DEFAULT_ACCESS_MODE,
  preserve_host: true,
  service_role: "app",
});

const searchQuery = ref("");
const isDialogOpen = ref(false);
const deleteDialogState = ref<DeleteDialogState | null>(null);
const editingHost = ref<string | null>(null);
const mappingInputMode = ref<MappingInputMode>("subdomain");
const mappingSubdomain = ref("");
const modeForm = reactive<SubdomainModeConfig>(createDefaultModeForm());
const mappingForm = reactive<HostMapping>(createDefaultMapping());

const currentModeConfig = computed(
  () => configStore.config?.subdomain_mode ?? createDefaultModeForm(),
);
const authServicePort = computed(
  () => parseTargetPort(currentModeConfig.value.auth_target) ?? 7997,
);
const isAuthServiceTarget = (target: string): boolean =>
  parseTargetPort(target) === authServicePort.value;
const savedRootDomain = computed(() =>
  normalizeRootDomainValue(currentModeConfig.value.root_domain),
);
const currentDraftRootDomain = computed(() =>
  normalizeRootDomainValue(modeForm.root_domain),
);
const isRootDomainPendingSave = computed(
  () => currentDraftRootDomain.value !== savedRootDomain.value,
);
const canManageNewMappings = computed(
  () => Boolean(savedRootDomain.value) && !isRootDomainPendingSave.value,
);
const allMappings = computed(() => configStore.config?.host_mappings ?? []);
const existingMappingPorts = computed(() => {
  const ports = new Set<number>();

  for (const mapping of allMappings.value) {
    const port = extractPortFromTarget(mapping.target);
    if (port !== null) {
      ports.add(port);
    }
  }

  return ports;
});
const authServiceMapping = computed(
  () =>
    allMappings.value.find((mapping) => isAuthServiceTarget(mapping.target)) ??
    null,
);
const discoverButtonVariant = computed(() =>
  authServiceMapping.value ? "default" : "secondary",
);
const discoverButtonDividerClass = computed(() =>
  authServiceMapping.value
    ? "border-primary-foreground/20"
    : "border-border/70",
);
const isSubdomainModeConfigured = computed(() => {
  const config = currentModeConfig.value;
  return Boolean(
    savedRootDomain.value ||
    normalizeHostLike(config.auth_host) ||
    authServiceMapping.value,
  );
});
const isMappingAuthService = computed(() =>
  isAuthServiceTarget(mappingForm.target),
);
const isDeleteDialogOpen = computed(() => deleteDialogState.value !== null);
const deleteDialogTitle = computed(() =>
  deleteDialogState.value?.kind === "auth_service"
    ? "确认删除鉴权服务？"
    : "确认删除 Host 映射？",
);
const deleteDialogDescription = computed(() => {
  const target = deleteDialogState.value;
  if (!target) return "";

  if (target.kind === "auth_service") {
    return `将删除 ${target.host} 对应的鉴权映射。删除后需要重新添加鉴权服务`;
  }

  return `您即将删除 Host 映射 ${target.host}，此操作不可逆转。`;
});
const deleteDialogConfirmLabel = computed(() =>
  deleteDialogState.value?.kind === "auth_service"
    ? "删除鉴权服务"
    : "删除映射",
);
const composedPreviewHost = computed(() => {
  if (mappingInputMode.value === "full_host") {
    return normalizeHostLike(mappingSubdomain.value) || "";
  }
  return composeHostFromSubdomain(
    mappingSubdomain.value,
    savedRootDomain.value,
  );
});

const filteredMappings = computed(() => {
  const query = searchQuery.value.trim().toLowerCase();
  const visibleMappings = allMappings.value.filter(
    (mapping) => !isAuthServiceTarget(mapping.target),
  );

  if (!query) return visibleMappings;
  return visibleMappings.filter(
    (mapping) =>
      mapping.host.toLowerCase().includes(query) ||
      mapping.target.toLowerCase().includes(query),
  );
});

const isModeValid = computed(() => true);

const isModeDirty = computed(
  () => JSON.stringify(modeForm) !== JSON.stringify(currentModeConfig.value),
);

const resolveDefaultAuthServiceTarget = (): string => {
  const configuredTarget =
    modeForm.auth_target?.trim() ||
    currentModeConfig.value.auth_target?.trim() ||
    createDefaultModeForm().auth_target;

  try {
    const parsed = new URL(configuredTarget);
    const port =
      parsed.port ||
      (parsed.protocol === "https:"
        ? "443"
        : parsed.protocol === "http:"
          ? "80"
          : "");

    if (!port) return configuredTarget;

    const normalized = new URL(`http://localhost:${port}`);
    normalized.pathname =
      parsed.pathname && parsed.pathname !== "/" ? parsed.pathname : "/";
    normalized.search = parsed.search;
    normalized.hash = parsed.hash;
    return normalized
      .toString()
      .replace(/\/$/, normalized.pathname === "/" ? "" : normalized.pathname);
  } catch {
    return configuredTarget || createDefaultModeForm().auth_target;
  }
};

const isMappingValid = computed(() => {
  const host =
    mappingInputMode.value === "full_host"
      ? normalizeHostLike(mappingSubdomain.value)
      : composeHostFromSubdomain(mappingSubdomain.value, savedRootDomain.value);
  const target = mappingForm.target.trim();

  if (!host || !target) return false;
  if (mappingInputMode.value === "subdomain" && !savedRootDomain.value) {
    return false;
  }

  try {
    const parsed = new URL(target);
    return parsed.protocol === "http:" || parsed.protocol === "https:";
  } catch {
    return false;
  }
});

const { isPending: isSavingMode, run: runSaveMode } = useAsyncAction({
  onError: (error) => {
    toast.error("保存失败", {
      description: extractErrorMessage(error, "子域模式配置保存失败"),
    });
  },
});

const { isPending: isSavingMappings, run: runSaveMappings } = useAsyncAction({
  onError: (error) => {
    toast.error("保存失败", {
      description: extractErrorMessage(error, "Host 映射保存失败"),
    });
  },
});

const { isPending: isSyncing, run: runSyncRoutes } = useAsyncAction({
  onError: (error) => {
    toast.error("同步失败", {
      description: extractErrorMessage(error, "同步网关配置失败"),
    });
  },
});

const { isPending: isDiscovering, run: runDiscoverServices } = useAsyncAction({
  onError: (error) => {
    toast.error("发现失败", {
      description: extractErrorMessage(error, "本地服务扫描失败"),
    });
  },
});

const applyModeForm = (next: SubdomainModeConfig) => {
  modeForm.root_domain = next.root_domain;
  modeForm.auth_host = next.auth_host;
  modeForm.auth_target = next.auth_target;
  modeForm.cookie_domain = next.cookie_domain;
  modeForm.public_auth_base_url = next.public_auth_base_url;
  modeForm.default_access_mode = next.default_access_mode;
  modeForm.auto_add_whitelist_on_login = next.auto_add_whitelist_on_login;
  modeForm.passkey_rp_mode = next.passkey_rp_mode;
  modeForm.passkey_rp_id = next.passkey_rp_id || "";
};

watch(
  () => configStore.config?.subdomain_mode,
  (next) => {
    if (next) {
      applyModeForm(next);
    }
  },
  { immediate: true },
);

const {
  open: isDiscoverDialogOpen,
  discoveredData,
  selectedServices,
  isAllSelected,
  isSelectionValid: isDiscoverSelectionValid,
  setAllSelected,
  resetSelection,
  setDiscoveredData,
  openDialog: openDiscoverDialogState,
  closeDialog: closeDiscoverDialog,
} = useDiscoverServicesSelection<DiscoveredHostService, DiscoveredHostResponse>(
  {
    getPath: (service) => service.suggestedSubdomain,
  },
);

onMounted(async () => {
  if (!configStore.config) {
    await configStore.loadConfig();
  }
});

function resetModeForm() {
  applyModeForm(currentModeConfig.value);
}

async function saveMode() {
  if (!isModeValid.value || !isModeDirty.value) return;
  await runSaveMode(async () => {
    const result = await configStore.saveSubdomainMode({
      ...modeForm,
      root_domain: modeForm.root_domain.trim().toLowerCase(),
      auth_host: modeForm.auth_host.trim().toLowerCase(),
      auth_target: modeForm.auth_target.trim(),
      cookie_domain: modeForm.cookie_domain.trim(),
      public_auth_base_url: modeForm.public_auth_base_url.trim(),
      passkey_rp_id: (modeForm.passkey_rp_id || "").trim().toLowerCase(),
    });
    toast.success("子域模式配置已保存");
    if (result?.ssl_auto_selection?.message) {
      if (result.ssl_auto_selection.applied) {
        toast.success(result.ssl_auto_selection.message, {
          description: result.ssl_auto_selection.label
            ? `已切换到证书：${result.ssl_auto_selection.label}`
            : undefined,
        });
      } else {
        toast.error("SSL 自动切换未完成", {
          description: result.ssl_auto_selection.message,
        });
      }
    }
  });
}

function openCreateDialog() {
  if (!canManageNewMappings.value) {
    toast.error("暂时无法添加映射", {
      description: !savedRootDomain.value
        ? "请先保存根域名配置。"
        : "根域名有未保存修改，请先保存后再添加映射。",
    });
    return;
  }

  editingHost.value = null;
  mappingInputMode.value = "subdomain";
  mappingSubdomain.value = "";
  Object.assign(mappingForm, createDefaultMapping());
  isDialogOpen.value = true;
}

function openEditDialog(mapping: HostMapping) {
  editingHost.value = mapping.host;

  const stripped = stripRootDomainSuffix(mapping.host, savedRootDomain.value);
  if (savedRootDomain.value && stripped) {
    mappingInputMode.value = "subdomain";
    mappingSubdomain.value = stripped;
  } else {
    mappingInputMode.value = "full_host";
    mappingSubdomain.value = mapping.host;
  }

  Object.assign(mappingForm, { ...mapping });
  isDialogOpen.value = true;
}

function closeDialog() {
  isDialogOpen.value = false;
  editingHost.value = null;
  mappingInputMode.value = "subdomain";
  mappingSubdomain.value = "";
  Object.assign(mappingForm, createDefaultMapping());
}

function closeDeleteDialog() {
  deleteDialogState.value = null;
}

function handleDialogOpenChange(nextOpen: boolean) {
  if (!nextOpen) {
    closeDialog();
  }
}

function handleDeleteDialogOpenChange(nextOpen: boolean) {
  if (!nextOpen) {
    closeDeleteDialog();
  }
}

function normalizeMapping(input: HostMapping): HostMapping {
  const serviceRole = isAuthServiceTarget(input.target.trim()) ? "auth" : "app";
  const host =
    mappingInputMode.value === "full_host"
      ? normalizeHostLike(mappingSubdomain.value)
      : composeHostFromSubdomain(mappingSubdomain.value, savedRootDomain.value);

  return {
    host,
    target: input.target.trim(),
    use_auth: serviceRole === "auth" ? false : input.use_auth,
    access_mode: DEFAULT_ACCESS_MODE,
    preserve_host: true,
    service_role: serviceRole,
  };
}

async function addAuthService() {
  if (!canManageNewMappings.value) {
    toast.error("暂时无法添加鉴权服务", {
      description: !savedRootDomain.value
        ? "请先保存根域名配置。"
        : "根域名有未保存修改，请先保存后再添加鉴权服务。",
    });
    return;
  }

  if (authServiceMapping.value) {
    toast.error("鉴权服务已存在", {
      description: `当前已配置 ${authServiceMapping.value.host} 作为鉴权服务。`,
    });
    return;
  }

  const host = composeHostFromSubdomain(
    DEFAULT_AUTH_SUBDOMAIN,
    savedRootDomain.value,
  );
  const target = resolveDefaultAuthServiceTarget();

  if (!host) {
    toast.error("默认鉴权服务生成失败", {
      description: "请先确认根域名已正确保存。",
    });
    return;
  }

  const duplicateHost = allMappings.value.find((item) => item.host === host);
  if (duplicateHost) {
    toast.error("默认鉴权子域已存在", {
      description: `${host} 已存在，请将该映射的 Target 调整到鉴权端口。`,
    });
    return;
  }

  await runSaveMappings(async () => {
    await configStore.saveHostMappings([
      ...allMappings.value,
      {
        host,
        target,
        use_auth: false,
        access_mode: DEFAULT_ACCESS_MODE,
        preserve_host: true,
        service_role: "auth",
      },
    ]);

    toast.success("鉴权服务已添加", {
      description: `${host} -> ${target}`,
    });
  });
}

function openDeleteAuthServiceDialog() {
  if (!authServiceMapping.value) {
    toast.error("当前没有鉴权服务");
    return;
  }

  deleteDialogState.value = {
    kind: "auth_service",
    host: authServiceMapping.value.host,
  };
}

function openDeleteMappingDialog(host: string) {
  deleteDialogState.value = {
    kind: "mapping",
    host,
  };
}

async function removeAuthService(): Promise<boolean> {
  if (!authServiceMapping.value) {
    toast.error("当前没有鉴权服务");
    return false;
  }

  const authHost = authServiceMapping.value.host;

  const removed = await runSaveMappings(async () => {
    await configStore.saveHostMappings(
      allMappings.value.filter((item) => !isAuthServiceTarget(item.target)),
    );

    toast.success("鉴权服务已删除", {
      description: authHost,
    });

    return true;
  });

  return removed === true;
}

async function saveMapping() {
  if (!isMappingValid.value) return;

  const normalized = normalizeMapping(mappingForm);
  const duplicateHost = allMappings.value.find(
    (item) => item.host === normalized.host && item.host !== editingHost.value,
  );
  if (duplicateHost) {
    toast.error("Host 已存在", {
      description: `${normalized.host} 已经配置过映射。`,
    });
    return;
  }

  const duplicateAuthService = allMappings.value.find(
    (item) =>
      isAuthServiceTarget(item.target) && item.host !== editingHost.value,
  );
  if (normalized.service_role === "auth" && duplicateAuthService) {
    toast.error("鉴权服务已存在", {
      description: `当前已配置 ${duplicateAuthService.host} 作为鉴权服务，请先调整那条映射。`,
    });
    return;
  }

  await runSaveMappings(async () => {
    const next = [...allMappings.value];
    const index = editingHost.value
      ? next.findIndex((item) => item.host === editingHost.value)
      : -1;

    if (index >= 0) {
      next[index] = normalized;
    } else {
      next.push(normalized);
    }

    await configStore.saveHostMappings(next);
    toast.success(index >= 0 ? "Host 映射已更新" : "Host 映射已添加");
    closeDialog();
  });
}

async function removeMapping(host: string): Promise<boolean> {
  const target = allMappings.value.find((item) => item.host === host);
  if (!target) return false;

  const removed = await runSaveMappings(async () => {
    await configStore.saveHostMappings(
      allMappings.value.filter((item) => item.host !== host),
    );
    toast.success("Host 映射已删除");

    return true;
  });

  return removed === true;
}

async function confirmDelete() {
  const target = deleteDialogState.value;
  if (!target) return;

  const removed =
    target.kind === "auth_service"
      ? await removeAuthService()
      : await removeMapping(target.host);

  if (removed) {
    closeDeleteDialog();
  }
}

const onToggleAllDiscoverSelect = (event: Event) => {
  const checked = (event.target as HTMLInputElement).checked;
  setAllSelected(checked);
};

function dismissDiscoverDialog() {
  setDiscoveredData(null);
  closeDiscoverDialog(true);
}

const handleDiscoverDialogOpenChange = (nextOpen: boolean) => {
  if (!nextOpen) {
    dismissDiscoverDialog();
  }
};

function openDiscoverDialog() {
  if (!canManageNewMappings.value) {
    toast.error("暂时无法发现服务", {
      description: !savedRootDomain.value
        ? "请先保存根域名配置。"
        : "根域名有未保存修改，请先保存后再发现服务。",
    });
    return;
  }

  openDiscoverDialogState();
  if (!discoveredData.value) {
    void triggerScan();
  }
}

async function triggerScan() {
  resetSelection();
  await runDiscoverServices(() => ScanAPI.discover(), {
    onSuccess: (data) => {
      const nextData: DiscoveredHostResponse = {
        ...data,
        services: data.services
          .map((service) => ({
            ...service,
            detail: {
              ...service.detail,
              rule: { ...service.detail.rule },
            },
            suggestedSubdomain: buildSuggestedSubdomain(service),
          }))
          .filter((service) => !existingMappingPorts.value.has(service.port)),
      };
      setDiscoveredData(nextData);
      selectedServices.value = nextData.services.filter((service) =>
        Boolean(service.suggestedSubdomain.trim()),
      );
    },
  });
}

const collectDuplicateValues = (values: string[]): string[] => {
  const seen = new Set<string>();
  const duplicates = new Set<string>();
  for (const value of values) {
    if (!value) continue;
    if (seen.has(value)) {
      duplicates.add(value);
      continue;
    }
    seen.add(value);
  }
  return [...duplicates];
};

async function saveDiscoveredServices() {
  if (!isDiscoverSelectionValid.value || !savedRootDomain.value) return;

  const candidateHosts = selectedServices.value.map((service) =>
    composeHostFromSubdomain(service.suggestedSubdomain, savedRootDomain.value),
  );
  const existingHostSet = new Set(allMappings.value.map((item) => item.host));
  const duplicateHosts = [
    ...new Set([
      ...candidateHosts.filter((host) => existingHostSet.has(host)),
      ...collectDuplicateValues(candidateHosts),
    ]),
  ];

  if (duplicateHosts.length > 0) {
    toast.error("发现结果包含重复 Host", {
      description: duplicateHosts.join("、"),
    });
    return;
  }

  await runSaveMappings(async () => {
    const next = [...allMappings.value];

    for (const service of selectedServices.value) {
      next.push({
        host: composeHostFromSubdomain(
          service.suggestedSubdomain,
          savedRootDomain.value,
        ),
        target: `http://127.0.0.1:${service.port}/`,
        use_auth: service.detail.rule.use_auth,
        access_mode: DEFAULT_ACCESS_MODE,
        preserve_host: true,
        service_role: "app",
      });
    }

    await configStore.saveHostMappings(next);
    toast.success(`已添加 ${selectedServices.value.length} 条 Host 映射`);
    dismissDiscoverDialog();
  });
}

async function syncRoutes() {
  await runSyncRoutes(() => ConfigAPI.syncRoutes(), {
    onSuccess: (result) => {
      if (result.success) {
        toast.success("已同步到网关", {
          description: `路径路由 ${result.data?.synced_rules ?? 0} 条，Host 路由 ${result.data?.synced_host_rules ?? 0} 条。`,
        });
        return;
      }
      toast.error("同步失败", {
        description: result.message || "网关未返回成功结果",
      });
    },
  });
}
</script>
