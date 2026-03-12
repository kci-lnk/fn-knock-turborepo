<template>
  <div class="space-y-4">
    <Breadcrumb>
      <BreadcrumbList>
        <BreadcrumbItem>
          <BreadcrumbLink href="#/auth">TOTP 管理</BreadcrumbLink>
        </BreadcrumbItem>
        <BreadcrumbSeparator />
        <BreadcrumbItem>
          <BreadcrumbPage>{{ totpName ? `${totpName} Passkey` : 'Passkey 管理' }}</BreadcrumbPage>
        </BreadcrumbItem>
      </BreadcrumbList>
    </Breadcrumb>

    <Card>
      <CardHeader>
        <CardTitle>Passkey 管理</CardTitle>
        <CardDescription>管理已绑定的 Passkey 设备与登录凭据。</CardDescription>
      </CardHeader>
    <CardContent class="space-y-4">
      <div v-if="isLoading" class="flex items-center justify-center py-10 text-sm text-muted-foreground">
        <span class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-primary border-t-transparent"></span>
        正在加载 Passkey 列表...
      </div>
      <Table v-else>
        <TableHeader>
          <TableRow>
            <TableHead>Passkey</TableHead>
            <TableHead>设备</TableHead>
            <TableHead>绑定时间</TableHead>
            <TableHead class="text-right">操作</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          <TableRow v-for="passkey in passkeys" :key="passkey.id">
            <TableCell class="font-mono text-xs text-muted-foreground">{{ formatId(passkey.id) }}</TableCell>
            <TableCell>{{ passkey.deviceName }}</TableCell>
            <TableCell><HumanFriendlyTime :value="passkey.createdAt" /></TableCell>
            <TableCell class="text-right">
              <ConfirmDangerPopover
                title="删除 Passkey"
                description="确认删除该 Passkey 吗？删除后将无法使用该设备一键登录。"
                :loading="isDeleting"
                :disabled="isDeleting"
                :on-confirm="() => handleDelete(passkey.id)"
              >
                <template #trigger>
                  <Button variant="destructive" size="sm" :disabled="isDeleting">
                    删除
                  </Button>
                </template>
              </ConfirmDangerPopover>
            </TableCell>
          </TableRow>
          <TableEmpty v-if="passkeys.length === 0" :colspan="4">
            暂无已绑定的 Passkey
          </TableEmpty>
        </TableBody>
      </Table>
      <div v-if="errorMessage" class="text-sm text-destructive">{{ errorMessage }}</div>
    </CardContent>
  </Card>

  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { useRoute } from 'vue-router';
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from '@/components/ui/card';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow, TableEmpty } from '@/components/ui/table';
import { Button } from '@/components/ui/button';
import { Breadcrumb, BreadcrumbItem, BreadcrumbLink, BreadcrumbList, BreadcrumbPage, BreadcrumbSeparator } from '@/components/ui/breadcrumb';
import ConfirmDangerPopover from '@admin-shared/components/common/ConfirmDangerPopover.vue';
import HumanFriendlyTime from '@admin-shared/components/common/HumanFriendlyTime.vue';
import { extractErrorMessage, useAsyncAction } from '@admin-shared/composables/useAsyncAction';
import { toast } from '@admin-shared/utils/toast';
import { ConfigAPI } from '../lib/api';
import type { PasskeyCredential } from '../types';

const route = useRoute();
const totpId = route.params.totpId as string;

const passkeys = ref<PasskeyCredential[]>([]);
const errorMessage = ref('');
const totpName = ref('');
const { isPending: isLoading, run: runLoadPasskeys } = useAsyncAction({
  onError: (error) => {
    errorMessage.value = extractErrorMessage(error, '获取 Passkey 列表失败');
  },
});
const { isPending: isDeleting, run: runDeletePasskey } = useAsyncAction({
  onError: (error) => {
    const message = extractErrorMessage(error, '删除 Passkey 失败');
    errorMessage.value = message;
    toast.error('删除失败', { description: message });
  },
});

onMounted(async () => {
  await fetchPasskeys();
});

async function fetchPasskeys() {
  errorMessage.value = '';
  await runLoadPasskeys(async () => {
    totpName.value = '';
    const [passkeysRes, statusRes] = await Promise.all([
      ConfigAPI.getPasskeys(totpId),
      ConfigAPI.getTOTPStatus().catch(() => null)
    ]);
    passkeys.value = passkeysRes;
    if (statusRes && statusRes.credentials) {
      const parentTotp = statusRes.credentials.find((c: any) => c.id === totpId);
      if (parentTotp && parentTotp.comment) {
        totpName.value = parentTotp.comment;
      }
    }
  });
}

function formatId(id: string) {
  if (id.length <= 12) return id;
  return `${id.slice(0, 6)}...${id.slice(-6)}`;
}

async function handleDelete(passkeyId: string) {
  errorMessage.value = '';
  await runDeletePasskey(async () => {
    await ConfigAPI.deletePasskey(passkeyId);
    await fetchPasskeys();
    toast.success('Passkey 已删除');
  });
}
</script>
