<template>
  <Card class="mb-6">
    <CardHeader>
      <CardTitle class="flex justify-between items-center">
        <span>IP白名单管理</span>
        <Button @click="showAddDialog = true">添加 IP</Button>
      </CardTitle>
      <CardDescription>管理直连模式下允许连接的IP地址。</CardDescription>
    </CardHeader>
    <CardContent>
      <div class="flex items-center mb-4 space-x-2" v-if="!isInitializing">
         <SearchInput v-model="searchQuery" placeholder="搜索IP地址或备注..." class="max-w-xs" />
         <RefreshButton icon-only :loading="loading" :disabled="loading" @click="fetchRecords" />
      </div>
      <div v-else-if="showInitializingSkeleton" class="flex items-center mb-4 space-x-2">
        <Skeleton class="h-9 w-60" />
        <Skeleton class="h-9 w-9 rounded-md" />
      </div>
      <div v-else class="h-9 mb-4" aria-hidden="true" ></div>
      
      <div class="border rounded-md" v-if="!isInitializing">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>IP / 归属地</TableHead>
              <TableHead>状态/过期时间</TableHead>
              <TableHead>来源</TableHead>
              <TableHead>创建时间</TableHead>
              <TableHead>备注</TableHead>
              <TableHead class="w-[100px] text-right">操作</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            <TableRow v-if="loading && records.length === 0">
              <TableCell colspan="6" class="text-center text-muted-foreground py-6">
                加载中...
              </TableCell>
            </TableRow>
            <TableRow v-else-if="paginatedRecords.length === 0">
              <TableCell colspan="6" class="text-center text-muted-foreground py-6">
                未找到记录。
              </TableCell>
            </TableRow>
            <TableRow v-for="record in paginatedRecords" :key="record.id">
              <TableCell class="font-medium">
                <div>{{ record.ip }}</div>
                <div v-if="record.ipLocation" class="text-xs text-muted-foreground mt-0.5">{{ record.ipLocation }}</div>
              </TableCell>
              <TableCell>
                <div v-if="!record.expireAt" class="flex items-center text-green-600">
                  <ShieldCheck class="w-4 h-4 mr-1" />
                  永久
                </div>
                <div v-else class="flex flex-col">
                  <span>{{ formatRemaining(record.expireAt) }}</span>
                  <span class="text-xs text-muted-foreground">过期于: <HumanFriendlyTime :value="record.expireAt * 1000" /></span>
                </div>
              </TableCell>
              <TableCell>
                <Badge :variant="record.source === 'manual' ? 'default' : 'secondary'">
                  {{ record.source === 'manual' ? '手动' : '自动' }}
                </Badge>
              </TableCell>
              <TableCell class="text-xs text-muted-foreground whitespace-nowrap">
                <HumanFriendlyTime :value="record.createdAt * 1000" />
              </TableCell>
              <TableCell>
                <InlineCommentEditor
                  :text="record.comment"
                  :save="(value) => saveComment(record.id, value)"
                />
              </TableCell>
              <TableCell class="text-right">
                <ConfirmDangerPopover
                  title="确认删除?"
                  :description="`您即将从白名单中删除 ${record.ip}，此操作不可逆转。`"
                  :loading="removingId === record.id"
                  :disabled="removingId === record.id"
                  :on-confirm="() => removeRecord(record.id)"
                  content-class="w-60 text-left"
                >
                  <template #trigger>
                    <Button
                      variant="ghost"
                      size="icon"
                      class="h-8 w-8 text-destructive hover:bg-destructive/10 hover:text-destructive"
                      :disabled="removingId === record.id"
                    >
                      <Trash2 class="h-4 w-4" />
                    </Button>
                  </template>
                </ConfirmDangerPopover>
              </TableCell>
            </TableRow>
          </TableBody>
        </Table>
      </div>
      <div class="border rounded-md" v-else-if="showInitializingSkeleton">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>IP / 归属地</TableHead>
              <TableHead>状态/过期时间</TableHead>
              <TableHead>来源</TableHead>
              <TableHead>创建时间</TableHead>
              <TableHead>备注</TableHead>
              <TableHead class="w-[100px] text-right">操作</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            <TableRow v-for="n in 6" :key="n">
              <TableCell><Skeleton class="h-4 w-40" /></TableCell>
              <TableCell><Skeleton class="h-4 w-24" /></TableCell>
              <TableCell><Skeleton class="h-4 w-14" /></TableCell>
              <TableCell><Skeleton class="h-4 w-28" /></TableCell>
              <TableCell><Skeleton class="h-4 w-32" /></TableCell>
              <TableCell class="text-right"><Skeleton class="h-8 w-20 rounded-md ml-auto" /></TableCell>
            </TableRow>
          </TableBody>
        </Table>
      </div>
      <div v-else class="h-[320px]" aria-hidden="true" ></div>

      <PagedTableFooter
        class="mt-4 border rounded-md"
        :total="filteredRecords.length"
        :page="currentPage"
        :limit="limit"
        :items-per-page="parsedLimit"
        @update:page="handlePageChange"
        @update:limit="handleLimitChange"
      />
    </CardContent>
  </Card>

  <Dialog v-model:open="showAddDialog">
    <DialogContent>
      <DialogHeader>
        <DialogTitle>添加IP到白名单</DialogTitle>
        <DialogDescription>
          请输入您希望允许访问的IP地址及可选配置。
        </DialogDescription>
      </DialogHeader>
      <div class="grid gap-4 py-4">
        <div class="grid grid-cols-4 items-center gap-4">
          <Label for="ip" class="text-right">IP地址</Label>
          <Input id="ip" v-model="newRecord.ip" placeholder="例如：192.168.1.100" class="col-span-3" />
        </div>
        
        <div class="grid grid-cols-4 items-center gap-4">
          <Label for="duration" class="text-right">有效期</Label>
          <Select v-model="durationSetting">
            <SelectTrigger class="col-span-3">
              <SelectValue placeholder="选择有效期" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="permanent">永久</SelectItem>
              <SelectItem value="1h">1 小时</SelectItem>
              <SelectItem value="24h">24 小时</SelectItem>
              <SelectItem value="7d">7 天</SelectItem>
              <SelectItem value="custom">自定义小时</SelectItem>
            </SelectContent>
          </Select>
        </div>
        
        <div v-if="durationSetting === 'custom'" class="grid grid-cols-4 items-center gap-4">
          <Label for="customHours" class="text-right">自定义小时</Label>
          <Input id="customHours" type="number" min="1" v-model.number="customHours" placeholder="输入小时数" class="col-span-3" />
        </div>

        <div class="grid grid-cols-4 items-center gap-4">
          <Label for="comment" class="text-right">备注 (可选)</Label>
          <Input id="comment" v-model="newRecord.comment" placeholder="输入用途说明..." class="col-span-3" @keyup.enter="addRecord" />
        </div>
      </div>
      <DialogFooter>
        <Button variant="outline" @click="showAddDialog = false">取消</Button>
        <Button @click="addRecord" :disabled="!newRecord.ip || isSaving">保存</Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue';
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Skeleton } from '@/components/ui/skeleton';
import InlineCommentEditor from '@admin-shared/components/InlineCommentEditor.vue';
import SearchInput from '@admin-shared/components/SearchInput.vue';
import { Table, TableHeader, TableRow, TableHead, TableBody, TableCell } from '@/components/ui/table';
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from '@/components/ui/dialog';
import { Label } from '@/components/ui/label';
import { Badge } from '@/components/ui/badge';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { ShieldCheck, Trash2 } from 'lucide-vue-next';
import RefreshButton from '@/components/RefreshButton.vue';
import { toast } from '@admin-shared/utils/toast';
import ConfirmDangerPopover from '@admin-shared/components/common/ConfirmDangerPopover.vue';
import PagedTableFooter from '@admin-shared/components/list/PagedTableFooter.vue';
import HumanFriendlyTime from '@admin-shared/components/common/HumanFriendlyTime.vue';
import { extractErrorMessage, useAsyncAction } from '@admin-shared/composables/useAsyncAction';
import { useLocalPagedList } from '@admin-shared/composables/useLocalPagedList';
import { useDelayedLoading } from '@admin-shared/composables/useDelayedLoading';

