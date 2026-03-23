<template>
  <div class="space-y-6">
    <Card>
      <CardHeader>
        <CardTitle
          class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between"
        >
          <span>协议映射</span>
          <div class="flex flex-wrap items-center gap-2">
            <div class="flex">
              <Button class="rounded-r-none" @click="openCreateDialog">
                <Plus class="mr-2 h-4 w-4" />
                添加映射
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
                  <DropdownMenuItem @click="syncRoutes" :disabled="isSyncing">
                    <RefreshCw
                      class="mr-2 h-4 w-4"
                      :class="{ 'animate-spin': isSyncing }"
                    />
                    {{ isSyncing ? "同步中..." : "同步网关" }}
                  </DropdownMenuItem>
                </DropdownMenuContent>
              </DropdownMenu>
            </div>
          </div>
        </CardTitle>
        <CardDescription>
          每条规则都会把一个外部 TCP 或 UDP 端口转发到指定目标地址，适合
          SSH、MySQL、Redis、DNS 等业务
        </CardDescription>
      </CardHeader>

      <CardContent class="space-y-4">
        <Alert
          class="items-start rounded-xl border-zinc-200 bg-zinc-50/70 text-zinc-900 shadow-none"
        >
          <Info class="mt-0.5 h-4 w-4 shrink-0" />
          <div class="space-y-1">
            <AlertTitle>外部访问方式</AlertTitle>
            <AlertDescription class="text-sm leading-6 text-zinc-700">
              使用任意一个解析到本机的域名，加上这里配置的对外端口即可访问，
              例如 <code>demo.example.com:3306</code>如果映射开启了鉴权，
              需要先在网页端完成登录，否则连接会被直接拒绝。
            </AlertDescription>
          </div>
        </Alert>

        <div
          class="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between"
        >
          <SearchInput
            v-model="searchQuery"
            placeholder="搜索协议、对外端口、目标地址..."
            class="max-w-xs"
          />
        </div>

        <div class="overflow-hidden rounded-md border">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>协议</TableHead>
                <TableHead>对外端口</TableHead>
                <TableHead>目标地址</TableHead>
                <TableHead>鉴权状态</TableHead>
                <TableHead class="text-right">操作</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              <TableRow v-if="filteredMappings.length === 0">
                <TableCell
                  colspan="5"
                  class="py-8 text-center text-muted-foreground"
                >
                  还没有配置任何 协议映射。
                </TableCell>
              </TableRow>
              <TableRow
                v-for="mapping in filteredMappings"
                :key="getMappingKey(mapping)"
                class="group"
              >
                <TableCell>
                  <Badge
                    variant="outline"
                    class="font-mono uppercase tracking-[0.16em]"
                  >
                    {{ mapping.protocol }}
                  </Badge>
                </TableCell>
                <TableCell class="font-medium">
                  <div
                    class="inline-flex items-center gap-2 rounded-full border px-3 py-1 text-sm"
                  >
                    <span>{{ mapping.listen_port }}</span>
                  </div>
                </TableCell>
                <TableCell class="font-mono text-sm">{{
                  mapping.target
                }}</TableCell>
                <TableCell class="min-w-[15rem]">
                  <div
                    class="flex flex-wrap items-center gap-2 text-xs text-muted-foreground"
                  >
                    <Badge v-if="mapping.use_auth" variant="default">
                      需要鉴权
                    </Badge>
                    <Badge v-else variant="secondary">公开访问</Badge>
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
                    <ConfirmDangerPopover
                      :title="`确认删除 ${formatProtocolLabel(mapping.protocol)} 协议映射？`"
                      :description="`将停止 ${formatMappingLabel(mapping)}，并移除到 ${mapping.target} 的转发规则。`"
                      :loading="removingMappingKey === getMappingKey(mapping)"
                      :disabled="removingMappingKey === getMappingKey(mapping)"
                      :on-confirm="() => removeMapping(mapping)"
                      content-class="w-72 text-left"
                    >
                      <template #trigger>
                        <Button
                          variant="ghost"
                          size="sm"
                          class="text-destructive hover:bg-destructive/10 hover:text-destructive"
                          :disabled="
                            removingMappingKey === getMappingKey(mapping)
                          "
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
      </CardContent>
    </Card>

    <Dialog :open="isDialogOpen" @update:open="handleDialogOpenChange">
      <DialogContent class="sm:max-w-[520px]">
        <DialogHeader>
          <DialogTitle>
            {{ isEditing ? "编辑 协议映射" : "添加 协议映射" }}
          </DialogTitle>
          <DialogDescription>
            保存后会更新管理配置，并同步到网关刷新对应的 TCP 或 UDP 对外入口。
          </DialogDescription>
        </DialogHeader>

        <div class="grid gap-4 py-4">
          <div class="space-y-2">
            <Label for="stream-protocol">传输协议</Label>
            <Select v-model="form.protocol">
              <SelectTrigger id="stream-protocol">
                <SelectValue placeholder="选择协议" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="tcp">TCP</SelectItem>
                <SelectItem value="udp">UDP</SelectItem>
              </SelectContent>
            </Select>
            <p class="text-xs text-muted-foreground">
              同一个对外端口可以分别配置一条 TCP 和一条 UDP 规则。
            </p>
          </div>

          <div class="space-y-2">
            <Label for="stream-listen-port">对外端口</Label>
            <Input
              id="stream-listen-port"
              v-model="form.listen_port"
              inputmode="numeric"
              placeholder="例如 3306"
              @blur="markPortBlurred"
            />
            <p class="text-xs text-muted-foreground">
              外部客户端将直接访问这个端口。
            </p>
          </div>

          <div class="space-y-2">
            <Label for="stream-target">目标地址</Label>
            <Input
              id="stream-target"
              v-model="form.target"
              placeholder="例如 127.0.0.1:3306"
              @blur="markTargetBlurred"
            />
            <p class="text-xs text-muted-foreground">
              只支持 <code>host:port</code>，不需要填写
              <code>http://</code> 或其他协议前缀。
            </p>
          </div>

          <div
            class="flex items-center justify-between rounded-lg border px-4 py-3"
          >
            <div class="space-y-1">
              <Label for="stream-auth">要求鉴权</Label>
              <p class="text-xs text-muted-foreground">
                开启后会先按来源 IP 调用鉴权服务，未通过则直接断开连接。
              </p>
            </div>
            <Switch id="stream-auth" v-model="form.use_auth" />
          </div>

          <div
            v-if="showValidation && validationMessage"
            class="rounded-lg border border-destructive/30 bg-destructive/5 px-4 py-3 text-sm text-destructive"
          >
            {{ validationMessage }}
          </div>
        </div>

        <DialogFooter>
          <Button variant="outline" @click="closeDialog">取消</Button>
          <Button :disabled="isSaving" @click="saveMapping">
            <span
              v-if="isSaving"
              class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
            ></span>
            保存映射
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </div>
</template>

