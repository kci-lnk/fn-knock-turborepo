<template>
  <Card class="mb-6">
    <CardHeader>
      <CardTitle class="flex justify-between items-center">
        <span>{{ t("admin.ipWhitelist.title") }}</span>
        <div class="flex items-center gap-2">
          <DocsLinkButton :href="docsUrls.guides.whitelist" />
          <Button @click="showAddDialog = true">{{
            t("admin.ipWhitelist.addTarget")
          }}</Button>
        </div>
      </CardTitle>
      <CardDescription>{{ pageDescription }}</CardDescription>
    </CardHeader>
    <CardContent>
      <div class="flex items-center mb-4 space-x-2" v-if="!isInitializing">
        <SearchInput
          v-model="searchQuery"
          :placeholder="t('admin.ipWhitelist.searchPlaceholder')"
          class="max-w-xs"
        />
        <RefreshButton
          icon-only
          :loading="loading"
          :disabled="loading"
          @click="fetchRecords"
        />
      </div>
      <div
        v-else-if="showInitializingSkeleton"
        class="flex items-center mb-4 space-x-2"
      >
        <Skeleton class="h-9 w-60" />
        <Skeleton class="h-9 w-9 rounded-md" />
      </div>
      <div v-else class="h-9 mb-4" aria-hidden="true"></div>

      <div class="border rounded-md" v-if="!isInitializing">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>{{ t("admin.ipWhitelist.target") }}</TableHead>
              <TableHead>{{ t("admin.ipWhitelist.status") }}</TableHead>
              <TableHead>{{ t("admin.ipWhitelist.source") }}</TableHead>
              <TableHead>{{ t("admin.ipWhitelist.createdAt") }}</TableHead>
              <TableHead>{{ t("admin.ipWhitelist.comment") }}</TableHead>
              <TableHead class="w-[180px] text-right">{{
                t("admin.ipWhitelist.actions")
              }}</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            <TableRow v-if="loading && records.length === 0">
              <TableCell
                colspan="6"
                class="text-center text-muted-foreground py-6"
              >
                {{ t("admin.ipWhitelist.loading") }}
              </TableCell>
            </TableRow>
            <TableRow v-else-if="paginatedRecords.length === 0">
              <TableCell
                colspan="6"
                class="text-center text-muted-foreground py-6"
              >
                {{ t("admin.ipWhitelist.empty") }}
              </TableCell>
            </TableRow>
            <TableRow v-for="record in paginatedRecords" :key="record.id">
              <TableCell class="font-medium">
                <div class="flex items-center gap-2">
                  <span>{{ record.ip }}</span>
                  <Badge variant="outline">
                    {{
                      targetTypeBadgeLabel(record.targetType)
                    }}
                  </Badge>
                </div>
                <div
                  v-if="record.targetType === 'cname'"
                  class="mt-2 space-y-1"
                >
                  <div
                    v-for="resolvedTarget in record.resolvedTargets || []"
                    :key="resolvedTarget"
                  >
                    <Badge variant="secondary" class="font-normal">
                      {{ resolvedTarget }}
                    </Badge>
                  </div>
                  <span
                    v-if="!(record.resolvedTargets || []).length"
                    class="block text-xs text-muted-foreground"
                  >
                    {{ t("admin.ipWhitelist.noResolvedRecords") }}
                  </span>
                </div>
                <div
                  v-if="record.targetType === 'cname' && record.resolveMessage"
                  class="text-xs text-muted-foreground mt-1"
                >
                  {{ record.resolveMessage }}
                </div>
                <div
                  v-if="record.ipLocation"
                  class="text-xs text-muted-foreground mt-0.5"
                >
                  {{ record.ipLocation }}
                </div>
              </TableCell>
              <TableCell>
                <template v-if="record.targetType === 'cname'">
                  <div class="flex flex-col items-start gap-1.5">
                    <Badge :variant="getResolveStatusVariant(record)">
                      {{ getResolveStatusLabel(record) }}
                    </Badge>
                    <span class="text-xs text-muted-foreground">
                      {{
                        t("admin.ipWhitelist.checkInterval", {
                          minutes: record.checkIntervalMinutes || 5,
                        })
                      }}
                    </span>
                    <span
                      v-if="record.lastCheckedAt"
                      class="text-xs text-muted-foreground"
                    >
                      {{ t("admin.ipWhitelist.lastCheckedAt") }}
                      <HumanFriendlyTime :value="record.lastCheckedAt * 1000" />
                    </span>
                    <span
                      v-if="record.expireAt"
                      class="text-xs text-muted-foreground"
                    >
                      {{ t("admin.ipWhitelist.expiresAt") }}
                      <HumanFriendlyTime :value="record.expireAt * 1000" />
                    </span>
                    <div
                      v-else
                      class="flex items-center text-green-600 text-sm"
                    >
                      <ShieldCheck class="w-4 h-4 mr-1" />
                      {{ t("admin.ipWhitelist.permanent") }}
                    </div>
                  </div>
                </template>
                <template v-else>
                  <div
                    v-if="!record.expireAt"
                    class="flex items-center text-green-600"
                  >
                    <ShieldCheck class="w-4 h-4 mr-1" />
                    {{ t("admin.ipWhitelist.permanent") }}
                  </div>
                  <div v-else class="flex flex-col">
                    <span>{{ formatRemaining(record.expireAt) }}</span>
                    <span class="text-xs text-muted-foreground"
                      >{{ t("admin.ipWhitelist.expiresAt") }}
                      <HumanFriendlyTime :value="record.expireAt * 1000"
                    /></span>
                  </div>
                </template>
              </TableCell>
              <TableCell>
                <Badge
                  :variant="
                    record.source === 'manual' ? 'default' : 'secondary'
                  "
                >
                  {{
                    record.source === "manual"
                      ? t("admin.ipWhitelist.sourceManual")
                      : t("admin.ipWhitelist.sourceLoginGrant")
                  }}
                </Badge>
              </TableCell>
              <TableCell
                class="text-xs text-muted-foreground whitespace-nowrap"
              >
                <HumanFriendlyTime :value="record.createdAt * 1000" />
              </TableCell>
              <TableCell>
                <InlineCommentEditor
                  :text="record.comment"
                  :save="(value) => saveComment(record.id, value)"
                />
              </TableCell>
              <TableCell class="text-right">
                <div class="flex justify-end gap-2">
                  <Button
                    v-if="record.targetType === 'cname'"
                    variant="outline"
                    size="sm"
                    :disabled="refreshingId === record.id"
                    @click="refreshRecord(record.id)"
                  >
                    <RefreshCw
                      :class="[
                        'h-4 w-4 mr-1',
                        refreshingId === record.id ? 'animate-spin' : '',
                      ]"
                    />
                    {{ t("admin.ipWhitelist.refreshNow") }}
                  </Button>
                  <ConfirmDangerPopover
                    :title="t('admin.ipWhitelist.deleteTitle')"
                    :description="
                      t('admin.ipWhitelist.deleteDescription', {
                        target: record.ip,
                      })
                    "
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
                </div>
              </TableCell>
            </TableRow>
          </TableBody>
        </Table>
      </div>
      <div class="border rounded-md" v-else-if="showInitializingSkeleton">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>{{ t("admin.ipWhitelist.target") }}</TableHead>
              <TableHead>{{ t("admin.ipWhitelist.statusExpires") }}</TableHead>
              <TableHead>{{ t("admin.ipWhitelist.source") }}</TableHead>
              <TableHead>{{ t("admin.ipWhitelist.createdAt") }}</TableHead>
              <TableHead>{{ t("admin.ipWhitelist.comment") }}</TableHead>
              <TableHead class="w-[180px] text-right">{{
                t("admin.ipWhitelist.actions")
              }}</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            <TableRow v-for="n in 6" :key="n">
              <TableCell><Skeleton class="h-4 w-40" /></TableCell>
              <TableCell><Skeleton class="h-4 w-24" /></TableCell>
              <TableCell><Skeleton class="h-4 w-14" /></TableCell>
              <TableCell><Skeleton class="h-4 w-28" /></TableCell>
              <TableCell><Skeleton class="h-4 w-32" /></TableCell>
              <TableCell class="text-right"
                ><Skeleton class="h-8 w-20 rounded-md ml-auto"
              /></TableCell>
            </TableRow>
          </TableBody>
        </Table>
      </div>
      <div v-else class="h-[320px]" aria-hidden="true"></div>

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
        <DialogTitle>{{ t("admin.ipWhitelist.addDialogTitle") }}</DialogTitle>
        <DialogDescription>
          {{ t("admin.ipWhitelist.addDialogDescription") }}
        </DialogDescription>
      </DialogHeader>
      <div class="grid gap-4 py-4">
        <div class="grid grid-cols-4 items-center gap-4">
          <Label for="targetType" class="text-right">{{
            t("admin.ipWhitelist.type")
          }}</Label>
          <Select v-model="newRecord.targetType">
            <SelectTrigger class="col-span-3">
              <SelectValue :placeholder="t('admin.ipWhitelist.selectType')" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="ip">{{
                t("admin.ipWhitelist.typeIp")
              }}</SelectItem>
              <SelectItem value="cidr">{{
                t("admin.ipWhitelist.typeCidr")
              }}</SelectItem>
              <SelectItem value="cname">{{
                t("admin.ipWhitelist.typeCname")
              }}</SelectItem>
            </SelectContent>
          </Select>
        </div>

        <div class="grid grid-cols-4 items-center gap-4">
          <Label for="ip" class="text-right">{{
            t("admin.ipWhitelist.target")
          }}</Label>
          <Input
            id="ip"
            v-model="newRecord.ip"
            :placeholder="newRecordPlaceholder"
            class="col-span-3"
          />
        </div>

        <div
          v-if="newRecord.targetType === 'cname'"
          class="grid grid-cols-4 items-center gap-4"
        >
          <Label for="checkIntervalMinutes" class="text-right"
            >{{ t("admin.ipWhitelist.checkIntervalLabel") }}</Label
          >
          <div class="col-span-3 flex items-center gap-2">
            <Input
              id="checkIntervalMinutes"
              type="number"
              min="1"
              v-model.number="newRecord.checkIntervalMinutes"
              :placeholder="t('admin.ipWhitelist.defaultFive')"
            />
            <span class="text-sm text-muted-foreground whitespace-nowrap"
              >{{ t("admin.ipWhitelist.minutes") }}</span
            >
          </div>
        </div>

        <div class="grid grid-cols-4 items-center gap-4">
          <Label for="duration" class="text-right">{{
            t("admin.ipWhitelist.duration")
          }}</Label>
          <Select v-model="durationSetting">
            <SelectTrigger class="col-span-3">
              <SelectValue :placeholder="t('admin.ipWhitelist.selectDuration')" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="permanent">{{
                t("admin.ipWhitelist.permanent")
              }}</SelectItem>
              <SelectItem value="1h">{{
                t("admin.ipWhitelist.oneHour")
              }}</SelectItem>
              <SelectItem value="24h">{{
                t("admin.ipWhitelist.twentyFourHours")
              }}</SelectItem>
              <SelectItem value="7d">{{
                t("admin.ipWhitelist.sevenDays")
              }}</SelectItem>
              <SelectItem value="custom">{{
                t("admin.ipWhitelist.customHours")
              }}</SelectItem>
            </SelectContent>
          </Select>
        </div>

        <div
          v-if="durationSetting === 'custom'"
          class="grid grid-cols-4 items-center gap-4"
        >
          <Label for="customHours" class="text-right">{{
            t("admin.ipWhitelist.customHours")
          }}</Label>
          <Input
            id="customHours"
            type="number"
            min="1"
            v-model.number="customHours"
            :placeholder="t('admin.ipWhitelist.customHoursPlaceholder')"
            class="col-span-3"
          />
        </div>

        <div class="grid grid-cols-4 items-center gap-4">
          <Label for="comment" class="text-right">{{
            t("admin.ipWhitelist.commentOptional")
          }}</Label>
          <Input
            id="comment"
            v-model="newRecord.comment"
            :placeholder="t('admin.ipWhitelist.commentPlaceholder')"
            class="col-span-3"
            @keyup.enter="addRecord"
          />
        </div>
      </div>
      <DialogFooter>
        <Button variant="outline" @click="showAddDialog = false">{{
          t("common.cancel")
        }}</Button>
        <Button @click="addRecord" :disabled="!newRecord.ip || isSaving"
          >{{ t("common.save") }}</Button
        >
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import {
  Card,
  CardHeader,
  CardTitle,
  CardDescription,
  CardContent,
} from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Skeleton } from "@/components/ui/skeleton";
import InlineCommentEditor from "@admin-shared/components/InlineCommentEditor.vue";
import SearchInput from "@admin-shared/components/SearchInput.vue";
import {
  Table,
  TableHeader,
  TableRow,
  TableHead,
  TableBody,
  TableCell,
} from "@/components/ui/table";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";
import { Badge } from "@/components/ui/badge";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { RefreshCw, ShieldCheck, Trash2 } from "lucide-vue-next";
import RefreshButton from "@/components/RefreshButton.vue";
import DocsLinkButton from "@/components/DocsLinkButton.vue";
import { toast } from "@admin-shared/utils/toast";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import PagedTableFooter from "@admin-shared/components/list/PagedTableFooter.vue";
import HumanFriendlyTime from "@admin-shared/components/common/HumanFriendlyTime.vue";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { useLocalPagedList } from "@admin-shared/composables/useLocalPagedList";
import { useDelayedLoading } from "@admin-shared/composables/useDelayedLoading";
import { isValidCIDR } from "@admin-shared/utils/cidr";
import { docsUrls } from "../lib/docs";