// 引入统一封装的 API 和类型
import { WhitelistAPI, type WhiteListRecord } from '../lib/api';

const records = ref<WhiteListRecord[]>([]);
const isInitializing = ref(true);
const showInitializingSkeleton = useDelayedLoading(isInitializing);

const removingId = ref<string | null>(null);
const { run: runRemoveRecord } = useAsyncAction({
  onError: (error) => {
    toast.error('删除发生网络错误', { description: extractErrorMessage(error, '删除失败') });
  },
});
const { run: runSaveComment } = useAsyncAction({
  rethrow: true,
});
const { isPending: loading, run: runFetchRecords } = useAsyncAction({
  onError: (error) => {
    toast.error('加载白名单发生网络错误', { description: extractErrorMessage(error, '加载白名单失败') });
  },
});

// Add dialog states
const showAddDialog = ref(false);
const { isPending: isSaving, run: runAddRecord } = useAsyncAction({
  onError: (error) => {
    toast.error('添加发生网络错误', { description: extractErrorMessage(error, '添加失败') });
  },
});
const durationSetting = ref('permanent');
const customHours = ref(24);
const newRecord = ref({
  ip: '',
  comment: ''
});

const fetchRecords = async () => {
  await runFetchRecords(async () => {
    const res = await WhitelistAPI.getRecords();
    if (res.success) {
      records.value = res.data;
    } else {
      toast.error('获取白名单失败', { description: res.message });
    }
  }, {
    onFinally: () => {
      isInitializing.value = false;
    },
  });
};