<script setup lang="ts">
import { computed, reactive, ref } from "vue";
import { ChevronDown, Info, Plus, RefreshCw } from "lucide-vue-next";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
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
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import SearchInput from "@admin-shared/components/SearchInput.vue";
import { extractErrorMessage } from "@admin-shared/composables/useAsyncAction";
import { toast } from "@admin-shared/utils/toast";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { ConfigAPI } from "../lib/api";
import { useConfigStore } from "../store/config";
import type { StreamMapping, StreamMappingProtocol } from "../types";

const configStore = useConfigStore();
const DEFAULT_STREAM_PROTOCOL: StreamMappingProtocol = "tcp";

const searchQuery = ref("");
const isDialogOpen = ref(false);
const isSaving = ref(false);
const isSyncing = ref(false);
const editingMappingKey = ref<string | null>(null);
const removingMappingKey = ref<string | null>(null);
const hasAttemptedSubmit = ref(false);
const hasPortBlurred = ref(false);
const hasTargetBlurred = ref(false);

const form = reactive<{
  protocol: StreamMappingProtocol;
  listen_port: string;
  target: string;
  use_auth: boolean;
}>({
  protocol: DEFAULT_STREAM_PROTOCOL,
  listen_port: "",
  target: "",
  use_auth: true,
});

const allMappings = computed(() =>
  [...(configStore.config?.stream_mappings ?? [])]
    .map(normalizeStreamMapping)
    .sort(compareStreamMappings),
);

