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
                    {{ targetTypeBadgeLabel(record.targetType) }}
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

      <div
        v-if="!isInitializing && regionGroups.length > 0"
        class="mt-6 rounded-md border"
      >
        <div
          class="flex flex-wrap items-start justify-between gap-3 border-b px-4 py-3"
        >
          <div class="min-w-0 space-y-1">
            <h3 class="text-sm font-medium">
              {{ t("admin.ipWhitelist.regionGroupsTitle") }}
            </h3>
            <p class="text-sm text-muted-foreground">
              {{ t("admin.ipWhitelist.regionGroupsDescription") }}
            </p>
          </div>
          <Badge variant="secondary">
            {{
              t("admin.ipWhitelist.regionGroupsCount", {
                count: regionGroups.length,
              })
            }}
          </Badge>
        </div>

        <div class="divide-y">
          <div
            v-for="group in regionGroups"
            :key="group.id"
            class="flex flex-wrap items-start justify-between gap-4 px-4 py-4"
          >
            <div class="min-w-0 flex-1 space-y-2">
              <div class="flex flex-wrap gap-2">
                <Badge
                  v-for="region in group.regions"
                  :key="`${group.id}:${formatRegionInput(region)}`"
                  variant="outline"
                  class="max-w-full whitespace-normal text-left font-normal"
                >
                  {{ formatRegionInput(region) }}
                </Badge>
              </div>
              <div
                class="flex flex-wrap items-center gap-x-4 gap-y-1 text-xs text-muted-foreground"
              >
                <span>
                  {{
                    t("admin.ipWhitelist.regionGroupCidrCount", {
                      count: group.cidrCount,
                    })
                  }}
                </span>
                <span v-if="group.expireAt">
                  {{ formatRemaining(group.expireAt) }}
                </span>
                <span v-else class="text-green-600">
                  {{ t("admin.ipWhitelist.permanent") }}
                </span>
                <span>
                  {{ t("admin.ipWhitelist.createdAt") }}
                  <HumanFriendlyTime :value="group.createdAt * 1000" />
                </span>
              </div>
              <p v-if="group.comment" class="text-sm text-muted-foreground">
                {{ group.comment }}
              </p>
            </div>

            <ConfirmDangerPopover
              :title="t('admin.ipWhitelist.regionGroupDeleteTitle')"
              :description="
                t('admin.ipWhitelist.regionGroupDeleteDescription', {
                  target: regionGroupLabel(group),
                })
              "
              :loading="removingRegionGroupId === group.id"
              :disabled="removingRegionGroupId === group.id"
              :on-confirm="() => removeRegionGroup(group.id)"
              content-class="w-64 text-left"
            >
              <template #trigger>
                <Button
                  variant="ghost"
                  size="icon"
                  class="h-8 w-8 text-destructive hover:bg-destructive/10 hover:text-destructive"
                  :disabled="removingRegionGroupId === group.id"
                >
                  <Trash2 class="h-4 w-4" />
                </Button>
              </template>
            </ConfirmDangerPopover>
          </div>
        </div>
      </div>
    </CardContent>
  </Card>

  <Dialog v-model:open="showAddDialog">
    <DialogContent class="sm:max-w-[640px]">
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

        <div
          v-if="newRecord.targetType === 'cidr'"
          class="grid grid-cols-4 items-center gap-4"
        >
          <Label class="text-right">{{
            t("admin.ipWhitelist.cidrInputMode")
          }}</Label>
          <div
            class="col-span-3 inline-flex w-fit rounded-md border border-border bg-muted/20 p-1"
          >
            <Button
              type="button"
              size="sm"
              :variant="cidrInputMode === 'manual' ? 'default' : 'ghost'"
              class="h-8"
              @click="cidrInputMode = 'manual'"
            >
              {{ t("admin.ipWhitelist.cidrInputManual") }}
            </Button>
            <Button
              type="button"
              size="sm"
              :variant="cidrInputMode === 'region' ? 'default' : 'ghost'"
              class="h-8"
              @click="cidrInputMode = 'region'"
            >
              {{ t("admin.ipWhitelist.cidrInputRegion") }}
            </Button>
          </div>
        </div>

        <div
          v-if="!isRegionCidrMode"
          class="grid grid-cols-4 items-center gap-4"
        >
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

        <div v-else class="grid grid-cols-4 items-start gap-4">
          <Label class="pt-2 text-right">{{
            t("admin.ipWhitelist.regionScope")
          }}</Label>
          <div class="col-span-3 space-y-3">
            <Alert variant="destructive" class="items-start">
              <AlertTriangle class="h-4 w-4" />
              <AlertTitle>
                {{ t("admin.ipWhitelist.regionSecurityWarningTitle") }}
              </AlertTitle>
              <AlertDescription>
                {{ t("admin.ipWhitelist.regionSecurityWarningDescription") }}
              </AlertDescription>
            </Alert>
            <CidrRegionSelector
              v-model="whitelistRegionSelections"
              :disabled="regionInputsDisabled"
              :description="t('admin.ipWhitelist.regionScopeDescription')"
              :text="{
                add: t('admin.ipWhitelist.add'),
                addRegion: t('admin.ipWhitelist.addRegion'),
                cancel: t('common.cancel'),
                dialogDescription: t('admin.ipWhitelist.addRegionDescription'),
                loadFailed: t('admin.ipWhitelist.regionsLoadFailed'),
                loadFailedDescription: t(
                  'admin.ipWhitelist.regionsLoadDescription',
                ),
                loading: t('admin.ipWhitelist.loading'),
                noRegions: t('admin.ipWhitelist.noRegions'),
                province: t('admin.ipWhitelist.province'),
                retry: t('admin.subdomainProxy.retry'),
                scope: t('admin.ipWhitelist.scope'),
                selectCity: t('admin.ipWhitelist.selectCity'),
                selectCityOrProvince: t(
                  'admin.ipWhitelist.selectCityOrProvince',
                ),
                selectProvince: t('admin.ipWhitelist.selectProvince'),
                selectProvinceFirst: t('admin.ipWhitelist.selectProvinceFirst'),
              }"
            />
          </div>
        </div>

        <div
          v-if="newRecord.targetType === 'cname'"
          class="grid grid-cols-4 items-center gap-4"
        >
          <Label for="checkIntervalMinutes" class="text-right">{{
            t("admin.ipWhitelist.checkIntervalLabel")
          }}</Label>
          <div class="col-span-3 flex items-center gap-2">
            <Input
              id="checkIntervalMinutes"
              type="number"
              min="1"
              v-model.number="newRecord.checkIntervalMinutes"
              :placeholder="t('admin.ipWhitelist.defaultFive')"
            />
            <span class="text-sm text-muted-foreground whitespace-nowrap">{{
              t("admin.ipWhitelist.minutes")
            }}</span>
          </div>
        </div>

        <div class="grid grid-cols-4 items-center gap-4">
          <Label for="duration" class="text-right">{{
            t("admin.ipWhitelist.duration")
          }}</Label>
          <Select v-model="durationSetting">
            <SelectTrigger class="col-span-3">
              <SelectValue
                :placeholder="t('admin.ipWhitelist.selectDuration')"
              />
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
        <Button @click="addRecord" :disabled="!canSaveNewRecord || isSaving">{{
          t("common.save")
        }}</Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
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
import { AlertTriangle, RefreshCw, ShieldCheck, Trash2 } from "lucide-vue-next";
import CidrRegionSelector from "@/components/CidrRegionSelector.vue";
import RefreshButton from "@/components/RefreshButton.vue";
import DocsLinkButton from "@/components/DocsLinkButton.vue";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import PagedTableFooter from "@admin-shared/components/list/PagedTableFooter.vue";
import HumanFriendlyTime from "@admin-shared/components/common/HumanFriendlyTime.vue";
import { useLocalPagedList } from "@admin-shared/composables/useLocalPagedList";
import { useDelayedLoading } from "@admin-shared/composables/useDelayedLoading";
import { docsUrls } from "../lib/docs";
import { useWhitelistAddRecord } from "./ip-whitelist/useWhitelistAddRecord";
import { useWhitelistRecordActions } from "./ip-whitelist/useWhitelistRecordActions";
import { useWhitelistRecords } from "./ip-whitelist/useWhitelistRecords";

