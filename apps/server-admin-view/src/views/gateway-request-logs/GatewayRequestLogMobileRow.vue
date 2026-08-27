<script setup lang="ts">
import { useI18n } from "vue-i18n";
import {
  Ban,
  Eye,
  Route,
  ShieldAlert,
  ShieldCheck,
  ShieldX,
  Unlock,
} from "lucide-vue-next";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import HumanFriendlyTime from "@admin-shared/components/common/HumanFriendlyTime.vue";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import type { GatewayRequestLogRowProps } from "./gateway-request-log-row-contract";
import {
  authDecisionLabel as resolveAuthDecisionLabel,
  formatDuration,
  getEntryClientIp,
  getStatusDotClass,
  getStatusTextClass,
  getWAFAction,
  getWAFBadgeClass,
  hasWAFSignal,
  isWAFBlocked,
  routeTypeLabel as resolveRouteTypeLabel,
  wafBadgeLabel as resolveWAFBadgeLabel,
  wafBadgeMeta as resolveWAFBadgeMeta,
  wafBadgeTitle as resolveWAFBadgeTitle,
} from "./model";

defineProps<GatewayRequestLogRowProps>();
const { t, locale } = useI18n();
const wafBadgeLabel = (entry: GatewayRequestLogRowProps["entry"]) =>
  resolveWAFBadgeLabel(entry, t);
const wafBadgeMeta = (entry: GatewayRequestLogRowProps["entry"]) =>
  resolveWAFBadgeMeta(entry, t);
const wafBadgeTitle = (entry: GatewayRequestLogRowProps["entry"]) =>
  resolveWAFBadgeTitle(entry, t);
const routeTypeLabel = (value?: string) => resolveRouteTypeLabel(value, t);
const authDecisionLabel = (value?: string) =>
  resolveAuthDecisionLabel(value, t);
</script>