let refreshIntervalId: ReturnType<typeof setInterval> | null = null;

onMounted(() => {
  fetchRecords();
  // Optional: Auto-refresh every 30 seconds
  refreshIntervalId = setInterval(fetchRecords, 30000);
});

onUnmounted(() => {
  if (refreshIntervalId) {
    clearInterval(refreshIntervalId);
    refreshIntervalId = null;
  }
});

const {
  searchQuery,
  currentPage,
  limit,
  parsedLimit,
  filteredItems: filteredRecords,
  pagedItems: paginatedRecords,
  handlePageChange,
  handleLimitChange,
} = useLocalPagedList<WhiteListRecord>({
  items: records,
  normalizeQuery: (q) => q.toLowerCase(),
  filter: (record, query) =>
    record.ip.includes(query) || Boolean(record.comment?.toLowerCase().includes(query)),
});

const formatRemaining = (expireAt: number) => {
  const now = Math.floor(Date.now() / 1000);
  const diff = expireAt - now;
  
  if (diff <= 0) return '已过期';
  
  const days = Math.floor(diff / 86400);
  const hours = Math.floor((diff % 86400) / 3600);
  const mins = Math.floor((diff % 3600) / 60);
  
  const parts = [];
  if (days > 0) parts.push(`${days}天`);
  if (hours > 0) parts.push(`${hours}小时`);
  if (mins > 0 || (days === 0 && hours === 0)) parts.push(`${mins}分钟`);
  
  return `剩余 ${parts.join('')}`;
};

// Actions
const addRecord = async () => {
  const ip = newRecord.value.ip.trim();
  if (!ip) return;

  let expireAt: number | null = null;
  const now = Math.floor(Date.now() / 1000);

  if (durationSetting.value !== 'permanent') {
    let addHours = 0;
    switch(durationSetting.value) {
      case '1h': addHours = 1; break;
      case '24h': addHours = 24; break;
      case '7d': addHours = 24 * 7; break;
      case 'custom': addHours = customHours.value || 1; break;
    }
    expireAt = now + (addHours * 3600);
  }

  await runAddRecord(async () => {
    const res = await WhitelistAPI.addRecord({
      ip,
      expireAt,
      source: 'manual',
      comment: newRecord.value.comment.trim() || undefined
    });
    
    if (res.success) {
      toast.success('已成功添加白名单');
      showAddDialog.value = false;
      newRecord.value = { ip: '', comment: '' };
      durationSetting.value = 'permanent';
      currentPage.value = 1;
      searchQuery.value = '';
      await fetchRecords();
    } else {
      toast.error('添加失败', { description: res.message });
    }
  });
};

const removeRecord = async (id: string) => {
  removingId.value = id;
  await runRemoveRecord(async () => {
    const res = await WhitelistAPI.deleteRecord(id);
    if (res.success) {
      toast.success('已删除白名单记录');
      await fetchRecords();
      if (paginatedRecords.value.length === 1 && currentPage.value > 1) {
        currentPage.value--;
      }
    } else {
      toast.error('删除失败', { description: res.message });
    }
  }, {
    onFinally: () => {
      removingId.value = null;
    },
  });
};

const saveComment = async (id: string, newComment: string) => {
  const record = records.value.find(r => r.id === id);
  
  if (record && (record.comment || '') === newComment) {
    return;
  }

  await runSaveComment(
    () => WhitelistAPI.updateComment(id, newComment),
    {
      onSuccess: (res) => {
        if (!res.success) {
          throw new Error(res.message || '更新备注失败');
        }
        if (record) record.comment = newComment;
        toast.success('备注已更新');
      },
      onError: (error) => {
        throw new Error(extractErrorMessage(error, '更新备注失败'));
      },
    },
  );
};
</script>