import { WhitelistAPI, type WhiteListRecord } from "../lib/api";

const { t } = useI18n();
const records = ref<WhiteListRecord[]>([]);
const isInitializing = ref(true);
const showInitializingSkeleton = useDelayedLoading(isInitializing);
const pageDescription = computed(
  () => t("admin.ipWhitelist.pageDescription"),
);

const removingId = ref<string | null>(null);
const refreshingId = ref<string | null>(null);
const { run: runRemoveRecord } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.ipWhitelist.networkDeleteTitle"), {
      description: extractErrorMessage(
        error,
        t("admin.ipWhitelist.deleteFailed"),
      ),
    });
  },
});
const { run: runSaveComment } = useAsyncAction({
  rethrow: true,
});
const { run: runRefreshRecord } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.ipWhitelist.networkRefreshTitle"), {
      description: extractErrorMessage(
        error,
        t("admin.ipWhitelist.refreshFailed"),
      ),
    });
  },
});
const { isPending: loading, run: runFetchRecords } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.ipWhitelist.networkLoadTitle"), {
      description: extractErrorMessage(
        error,
        t("admin.ipWhitelist.loadFailed"),
      ),
    });
  },
});

// Add dialog states
const showAddDialog = ref(false);
const { isPending: isSaving, run: runAddRecord } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.ipWhitelist.networkAddTitle"), {
      description: extractErrorMessage(
        error,
        t("admin.ipWhitelist.addFailed"),
      ),
    });
  },
});
const durationSetting = ref("permanent");
const customHours = ref(24);
const newRecord = ref({
  ip: "",
  targetType: "ip" as "ip" | "cidr" | "cname",
  checkIntervalMinutes: 5,
  comment: "",
});
const newRecordPlaceholder = computed(() =>
  newRecord.value.targetType === "cidr"
    ? t("admin.ipWhitelist.placeholderCidr")
    : newRecord.value.targetType === "cname"
      ? t("admin.ipWhitelist.placeholderCname")
      : t("admin.ipWhitelist.placeholderIp"),
);

