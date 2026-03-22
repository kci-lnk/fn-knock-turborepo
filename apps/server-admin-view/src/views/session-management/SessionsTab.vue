<script setup lang="ts">
import { ref, computed, watch } from "vue";
import { useRouter } from "vue-router";
import InlineCommentEditor from "@admin-shared/components/InlineCommentEditor.vue";
import { Button } from "@/components/ui/button";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { toast } from "@admin-shared/utils/toast";
import type { SessionRecord } from "../../types";
import { SessionAPI } from "../../lib/api";
import { Eye, GitBranch, Trash2 } from "lucide-vue-next";
import RefreshButton from "@/components/RefreshButton.vue";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { useConfigStore } from "../../store/config";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import DetailDialog from "@admin-shared/components/common/DetailDialog.vue";
import DetailFieldsGrid from "@admin-shared/components/common/DetailFieldsGrid.vue";
import HumanFriendlyTime from "@admin-shared/components/common/HumanFriendlyTime.vue";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { buildDetailFields } from "@admin-shared/utils/buildDetailFields";
import { formatDateTimeSafe } from "@admin-shared/utils/formatDateTimeSafe";

const router = useRouter();
const sessions = ref<SessionRecord[]>([]);
const showDetail = ref(false);
const detailSession = ref<SessionRecord | null>(null);

const { isPending: isLoading, run: runLoadSessions } = useAsyncAction({
  onError: (error) => {
    toast.error("加载失败", {
      description: extractErrorMessage(error, "加载失败"),
    });
  },
});

const { isPending: isKicking, run: runKickSession } = useAsyncAction({
  onError: (error) => {
    toast.error("踢出失败", {
      description: extractErrorMessage(error, "操作失败"),
    });
  },
});

const { run: runUpdateComment } = useAsyncAction({
  rethrow: true,
});

const configStore = useConfigStore();

const detailFieldDefinitions = [
  { key: "id", label: "会话 ID" },
  { key: "method", label: "登录方式" },
  { key: "credentialName", label: "凭证名称" },
  { key: "comment", label: "备注" },
  { key: "ip", label: "当前 IP" },
  { key: "ipLocation", label: "归属信息" },
  { key: "userAgent", label: "User-Agent" },
  { key: "loginTime", label: "登录时间" },
  { key: "expiresAt", label: "过期时间" },
] as const;

const hasSessions = computed(() => sessions.value.length > 0);

const detailItems = computed(() => {
  return buildDetailFields(detailSession.value, detailFieldDefinitions, {
    format: (key, value) => {
      if (key === "loginTime" || key === "expiresAt") {
        return formatDateTimeSafe(
          value as string | number | Date | null | undefined,
        );
      }
      return value;
    },
  });
});

const middleEllipsis = (text: string, max = 16) => {
  if (!text) return "";
  if (text.length <= max) return text;
  const head = Math.ceil((max - 1) / 2);
  const tail = Math.floor((max - 1) / 2);
  return `${text.slice(0, head)}……${text.slice(text.length - tail)}`;
};

async function fetchSessions() {
  await runLoadSessions(async () => {
    const nextSessions = await SessionAPI.list();
    sessions.value = Array.isArray(nextSessions) ? nextSessions : [];
  });
}

function openDetail(session: SessionRecord) {
  detailSession.value = session;
  showDetail.value = true;
}

function openMobility(session: SessionRecord) {
  router.push(`/sessions/mobility/${encodeURIComponent(session.id)}`);
}

async function kickSession(sessionId: string) {
  await runKickSession(() => SessionAPI.kick(sessionId), {
    onSuccess: async () => {
      sessions.value = sessions.value.filter((session) => session.id !== sessionId);
      if (detailSession.value?.id === sessionId) {
        detailSession.value = null;
        showDetail.value = false;
      }
      toast.success("已踢出会话");
      await fetchSessions();
    },
  });
}

async function updateComment(sessionId: string, comment: string) {
  const target = sessions.value.find((session) => session.id === sessionId);
  if (target && (target.comment ?? "") === comment) {
    return;
  }

  await runUpdateComment(() => SessionAPI.updateComment(sessionId, comment), {
    onSuccess: (updated) => {
      if (target) {
        Object.assign(target, updated);
      }
      if (detailSession.value?.id === sessionId) {
        detailSession.value = {
          ...detailSession.value,
          ...updated,
        };
      }
      toast.success("备注已更新");
    },
    onError: (error) => {
      throw new Error(extractErrorMessage(error, "更新备注失败"));
    },
  });
}