import {
  type WhiteListRecord,
  type WhitelistRegionGroupRecord,
  type WhitelistRegionInput,
} from "../lib/api";

const { t } = useI18n();
const { fetchRecords, isInitializing, loading, records, regionGroups } =
  useWhitelistRecords((key) => t(key));
const showInitializingSkeleton = useDelayedLoading(isInitializing);
const pageDescription = computed(() => t("admin.ipWhitelist.pageDescription"));

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
const {
  refreshRecord,
  refreshingId,
  removeRecord,
  removeRegionGroup,
  removingId,
  removingRegionGroupId,
  saveComment,
} = useWhitelistRecordActions({
  currentPage,
  fetchRecords,
  paginatedRecords,
  records,
  translate: (key, params) => (params ? t(key, params) : t(key)),
});
const {
  addRecord,
  canSaveNewRecord,
  cidrInputMode,
  customHours,
  durationSetting,
  isRegionCidrMode,
  isSaving,
  newRecord,
  newRecordPlaceholder,
  regionInputsDisabled,
  showAddDialog,
  whitelistRegionSelections,
} = useWhitelistAddRecord({
  currentPage,
  fetchRecords,
  searchQuery,
  translate: (key, params) => (params ? t(key, params) : t(key)),
});

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

const formatRegionInput = (region: WhitelistRegionInput) =>
  region.query_city
    ? `${region.province} / ${region.query_city}`
    : region.province;

const regionGroupLabel = (group: WhitelistRegionGroupRecord) =>
  group.regions.map(formatRegionInput).join(", ");
</script>
