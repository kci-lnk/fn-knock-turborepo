<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useRouter } from "vue-router";
import { Info, Eye, Settings, Trash2 } from "lucide-vue-next";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import RefreshButton from "@/components/RefreshButton.vue";
import SearchInput from "@admin-shared/components/SearchInput.vue";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { toast } from "@admin-shared/utils/toast";
import { GatewayLogsAPI } from "../lib/api";
import type { GatewayLogEntry } from "../types";
import { useConfigStore } from "../store/config";
import PagedTableFooter from "@admin-shared/components/list/PagedTableFooter.vue";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import DetailDialog from "@admin-shared/components/common/DetailDialog.vue";
import DetailFieldsGrid from "@admin-shared/components/common/DetailFieldsGrid.vue";
import TableSkeletonBlock from "@admin-shared/components/list/TableSkeletonBlock.vue";
import HumanFriendlyTime from "@admin-shared/components/common/HumanFriendlyTime.vue";
import { extractErrorMessage, useAsyncAction } from "@admin-shared/composables/useAsyncAction";
import { useDelayedLoading } from "@admin-shared/composables/useDelayedLoading";
import { buildDetailFields } from "@admin-shared/utils/buildDetailFields";
import { formatDateTimeSafe } from "@admin-shared/utils/formatDateTimeSafe";

const router = useRouter();
const configStore = useConfigStore();