<template>
  <article class="space-y-3 px-3 py-3">
    <div class="flex items-start gap-2.5">
      <Checkbox
        class="mt-0.5"
        :model-value="isSelected"
        :aria-label="
          t('common.selectItem', {
            item: entry.actionIp || entry.selectionKey,
          })
        "
        :disabled="!entry.actionIp"
        @update:model-value="toggleSelection(entry.selectionKey)"
      />
      <div class="min-w-0 flex-1 space-y-1.5">
        <div class="flex min-w-0 items-center gap-2">
          <span
            class="inline-flex h-5 shrink-0 items-center rounded-full bg-muted px-2 text-[10px] font-medium leading-none text-muted-foreground"
          >
            <HumanFriendlyTime :value="entry.time" :locale="locale" />
          </span>
          <span
            class="shrink-0 font-mono text-[10px] tracking-[0.1em] text-muted-foreground"
          >
            {{ entry.method || "-" }}
          </span>
          <span
            class="ml-auto inline-flex shrink-0 items-center gap-1.5 font-mono text-xs"
            :class="getStatusTextClass(entry.status)"
          >
            <span
              class="h-1.5 w-1.5 rounded-full"
              :class="getStatusDotClass(entry.status)"
            ></span>
            {{ entry.status }}
          </span>
        </div>
        <p class="truncate text-sm font-medium" :title="entry.host">
          {{ entry.host || "-" }}
        </p>
        <p
          class="break-all font-mono text-[11px] leading-4 text-muted-foreground"
        >
          {{ entry.request_uri || entry.path || "-" }}
        </p>
        <button
          v-if="hasWAFSignal(entry)"
          type="button"
          class="inline-flex max-w-full items-center gap-1 rounded-full border px-1.5 py-px text-[10px] font-normal leading-4 transition-colors disabled:cursor-default disabled:opacity-70"
          :class="getWAFBadgeClass(entry)"
          :title="wafBadgeTitle(entry)"
          :disabled="!entry.waf_trace_id"
          @click.stop="goToWafTrace(entry.waf_trace_id)"
        >
          <ShieldX v-if="isWAFBlocked(entry)" class="h-2.5 w-2.5 shrink-0" />
          <ShieldCheck
            v-else-if="getWAFAction(entry) === 'pass'"
            class="h-2.5 w-2.5 shrink-0"
          />
          <ShieldAlert v-else class="h-2.5 w-2.5 shrink-0" />
          <span class="shrink-0">{{ wafBadgeLabel(entry) }}</span>
          <span class="truncate font-mono">{{ wafBadgeMeta(entry) }}</span>
        </button>
      </div>
    </div>

    <dl class="grid grid-cols-2 gap-x-3 gap-y-2 text-xs">
      <div class="min-w-0">
        <dt class="text-[10px] text-muted-foreground">
          {{ t("admin.gatewayRequestLogs.columns.clientIp") }}
        </dt>
        <dd class="truncate font-mono" :title="getEntryClientIp(entry)">
          {{ getEntryClientIp(entry) || "-" }}
        </dd>
        <dd
          v-if="getEntryIpLocationText(entry)"
          class="truncate text-[10px] text-muted-foreground"
          :title="getEntryIpLocationText(entry)"
        >
          {{ getEntryIpLocationText(entry) }}
        </dd>
      </div>
      <div class="min-w-0">
        <dt class="text-[10px] text-muted-foreground">
          {{ t("admin.gatewayRequestLogs.columns.login") }}
        </dt>
        <dd class="truncate">
          {{
            entry.logged_in
              ? t("admin.gatewayRequestLogs.loggedIn")
              : t("admin.gatewayRequestLogs.notLoggedIn")
          }}
        </dd>
        <dd
          class="truncate text-[10px] text-muted-foreground"
          :title="authDecisionLabel(entry.auth_decision)"
        >
          {{ authDecisionLabel(entry.auth_decision) }}
        </dd>
      </div>
      <div class="min-w-0">
        <dt class="text-[10px] text-muted-foreground">
          {{ t("admin.gatewayRequestLogs.columns.route") }}
        </dt>
        <dd class="truncate" :title="routeTypeLabel(entry.route_type)">
          {{ routeTypeLabel(entry.route_type) }}
        </dd>
        <dd
          class="truncate text-[10px] text-muted-foreground"
          :title="entry.route_key || '-'"
        >
          {{ entry.route_key || "-" }}
        </dd>
      </div>
      <div class="min-w-0">
        <dt class="text-[10px] text-muted-foreground">
          {{ t("admin.gatewayRequestLogs.columns.duration") }}
        </dt>
        <dd class="font-mono">{{ formatDuration(entry.duration_ms) }}</dd>
      </div>
    </dl>

    <div class="flex items-center justify-end gap-1 border-t pt-2">
      <Button
        variant="ghost"
        size="sm"
        class="h-8 px-2.5 text-xs text-muted-foreground hover:text-foreground"
        :disabled="!entry.trace_id && !entry.waf_trace_id"
        @click="goToWafTrace(entry.trace_id || entry.waf_trace_id)"
      >
        <Route class="mr-1.5 h-3.5 w-3.5" />
        {{ t("admin.trace.label") }}
      </Button>
      <ConfirmDangerPopover
        :title="
          isGeneralBlacklisted(entry.actionIp)
            ? t('admin.gatewayRequestLogs.unblacklistOneTitle')
            : t('admin.gatewayRequestLogs.blacklistOneTitle')
        "
        :description="
          isGeneralBlacklisted(entry.actionIp)
            ? t('admin.gatewayRequestLogs.unblacklistOneDescription', {
                ip: entry.actionIp || '-',
              })
            : t('admin.gatewayRequestLogs.blacklistOneDescription', {
                ip: entry.actionIp || '-',
              })
        "
        :loading="isMutatingBlacklistIps"
        :disabled="!entry.actionIp || isMutatingBlacklistIps"
        :on-confirm="
          () =>
            isGeneralBlacklisted(entry.actionIp)
              ? releaseIpsFromLogs([entry.actionIp])
              : blockIpsFromLogs([entry.actionIp])
        "
      >
        <template #trigger>
          <Button
            variant="ghost"
            size="sm"
            class="h-8 px-2.5 text-xs"
            :class="
              isGeneralBlacklisted(entry.actionIp)
                ? 'text-foreground hover:text-foreground'
                : 'text-destructive hover:text-destructive'
            "
            :disabled="!entry.actionIp || isMutatingBlacklistIps"
          >
            <Unlock
              v-if="isGeneralBlacklisted(entry.actionIp)"
              class="mr-1.5 h-3.5 w-3.5"
            />
            <Ban v-else class="mr-1.5 h-3.5 w-3.5" />
            {{
              isGeneralBlacklisted(entry.actionIp)
                ? t("admin.gatewayRequestLogs.unblacklistOne")
                : t("admin.gatewayRequestLogs.blacklistOne")
            }}
          </Button>
        </template>
      </ConfirmDangerPopover>
      <Button
        variant="ghost"
        size="sm"
        class="h-8 px-2.5 text-xs text-muted-foreground hover:text-foreground"
        @click="viewDetails(entry)"
      >
        <Eye class="mr-1.5 h-3.5 w-3.5" />
        {{ t("common.viewDetails") }}
      </Button>
    </div>
  </article>
</template>