const filteredMappings = computed(() => {
  const query = searchQuery.value.trim().toLowerCase();
  if (!query) return allMappings.value;

  return allMappings.value.filter((mapping) => {
    const authStatus = mapping.use_auth ? "需要鉴权" : "公开访问";
    return (
      mapping.protocol.includes(query) ||
      formatProtocolLabel(mapping.protocol).toLowerCase().includes(query) ||
      String(mapping.listen_port).includes(query) ||
      mapping.target.toLowerCase().includes(query) ||
      authStatus.includes(query)
    );
  });
});

const isEditing = computed(() => editingMappingKey.value !== null);
const parsedListenPort = computed(() => {
  const value = Number.parseInt(form.listen_port.trim(), 10);
  if (!Number.isFinite(value)) return null;
  return value;
});

const duplicateMapping = computed(() => {
  const port = parsedListenPort.value;
  if (port === null) return false;
  const nextKey = createMappingKey(form.protocol, port);

  return allMappings.value.some(
    (mapping) =>
      getMappingKey(mapping) === nextKey &&
      getMappingKey(mapping) !== editingMappingKey.value,
  );
});

const isTargetValid = computed(() => isValidStreamTarget(form.target));

function getPortValidationMessage(showRequired: boolean): string {
  const rawPort = form.listen_port.trim();
  if (!rawPort) {
    return showRequired ? "对外端口不能为空。" : "";
  }

  const port = parsedListenPort.value;
  if (port === null) return "对外端口必须是 1 到 65535 的整数。";
  if (port <= 0 || port > 65535) {
    return "对外端口必须位于 1 到 65535 之间。";
  }
  if (duplicateMapping.value) {
    return `${formatProtocolLabel(form.protocol)} 对外端口 ${port} 已存在，请保持协议 + 端口唯一。`;
  }
  return "";
}

function getTargetValidationMessage(showRequired: boolean): string {
  const rawTarget = form.target.trim();
  if (!rawTarget) {
    return showRequired ? "目标地址不能为空。" : "";
  }
  if (!isTargetValid.value) {
    return "目标地址必须使用 host:port 格式，例如 127.0.0.1:3306。";
  }
  return "";
}

const validationMessage = computed(() => {
  if (hasAttemptedSubmit.value) {
    const submitMessage = submitValidationMessage.value;
    if (submitMessage) return submitMessage;
    return "";
  }

  const shouldValidatePort =
    hasPortBlurred.value && form.listen_port.trim() !== "";
  if (shouldValidatePort) {
    const portMessage = getPortValidationMessage(false);
    if (portMessage) return portMessage;
  }

  const shouldValidateTarget =
    hasTargetBlurred.value && form.target.trim() !== "";
  if (shouldValidateTarget) {
    const targetMessage = getTargetValidationMessage(false);
    if (targetMessage) return targetMessage;
  }

  return "";
});

const showValidation = computed(() => Boolean(validationMessage.value));
const submitValidationMessage = computed(() => {
  const portMessage = getPortValidationMessage(true);
  if (portMessage) return portMessage;
  return getTargetValidationMessage(true);
});

function isValidStreamTarget(target: string): boolean {
  const trimmed = target.trim();
  if (!trimmed) return false;

  try {
    const parsed = new URL(`tcp://${trimmed}`);
    const port = Number.parseInt(parsed.port, 10);
    if (!parsed.hostname) return false;
    if (!Number.isFinite(port) || port <= 0 || port > 65535) return false;
    if (parsed.pathname && parsed.pathname !== "/") return false;
    if (parsed.search || parsed.hash) return false;
    return true;
  } catch {
    return false;
  }
}

function normalizeProtocol(
  protocol?: StreamMappingProtocol | string | null,
): StreamMappingProtocol {
  return protocol === "udp" ? "udp" : DEFAULT_STREAM_PROTOCOL;
}

function normalizeStreamMapping(mapping: StreamMapping): StreamMapping {
  return {
    ...mapping,
    protocol: normalizeProtocol(mapping.protocol),
  };
}

function createMappingKey(
  protocol: StreamMappingProtocol,
  listenPort: number,
): string {
  return `${protocol}:${listenPort}`;
}