const fetchRecords = async () => {
  await runFetchRecords(
    async () => {
      const res = await WhitelistAPI.getRecords();
      if (res.success) {
        records.value = res.data;
      } else {
        toast.error(t("admin.ipWhitelist.getFailed"), {
          description: res.message,
        });
      }
    },
    {
      onFinally: () => {
        isInitializing.value = false;
      },
    },
  );
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
    record.ip.toLowerCase().includes(query) ||
    Boolean(record.comment?.toLowerCase().includes(query)) ||
    Boolean(
      record.resolvedTargets?.some((target) =>
        target.toLowerCase().includes(query),
      ),
    ),
});

const replaceRecord = (nextRecord: WhiteListRecord) => {
  const index = records.value.findIndex((record) => record.id === nextRecord.id);
  if (index < 0) return;
  records.value.splice(index, 1, nextRecord);
};

const getResolveStatusLabel = (record: WhiteListRecord) => {
  switch (record.resolveStatus) {
    case "resolved":
      return t("admin.ipWhitelist.resolveSuccess");
    case "empty":
      return t("admin.ipWhitelist.resolveEmpty");
    case "error":
      return t("admin.ipWhitelist.resolveError");
    default:
      return t("admin.ipWhitelist.resolvePending");
  }
};

const targetTypeBadgeLabel = (type: WhiteListRecord["targetType"]) => {
  if (type === "cidr") return "CIDR";
  if (type === "cname") return "CNAME";
  return "IP";
};

