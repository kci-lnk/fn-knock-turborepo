<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { Plus, RefreshCw, Trash2 } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import LiveStatusBadge from "@/components/LiveStatusBadge.vue";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import HumanFriendlyTime from "@admin-shared/components/common/HumanFriendlyTime.vue";
import OverflowTooltipText from "@admin-shared/components/common/OverflowTooltipText.vue";
import { type DDNSTargetSummaryPayload } from "@/lib/api/ddns";
import {
  getTargetDisplayName,
  shouldShowIPv4ForScope,
  shouldShowIPv6ForScope,
} from "./model";

const props = defineProps<{
  copyIpAddress: (label: "IPv4" | "IPv6", value: string | null) => void;
  deletingTargetId: string;
  deleteTarget: (target: DDNSTargetSummaryPayload) => Promise<void>;
  editTarget: (targetId: string) => void;
  getLastCheckTooltipLines: (target: DDNSTargetSummaryPayload) => string[];
  isSavingTarget: boolean;
  targets: DDNSTargetSummaryPayload[];
  testingTargetId: string;
  testTarget: (target: DDNSTargetSummaryPayload) => Promise<void>;
  togglingTargetId: string;
  toggleTarget: (
    target: DDNSTargetSummaryPayload,
    enabled: boolean,
  ) => Promise<void>;
}>();

const emit = defineEmits<{
  create: [];
}>();

const { t } = useI18n();

const hasTargets = computed(() => props.targets.length > 0);
</script>

