<script setup lang="ts">
import { computed, onMounted, reactive, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import {
  Breadcrumb,
  BreadcrumbItem,
  BreadcrumbLink,
  BreadcrumbList,
  BreadcrumbPage,
  BreadcrumbSeparator,
} from "@/components/ui/breadcrumb";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Skeleton } from "@/components/ui/skeleton";
import { Switch } from "@/components/ui/switch";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import ProxyTargetInputField from "@admin-shared/components/common/ProxyTargetInputField.vue";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { useDelayedLoading } from "@admin-shared/composables/useDelayedLoading";
import { toast } from "@admin-shared/utils/toast";
import { Pencil, Trash2 } from "lucide-vue-next";
import ResponseBodyEditor from "../../components/ResponseBodyEditor.vue";
import ResponseContentTypeField from "../../components/ResponseContentTypeField.vue";
import { isAnySubdomainRoutingMode } from "../../lib/reverse-proxy-submode";
import { useConfigStore } from "../../store/config";
import type {
  HostMapping,
  HostLocation,
  HostLocationAction,
} from "../../types";

type HeaderRow = {
  name: string;
  value: string;
};

type LocationForm = Omit<HostLocation, "response"> & {
  response: HostLocation["response"];
  headers: HeaderRow[];
};

type HostMappingTitleInfo = Pick<HostMapping, "title" | "title_override">;

const DEFAULT_RESPONSE_CONTENT_TYPE = "text/plain; charset=utf-8";
const forbiddenResponseHeaders = new Set([
  "connection",
  "keep-alive",
  "proxy-connection",
  "proxy-authenticate",
  "proxy-authorization",
  "te",
  "trailer",
  "transfer-encoding",
  "upgrade",
  "content-length",
  "content-type",
]);

const route = useRoute();
const router = useRouter();
const configStore = useConfigStore();
const selectedHost = ref("");
const editingIndex = ref<number | null>(null);
const isDialogOpen = ref(false);
const isHostPickerOpen = ref(false);
const draftLocations = ref<HostLocation[]>([]);
const form = reactive<LocationForm>({
  path: "",
  match: "exact",
  action: "proxy",
  target: "",
  strip_path: true,
  rewrite_html: true,
  response: {
    status: 200,
    content_type: DEFAULT_RESPONSE_CONTENT_TYPE,
    headers: {},
    body: "",
  },
  headers: [],
});

const { isPending: isLoading, run: runLoad } = useAsyncAction({
  onError: (error) => {
    toast.error("加载失败", {
      description: extractErrorMessage(error, "无法获取 Host 映射"),
    });
  },
});
const showLoadingSkeleton = useDelayedLoading(isLoading);
const { isPending: isSaving, run: runSave } = useAsyncAction({
  onError: (error) => {
    toast.error("保存失败", {
      description: extractErrorMessage(error, "保存路径响应失败"),
    });
  },
});

const availableMappings = computed(() =>
  (configStore.config?.host_mappings ?? []).filter(
    (mapping) => mapping.service_role !== "auth",
  ),
);
const selectedMapping = computed(
  () =>
    availableMappings.value.find(
      (mapping) => mapping.host === selectedHost.value,
    ) ?? null,
);
const isAvailable = computed(() =>
  isAnySubdomainRoutingMode(configStore.config),
);
const isDirty = computed(() => {
  const saved = selectedMapping.value?.locations ?? [];
  return JSON.stringify(saved) !== JSON.stringify(draftLocations.value);
});
const sortedDraftLocations = computed(() =>
  draftLocations.value.map((location, index) => ({ location, index })),
);
const canSave = computed(
  () => Boolean(selectedMapping.value) && isDirty.value && !isSaving.value,
);

const getMappingDisplayTitle = (mapping?: HostMappingTitleInfo | null) =>
  mapping?.title_override.trim() || mapping?.title.trim() || "";