const getResolveStatusVariant = (record: WhiteListRecord) => {
  switch (record.resolveStatus) {
    case "resolved":
      return "default";
    case "empty":
      return "secondary";
    case "error":
      return "destructive";
    default:
      return "outline";
  }
};

const formatRemaining = (expireAt: number) => {
  const now = Math.floor(Date.now() / 1000);
  const diff = expireAt - now;

  if (diff <= 0) return t("admin.ipWhitelist.expired");

  const days = Math.floor(diff / 86400);
  const hours = Math.floor((diff % 86400) / 3600);
  const mins = Math.floor((diff % 3600) / 60);

  const parts = [];
  if (days > 0) parts.push(t("admin.ipWhitelist.days", { count: days }));
  if (hours > 0) parts.push(t("admin.ipWhitelist.hours", { count: hours }));
  if (mins > 0 || (days === 0 && hours === 0)) {
    parts.push(t("admin.ipWhitelist.minutesCount", { count: mins }));
  }

  return t("admin.ipWhitelist.remaining", { value: parts.join("") });
};

// Actions
const addRecord = async () => {
  const ip = newRecord.value.ip.trim();
  if (!ip) return;
  if (newRecord.value.targetType === "cidr" && !isValidCIDR(ip)) {
    toast.error(t("admin.ipWhitelist.invalidCidrTitle"), {
      description: t("admin.ipWhitelist.invalidCidrDescription"),
    });
    return;
  }
  if (
    newRecord.value.targetType === "cname" &&
    (!Number.isFinite(newRecord.value.checkIntervalMinutes) ||
      newRecord.value.checkIntervalMinutes < 1)
  ) {
    toast.error(t("admin.ipWhitelist.invalidIntervalTitle"), {
      description: t("admin.ipWhitelist.invalidIntervalDescription"),
    });
    return;
  }

  let expireAt: number | null = null;
  const now = Math.floor(Date.now() / 1000);

  if (durationSetting.value !== "permanent") {
    let addHours = 0;
    switch (durationSetting.value) {
      case "1h":
        addHours = 1;
        break;
      case "24h":
        addHours = 24;
        break;
      case "7d":
        addHours = 24 * 7;
        break;
      case "custom":
        addHours = customHours.value || 1;
        break;
    }
    expireAt = now + addHours * 3600;
  }

  await runAddRecord(async () => {
    const res = await WhitelistAPI.addRecord({
      ip,
      targetType: newRecord.value.targetType,
      expireAt,
      source: "manual",
      comment: newRecord.value.comment.trim() || undefined,
      checkIntervalMinutes:
        newRecord.value.targetType === "cname"
          ? Math.floor(newRecord.value.checkIntervalMinutes || 5)
          : undefined,
    });

    if (res.success) {
      toast.success(t("admin.ipWhitelist.addSuccess"));
      showAddDialog.value = false;
      newRecord.value = {
        ip: "",
        targetType: "ip",
        checkIntervalMinutes: 5,
        comment: "",
      };
      durationSetting.value = "permanent";
      currentPage.value = 1;
      searchQuery.value = "";
      await fetchRecords();
    } else {
      toast.error(t("admin.ipWhitelist.addFailed"), {
        description: res.message,
      });
    }
  });
};