<template>
  <Card class="gap-2">
    <CardHeader>
      <div
        class="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between"
      >
        <div class="space-y-1">
          <CardTitle class="text-base">
            {{ t("admin.ddns.extraDomainsTitle") }}
          </CardTitle>
          <p class="text-sm text-muted-foreground">
            {{ t("admin.ddns.extraDomainsDescription") }}
          </p>
        </div>
        <Button size="sm" @click="emit('create')">
          <Plus class="mr-1.5 h-4 w-4" />
          {{ t("admin.ddns.addDomain") }}
        </Button>
      </div>
    </CardHeader>
    <CardContent class="space-y-3">
      <div
        v-if="!hasTargets"
        class="rounded-xl border border-dashed px-4 py-6 text-sm text-muted-foreground"
      >
        {{ t("admin.ddns.extraDomainsEmpty") }}
      </div>

      <div v-else class="space-y-3">
        <div
          v-for="target in targets"
          :key="target.id"
          class="rounded-xl border bg-card px-4 py-4"
        >
          <div
            class="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between"
          >
            <div class="min-w-0 space-y-2">
              <div class="flex flex-wrap items-center gap-2">
                <p class="text-sm font-medium">
                  {{ getTargetDisplayName(target) }}
                </p>
                <LiveStatusBadge
                  :active="target.enabled"
                  :active-label="t('admin.ddns.activeLabel')"
                  :inactive-label="t('admin.ddns.stoppedLabel')"
                />
              </div>
              <p
                v-if="target.domainSummary"
                class="text-sm text-muted-foreground break-all"
              >
                {{ target.domainSummary }}
              </p>
              <p class="text-xs text-muted-foreground">
                {{ target.providerLabel }}
              </p>
              <p
                v-if="target.lastCheck.message"
                class="text-xs text-muted-foreground"
              >
                {{ target.lastCheck.message }}
              </p>
            </div>

            <div class="grid gap-3 sm:grid-cols-3 lg:min-w-[360px]">
              <div
                v-if="shouldShowIPv4ForScope(target.updateScope)"
                class="rounded-lg px-3 py-3"
              >
                <p
                  class="text-[10px] uppercase tracking-wider text-muted-foreground"
                >
                  {{ t("admin.ddns.ipv4Address") }}
                </p>
                <button
                  type="button"
                  class="mt-1 inline-flex items-center rounded-sm text-left text-sm font-mono font-medium transition-colors hover:text-primary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:pointer-events-none disabled:text-foreground"
                  :disabled="!target.lastIP.ipv4"
                  :aria-label="
                    target.lastIP.ipv4
                      ? t('admin.ddns.copyAddressAria', {
                          version: 'IPv4',
                          address: target.lastIP.ipv4,
                        })
                      : t('admin.ddns.copyUnavailable', { version: 'IPv4' })
                  "
                  @click="copyIpAddress('IPv4', target.lastIP.ipv4)"
                >
                  {{ target.lastIP.ipv4 || "---.---.---.---" }}
                </button>
              </div>
              <div
                v-if="shouldShowIPv6ForScope(target.updateScope)"
                class="rounded-lg px-3 py-3"
              >
                <p
                  class="text-[10px] uppercase tracking-wider text-muted-foreground"
                >
                  {{ t("admin.ddns.ipv6Address") }}
                </p>
                <button
                  type="button"
                  class="mt-1 inline-flex min-w-0 max-w-full items-center rounded-sm text-left transition-colors hover:text-primary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:pointer-events-none disabled:text-foreground"
                  :disabled="!target.lastIP.ipv6"
                  :aria-label="
                    target.lastIP.ipv6
                      ? t('admin.ddns.copyAddressAria', {
                          version: 'IPv6',
                          address: target.lastIP.ipv6,
                        })
                      : t('admin.ddns.copyUnavailable', { version: 'IPv6' })
                  "
                  @click="copyIpAddress('IPv6', target.lastIP.ipv6)"
                >
                  <OverflowTooltipText
                    as="span"
                    :text="
                      target.lastIP.ipv6 || t('admin.ddns.addressNotDetected')
                    "
                    class="text-sm font-mono font-medium"
                  />
                </button>
              </div>
              <div class="rounded-lg px-3 py-3">
                <p
                  class="text-[10px] uppercase tracking-wider text-muted-foreground"
                >
                  {{ t("admin.ddns.lastCheck") }}
                </p>
                <div class="mt-1 text-sm">
                  <HumanFriendlyTime
                    :value="target.lastCheck.checked_at"
                    :empty-text="t('admin.ddns.never')"
                    :tooltip-lines="getLastCheckTooltipLines(target)"
                  />
                </div>
              </div>
            </div>
          </div>

          <div class="mt-4 flex flex-wrap justify-end gap-2">
            <Button
              variant="outline"
              size="sm"
              :disabled="isSavingTarget"
              @click="editTarget(target.id)"
            >
              {{ t("admin.ddns.edit") }}
            </Button>
            <Button
              variant="outline"
              size="sm"
              :disabled="testingTargetId === target.id"
              @click="testTarget(target)"
            >
              <RefreshCw
                v-if="testingTargetId === target.id"
                class="mr-1.5 h-3.5 w-3.5 animate-spin"
              />
              {{
                testingTargetId === target.id
                  ? t("admin.ddns.updating")
                  : t("admin.ddns.updateNow")
              }}
            </Button>
            <Button
              variant="outline"
              size="sm"
              :disabled="togglingTargetId === target.id"
              @click="toggleTarget(target, !target.enabled)"
            >
              {{
                target.enabled ? t("admin.ddns.stop") : t("admin.ddns.start")
              }}
            </Button>
            <ConfirmDangerPopover
              :title="t('admin.ddns.deleteExtraTitle')"
              :description="
                t('admin.ddns.deleteExtraDescription', {
                  name: getTargetDisplayName(target),
                })
              "
              :loading="deletingTargetId === target.id"
              :disabled="deletingTargetId === target.id"
              :on-confirm="() => deleteTarget(target)"
              content-class="w-72 text-left"
            >
              <template #trigger>
                <Button
                  variant="outline"
                  size="sm"
                  :disabled="deletingTargetId === target.id"
                  class="text-destructive hover:text-destructive"
                >
                  <Trash2 class="mr-1.5 h-3.5 w-3.5" />
                  {{
                    deletingTargetId === target.id
                      ? t("admin.ddns.deleting")
                      : t("admin.ddns.delete")
                  }}
                </Button>
              </template>
            </ConfirmDangerPopover>
          </div>
        </div>
      </div>
    </CardContent>
  </Card>
</template>