const getMappingTitleForDisplay = (mapping?: HostMappingTitleInfo | null) =>
  getMappingDisplayTitle(mapping) || "-";

const createDefaultLocation = (): HostLocation => ({
  path: "",
  match: "exact",
  action: "proxy",
  target: "",
  strip_path: true,
  rewrite_html: true,
  response: {
    status: 200,
    content_type: DEFAULT_RESPONSE_CONTENT_TYPE,
    headers: {},
    body: "",
  },
});

const cloneLocation = (location: HostLocation): HostLocation => ({
  ...location,
  response: {
    status: location.response?.status ?? 200,
    content_type:
      location.response?.content_type?.trim() || DEFAULT_RESPONSE_CONTENT_TYPE,
    headers: { ...(location.response?.headers ?? {}) },
    body: location.response?.body ?? "",
  },
});

const headersToRows = (headers: Record<string, string>): HeaderRow[] =>
  Object.entries(headers).map(([name, value]) => ({ name, value }));

const rowsToHeaders = (rows: HeaderRow[]): Record<string, string> => {
  const headers: Record<string, string> = {};
  for (const row of rows) {
    const name = row.name.trim();
    if (!name) continue;
    headers[name] = row.value;
  }
  return headers;
};

const resetDraftFromSelected = () => {
  draftLocations.value = (selectedMapping.value?.locations ?? []).map(
    cloneLocation,
  );
};

const selectHost = (host: string) => {
  selectedHost.value = host;
  void router.replace({
    path: "/system/gateway-locations",
    query: host ? { host } : {},
  });
  resetDraftFromSelected();
};

const openHostPicker = () => {
  if (!isAvailable.value || availableMappings.value.length === 0) return;
  isHostPickerOpen.value = true;
};

const selectHostFromDialog = (host: string) => {
  selectHost(host);
  isHostPickerOpen.value = false;
};

const handleHostPickerOpenChange = (open: boolean) => {
  isHostPickerOpen.value = open;
};

const ensureSelectedHost = () => {
  const requestedHost =
    typeof route.query.host === "string" ? route.query.host.trim() : "";
  const hostExists = availableMappings.value.some(
    (mapping) => mapping.host === requestedHost,
  );
  selectedHost.value = hostExists
    ? requestedHost
    : (availableMappings.value[0]?.host ?? "");
  resetDraftFromSelected();
};

const openCreateDialog = () => {
  editingIndex.value = null;
  Object.assign(form, createDefaultLocation(), { headers: [] });
  isDialogOpen.value = true;
};

const openEditDialog = (index: number) => {
  const location = draftLocations.value[index];
  if (!location) return;
  editingIndex.value = index;
  Object.assign(form, cloneLocation(location), {
    headers: headersToRows(location.response?.headers ?? {}),
  });
  isDialogOpen.value = true;
};

const closeDialog = () => {
  isDialogOpen.value = false;
  editingIndex.value = null;
  Object.assign(form, createDefaultLocation(), { headers: [] });
};

const setAction = (action: HostLocationAction) => {
  form.action = action;
  if (action === "response") {
    form.strip_path = false;
    form.rewrite_html = false;
  } else {
    form.strip_path = true;
    form.rewrite_html = true;
  }
};

const addHeaderRow = () => {
  form.headers.push({ name: "", value: "" });
};

const removeHeaderRow = (index: number) => {
  form.headers.splice(index, 1);
};