const removeRecord = async (id: string) => {
  removingId.value = id;
  await runRemoveRecord(
    async () => {
      const res = await WhitelistAPI.deleteRecord(id);
      if (res.success) {
        toast.success(t("admin.ipWhitelist.deleteSuccess"));
        await fetchRecords();
        if (paginatedRecords.value.length === 1 && currentPage.value > 1) {
          currentPage.value--;
        }
      } else {
        toast.error(t("admin.ipWhitelist.deleteFailed"), {
          description: res.message,
        });
      }
    },
    {
      onFinally: () => {
        removingId.value = null;
      },
    },
  );
};

const refreshRecord = async (id: string) => {
  refreshingId.value = id;
  await runRefreshRecord(
    async () => {
      const res = await WhitelistAPI.refreshRecord(id);
      const result = res.data;
      const nextRecord = result?.record;
      if (nextRecord) {
        replaceRecord(nextRecord);
      }

      if (
        !res.success ||
        !result ||
        !nextRecord ||
        nextRecord.resolveStatus === "error"
      ) {
        toast.error(t("admin.ipWhitelist.refreshFailed"), {
          description:
            res.message ||
            nextRecord?.resolveMessage ||
            t("admin.ipWhitelist.refreshFallbackError"),
        });
        return;
      }

      toast.success(t("admin.ipWhitelist.refreshSuccessTitle"), {
        description: result.changed
          ? t("admin.ipWhitelist.refreshChanged")
          : t("admin.ipWhitelist.refreshUnchanged"),
      });
    },
    {
      onFinally: () => {
        refreshingId.value = null;
      },
    },
  );
};

const saveComment = async (id: string, newComment: string) => {
  const record = records.value.find((r) => r.id === id);

  if (record && (record.comment || "") === newComment) {
    return;
  }

  await runSaveComment(() => WhitelistAPI.updateComment(id, newComment), {
    onSuccess: (res) => {
      if (!res.success) {
        throw new Error(res.message || t("admin.ipWhitelist.commentUpdateFailed"));
      }
      if (record) record.comment = newComment;
      toast.success(t("admin.ipWhitelist.commentUpdated"));
    },
    onError: (error) => {
      throw new Error(
        extractErrorMessage(error, t("admin.ipWhitelist.commentUpdateFailed")),
      );
    },
  });
};
</script>