const getTodayString = () => {
  const now = new Date();
  const year = now.getFullYear();
  const month = String(now.getMonth() + 1).padStart(2, "0");
  const day = String(now.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
};

const entries = ref<GatewayLogEntry[]>([]);
const logsDir = ref("");
const availableDates = ref<string[]>([]);
const selectedDate = ref(getTodayString());
const total = ref(0);
const currentPage = ref(1);
const limit = ref("20");
const searchQuery = ref("");
const loading = ref(false);
const isDetailsOpen = ref(false);
const activeEntry = ref<GatewayLogEntry | null>(null);

const parsedLimit = computed(() => Number.parseInt(limit.value, 10) || 20);
const showTableSkeleton = useDelayedLoading(
  () => loading.value && entries.value.length === 0,
);
const isLoggingEnabled = computed(
  () => configStore.config?.gateway_logging?.enabled ?? false,
);

const { isPending: isDeleting, run: runDelete } = useAsyncAction({
  onError: (error) => {
    toast.error("删除失败", {
      description: extractErrorMessage(error, "删除请求日志失败"),
    });
  },
});

const applyDates = (dates: string[], preferred?: string) => {
  const fallbackToday = getTodayString();
  const nextDates = dates.length > 0 ? dates : [fallbackToday];
  availableDates.value = nextDates;

  if (preferred && nextDates.includes(preferred)) {
    selectedDate.value = preferred;
    return;
  }
  if (nextDates.includes(selectedDate.value)) {
    return;
  }
  if (nextDates.includes(fallbackToday)) {
    selectedDate.value = fallbackToday;
    return;
  }
  selectedDate.value = nextDates[0] || fallbackToday;
};

const fetchDates = async (preferred?: string) => {
  const data = await GatewayLogsAPI.getDates();
  logsDir.value = data.logs_dir || "";
  applyDates(data.dates || [], preferred || data.today || selectedDate.value);
};

const fetchEntries = async () => {
  loading.value = true;
  try {
    const data = await GatewayLogsAPI.getEntries(
      selectedDate.value,
      currentPage.value,
      limit.value,
      searchQuery.value,
    );
    logsDir.value = data.logs_dir || "";
    total.value = data.total || 0;
    entries.value = data.items || [];
    applyDates(data.available_dates || [], data.date || selectedDate.value);

    const maxPage = Math.max(1, Math.ceil(total.value / parsedLimit.value));
    if (currentPage.value > maxPage) {
      currentPage.value = maxPage;
      return await fetchEntries();
    }
  } catch (error) {
    entries.value = [];
    total.value = 0;
    toast.error("加载失败", {
      description: extractErrorMessage(error, "请求日志加载失败"),
    });
  } finally {
    loading.value = false;
  }
};

const refreshAll = async () => {
  await fetchDates(selectedDate.value);
  await fetchEntries();
};

const handleDateChange = async (value: unknown) => {
  if (!value) return;
  selectedDate.value = String(value);
  currentPage.value = 1;
  await fetchEntries();
};

const handleSearch = async () => {
  currentPage.value = 1;
  await fetchEntries();
};

const handlePageChange = async (page: number) => {
  currentPage.value = page;
  await fetchEntries();
};

const handleLimitChange = async (value: string) => {
  limit.value = value;
  currentPage.value = 1;
  await fetchEntries();
};

const viewDetails = (entry: GatewayLogEntry) => {
  activeEntry.value = entry;
  isDetailsOpen.value = true;
};

const deleteSelectedDate = async () => {
  await runDelete(
    () => GatewayLogsAPI.deleteDate(selectedDate.value),
    {
      onSuccess: async (data) => {
        toast.success(
          data.deleted
            ? `${selectedDate.value} 日志已删除`
            : `${selectedDate.value} 没有可删除的日志`,
        );
        searchQuery.value = "";
        currentPage.value = 1;
        const nextPreferred =
          data.available_dates.find((item) => item !== selectedDate.value) ||
          getTodayString();
        await fetchDates(nextPreferred);
        await fetchEntries();
      },
    },
  );
};

const goToSettings = () => {
  router.push({ path: "/system", query: { tab: "gateway-logging" } });
};

const statusTextClass = (status: number) => {
  if (status >= 500) return "text-red-600";
  if (status >= 400) return "text-amber-600";
  return "text-foreground";
};

const statusDotClass = (status: number) => {
  if (status >= 500) return "bg-red-500";
  if (status >= 400) return "bg-amber-500";
  return "bg-muted-foreground/35";
};

const routeTypeLabel = (value?: string) => {
  switch (value) {
    case "path_rule":
      return "路径规则";
    case "host_rule":
      return "Host 规则";
    case "auth_proxy":
      return "鉴权代理";
    case "select":
      return "选择页";
    case "preflight":
      return "预检";
    case "slash_redirect":
      return "补斜杠";
    case "favicon":
      return "图标";
    case "not_found":
      return "未命中";
    default:
      return value || "-";
  }
};

const authDecisionLabel = (value?: string) => {
  switch (value) {
    case "passed":
      return "已通过";
    case "redirected":
      return "已跳转";
    case "denied":
      return "已拒绝";
    case "root_mode_redirect":
      return "根路径跳转";
    case "not_required":
      return "无需鉴权";
    case "proxy":
      return "代理转发";
    case "error":
      return "鉴权异常";
    default:
      return value || "-";
  }
};

const formatDuration = (value?: number) => {
  if (!Number.isFinite(value)) return "-";
  return `${value} ms`;
};

const formatBoolean = (value?: boolean) => {
  return value ? "是" : "否";
};

const formatDate = (value?: string) => formatDateTimeSafe(value);

const detailFields = [
  { key: "time", label: "时间" },
  { key: "method", label: "方法" },
  { key: "scheme", label: "协议" },
  { key: "host", label: "Host" },
  { key: "path", label: "路径" },
  { key: "query", label: "Query" },
  { key: "request_uri", label: "请求地址" },
  { key: "protocol", label: "HTTP 协议" },
  { key: "status", label: "状态码" },
  { key: "duration_ms", label: "耗时" },
  { key: "remote_ip", label: "客户端 IP" },
  { key: "remote_addr", label: "远端地址" },
  { key: "user_agent", label: "User-Agent" },
  { key: "referer", label: "Referer" },
  { key: "logged_in", label: "已登录" },
  { key: "auth_required", label: "需要鉴权" },
  { key: "auth_decision", label: "鉴权结果" },
  { key: "access_mode", label: "访问模式" },
  { key: "route_type", label: "路由类型" },
  { key: "route_key", label: "路由键" },
  { key: "upstream", label: "上游目标" },
  { key: "matched", label: "命中规则" },
  { key: "bytes_in", label: "请求字节" },
  { key: "bytes_out", label: "响应字节" },
  { key: "tls", label: "TLS" },
  { key: "websocket", label: "WebSocket" },
  { key: "x_forwarded_for", label: "X-Forwarded-For" },
  { key: "x_real_ip", label: "X-Real-IP" },
] as const;

const detailItems = computed(() =>
  buildDetailFields(activeEntry.value, detailFields, {
    format: (key, value) => {
      if (key === "time") return formatDate(value);
      if (key === "duration_ms") return formatDuration(value);
      if (
        key === "logged_in" ||
        key === "auth_required" ||
        key === "matched" ||
        key === "tls" ||
        key === "websocket"
      ) {
        return formatBoolean(Boolean(value));
      }
      if (key === "route_type") return routeTypeLabel(String(value || ""));
      if (key === "auth_decision") return authDecisionLabel(String(value || ""));
      if (value === undefined || value === null || value === "") return "-";
      return value;
    },
  }),
);

onMounted(async () => {
  await fetchDates(selectedDate.value);
  await fetchEntries();
});
</script>

<template>
  <div class="h-full flex flex-col gap-4">
    <Alert
      v-if="!isLoggingEnabled"
      class="items-start rounded-xl border-zinc-200 bg-zinc-50/70 text-zinc-900 shadow-none"
    >
      <Info class="mt-0.5 h-4 w-4 shrink-0" />
      <div class="flex w-full flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <div>
          <AlertTitle>请求日志当前未启用</AlertTitle>
          <AlertDescription class="text-sm leading-6 text-zinc-700">
            你仍然可以查看已保留的历史日志；如果希望继续记录新的请求，请到系统设置中的日志页开启。
          </AlertDescription>
        </div>
        <Button variant="outline" class="shrink-0" @click="goToSettings">
          <Settings class="mr-2 h-4 w-4" />
          打开设置
        </Button>
      </div>
    </Alert>

    <div class="flex flex-col gap-3 rounded-xl border bg-background p-4">
      <div class="flex flex-col gap-3 lg:flex-row lg:items-center">
        <div class="flex flex-col gap-2 sm:flex-row sm:items-center">
          <Label class="text-sm font-medium">日期</Label>
          <Select :model-value="selectedDate" @update:model-value="handleDateChange">
            <SelectTrigger class="w-full sm:w-[220px]">
              <SelectValue placeholder="选择日期" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem
                v-for="date in availableDates"
                :key="date"
                :value="date"
              >
                {{ date }}
              </SelectItem>
            </SelectContent>
          </Select>
        </div>

        <SearchInput
          v-model="searchQuery"
          placeholder="搜索 IP、Host、路径、状态码、UA..."
          class="w-full lg:max-w-[340px]"
          @search="handleSearch"
        />

        <div class="flex-1"></div>

        <div class="flex items-center gap-2">
          <RefreshButton :loading="loading" :disabled="loading" @click="refreshAll" />
          <ConfirmDangerPopover
            :title="`确认删除 ${selectedDate} 的请求日志？`"
            description="删除后当天日志文件将不可恢复。"
            :loading="isDeleting"
            :disabled="isDeleting"
            :on-confirm="deleteSelectedDate"
          >
            <template #trigger>
              <Button variant="destructive" :disabled="isDeleting">
                <Trash2 class="mr-2 h-4 w-4" />
                删除当天
              </Button>
            </template>
          </ConfirmDangerPopover>
        </div>
      </div>

      <div class="text-xs text-muted-foreground break-all">
        日志目录：{{ logsDir || "-" }}
      </div>
    </div>

    <div class="border rounded-md overflow-hidden bg-background flex-1 flex flex-col">
      <div class="flex-1 w-full overflow-hidden">
        <div class="h-full overflow-auto">
          <Table v-if="!(loading && entries.length === 0)">
            <TableHeader class="sticky top-0 z-10 bg-background/95 backdrop-blur">
              <TableRow>
                <TableHead class="h-11 text-[11px] font-medium text-muted-foreground">时间</TableHead>
                <TableHead class="h-11 text-[11px] font-medium text-muted-foreground">请求</TableHead>
                <TableHead class="h-11 text-[11px] font-medium text-muted-foreground">状态</TableHead>
                <TableHead class="h-11 text-[11px] font-medium text-muted-foreground">登录</TableHead>
                <TableHead class="h-11 text-[11px] font-medium text-muted-foreground">来源 IP</TableHead>
                <TableHead class="h-11 text-[11px] font-medium text-muted-foreground">路由</TableHead>
                <TableHead class="h-11 text-[11px] font-medium text-muted-foreground">耗时</TableHead>
                <TableHead class="h-11 pr-6 text-right text-[11px] font-medium text-muted-foreground">操作</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              <TableRow v-if="loading">
                <TableCell colspan="8" class="text-center py-10 text-muted-foreground">
                  加载中...
                </TableCell>
              </TableRow>
              <TableRow v-else-if="entries.length === 0">
                <TableCell colspan="8" class="text-center py-10 text-muted-foreground">
                  暂无请求日志
                </TableCell>
              </TableRow>
              <TableRow
                v-else
                v-for="entry in entries"
                :key="`${entry.time}-${entry.request_uri}-${entry.remote_ip}`"
                class="align-top"
              >
                <TableCell class="whitespace-nowrap py-3 text-sm">
                  <HumanFriendlyTime :value="entry.time" />
                </TableCell>
                <TableCell class="min-w-[320px] py-3">
                  <div class="space-y-1">
                    <div class="flex items-center gap-2 text-sm text-foreground">
                      <span class="font-mono text-[11px] tracking-[0.12em] text-muted-foreground">
                        {{ entry.method || "-" }}
                      </span>
                      <span class="truncate">{{ entry.host || "-" }}</span>
                    </div>
                    <div class="break-all font-mono text-[12px] leading-5 text-muted-foreground">
                      {{ entry.request_uri || entry.path || "-" }}
                    </div>
                    <div
                      v-if="entry.upstream"
                      class="break-all text-[11px] text-muted-foreground/80"
                    >
                      {{ entry.upstream }}
                    </div>
                  </div>
                </TableCell>
                <TableCell class="py-3">
                  <div
                    class="flex items-center gap-2 font-mono text-sm"
                    :class="statusTextClass(entry.status)"
                  >
                    <span class="h-1.5 w-1.5 rounded-full" :class="statusDotClass(entry.status)"></span>
                    <span>{{ entry.status }}</span>
                  </div>
                </TableCell>
                <TableCell class="py-3">
                  <div class="text-sm text-foreground">
                    {{ entry.logged_in ? "已登录" : "未登录" }}
                  </div>
                  <div class="mt-1 text-[11px] text-muted-foreground">
                    {{ authDecisionLabel(entry.auth_decision) }}
                  </div>
                </TableCell>
                <TableCell class="min-w-[140px] py-3">
                  <div class="font-mono text-sm text-foreground">{{ entry.remote_ip || "-" }}</div>
                  <div class="mt-1 text-[11px] text-muted-foreground">
                    {{ entry.x_forwarded_for || entry.x_real_ip || "-" }}
                  </div>
                </TableCell>
                <TableCell class="min-w-[140px] py-3">
                  <div class="text-sm text-foreground">{{ routeTypeLabel(entry.route_type) }}</div>
                  <div class="mt-1 break-all text-[11px] text-muted-foreground">
                    {{ entry.route_key || "-" }}
                  </div>
                </TableCell>
                <TableCell class="whitespace-nowrap py-3 font-mono text-sm text-muted-foreground">
                  {{ formatDuration(entry.duration_ms) }}
                </TableCell>
                <TableCell class="py-3 text-right pr-6">
                  <Button
                    variant="ghost"
                    size="icon"
                    class="text-muted-foreground hover:text-foreground"
                    @click="viewDetails(entry)"
                  >
                    <Eye class="h-4 w-4" />
                  </Button>
                </TableCell>
              </TableRow>
            </TableBody>
          </Table>
          <TableSkeletonBlock
            v-else-if="showTableSkeleton"
            :header-widths="['w-24', 'w-40', 'w-16', 'w-16', 'w-20', 'w-20', 'w-16', 'w-10']"
            :row-widths="['w-28', 'w-56', 'w-12', 'w-20', 'w-24', 'w-24', 'w-16', 'w-10']"
          />
          <div v-else class="h-[380px]" aria-hidden="true"></div>
        </div>
      </div>

      <PagedTableFooter
        :total="total"
        :page="currentPage"
        :limit="limit"
        :items-per-page="parsedLimit"
        total-text="条请求"
        @update:page="handlePageChange"
        @update:limit="handleLimitChange"
      />
    </div>

    <DetailDialog
      v-model:open="isDetailsOpen"
      title="请求日志详情"
      description="查看此条网关请求日志的完整字段。"
      max-width-class="sm:max-w-[640px]"
      close-variant="default"
    >
      <div v-if="activeEntry" class="max-h-[65vh] overflow-y-auto py-4">
        <DetailFieldsGrid :items="detailItems" />
      </div>
    </DetailDialog>
  </div>
</template>