const isValidHeaderName = (value: string): boolean =>
  /^[!#$%&'*+\-.^_`|~0-9A-Za-z]+$/.test(value);

const cleanHostLocationPath = (value: string): string => {
  const raw = value.trim();
  if (!raw.startsWith("/")) return raw;

  const segments: string[] = [];
  for (const segment of raw.split("/")) {
    if (!segment || segment === ".") continue;
    if (segment === "..") {
      segments.pop();
      continue;
    }
    segments.push(segment);
  }

  return `/${segments.join("/")}`;
};

const formError = computed(() => {
  const rawLocationPath = form.path.trim();
  if (!rawLocationPath) return "请填写路径";
  if (!rawLocationPath.startsWith("/")) return "路径必须以 / 开头";
  const locationPath = cleanHostLocationPath(rawLocationPath);
  if (locationPath === "/") return "不允许配置根路径 /";
  if (
    locationPath.startsWith("/__") ||
    locationPath === "/s" ||
    locationPath === "/s/"
  ) {
    return "不能使用保留路径";
  }
  const duplicate = draftLocations.value.some(
    (location, index) =>
      index !== editingIndex.value &&
      location.path === locationPath &&
      location.match === form.match,
  );
  if (duplicate) return "同一 Host 下已存在相同匹配方式和路径";
  if (form.action === "proxy" && !form.target.trim()) {
    return "请填写反代目标";
  }
  if (form.action === "response") {
    const status = Math.floor(Number(form.response.status) || 0);
    if (status < 100 || status > 599) {
      return "响应状态码必须在 100 到 599 之间";
    }
    const seen = new Set<string>();
    for (const row of form.headers) {
      const name = row.name.trim();
      if (!name && !row.value) continue;
      if (!name) return "响应头名称不能为空";
      if (!isValidHeaderName(name)) return `响应头 ${name} 不合法`;
      const key = name.toLowerCase();
      if (forbiddenResponseHeaders.has(key)) {
        return `不能自定义响应头 ${name}`;
      }
      if (seen.has(key)) return `响应头 ${name} 重复`;
      seen.add(key);
    }
  }
  return "";
});

const buildLocationFromForm = (): HostLocation => {
  const action = form.action;
  return {
    path: cleanHostLocationPath(form.path),
    match: form.match,
    action,
    target: action === "proxy" ? form.target.trim() : "",
    strip_path: action === "proxy" ? form.strip_path : false,
    rewrite_html: action === "proxy" ? form.rewrite_html : false,
    response:
      action === "response"
        ? {
            status: Math.floor(Number(form.response.status) || 200),
            content_type:
              form.response.content_type.trim() ||
              DEFAULT_RESPONSE_CONTENT_TYPE,
            headers: rowsToHeaders(form.headers),
            body: form.response.body,
          }
        : {
            status: 200,
            content_type: DEFAULT_RESPONSE_CONTENT_TYPE,
            headers: {},
            body: "",
          },
  };
};

const persistLocations = async (locations: HostLocation[]) => {
  const host = selectedHost.value;
  const mapping = selectedMapping.value;
  if (!host || !mapping) return false;

  const result = await runSave(
    () =>
      configStore.saveHostMappings(
        (configStore.config?.host_mappings ?? []).map((item) =>
          item.host === host
            ? {
                ...item,
                locations: locations.map(cloneLocation),
              }
            : item,
        ),
      ),
    {
      onSuccess: () => {
        resetDraftFromSelected();
        toast.success("路径响应已保存并同步到网关");
      },
    },
  );
  return result !== undefined;
};

const saveDialogLocation = async () => {
  if (formError.value) {
    toast.error("路径规则未保存", { description: formError.value });
    return;
  }

  const nextLocation = buildLocationFromForm();
  const nextLocations =
    editingIndex.value === null
      ? [...draftLocations.value, nextLocation]
      : draftLocations.value.map((location, index) =>
          index === editingIndex.value ? nextLocation : location,
        );

  const saved = await persistLocations(nextLocations);
  if (saved) closeDialog();
};

const removeLocation = (index: number) => {
  draftLocations.value = draftLocations.value.filter(
    (_, itemIndex) => itemIndex !== index,
  );
};

const saveLocations = async () => {
  await persistLocations(draftLocations.value);
};

const formatAction = (location: HostLocation) =>
  location.action === "response" ? "固定响应" : "反代";

const formatTarget = (location: HostLocation) => {
  if (location.action === "response") {
    return `${location.response.status || 200} ${location.response.content_type || DEFAULT_RESPONSE_CONTENT_TYPE}`;
  }
  return location.target;
};

watch(
  () => route.query.host,
  () => {
    ensureSelectedHost();
  },
);

onMounted(async () => {
  if (!configStore.config) {
    await runLoad(() => configStore.loadConfig());
  }
  ensureSelectedHost();
});
</script>

<template>
  <div class="space-y-5">
    <Breadcrumb>
      <BreadcrumbList>
        <BreadcrumbItem>
          <BreadcrumbLink href="#/system">系统设置</BreadcrumbLink>
        </BreadcrumbItem>
        <BreadcrumbSeparator />
        <BreadcrumbItem>
          <BreadcrumbLink href="#/system?tab=gateway">网关</BreadcrumbLink>
        </BreadcrumbItem>
        <BreadcrumbSeparator />
        <BreadcrumbItem>
          <BreadcrumbPage>路径响应</BreadcrumbPage>
        </BreadcrumbItem>
      </BreadcrumbList>
    </Breadcrumb>

    <div
      class="flex flex-col gap-4 sm:flex-row sm:items-end sm:justify-between"
    >
      <div class="max-w-3xl space-y-1.5">
        <h1 class="text-2xl font-semibold tracking-normal">路径响应</h1>
        <p class="text-sm leading-6 text-muted-foreground">
          为指定 Host 添加路径级反代或固定响应。未命中的请求仍访问该 Host
          的目标地址。
        </p>
      </div>
      <Button
        class="w-full sm:w-auto"
        :disabled="!selectedMapping || !isAvailable"
        @click="openCreateDialog"
      >
        添加规则
      </Button>
    </div>

    <Card class="border-border/60 shadow-none">
      <CardContent class="space-y-5 pt-6">
        <div
          v-if="isLoading && showLoadingSkeleton"
          class="space-y-4 rounded-md border border-border/60 bg-muted/20 p-5"
        >
          <Skeleton class="h-10 w-full rounded-md" />
          <Skeleton class="h-24 w-full rounded-md" />
        </div>

        <template v-else>
          <Alert v-if="!isAvailable" class="border-zinc-200 bg-zinc-50">
            <AlertTitle>当前模式暂不可用</AlertTitle>
            <AlertDescription class="text-sm leading-6 text-zinc-700">
              路径响应仅在子域映射模式下生效。
            </AlertDescription>
          </Alert>

          <button
            type="button"
            class="grid w-full gap-4 rounded-md border border-border/60 bg-background px-5 py-4 text-left transition-colors hover:border-primary/30 hover:bg-muted/20 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-60 sm:grid-cols-[minmax(0,1.2fr)_minmax(0,1fr)_minmax(0,1fr)_5rem] sm:items-center"
            :disabled="!isAvailable || availableMappings.length === 0"
            :aria-label="`切换路径响应 Host，当前为 ${selectedMapping?.host || '暂无可用 Host'}，站点标题 ${getMappingTitleForDisplay(selectedMapping)}`"
            @click="openHostPicker"
          >
            <span class="min-w-0 space-y-1">
              <span class="block text-xs font-medium text-muted-foreground">
                当前 Host
              </span>
              <span class="block truncate text-base font-semibold leading-6">
                {{ selectedMapping?.host || "暂无可用 Host" }}
              </span>
              <span class="block truncate text-sm text-muted-foreground">
                {{
                  availableMappings.length > 0
                    ? "点击切换配置对象"
                    : "创建 Host 后可配置路径响应"
                }}
              </span>
            </span>

            <span
              class="min-w-0 space-y-1 border-t border-border/60 pt-3 sm:border-l sm:border-t-0 sm:pl-5 sm:pt-0"
            >
              <span class="block text-xs font-medium text-muted-foreground">
                站点标题
              </span>
              <span class="flex min-w-0 items-center gap-2">
                <span class="truncate text-sm font-medium">
                  {{ getMappingTitleForDisplay(selectedMapping) }}
                </span>
              </span>
            </span>

            <span
              class="min-w-0 space-y-1 border-t border-border/60 pt-3 sm:border-l sm:border-t-0 sm:pl-5 sm:pt-0"
            >
              <span class="block text-xs font-medium text-muted-foreground">
                目标
              </span>
              <span class="block truncate text-sm font-medium">
                {{ selectedMapping?.target || "未选择" }}
              </span>
            </span>

            <span
              class="space-y-1 border-t border-border/60 pt-3 sm:border-l sm:border-t-0 sm:pl-5 sm:pt-0 sm:text-right"
            >
              <span class="block text-xs font-medium text-muted-foreground">
                规则数
              </span>
              <span class="block text-sm font-medium">
                {{ draftLocations.length }}
              </span>
            </span>
          </button>

          <div
            v-if="availableMappings.length === 0"
            class="rounded-md border px-5 py-8 text-center text-sm text-muted-foreground"
          >
            还没有可配置的 Host 映射。
          </div>

          <div v-else class="overflow-hidden rounded-md border">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>匹配</TableHead>
                  <TableHead>路径</TableHead>
                  <TableHead>动作</TableHead>
                  <TableHead>目标/响应</TableHead>
                  <TableHead>处理</TableHead>
                  <TableHead class="text-right">操作</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                <TableRow v-if="draftLocations.length === 0">
                  <TableCell
                    colspan="6"
                    class="py-8 text-center text-muted-foreground"
                  >
                    当前 Host 还没有路径规则。
                  </TableCell>
                </TableRow>
                <TableRow
                  v-for="{ location, index } in sortedDraftLocations"
                  :key="`${location.match}:${location.path}:${index}`"
                >
                  <TableCell class="text-sm font-medium">
                    {{ location.match === "exact" ? "精确" : "模糊匹配" }}
                  </TableCell>
                  <TableCell class="font-medium">{{ location.path }}</TableCell>
                  <TableCell>{{ formatAction(location) }}</TableCell>
                  <TableCell class="max-w-[22rem] truncate">
                    {{ formatTarget(location) }}
                  </TableCell>
                  <TableCell class="text-xs text-muted-foreground">
                    <template v-if="location.action === 'proxy'">
                      {{ location.strip_path ? "剥离路径" : "保留路径" }}
                      ·
                      {{ location.rewrite_html ? "改写 HTML" : "不改写 HTML" }}
                    </template>
                    <template v-else>
                      {{ Object.keys(location.response.headers || {}).length }}
                      个响应头
                    </template>
                  </TableCell>
                  <TableCell class="text-right">
                    <div class="flex justify-end gap-2">
                      <Button
                        variant="ghost"
                        size="icon"
                        @click="openEditDialog(index)"
                      >
                        <Pencil class="h-4 w-4" />
                        <span class="sr-only">编辑路径规则</span>
                      </Button>
                      <ConfirmDangerPopover
                        title="确认删除路径规则?"
                        :description="`您即将删除 ${location.path} 的路径响应规则，此操作不可逆转。`"
                        confirm-text="确认删除"
                        :on-confirm="() => removeLocation(index)"
                        content-class="w-64 text-left"
                      >
                        <template #trigger>
                          <Button
                            variant="ghost"
                            size="icon"
                            class="text-destructive hover:bg-destructive/10 hover:text-destructive"
                          >
                            <Trash2 class="h-4 w-4" />
                            <span class="sr-only">删除路径规则</span>
                          </Button>
                        </template>
                      </ConfirmDangerPopover>
                    </div>
                  </TableCell>
                </TableRow>
              </TableBody>
            </Table>
          </div>

          <div class="flex flex-col-reverse gap-2 sm:flex-row sm:justify-end">
            <Button
              variant="outline"
              :disabled="!isDirty || isSaving"
              @click="resetDraftFromSelected"
            >
              放弃更改
            </Button>
            <Button :disabled="!canSave" @click="saveLocations">
              保存路径响应
            </Button>
          </div>
        </template>
      </CardContent>
    </Card>

    <Dialog :open="isHostPickerOpen" @update:open="handleHostPickerOpenChange">
      <DialogContent class="sm:max-w-[760px]">
        <DialogHeader>
          <DialogTitle>选择 Host</DialogTitle>
          <DialogDescription class="leading-6">
            选择要维护路径响应规则的 Host。当前选中
            <span class="font-medium text-foreground">
              {{ selectedMapping?.host || "未选择" }}
            </span>
            <template v-if="selectedMapping">
              · {{ getMappingTitleForDisplay(selectedMapping) }}
            </template>
          </DialogDescription>
        </DialogHeader>

        <div class="grid max-h-[60vh] gap-2 overflow-y-auto pr-1">
          <button
            v-for="mapping in availableMappings"
            :key="mapping.host"
            type="button"
            class="w-full rounded-md border px-4 py-3 text-left transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring/40"
            :class="
              mapping.host === selectedHost
                ? 'border-border bg-muted/40'
                : 'border-border/60 bg-background hover:border-primary/30 hover:bg-muted/20'
            "
            @click="selectHostFromDialog(mapping.host)"
          >
            <span
              class="grid min-w-0 gap-3 sm:grid-cols-[minmax(0,1.2fr)_minmax(0,1fr)] sm:items-center"
            >
              <span class="min-w-0 space-y-1">
                <span class="flex min-w-0 flex-wrap items-center gap-2">
                  <span class="truncate text-sm font-semibold">
                    {{ mapping.host }}
                  </span>
                  <Badge
                    v-if="mapping.host === selectedHost"
                    variant="secondary"
                  >
                    当前
                  </Badge>
                  <span class="text-xs text-muted-foreground">
                    {{ mapping.locations?.length ?? 0 }}
                  </span>
                </span>
                <span class="block truncate text-sm text-muted-foreground">
                  {{ mapping.target || "未选择" }}
                </span>
              </span>

              <span class="min-w-0 space-y-1">
                <span class="flex items-center gap-2">
                  <span class="text-xs font-medium text-muted-foreground">
                    站点标题
                  </span>
                </span>
                <span class="block truncate text-sm font-medium">
                  {{ getMappingTitleForDisplay(mapping) }}
                </span>
              </span>
            </span>
          </button>
        </div>
      </DialogContent>
    </Dialog>

    <Dialog
      :open="isDialogOpen"
      @update:open="(open) => !open && closeDialog()"
    >
      <DialogContent class="max-h-[85vh] overflow-y-auto sm:max-w-[800px]">
        <DialogHeader>
          <DialogTitle>
            {{ editingIndex === null ? "添加路径规则" : "编辑路径规则" }}
          </DialogTitle>
          <DialogDescription>
            这条规则会继承当前 Host 的登录、白名单、Host 响应和凭证注入设置。
          </DialogDescription>
        </DialogHeader>

        <div class="grid gap-5">
          <div class="grid gap-3 sm:grid-cols-[minmax(0,1fr)_14rem]">
            <div class="space-y-2">
              <Label for="location-path">路径</Label>
              <Input
                id="location-path"
                v-model="form.path"
                placeholder="/api"
              />
            </div>
            <div class="space-y-2">
              <Label for="location-match">匹配</Label>
              <Select v-model="form.match">
                <SelectTrigger id="location-match" class="w-full">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="exact">精确</SelectItem>
                  <SelectItem value="prefix">模糊匹配</SelectItem>
                </SelectContent>
              </Select>
            </div>
          </div>

          <div class="space-y-2">
            <Label>动作</Label>
            <div
              class="grid grid-cols-2 rounded-lg bg-muted p-[3px] text-sm text-muted-foreground"
            >
              <button
                type="button"
                class="h-9 rounded-md font-medium transition-colors"
                :class="
                  form.action === 'proxy'
                    ? 'bg-background text-foreground shadow-sm'
                    : 'hover:text-foreground'
                "
                @click="setAction('proxy')"
              >
                反代
              </button>
              <button
                type="button"
                class="h-9 rounded-md font-medium transition-colors"
                :class="
                  form.action === 'response'
                    ? 'bg-background text-foreground shadow-sm'
                    : 'hover:text-foreground'
                "
                @click="setAction('response')"
              >
                固定响应
              </button>
            </div>
          </div>

          <template v-if="form.action === 'proxy'">
            <div class="space-y-2">
              <Label for="location-target">目标</Label>
              <ProxyTargetInputField
                v-model="form.target"
                input-id="location-target"
                protocol-id="location-target-protocol"
                placeholder="127.0.0.1:8080"
              />
            </div>
            <div class="grid gap-3 sm:grid-cols-2">
              <div
                class="flex items-center justify-between gap-4 rounded-md border px-4 py-3"
              >
                <Label for="location-strip-path">剥离匹配路径</Label>
                <Switch id="location-strip-path" v-model="form.strip_path" />
              </div>
              <div
                class="flex items-center justify-between gap-4 rounded-md border px-4 py-3"
              >
                <Label for="location-rewrite-html">改写 HTML 路径</Label>
                <Switch
                  id="location-rewrite-html"
                  v-model="form.rewrite_html"
                />
              </div>
            </div>
          </template>

          <template v-else>
            <div
              class="grid gap-3 rounded-md border border-border/60 bg-muted/10 p-4 sm:grid-cols-[8.5rem_minmax(0,1fr)]"
            >
              <div class="space-y-2">
                <Label for="response-status">状态码</Label>
                <Input
                  id="response-status"
                  v-model.number="form.response.status"
                  type="number"
                  min="100"
                  max="599"
                />
              </div>
              <ResponseContentTypeField
                v-model="form.response.content_type"
                input-id="response-content-type"
                select-id="response-content-type-preset"
              />
            </div>

            <div class="space-y-3 rounded-md border px-4 py-3">
              <div class="flex items-center justify-between gap-3">
                <Label>响应头</Label>
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  @click="addHeaderRow"
                >
                  添加响应头
                </Button>
              </div>
              <div
                v-if="form.headers.length === 0"
                class="text-sm text-muted-foreground"
              >
                未配置自定义响应头。
              </div>
              <div
                v-for="(header, index) in form.headers"
                :key="index"
                class="grid gap-2 sm:grid-cols-[minmax(0,1fr)_minmax(0,1fr)_2.5rem]"
              >
                <Input v-model="header.name" placeholder="X-Example" />
                <Input v-model="header.value" placeholder="value" />
                <ConfirmDangerPopover
                  title="确认删除响应头?"
                  :description="`您即将删除响应头 ${header.name.trim() || '未命名响应头'}。`"
                  confirm-text="确认删除"
                  :on-confirm="() => removeHeaderRow(index)"
                  content-class="w-64 text-left"
                >
                  <template #trigger>
                    <Button
                      type="button"
                      variant="ghost"
                      size="icon"
                      class="text-destructive hover:bg-destructive/10 hover:text-destructive"
                    >
                      <Trash2 class="h-4 w-4" />
                      <span class="sr-only">删除响应头</span>
                    </Button>
                  </template>
                </ConfirmDangerPopover>
              </div>
            </div>

            <ResponseBodyEditor
              v-model="form.response.body"
              :content-type="form.response.content_type"
            />
          </template>

          <p v-if="formError" class="text-sm text-destructive">
            {{ formError }}
          </p>
        </div>

        <DialogFooter>
          <Button variant="outline" @click="closeDialog">取消</Button>
          <Button
            :disabled="!!formError || isSaving"
            @click="saveDialogLocation"
          >
            保存规则
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </div>
</template>
