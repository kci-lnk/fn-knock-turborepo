<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { ArrowUpRight } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import type { GatewayLogIpGroups } from "@/lib/api/gateway";
import { formatLogClock } from "./ip-view-model";

defineProps<{
  data: GatewayLogIpGroups | null;
  loading: boolean;
  sort: string;
  location: (ip: string) => string;
}>();
const emit = defineEmits<{ open: [ip: string]; sort: [value: unknown] }>();
const { t } = useI18n();
const sorts = [
  "requests",
  "last_seen",
  "client_errors",
  "server_errors",
  "waf_hits",
] as const;
</script>

<template>
  <section :aria-busy="loading">
    <div
      class="flex flex-wrap items-center justify-between gap-3 border-b px-4 py-3"
    >
      <p class="text-xs text-muted-foreground">
        {{ t("admin.gatewayRequestLogs.ipView.hint") }}
      </p>
      <Select :model-value="sort" @update:model-value="emit('sort', $event)">
        <SelectTrigger
          class="w-44"
          :aria-label="t('admin.gatewayRequestLogs.ipView.sortLabel')"
        >
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem v-for="key in sorts" :key="key" :value="key">{{
            t(`admin.gatewayRequestLogs.ipView.sorts.${key}`)
          }}</SelectItem>
        </SelectContent>
      </Select>
    </div>
    <div
      v-if="loading"
      role="status"
      class="flex min-h-48 items-center justify-center text-sm text-muted-foreground"
    >
      {{ t("admin.gatewayRequestLogs.ipView.loading") }}
    </div>
    <div
      v-else-if="!data?.items.length"
      class="flex min-h-48 items-center justify-center text-sm text-muted-foreground"
    >
      {{ t("admin.gatewayRequestLogs.empty") }}
    </div>
    <template v-else>
      <div class="hidden overflow-x-auto md:block">
        <table class="w-full text-left text-sm">
          <thead class="border-b bg-muted/20 text-xs text-muted-foreground">
            <tr>
              <th class="px-4 py-3 font-medium">
                {{ t("admin.gatewayRequestLogs.columns.clientIp") }}
              </th>
              <th class="px-3 py-3 text-right font-medium">
                {{ t("admin.gatewayRequestLogs.ipView.requests") }}
              </th>
              <th class="px-3 py-3 font-medium">
                {{ t("admin.gatewayRequestLogs.ipView.hosts") }}
              </th>
              <th class="px-3 py-3 font-medium">
                {{ t("admin.gatewayRequestLogs.ipView.responses") }}
              </th>
              <th class="px-3 py-3 text-right font-medium">
                {{ t("admin.gatewayRequestLogs.ipView.waf") }}
              </th>
              <th class="px-3 py-3 font-medium">
                {{ t("admin.gatewayRequestLogs.ipView.times") }}
              </th>
              <th class="px-4 py-3 text-right font-medium">
                {{ t("admin.gatewayRequestLogs.columns.actions") }}
              </th>
            </tr>
          </thead>
          <tbody class="divide-y">
            <tr
              v-for="item in data.items"
              :key="item.client_ip"
              class="hover:bg-muted/20"
            >
              <td class="px-4 py-3">
                <button
                  type="button"
                  class="break-all text-left font-mono font-medium text-primary hover:underline focus-visible:outline focus-visible:outline-ring"
                  @click="emit('open', item.client_ip)"
                >
                  {{
                    item.client_ip ||
                    t("admin.gatewayRequestLogs.ipView.unknown")
                  }}
                </button>
                <p class="mt-1 text-xs text-muted-foreground">
                  {{ location(item.client_ip) }}
                </p>
              </td>
              <td class="px-3 py-3 text-right font-medium tabular-nums">
                {{ item.requests.toLocaleString() }}
              </td>
              <td class="px-3 py-3">
                <details v-if="item.hosts.length" class="max-w-52 text-xs">
                  <summary
                    class="cursor-pointer"
                    :title="item.hosts.join('\n')"
                  >
                    {{
                      t("admin.gatewayRequestLogs.ipView.hostCount", {
                        count: item.hosts.length,
                      })
                    }}
                  </summary>
                  <p
                    v-for="host in item.hosts"
                    :key="host"
                    class="mt-1 break-all text-muted-foreground"
                  >
                    {{ host }}
                  </p>
                </details>
                <span v-else>—</span>
              </td>
              <td
                class="space-y-1 whitespace-nowrap px-3 py-3 text-xs tabular-nums"
              >
                <p
                  :class="
                    item.client_errors
                      ? 'text-amber-600 dark:text-amber-400'
                      : 'text-muted-foreground'
                  "
                >
                  4xx · {{ item.client_errors }}
                </p>
                <p
                  :class="
                    item.server_errors
                      ? 'text-destructive'
                      : 'text-muted-foreground'
                  "
                >
                  5xx · {{ item.server_errors }}
                </p>
              </td>
              <td
                class="px-3 py-3 text-right tabular-nums"
                :class="
                  item.waf_hits
                    ? 'text-amber-600 dark:text-amber-400'
                    : 'text-muted-foreground'
                "
              >
                {{ item.waf_hits }}
              </td>
              <td
                class="space-y-1 whitespace-nowrap px-3 py-3 text-xs text-muted-foreground"
              >
                <p :title="item.first_seen">
                  {{ t("admin.gatewayRequestLogs.ipView.first") }}
                  {{ formatLogClock(item.first_seen) }}
                </p>
                <p :title="item.last_seen">
                  {{ t("admin.gatewayRequestLogs.ipView.last") }}
                  {{ formatLogClock(item.last_seen) }}
                </p>
              </td>
              <td class="px-4 py-3 text-right">
                <Button
                  variant="ghost"
                  size="sm"
                  @click="emit('open', item.client_ip)"
                  >{{ t("admin.gatewayRequestLogs.ipView.open")
                  }}<ArrowUpRight class="ml-1 h-3.5 w-3.5"
                /></Button>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
      <div class="divide-y md:hidden">
        <article
          v-for="item in data.items"
          :key="item.client_ip"
          class="space-y-3 p-4"
        >
          <div class="flex items-start justify-between gap-3">
            <div class="min-w-0">
              <button
                type="button"
                class="break-all text-left font-mono text-sm font-medium text-primary hover:underline"
                @click="emit('open', item.client_ip)"
              >
                {{
                  item.client_ip || t("admin.gatewayRequestLogs.ipView.unknown")
                }}
              </button>
              <p class="mt-1 text-xs text-muted-foreground">
                {{ location(item.client_ip) }}
              </p>
            </div>
            <span class="shrink-0 text-sm font-medium tabular-nums">{{
              t("admin.gatewayRequestLogs.ipView.requestCount", {
                count: item.requests,
              })
            }}</span>
          </div>
          <div
            class="flex flex-wrap gap-x-4 gap-y-2 text-xs text-muted-foreground"
          >
            <details v-if="item.hosts.length">
              <summary>
                {{
                  t("admin.gatewayRequestLogs.ipView.hostCount", {
                    count: item.hosts.length,
                  })
                }}
              </summary>
              <p v-for="host in item.hosts" :key="host" class="break-all">
                {{ host }}
              </p>
            </details>
            <span>4xx · {{ item.client_errors }}</span
            ><span>5xx · {{ item.server_errors }}</span
            ><span
              >{{ t("admin.gatewayRequestLogs.ipView.waf") }} ·
              {{ item.waf_hits }}</span
            >
          </div>
          <div class="flex items-center justify-between gap-2">
            <div class="text-xs text-muted-foreground">
              <p :title="item.first_seen">
                {{ t("admin.gatewayRequestLogs.ipView.first") }}
                {{ formatLogClock(item.first_seen) }}
              </p>
              <p :title="item.last_seen">
                {{ t("admin.gatewayRequestLogs.ipView.last") }}
                {{ formatLogClock(item.last_seen) }}
              </p>
            </div>
            <Button
              variant="outline"
              size="sm"
              @click="emit('open', item.client_ip)"
              >{{ t("admin.gatewayRequestLogs.ipView.open") }}</Button
            >
          </div>
        </article>
      </div>
    </template>
  </section>
</template>
