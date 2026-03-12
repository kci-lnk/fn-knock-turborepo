<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import { Button } from '@/components/ui/button';
import RefreshButton from '@/components/RefreshButton.vue';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table';
import { Badge } from '@/components/ui/badge';
import { toast } from '@admin-shared/utils/toast';
import { BackoffAPI, type BackoffItem } from '../../lib/api';
import ConfirmDangerPopover from '@admin-shared/components/common/ConfirmDangerPopover.vue';
import { extractErrorMessage, useAsyncAction } from '@admin-shared/composables/useAsyncAction';

const items = ref<BackoffItem[]>([]);

const { isPending: isLoading, run: runLoadBackoff } = useAsyncAction({
  onError: (error) => {
    items.value = [];
    toast.error('加载失败', { description: extractErrorMessage(error, '加载失败') });
  },
});

const { isPending: isResetting, run: runResetIp } = useAsyncAction({
  onError: (error) => {
    toast.error('解除失败', { description: extractErrorMessage(error, '解除失败') });
  },
});

const hasItems = computed(() => items.value.length > 0);

const load = async () => {
  await runLoadBackoff(async () => {
    items.value = await BackoffAPI.list();
  });
};

const resetIp = async (ip: string) => {
  await runResetIp(
    () => BackoffAPI.reset(ip),
    {
      onSuccess: async () => {
        toast.success(`已解除 ${ip}`);
        await load();
      },
    },
  );
};

onMounted(load);
</script>

<template>
  <div class="space-y-3">
    <div class="flex items-center justify-between">
      <div class="text-sm text-muted-foreground">当前受限 IP {{ items.length }} 个</div>
      <RefreshButton :loading="isLoading" :disabled="isLoading" @click="load" />
    </div>

    <div class="rounded-md border overflow-hidden">
      <Table>
        <TableHeader>
          <TableRow>
            <TableHead class="w-[220px]">IP</TableHead>
            <TableHead class="w-[140px]">失败次数</TableHead>
            <TableHead>状态</TableHead>
            <TableHead>剩余时间</TableHead>
            <TableHead class="text-right w-[160px]">操作</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody v-if="hasItems">
          <TableRow v-for="item in items" :key="item.ip">
            <TableCell class="font-mono text-sm">{{ item.ip }}</TableCell>
            <TableCell>{{ item.attempts }}</TableCell>
            <TableCell>
              <Badge :variant="item.blocked ? 'destructive' : 'secondary'">
                {{ item.blocked ? '限制中' : '未限制' }}
              </Badge>
            </TableCell>
            <TableCell>
              <span v-if="item.retryAfter">{{ item.retryAfter }} 秒</span>
              <span v-else>-</span>
            </TableCell>
            <TableCell class="text-right">
              <div class="flex justify-end">
                <ConfirmDangerPopover
                  title="确认解除限制？"
                  :description="`解除后 IP ${item.ip} 将恢复访问。`"
                  confirm-text="确认解除"
                  :loading="isResetting"
                  :disabled="isResetting"
                  :on-confirm="() => resetIp(item.ip)"
                >
                  <template #trigger>
                    <Button variant="destructive" size="sm" :disabled="isResetting">解除</Button>
                  </template>
                </ConfirmDangerPopover>
              </div>
            </TableCell>
          </TableRow>
        </TableBody>
        <TableBody v-else>
          <TableRow>
            <TableCell colspan="5" class="text-center text-muted-foreground py-6">
              当前没有被限制的 IP
            </TableCell>
          </TableRow>
        </TableBody>
      </Table>
    </div>
  </div>
</template>
