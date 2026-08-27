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
import { TableCell, TableRow } from "@/components/ui/table";
import type { GatewayRequestLogRowProps } from "./gateway-request-log-row-contract";
import {
  authDecisionLabel as resolveAuthDecisionLabel,
  formatAuthCredential as resolveAuthCredentialLabel,
  formatDuration,
  getEntryClientIp,
  getForwardedHeaderLines,
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
const authCredentialLabel = (entry: GatewayRequestLogRowProps["entry"]) =>
  resolveAuthCredentialLabel(entry, t);
</script>

<template>
  <TableRow class="group align-top">
    <TableCell class="py-2.5">
      <Checkbox
        :model-value="isSelected"
        :aria-label="
          t('common.selectItem', {
            item: entry.actionIp || entry.selectionKey,
          })
        "
        :disabled="!entry.actionIp"
        @update:model-value="toggleSelection(entry.selectionKey)"
      />
    </TableCell>
    <TableCell
      class="w-[320px] min-w-[320px] max-w-[320px] whitespace-normal py-2.5"
    >
      <div class="space-y-1.5">
        <div class="flex items-center gap-2">
          <div
            class="inline-flex h-5 shrink-0 items-center rounded-full bg-muted px-2 text-[11px] font-medium leading-none text-muted-foreground"
          >
            <HumanFriendlyTime :value="entry.time" :locale="locale" />
          </div>
          <div class="min-w-0 flex-1">
            <div class="flex items-center gap-2 text-sm text-foreground">
              <span
                class="font-mono text-[11px] tracking-[0.12em] text-muted-foreground"
              >
                {{ entry.method || "-" }}
              </span>
              <span class="min-w-0 flex-1 truncate">{{
                entry.host || "-"
              }}</span>
            </div>
          </div>
        </div>
        <div
          class="whitespace-normal break-all font-mono text-[11px] leading-5 text-muted-foreground"
        >
          {{ entry.request_uri || entry.path || "-" }}
        </div>
        <div
          v-if="entry.upstream"
          class="whitespace-normal break-all text-[11px] text-muted-foreground/75"
        >
          {{ entry.upstream }}
        </div>
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
    </TableCell>
    <TableCell class="w-[72px] min-w-[72px] py-2.5">
      <div
        class="flex items-center gap-2 font-mono text-sm"
        :class="getStatusTextClass(entry.status)"
      >
        <span
          class="h-1.5 w-1.5 rounded-full"
          :class="getStatusDotClass(entry.status)"
        ></span>
        <span>{{ entry.status }}</span>
      </div>
    </TableCell>
    <TableCell class="w-[150px] min-w-[150px] max-w-[150px] py-2.5">
      <div class="truncate text-sm text-foreground">
        {{
          entry.logged_in
            ? t("admin.gatewayRequestLogs.loggedIn")
            : t("admin.gatewayRequestLogs.notLoggedIn")
        }}
      </div>
      <div class="truncate text-[11px] text-muted-foreground">
        {{ authDecisionLabel(entry.auth_decision) }}
      </div>
      <div
        v-if="authCredentialLabel(entry)"
        class="max-w-full truncate text-[11px] text-muted-foreground/75"
        :title="authCredentialLabel(entry)"
      >
        {{ authCredentialLabel(entry) }}
      </div>
    </TableCell>
    <TableCell class="min-w-[140px] py-2.5">
      <div class="font-mono text-sm text-foreground">
        {{ getEntryClientIp(entry) || "-" }}
      </div>
      <div
        v-if="getConnectionSourceText(entry)"
        class="break-all text-[10px] text-muted-foreground/75"
      >
        {{ getConnectionSourceText(entry) }}
      </div>
      <div
        v-if="getEntryIpLocationText(entry)"
        class="text-[11px] text-muted-foreground"
      >
        {{ getEntryIpLocationText(entry) }}
      </div>
      <div
        v-for="headerLine in getForwardedHeaderLines(entry)"
        :key="headerLine"
        class="break-all text-[10px] text-muted-foreground/75"
      >
        {{ headerLine }}
      </div>
    </TableCell>
    <TableCell
      class="w-[220px] min-w-[160px] max-w-[220px] overflow-hidden py-2.5"
    >
      <div class="w-full max-w-[204px] overflow-hidden">
        <div
          class="truncate text-sm text-foreground"
          :title="routeTypeLabel(entry.route_type)"
        >
          {{ routeTypeLabel(entry.route_type) }}
        </div>
        <div
          class="truncate text-[11px] text-muted-foreground"
          :title="entry.route_key || '-'"
        >
          {{ entry.route_key || "-" }}
        </div>
      </div>
    </TableCell>
    <TableCell
      class="whitespace-nowrap py-2.5 font-mono text-sm text-muted-foreground"
    >
      {{ formatDuration(entry.duration_ms) }}
    </TableCell>
    <TableCell class="sticky right-0 z-10 bg-background py-2.5 pr-4 text-right">
      <div class="flex justify-end gap-1">
        <Button
          variant="ghost"
          size="icon"
          class="h-8 w-8 text-muted-foreground hover:text-foreground"
          :aria-label="t('admin.trace.open')"
          :disabled="!entry.trace_id && !entry.waf_trace_id"
          @click="goToWafTrace(entry.trace_id || entry.waf_trace_id)"
        >
          <Route class="h-4 w-4" />
        </Button>
        <div
          class="opacity-60 transition-opacity group-hover:opacity-100 group-focus-within:opacity-100"
        >
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
                size="icon"
                class="h-8 w-8"
                :class="
                  isGeneralBlacklisted(entry.actionIp)
                    ? 'text-foreground hover:text-foreground'
                    : 'text-destructive hover:text-destructive'
                "
                :disabled="!entry.actionIp || isMutatingBlacklistIps"
                :aria-label="
                  isGeneralBlacklisted(entry.actionIp)
                    ? t('admin.gatewayRequestLogs.unblacklistOne')
                    : t('admin.gatewayRequestLogs.blacklistOne')
                "
              >
                <Unlock
                  v-if="isGeneralBlacklisted(entry.actionIp)"
                  class="h-4 w-4"
                />
                <Ban v-else class="h-4 w-4" />
              </Button>
            </template>
          </ConfirmDangerPopover>
        </div>
        <Button
          variant="ghost"
          size="icon"
          class="h-8 w-8 text-muted-foreground hover:text-foreground"
          :aria-label="t('common.viewDetails')"
          @click="viewDetails(entry)"
        >
          <Eye class="h-4 w-4" />
        </Button>
      </div>
    </TableCell>
  </TableRow>
</template>
