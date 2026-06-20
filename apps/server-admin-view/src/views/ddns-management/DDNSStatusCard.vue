<script setup lang="ts">
import { useI18n } from "vue-i18n";
import {
  Cable,
  Clock,
  Globe,
  Network,
  Route as RouteIcon,
  Wifi,
} from "lucide-vue-next";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import LiveStatusBadge from "@/components/LiveStatusBadge.vue";
import HumanFriendlyTime from "@admin-shared/components/common/HumanFriendlyTime.vue";
import OverflowTooltipText from "@admin-shared/components/common/OverflowTooltipText.vue";
import type { LastCheck, LastIP } from "./model";

defineProps<{
  copyIpAddress: (
    label: "IPv4" | "IPv6",
    value: string | null,
  ) => void | Promise<void>;
  currentIpSourceLabel: string;
  currentNetworkInterfaceLabel: string;
  currentUpdateScopeLabel: string;
  enabled: boolean;
  lastCheck: LastCheck;
  lastCheckTooltipLines: string[];
  lastIp: LastIP;
  openUpdateIntervalDialog: () => void;
  showIpv4Status: boolean;
  showIpv6Status: boolean;
  updateIntervalLabel: string;
}>();

const { t } = useI18n();
</script>

<template>
  <Card class="overflow-hidden py-5 mb-6">
    <CardHeader>
      <div class="flex items-center justify-between">
        <CardTitle class="text-base font-medium flex items-center gap-2">
          {{ t("admin.ddns.statusTitle") }}
          <LiveStatusBadge
            :active="enabled"
            :active-label="t('admin.ddns.activeLabel')"
            :inactive-label="t('admin.ddns.pausedLabel')"
            class="mt-px"
          />
        </CardTitle>

        <button
          v-if="enabled"
          type="button"
          class="inline-flex items-center gap-1.5 rounded-md bg-muted/50 px-2 py-1 text-xs text-muted-foreground transition-colors hover:bg-muted hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
          :aria-label="
            t('admin.ddns.setIntervalAria', { label: updateIntervalLabel })
          "
          @click="openUpdateIntervalDialog"
        >
          <Clock class="h-3.5 w-3.5" />
          <span>{{ updateIntervalLabel }}</span>
        </button>
      </div>
    </CardHeader>

    <CardContent>
      <div
        class="grid gap-4 xl:grid-cols-[minmax(10rem,1fr)_auto] xl:items-center xl:gap-6"
      >
        <div
          class="flex min-w-0 flex-col gap-4 md:min-w-[min(100%,10rem)] md:flex-row md:items-center md:gap-6"
        >
          <div
            v-if="showIpv4Status"
            class="flex min-w-0 items-center gap-4 md:shrink-0"
          >
            <div class="p-2.5 rounded-xl">
              <Wifi class="h-5 w-5" />
            </div>
            <div class="space-y-1">
              <p
                class="text-[10px] uppercase tracking-wider font-semibold text-muted-foreground"
              >
                {{ t("admin.ddns.ipv4Address") }}
              </p>
              <button
                type="button"
                class="block text-left text-sm font-mono font-medium transition-colors hover:text-primary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 rounded-sm disabled:pointer-events-none disabled:text-foreground"
                :disabled="!lastIp.ipv4"
                @click="copyIpAddress('IPv4', lastIp.ipv4)"
              >
                {{ lastIp.ipv4 || "---.---.---.---" }}
              </button>
            </div>
          </div>

          <div
            v-if="showIpv6Status"
            class="flex min-w-0 flex-1 items-center gap-4"
            :class="showIpv4Status ? 'md:border-l md:pl-6' : ''"
          >
            <div class="p-2.5 rounded-xl shrink-0">
              <Globe class="h-5 w-5" />
            </div>
            <div class="min-w-0 flex-1 space-y-1 overflow-hidden">
              <p
                class="text-[10px] uppercase tracking-wider font-semibold text-muted-foreground"
              >
                {{ t("admin.ddns.ipv6Address") }}
              </p>
              <button
                type="button"
                class="block w-full min-w-0 rounded-sm text-left transition-colors hover:text-primary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:pointer-events-none disabled:text-foreground"
                :disabled="!lastIp.ipv6"
                @click="copyIpAddress('IPv6', lastIp.ipv6)"
              >
                <OverflowTooltipText
                  as="span"
                  :text="lastIp.ipv6 || t('admin.ddns.addressNotDetected')"
                  class="text-sm font-mono font-medium"
                />
              </button>
            </div>
          </div>
        </div>

        <div
          class="flex min-w-0 flex-wrap items-center gap-4 xl:ml-auto xl:min-w-max xl:flex-nowrap xl:border-l xl:pl-6 2xl:gap-5"
        >
          <div
            class="flex min-w-[7.5rem] flex-[1_1_7.5rem] items-center gap-4 lg:flex-none"
          >
            <div class="p-2.5 rounded-xl">
              <Clock class="h-5 w-5" />
            </div>
            <div class="space-y-1">
              <p
                class="whitespace-nowrap text-[10px] uppercase tracking-wider font-semibold text-muted-foreground"
              >
                {{ t("admin.ddns.lastCheck") }}
              </p>
              <p class="text-sm font-medium">
                <HumanFriendlyTime
                  :value="lastCheck.checked_at"
                  :empty-text="t('admin.ddns.never')"
                  :tooltip-lines="lastCheckTooltipLines"
                />
              </p>
            </div>
          </div>

          <div
            class="flex min-w-[8.5rem] flex-[1_1_8.5rem] items-center gap-4 lg:flex-none"
          >
            <div class="p-2.5 rounded-xl">
              <RouteIcon class="h-5 w-5" />
            </div>
            <div class="space-y-1">
              <p
                class="whitespace-nowrap text-[10px] uppercase tracking-wider font-semibold text-muted-foreground"
              >
                {{ t("admin.ddns.updateScopeLabel") }}
              </p>
              <p class="whitespace-nowrap text-sm font-medium">
                {{ currentUpdateScopeLabel }}
              </p>
            </div>
          </div>

          <div
            class="flex min-w-[8.5rem] flex-[1_1_8.5rem] items-center gap-4 lg:flex-none"
          >
            <div class="p-2.5 rounded-xl">
              <Network class="h-5 w-5" />
            </div>
            <div class="space-y-1">
              <p
                class="whitespace-nowrap text-[10px] uppercase tracking-wider font-semibold text-muted-foreground"
              >
                {{ t("admin.ddns.ipSourceLabel") }}
              </p>
              <p class="whitespace-nowrap text-sm font-medium">
                {{ currentIpSourceLabel }}
              </p>
            </div>
          </div>

          <div
            class="flex min-w-[9rem] flex-[1_1_10rem] items-center gap-4 lg:flex-none"
          >
            <div class="p-2.5 rounded-xl">
              <Cable class="h-5 w-5" />
            </div>
            <div class="min-w-0 max-w-[180px] space-y-1">
              <p
                class="whitespace-nowrap text-[10px] uppercase tracking-wider font-semibold text-muted-foreground"
              >
                {{ t("admin.ddns.outboundInterface") }}
              </p>
              <OverflowTooltipText
                as="p"
                :text="currentNetworkInterfaceLabel"
                class="text-sm font-medium"
              />
            </div>
          </div>
        </div>
      </div>
    </CardContent>
  </Card>
</template>