function getMappingKey(mapping: StreamMapping): string {
  return createMappingKey(
    normalizeProtocol(mapping.protocol),
    mapping.listen_port,
  );
}

function compareStreamMappings(a: StreamMapping, b: StreamMapping): number {
  if (a.listen_port !== b.listen_port) {
    return a.listen_port - b.listen_port;
  }
  return a.protocol.localeCompare(b.protocol);
}

function formatProtocolLabel(protocol: StreamMappingProtocol): string {
  return protocol.toUpperCase();
}

function formatMappingLabel(mapping: StreamMapping): string {
  return `${formatProtocolLabel(normalizeProtocol(mapping.protocol))}/${mapping.listen_port}`;
}

function resetForm() {
  form.protocol = DEFAULT_STREAM_PROTOCOL;
  form.listen_port = "";
  form.target = "";
  form.use_auth = true;
  editingMappingKey.value = null;
  hasAttemptedSubmit.value = false;
  hasPortBlurred.value = false;
  hasTargetBlurred.value = false;
}

function handleDialogOpenChange(nextOpen: boolean) {
  if (!nextOpen) {
    closeDialog();
  }
}

function openCreateDialog() {
  resetForm();
  isDialogOpen.value = true;
}

function openEditDialog(mapping: StreamMapping) {
  const normalized = normalizeStreamMapping(mapping);
  form.protocol = normalized.protocol;
  form.listen_port = String(mapping.listen_port);
  form.target = mapping.target;
  form.use_auth = mapping.use_auth;
  editingMappingKey.value = getMappingKey(normalized);
  isDialogOpen.value = true;
}

function closeDialog() {
  isDialogOpen.value = false;
  resetForm();
}

function markPortBlurred() {
  hasPortBlurred.value = true;
}

function markTargetBlurred() {
  hasTargetBlurred.value = true;
}

async function saveMapping() {
  hasAttemptedSubmit.value = true;
  if (submitValidationMessage.value || parsedListenPort.value === null) return;

  const nextMapping: StreamMapping = {
    protocol: form.protocol,
    listen_port: parsedListenPort.value,
    target: form.target.trim(),
    use_auth: form.use_auth,
  };

  isSaving.value = true;
  try {
    const next = [...allMappings.value];
    const existingIndex = next.findIndex(
      (mapping) => getMappingKey(mapping) === editingMappingKey.value,
    );

    if (existingIndex >= 0) {
      next.splice(existingIndex, 1, nextMapping);
    } else {
      next.push(nextMapping);
    }

    await configStore.saveStreamMappings(next);
    toast.success(isEditing.value ? "已更新 协议映射" : "已添加 协议映射");
    closeDialog();
  } catch (error: any) {
    toast.error("保存失败", {
      description: extractErrorMessage(error, "请稍后重试"),
    });
  } finally {
    isSaving.value = false;
  }
}

async function removeMapping(mapping: StreamMapping) {
  removingMappingKey.value = getMappingKey(mapping);
  try {
    await configStore.saveStreamMappings(
      allMappings.value.filter(
        (item) => getMappingKey(item) !== getMappingKey(mapping),
      ),
    );
    toast.success(`已移除 ${formatMappingLabel(mapping)} 协议映射`);
  } catch (error: any) {
    toast.error("删除失败", {
      description: extractErrorMessage(error, "请稍后重试"),
    });
  } finally {
    removingMappingKey.value = null;
  }
}

async function syncRoutes() {
  isSyncing.value = true;
  try {
    const result = await ConfigAPI.syncRoutes();
    if (result.success) {
      toast.success("已同步到网关", {
        description: `路径路由 ${result.data?.synced_rules ?? 0} 条，Host 路由 ${result.data?.synced_host_rules ?? 0} 条，协议映射 ${result.data?.synced_stream_rules ?? 0} 条。`,
      });
      return;
    }

    toast.error("同步失败", {
      description: result.message || "网关未返回成功结果",
    });
  } catch (error: any) {
    toast.error("同步失败", {
      description: extractErrorMessage(error, "请稍后重试"),
    });
  } finally {
    isSyncing.value = false;
  }
}
</script>
