<script setup lang="ts">
import { onMounted } from "vue";
import { useI18n } from "vue-i18n";
import HumanFriendlyTime from "@admin-shared/components/common/HumanFriendlyTime.vue";
import PagedTableFooter from "@admin-shared/components/list/PagedTableFooter.vue";
import SearchInput from "@admin-shared/components/SearchInput.vue";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent } from "@/components/ui/card";
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
import RefreshButton from "@/components/RefreshButton.vue";
import { Loader2 } from "lucide-vue-next";
import { SSHSecurityAPI } from "@/lib/api/security";
import { useSSHLoginLogs } from "./useSSHLoginLogs";

const { t } = useI18n();

const {
  handleLogLimitChange,
  handleLogPageChange,
  handleLogSearch,
  isLoadingLogs,
  loadLogs,
  logItems,
  logLimit,
  logOutcome,
  logPage,
  logParsedLimit,
  logSearch,
  logTotal,
} = useSSHLoginLogs({
  fetchLogs: (params) => SSHSecurityAPI.getLoginLogs(params),
  translate: (key) => t(key),
});

onMounted(() => {
  void loadLogs();
});
</script>

<template>
  <div class="space-y-3">
    <div class="flex flex-wrap items-center gap-2">
      <SearchInput
        v-model="logSearch"
        :placeholder="t('admin.sshSecurity.searchLogsPlaceholder')"
        class="w-full max-w-xs"
        @search="handleLogSearch"
      />
      <Select v-model="logOutcome">
        <SelectTrigger
          :aria-label="t('admin.sshSecurity.allResults')"
          class="w-[140px]"
        >
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem value="all">
            {{ t("admin.sshSecurity.allResults") }}
          </SelectItem>
          <SelectItem value="success">
            {{ t("admin.sshSecurity.success") }}
          </SelectItem>
          <SelectItem value="failure">
            {{ t("admin.sshSecurity.failure") }}
          </SelectItem>
        </SelectContent>
      </Select>
      <div class="flex-1"></div>
      <RefreshButton
        :loading="isLoadingLogs"
        :disabled="isLoadingLogs"
        @click="loadLogs"
      />
    </div>

    <Card class="border-border/60 shadow-none">
      <CardContent class="p-0">
        <div class="overflow-auto">
          <Table class="min-w-[760px]">
            <TableHeader>
              <TableRow>
                <TableHead class="h-11 w-[168px] px-4">
                  {{ t("admin.sshSecurity.time") }}
                </TableHead>
                <TableHead class="h-11 w-[92px] px-4">
                  {{ t("admin.sshSecurity.result") }}
                </TableHead>
                <TableHead class="h-11 min-w-[160px] px-4">
                  {{ t("admin.sshSecurity.user") }}
                </TableHead>
                <TableHead class="h-11 min-w-[220px] px-4">
                  {{ t("admin.sshSecurity.ipLocation") }}
                </TableHead>
                <TableHead class="h-11 min-w-[180px] px-4">
                  {{ t("admin.sshSecurity.method") }}
                </TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              <TableRow v-if="isLoadingLogs">
                <TableCell colspan="5" class="px-4 py-10 text-center">
                  <Loader2
                    class="mx-auto h-6 w-6 animate-spin text-muted-foreground"
                  />
                </TableCell>
              </TableRow>
              <TableRow v-else-if="logItems.length === 0">
                <TableCell
                  colspan="5"
                  class="px-4 py-10 text-center text-muted-foreground"
                >
                  {{ t("admin.sshSecurity.noLoginLogs") }}
                </TableCell>
              </TableRow>
              <TableRow v-for="entry in logItems" v-else :key="entry.id">
                <TableCell class="px-4 py-3 align-top whitespace-nowrap">
                  <HumanFriendlyTime :value="entry.happened_at" />
                </TableCell>
                <TableCell class="px-4 py-3 align-top">
                  <div class="flex flex-wrap items-center gap-1.5">
                    <Badge
                      :variant="
                        entry.outcome === 'success' ? 'default' : 'secondary'
                      "
                    >
                      {{
                        entry.outcome === "success"
                          ? t("admin.sshSecurity.success")
                          : t("admin.sshSecurity.failure")
                      }}
                    </Badge>
                  </div>
                </TableCell>
                <TableCell
                  class="min-w-[160px] px-4 py-3 align-top whitespace-normal"
                >
                  <div class="font-medium">{{ entry.username }}</div>
                  <div
                    v-if="entry.invalid_user"
                    class="text-xs text-muted-foreground"
                  >
                    {{ t("admin.sshSecurity.invalidUser") }}
                  </div>
                </TableCell>
                <TableCell
                  class="min-w-[220px] px-4 py-3 align-top whitespace-normal"
                >
                  <div class="font-mono text-sm">{{ entry.ip }}</div>
                  <div
                    v-if="entry.ipLocation"
                    class="mt-0.5 text-xs text-muted-foreground"
                  >
                    {{ entry.ipLocation }}
                  </div>
                </TableCell>
                <TableCell
                  class="min-w-[180px] px-4 py-3 align-top whitespace-normal"
                >
                  <span class="break-words">
                    {{ entry.auth_method || "-" }}
                  </span>
                  <span
                    v-if="entry.port"
                    class="text-muted-foreground whitespace-nowrap"
                  >
                    / {{ entry.port }}
                  </span>
                </TableCell>
              </TableRow>
            </TableBody>
          </Table>
        </div>
        <PagedTableFooter
          :total="logTotal"
          :page="logPage"
          :limit="logLimit"
          :items-per-page="logParsedLimit"
          @update:page="handleLogPageChange"
          @update:limit="handleLogLimitChange"
        />
      </CardContent>
    </Card>
  </div>
</template>