watch(
  () => configStore.config?.run_type,
  (runType) => {
    if (runType === 1 || runType === 3) {
      void fetchSessions();
    }
  },
  { immediate: true },
);
</script>

<template>
  <div class="space-y-3">
    <div class="flex items-center justify-between">
      <div class="text-sm text-muted-foreground">
        当前活跃会话 {{ sessions.length }} 个
      </div>
      <RefreshButton
        :loading="isLoading"
        :disabled="isLoading"
        @click="fetchSessions"
      />
    </div>

    <div class="overflow-hidden rounded-md border">
      <TooltipProvider>
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead class="w-[150px]">会话 ID</TableHead>
              <TableHead>凭证</TableHead>
              <TableHead>备注</TableHead>
              <TableHead>当前 IP</TableHead>
              <TableHead>登录时间</TableHead>
              <TableHead>过期时间</TableHead>
              <TableHead class="w-[210px] text-right">操作</TableHead>
            </TableRow>
          </TableHeader>

          <TableBody v-if="hasSessions">
            <TableRow v-for="session in sessions" :key="session.id">
              <TableCell>
                <Tooltip>
                  <TooltipTrigger as-child>
                    <div class="cursor-help font-mono text-xs">
                      {{ middleEllipsis(session.id, 16) }}
                    </div>
                  </TooltipTrigger>
                  <TooltipContent>
                    <p class="break-all font-mono text-xs">{{ session.id }}</p>
                  </TooltipContent>
                </Tooltip>
              </TableCell>

              <TableCell>
                <div class="text-sm">{{ session.credentialName }}</div>
              </TableCell>

              <TableCell class="min-w-[180px]">
                <InlineCommentEditor
                  :text="session.comment"
                  :save="(value) => updateComment(session.id, value)"
                />
              </TableCell>

              <TableCell>
                <Tooltip>
                  <TooltipTrigger as-child>
                    <div class="cursor-help font-mono text-sm">
                      {{ middleEllipsis(session.ip, 24) }}
                    </div>
                  </TooltipTrigger>
                  <TooltipContent>
                    <p class="break-all font-mono text-xs">{{ session.ip }}</p>
                  </TooltipContent>
                </Tooltip>
                <div
                  v-if="session.ipLocation"
                  class="line-clamp-1 text-xs text-muted-foreground"
                >
                  {{ session.ipLocation }}
                </div>
              </TableCell>

              <TableCell>
                <div class="text-sm">
                  <HumanFriendlyTime :value="session.loginTime" />
                </div>
              </TableCell>

              <TableCell>
                <div class="text-sm">
                  <HumanFriendlyTime :value="session.expiresAt" />
                </div>
              </TableCell>

              <TableCell class="text-right">
                <div class="flex justify-end gap-2">
                  <Button
                    variant="outline"
                    size="sm"
                    class="gap-1.5"
                    @click="openMobility(session)"
                  >
                    <GitBranch class="h-4 w-4" />
                    轨迹
                  </Button>
                  <Button
                    variant="outline"
                    size="sm"
                    class="gap-1.5"
                    @click="openDetail(session)"
                  >
                    <Eye class="h-4 w-4" />
                    详情
                  </Button>
                  <ConfirmDangerPopover
                    title="确认踢出会话？"
                    description="踢出后该会话将失效，需重新登录。"
                    confirm-text="确认踢出"
                    :loading="isKicking"
                    :disabled="isKicking"
                    :on-confirm="() => kickSession(session.id)"
                  >
                    <template #trigger>
                      <Button
                        variant="destructive"
                        size="sm"
                        :disabled="isKicking"
                        class="gap-1.5"
                      >
                        <Trash2 class="h-4 w-4" />
                        踢出
                      </Button>
                    </template>
                  </ConfirmDangerPopover>
                </div>
              </TableCell>
            </TableRow>
          </TableBody>

          <TableBody v-else>
            <TableRow>
              <TableCell
                colspan="8"
                class="py-6 text-center text-muted-foreground"
              >
                暂无会话
              </TableCell>
            </TableRow>
          </TableBody>
        </Table>
      </TooltipProvider>
    </div>

    <DetailDialog
      :open="showDetail"
      title="会话详情"
      description="查看该会话的详细信息"
      max-width-class="sm:max-w-[500px]"
      @update:open="showDetail = $event"
    >
      <div v-if="detailSession" class="max-h-[60vh] overflow-y-auto py-4">
        <DetailFieldsGrid :items="detailItems" layout="compact" />
      </div>
    </DetailDialog>
  </div>
</template>
