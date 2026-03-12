<script setup lang="ts">
import { ref, computed, watch } from 'vue';
import { Button } from '@/components/ui/button';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table';
import { Badge } from '@/components/ui/badge';
import { toast } from '@admin-shared/utils/toast';
import type { SessionRecord } from '../../types';
import { SessionAPI } from '../../lib/api';
import { Eye, Trash2 } from 'lucide-vue-next';
import RefreshButton from '@/components/RefreshButton.vue';
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip';
import { useConfigStore } from '../../store/config';
import ConfirmDangerPopover from '@admin-shared/components/common/ConfirmDangerPopover.vue';
import DetailDialog from '@admin-shared/components/common/DetailDialog.vue';
import DetailFieldsGrid from '@admin-shared/components/common/DetailFieldsGrid.vue';
import HumanFriendlyTime from '@admin-shared/components/common/HumanFriendlyTime.vue';
import { extractErrorMessage, useAsyncAction } from '@admin-shared/composables/useAsyncAction';
import { buildDetailFields } from '@admin-shared/utils/buildDetailFields';
import { formatDateTimeSafe } from '@admin-shared/utils/formatDateTimeSafe';

const sessions = ref<SessionRecord[]>([]);
const showDetail = ref(false);
const detailSession = ref<SessionRecord | null>(null);
const { isPending: isLoading, run: runLoadSessions } = useAsyncAction({
  onError: (error) => {
    toast.error('加载失败', { description: extractErrorMessage(error, '加载失败') });
  },
});
const { isPending: isKicking, run: runKickSession } = useAsyncAction({
  onError: (error) => {
    toast.error('踢出失败', { description: extractErrorMessage(error, '操作失败') });
  },
});

const configStore = useConfigStore();

const detailFieldDefinitions = [
  { key: 'id', label: '会话 ID' },
  { key: 'method', label: '登录方式' },
  { key: 'credentialName', label: '凭证名称' },
  { key: 'ip', label: 'IP 地址' },
  { key: 'ipLocation', label: '归属信息' },
  { key: 'userAgent', label: 'User-Agent' },
  { key: 'loginTime', label: '登录时间' },
  { key: 'expiresAt', label: '过期时间' },
] as const;

const formatDateTime = (value: unknown) => {
  return formatDateTimeSafe(value as string | number | Date | null | undefined);
};

const middleEllipsis = (text: string, max = 16) => {
  if (!text) return '';
  if (text.length <= max) return text;
  const head = Math.ceil((max - 1) / 2);
  const tail = Math.floor((max - 1) / 2);
  return `${text.slice(0, head)}……${text.slice(text.length - tail)}`;
};

async function fetchSessions() {
  await runLoadSessions(async () => {
    sessions.value = await SessionAPI.list();
  });
}

function openDetail(s: SessionRecord) {
  detailSession.value = s;
  showDetail.value = true;
}

async function kickSession(sessionId: string) {
  await runKickSession(
    () => SessionAPI.kick(sessionId),
    {
      onSuccess: async () => {
        toast.success('已踢出会话');
        await fetchSessions();
      },
    },
  );
}

const hasSessions = computed(() => sessions.value.length > 0);

const detailItems = computed(() => {
  return buildDetailFields(detailSession.value, detailFieldDefinitions, {
    format: (key, value) => {
      if (key === 'loginTime' || key === 'expiresAt') return formatDateTime(value);
      return value;
    },
  });
});

watch(
  () => configStore.config?.run_type,
  (rt) => {
    if (rt === 1) fetchSessions();
  },
  { immediate: true }
);
</script>

<template>
  <div class="space-y-3">
    <div class="flex items-center justify-between">
      <div class="text-sm text-muted-foreground">当前活跃会话 {{ sessions.length }} 个</div>
      <RefreshButton :loading="isLoading" :disabled="isLoading" @click="fetchSessions" />
    </div>
    <div class="rounded-md border overflow-hidden">
      <TooltipProvider>
      <Table>
        <TableHeader>
          <TableRow>
            <TableHead class="w-[150px]">会话ID</TableHead>
            <TableHead>登录方式</TableHead>
            <TableHead>凭证</TableHead>
            <TableHead>IP / 归属</TableHead>
            <TableHead>登录时间</TableHead>
            <TableHead>过期时间</TableHead>
            <TableHead class="text-right w-[160px]">操作</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody v-if="hasSessions">
          <TableRow v-for="s in sessions" :key="s.id">
            <TableCell>
              <Tooltip>
                <TooltipTrigger as-child>
                  <div class="font-mono text-xs cursor-help">{{ middleEllipsis(s.id, 16) }}</div>
                </TooltipTrigger>
                <TooltipContent>
                  <p class="font-mono text-xs break-all">{{ s.id }}</p>
                </TooltipContent>
              </Tooltip>
            </TableCell>
            <TableCell>
              <Badge variant="secondary">{{ s.method }}</Badge>
            </TableCell>
            <TableCell>
              <div class="text-sm">{{ s.credentialName }}</div>
            </TableCell>
            <TableCell>
              <div class="text-sm">{{ s.ip }}</div>
              <div v-if="s.ipLocation" class="text-xs text-muted-foreground line-clamp-1">{{ s.ipLocation }}</div>
            </TableCell>
            <TableCell>
              <div class="text-sm"><HumanFriendlyTime :value="s.loginTime" /></div>
            </TableCell>
            <TableCell>
              <div class="text-sm"><HumanFriendlyTime :value="s.expiresAt" /></div>
            </TableCell>
            <TableCell class="text-right">
              <div class="flex justify-end gap-2">
                <Button variant="outline" size="sm" @click="openDetail(s)">
                  <Eye class="h-4 w-4" /> 详情
                </Button>
                <ConfirmDangerPopover
                  title="确认踢出会话？"
                  description="踢出后该会话将失效，需重新登录。"
                  confirm-text="确认踢出"
                  :loading="isKicking"
                  :disabled="isKicking"
                  :on-confirm="() => kickSession(s.id)"
                >
                  <template #trigger>
                    <Button variant="destructive" size="sm" :disabled="isKicking">
                      <Trash2 class="h-4 w-4" /> 踢出
                    </Button>
                  </template>
                </ConfirmDangerPopover>
              </div>
            </TableCell>
          </TableRow>
        </TableBody>
        <TableBody v-else>
          <TableRow>
            <TableCell colspan="7" class="text-center text-muted-foreground py-6">
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
      <div v-if="detailSession" class="py-4 max-h-[60vh] overflow-y-auto">
        <DetailFieldsGrid :items="detailItems" layout="compact" />
      </div>
    </DetailDialog>
  </div>
</template>
