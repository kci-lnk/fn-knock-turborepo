<script setup lang="ts">
import { computed, onMounted, ref, useId } from "vue";
import { useI18n } from "vue-i18n";
import {
  Breadcrumb,
  BreadcrumbItem,
  BreadcrumbLink,
  BreadcrumbList,
  BreadcrumbPage,
  BreadcrumbSeparator,
} from "@/components/ui/breadcrumb";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import { Skeleton } from "@/components/ui/skeleton";
import { Switch } from "@/components/ui/switch";
import RefreshButton from "@/components/RefreshButton.vue";
import { toast } from "@admin-shared/utils/toast";
import { extractErrorMessage } from "@admin-shared/composables/useAsyncAction";
import { SystemAPI } from "../../lib/api";
import type {
  FnosCertificateSyncDetails,
  FnosCertificateSyncItem,
  FnosCertificateSyncStatus,
} from "../../types";

const a11yId = useId();

const { t, locale } = useI18n();
const details = ref<FnosCertificateSyncDetails | null>(null);
const loading = ref(false);
const saving = ref(false);
const syncingIds = ref<string[]>([]);

const busy = computed(
  () =>
    saving.value ||
    syncingIds.value.length > 0 ||
    details.value?.runtime.running,
);
const available = computed(
  () => details.value?.availability.available === true,
);
const syncableItems = computed(
  () =>
    details.value?.certificates.filter((item) =>
      ["syncable", "sync_failed"].includes(item.status),
    ) ?? [],
);

const load = async () => {
  loading.value = true;
  try {
    details.value = await SystemAPI.getFnosCertificateSyncDetails();
  } catch (error) {
    toast.error(t("admin.fnosCertificateSync.loadFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.fnosCertificateSync.loadFailed"),
      ),
    });
  } finally {
    loading.value = false;
  }
};

const updateAutoSync = async (enabled: boolean) => {
  if (saving.value) return;
  const previous = details.value?.config.auto_sync_enabled ?? false;
  if (details.value) details.value.config.auto_sync_enabled = enabled;
  saving.value = true;
  try {
    details.value = await SystemAPI.updateFnosCertificateSyncConfig(enabled);
    toast.success(t("admin.fnosCertificateSync.autoSyncUpdated"));
  } catch (error) {
    if (details.value) details.value.config.auto_sync_enabled = previous;
    toast.error(t("admin.fnosCertificateSync.saveFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.fnosCertificateSync.saveFailed"),
      ),
    });
  } finally {
    saving.value = false;
  }
};

const sync = async (ids: string[]) => {
  if (busy.value) return;
  syncingIds.value =
    ids.length > 0 ? ids : syncableItems.value.map((item) => item.target_id);
  try {
    const result = await SystemAPI.syncFnosCertificates(ids);
    details.value = result.details;
    toast.success(
      t("admin.fnosCertificateSync.syncCompleted", {
        synced: result.summary.synced,
        skipped: result.summary.skipped,
      }),
    );
  } catch (error) {
    toast.error(t("admin.fnosCertificateSync.syncFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.fnosCertificateSync.syncFailed"),
      ),
    });
    await load();
  } finally {
    syncingIds.value = [];
  }
};

const statusLabel = (status: FnosCertificateSyncStatus) =>
  t(`admin.fnosCertificateSync.status.${status}`);

const statusVariant = (status: FnosCertificateSyncStatus) => {
  if (status === "up_to_date") return "default";
  if (status === "syncable" || status === "sync_failed") return "secondary";
  if (status === "target_invalid" || status === "source_invalid")
    return "destructive";
  return "outline";
};

