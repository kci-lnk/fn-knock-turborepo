<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { Button } from "@/components/ui/button";
import RefreshButton from "@/components/RefreshButton.vue";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { Badge } from "@/components/ui/badge";
import { toast } from "@admin-shared/utils/toast";
import { BackoffAPI, type BackoffItem } from "../../lib/api";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";

const { t } = useI18n();
const items = ref<BackoffItem[]>([]);

const { isPending: isLoading, run: runLoadBackoff } = useAsyncAction({
  onError: (error) => {
    items.value = [];
    toast.error(t("admin.sessions.loginBackoff.loadFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.sessions.loginBackoff.loadFailed"),
      ),
    });
  },
});

const { isPending: isResetting, run: runResetIp } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.sessions.loginBackoff.resetFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.sessions.loginBackoff.resetFailed"),
      ),
    });
  },
});

const hasItems = computed(() => items.value.length > 0);

const load = async () => {
  await runLoadBackoff(async () => {
    items.value = await BackoffAPI.list();
  });
};

const resetIp = async (ip: string) => {
  await runResetIp(() => BackoffAPI.reset(ip), {
    onSuccess: async () => {
      toast.success(t("admin.sessions.loginBackoff.resetSuccess", { ip }));
      await load();
    },
  });
};

onMounted(load);
</script>

<template>
  <div class="space-y-3">
    <div class="flex items-center justify-between">
      <div class="text-sm text-muted-foreground">
        {{ t("admin.sessions.loginBackoff.summary", { count: items.length }) }}
      </div>
      <RefreshButton :loading="isLoading" :disabled="isLoading" @click="load" />
    </div>

    <div class="rounded-md border overflow-hidden">
      <Table>
        <TableHeader>
          <TableRow>
            <TableHead class="w-[220px]">IP</TableHead>
            <TableHead class="w-[140px]">{{
              t("admin.sessions.loginBackoff.attemptsInHour")
            }}</TableHead>
            <TableHead>{{ t("admin.sessions.loginBackoff.status") }}</TableHead>
            <TableHead>{{
              t("admin.sessions.loginBackoff.remainingTime")
            }}</TableHead>
            <TableHead class="text-right w-[160px]">{{
              t("admin.sessions.table.actions")
            }}</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody v-if="hasItems">
          <TableRow v-for="item in items" :key="item.ip">
            <TableCell class="font-mono text-sm">{{ item.ip }}</TableCell>
            <TableCell>{{ item.attempts }}</TableCell>
            <TableCell>
              <Badge :variant="item.blocked ? 'destructive' : 'secondary'">
                {{
                  item.blocked
                    ? t("admin.sessions.loginBackoff.blocked")
                    : t("admin.sessions.loginBackoff.notBlocked")
                }}
              </Badge>
            </TableCell>
            <TableCell>
              <span v-if="item.retryAfter">{{
                t("admin.sessions.loginBackoff.seconds", {
                  seconds: item.retryAfter,
                })
              }}</span>
              <span v-else>-</span>
            </TableCell>
            <TableCell class="text-right">
              <div class="flex justify-end">
                <ConfirmDangerPopover
                  :title="t('admin.sessions.loginBackoff.confirmTitle')"
                  :description="
                    t('admin.sessions.loginBackoff.confirmDescription', {
                      ip: item.ip,
                    })
                  "
                  :confirm-text="t('admin.sessions.loginBackoff.confirmText')"
                  :loading="isResetting"
                  :disabled="isResetting"
                  :on-confirm="() => resetIp(item.ip)"
                >
                  <template #trigger>
                    <Button
                      variant="destructive"
                      size="sm"
                      :disabled="isResetting"
                    >
                      {{ t("admin.sessions.loginBackoff.reset") }}
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
              colspan="5"
              class="text-center text-muted-foreground py-6"
            >
              {{ t("admin.sessions.loginBackoff.empty") }}
            </TableCell>
          </TableRow>
        </TableBody>
      </Table>
    </div>
  </div>
</template>