const formatDate = (value: number | null | undefined) => {
  if (!value) return "--";
  return new Intl.DateTimeFormat(locale.value, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(new Date(value));
};

const compactFingerprint = (value: string | null | undefined) => {
  if (!value) return "--";
  return value.length > 23 ? `${value.slice(0, 17)}…${value.slice(-5)}` : value;
};

const isItemSyncing = (item: FnosCertificateSyncItem) =>
  syncingIds.value.includes(item.target_id);

onMounted(load);
</script>

<template>
  <div class="space-y-5">
    <Breadcrumb>
      <BreadcrumbList>
        <BreadcrumbItem>
          <BreadcrumbLink href="#/system">{{
            t("admin.fnosCertificateSync.systemSettings")
          }}</BreadcrumbLink>
        </BreadcrumbItem>
        <BreadcrumbSeparator />
        <BreadcrumbItem>
          <BreadcrumbLink href="#/system?tab=fnos">{{
            t("admin.fnosCertificateSync.fnos")
          }}</BreadcrumbLink>
        </BreadcrumbItem>
        <BreadcrumbSeparator />
        <BreadcrumbItem
          ><BreadcrumbPage>{{
            t("admin.fnosCertificateSync.title")
          }}</BreadcrumbPage></BreadcrumbItem
        >
      </BreadcrumbList>
    </Breadcrumb>

    <Card class="border-border/50 shadow-none">
      <CardHeader>
        <div
          class="flex flex-col gap-4 sm:flex-row sm:items-start sm:justify-between"
        >
          <div class="space-y-1.5">
            <CardTitle class="text-xl tracking-tight">{{
              t("admin.fnosCertificateSync.title")
            }}</CardTitle>
            <CardDescription class="max-w-3xl leading-6">{{
              t("admin.fnosCertificateSync.description")
            }}</CardDescription>
          </div>
          <RefreshButton :loading="loading" :disabled="busy" @click="load" />
        </div>
      </CardHeader>
      <CardContent class="space-y-5">
        <div
          class="rounded-xl border border-amber-200 bg-amber-50 px-4 py-3 text-sm leading-6 text-amber-900"
        >
          {{ t("admin.fnosCertificateSync.noInsertNotice") }}
        </div>

        <template v-if="loading && !details">
          <Skeleton class="h-24 w-full rounded-xl" />
          <Skeleton class="h-64 w-full rounded-xl" />
        </template>

        <template v-else-if="details">
          <div
            v-if="!available"
            class="rounded-xl border border-destructive/25 bg-destructive/5 px-4 py-3 text-sm leading-6 text-destructive"
          >
            {{ t("admin.fnosCertificateSync.unavailable") }}
            <span v-if="details.availability.reason">
              · {{ details.availability.reason }}
            </span>
          </div>

          <div
            class="flex flex-col gap-4 rounded-xl border border-border/60 bg-muted/10 p-5 sm:flex-row sm:items-center sm:justify-between"
          >
            <div class="space-y-1">
              <Label
                :for="`${a11yId}-fnoscertificatesyncsettings-1`"
                class="text-base"
                >{{ t("admin.fnosCertificateSync.autoSync") }}</Label
              >
              <p class="text-sm text-muted-foreground">
                {{ t("admin.fnosCertificateSync.autoSyncDescription") }}
              </p>
              <p class="text-xs text-muted-foreground">
                {{
                  t("admin.fnosCertificateSync.lastSync", {
                    time: formatDate(details.runtime.last_sync_at),
                  })
                }}
                <span
                  v-if="details.runtime.last_error"
                  class="ml-2 text-destructive"
                  >{{ details.runtime.last_error }}</span
                >
              </p>
            </div>
            <Switch
              :id="`${a11yId}-fnoscertificatesyncsettings-1`"
              :model-value="details.config.auto_sync_enabled"
              :disabled="busy || !available"
              @update:model-value="updateAutoSync($event === true)"
            />
          </div>

          <div
            class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between"
          >
            <p class="text-sm text-muted-foreground">
              {{ t("admin.fnosCertificateSync.summary", details.summary) }}
            </p>
            <Button
              :disabled="busy || !available || syncableItems.length === 0"
              @click="sync([])"
            >
              {{
                t("admin.fnosCertificateSync.syncAll", {
                  count: syncableItems.length,
                })
              }}
            </Button>
          </div>

          <div class="overflow-x-auto rounded-xl border border-border/60">
            <table class="w-full min-w-[900px] text-sm">
              <thead
                class="bg-muted/40 text-left text-xs text-muted-foreground"
              >
                <tr>
                  <th class="px-4 py-3 font-medium">
                    {{ t("admin.fnosCertificateSync.columns.target") }}
                  </th>
                  <th class="px-4 py-3 font-medium">
                    {{ t("admin.fnosCertificateSync.columns.validity") }}
                  </th>
                  <th class="px-4 py-3 font-medium">
                    {{ t("admin.fnosCertificateSync.columns.local") }}
                  </th>
                  <th class="px-4 py-3 font-medium">
                    {{ t("admin.fnosCertificateSync.columns.status") }}
                  </th>
                  <th class="px-4 py-3 text-right font-medium">
                    {{ t("admin.fnosCertificateSync.columns.action") }}
                  </th>
                </tr>
              </thead>
              <tbody class="divide-y">
                <tr
                  v-for="item in details.certificates"
                  :key="item.target_id"
                  class="align-top"
                >
                  <td class="space-y-1 px-4 py-4">
                    <div class="font-medium">{{ item.domain }}</div>
                    <div
                      class="max-w-xs break-all text-xs text-muted-foreground"
                    >
                      {{ item.san.join(", ") }}
                    </div>
                    <div class="text-xs text-muted-foreground">
                      {{ item.source }} ·
                      {{ compactFingerprint(item.fingerprint) }}
                    </div>
                    <div v-if="item.renewal" class="text-xs text-amber-600">
                      {{ t("admin.fnosCertificateSync.renewalWarning") }}
                    </div>
                  </td>
                  <td class="px-4 py-4 text-xs leading-5 text-muted-foreground">
                    <div>{{ formatDate(item.valid_from) }}</div>
                    <div>{{ formatDate(item.valid_to) }}</div>
                  </td>
                  <td class="space-y-1 px-4 py-4">
                    <template v-if="item.local">
                      <div class="font-medium">
                        {{ item.local.label || item.local.id }}
                      </div>
                      <div class="text-xs text-muted-foreground">
                        {{ formatDate(item.local.valid_to) }}
                      </div>
                      <div class="text-xs text-muted-foreground">
                        {{ compactFingerprint(item.local.fingerprint) }}
                      </div>
                    </template>
                    <span v-else class="text-muted-foreground">--</span>
                  </td>
                  <td class="space-y-1 px-4 py-4">
                    <Badge :variant="statusVariant(item.status)">{{
                      statusLabel(item.status)
                    }}</Badge>
                    <p
                      v-if="item.reason"
                      class="max-w-xs text-xs leading-5 text-destructive"
                    >
                      {{ item.reason }}
                    </p>
                  </td>
                  <td class="px-4 py-4 text-right">
                    <Button
                      size="sm"
                      variant="outline"
                      :disabled="
                        busy ||
                        !available ||
                        !['syncable', 'sync_failed'].includes(item.status)
                      "
                      @click="sync([item.target_id])"
                    >
                      {{
                        isItemSyncing(item)
                          ? t("admin.fnosCertificateSync.syncing")
                          : t("admin.fnosCertificateSync.syncOne")
                      }}
                    </Button>
                  </td>
                </tr>
                <tr v-if="details.certificates.length === 0">
                  <td
                    colspan="5"
                    class="px-4 py-10 text-center text-muted-foreground"
                  >
                    {{ t("admin.fnosCertificateSync.empty") }}
                  </td>
                </tr>
              </tbody>
            </table>
          </div>
        </template>
      </CardContent>
    </Card>
  </div>
</template>
