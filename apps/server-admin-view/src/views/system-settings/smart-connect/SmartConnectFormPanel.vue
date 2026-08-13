<script setup lang="ts">
import { useId } from "vue";
import { useI18n } from "vue-i18n";
import { Badge, type BadgeVariants } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { Progress } from "@/components/ui/progress";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import FloatingActionDock from "@admin-shared/components/common/FloatingActionDock.vue";
import type {
  SmartConnectConfig,
  SmartConnectDetails,
  SmartConnectLocalIpOption,
} from "@/types";

defineProps<{
  canUseSmartConnect: boolean;
  capabilityBlockedReason: string;
  details: SmartConnectDetails;
  dnsmasqActionLabel: string;
  dnsmasqProgress: number;
  dnsmasqStatusLabel: string;
  dnsmasqStatusVariant: BadgeVariants["variant"];
  dnsmasqSummaryText: string;
  isDirty: boolean;
  isSaving: boolean;
  isSmartConnectAvailable: boolean;
  isStartingInstall: boolean;
  resolvedIpOptions: SmartConnectLocalIpOption[];
  saveBlockedReason: string;
  showAdvancedCards: boolean;
  showDnsmasqAction: boolean;
  showDnsmasqCard: boolean;
  showDnsmasqSetupCard: boolean;
}>();
const form = defineModel<SmartConnectConfig>({ required: true });
const emit = defineEmits<{
  cancel: [];
  save: [];
  startInstall: [];
}>();
const { t } = useI18n();
const a11yId = useId();
</script>

<template>
  <div
    v-if="!isSmartConnectAvailable || !canUseSmartConnect"
    class="rounded-xl border border-zinc-200 bg-zinc-50 px-4 py-3 text-sm leading-6 text-zinc-700"
  >
    {{
      !canUseSmartConnect
        ? capabilityBlockedReason
        : t("admin.smartConnectSettings.currentModeUnavailableWithReason", {
            reason: details.availability.reason,
          })
    }}
  </div>

  <div class="rounded-2xl border border-border/60 bg-muted/10 px-4 py-4">
    <div class="flex items-start justify-between gap-4">
      <Label
        :for="`${a11yId}-enabled`"
        class="text-base font-medium"
      >
        {{ t("admin.smartConnectSettings.title") }}
      </Label>
      <Switch
        :id="`${a11yId}-enabled`"
        class="mt-0.5 shrink-0"
        :model-value="form.enabled"
        :disabled="!canUseSmartConnect || isSaving || isStartingInstall"
        @update:model-value="form.enabled = $event === true"
      />
    </div>
  </div>

  <div class="overflow-hidden rounded-xl border border-border/60">
    <template v-if="showDnsmasqCard">
      <section v-if="showDnsmasqSetupCard" class="space-y-4 p-5">
        <div
          class="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between"
        >
          <div class="space-y-1">
            <div class="flex flex-wrap items-center gap-2">
              <div class="text-base font-medium">
                {{ t("admin.smartConnectSettings.runtimeEnvironment") }}
              </div>
              <Badge :variant="dnsmasqStatusVariant">
                {{ dnsmasqStatusLabel }}
              </Badge>
            </div>
            <p class="text-sm leading-6 text-muted-foreground">
              {{ details.dnsmasq.install_state.message }}
            </p>
            <p class="text-xs leading-5 text-muted-foreground">
              {{ dnsmasqSummaryText }}
            </p>
          </div>

          <Button
            v-if="showDnsmasqAction"
            class="w-full sm:w-auto"
            :disabled="
              isSaving ||
              isStartingInstall ||
              details.dnsmasq.install_state.status === 'installing'
            "
            @click="emit('startInstall')"
          >
            <span
              v-if="
                isStartingInstall ||
                details.dnsmasq.install_state.status === 'installing'
              "
              class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
            ></span>
            {{ dnsmasqActionLabel }}
          </Button>
        </div>

        <Progress
          v-if="details.dnsmasq.install_state.status === 'installing'"
          :model-value="dnsmasqProgress"
        />
      </section>

      <template v-if="showAdvancedCards">
        <section
          :class="[
            'space-y-4 p-5',
            showDnsmasqSetupCard ? 'border-t border-border/60' : '',
          ]"
        >
          <div
            class="grid gap-4 lg:grid-cols-[minmax(0,1fr)_minmax(280px,360px)] lg:items-start lg:gap-6"
          >
            <div class="space-y-1">
              <Label :for="`${a11yId}-local-ip`" class="text-base">
                {{ t("admin.smartConnectSettings.localLanIp") }}
              </Label>
              <p class="text-sm leading-6 text-muted-foreground">
                {{ t("admin.smartConnectSettings.localLanIpHint") }}
              </p>
            </div>

            <div class="space-y-2">
              <Select
                :model-value="form.selected_ipv4 || undefined"
                @update:model-value="form.selected_ipv4 = String($event ?? '')"
              >
                <SelectTrigger
                  :id="`${a11yId}-local-ip`"
                  class="h-10 w-full rounded-lg border-border/70 bg-background px-3 text-sm shadow-none"
                >
                  <SelectValue
                    :placeholder="t('admin.smartConnectSettings.selectLocalIpv4')"
                  />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem
                    v-for="option in resolvedIpOptions"
                    :key="`${option.interface}-${option.value}`"
                    :value="option.value"
                  >
                    {{ option.label }}
                  </SelectItem>
                </SelectContent>
              </Select>
              <p
                v-if="resolvedIpOptions.length === 0"
                class="text-sm leading-6 text-muted-foreground"
              >
                {{ t("admin.smartConnectSettings.noPrivateIpv4") }}
              </p>
            </div>
          </div>
        </section>

        <section class="space-y-4 border-t border-border/60 p-5">
          <div class="space-y-1">
            <div class="text-base font-medium">
              {{ t("admin.smartConnectSettings.syncedDomains") }}
            </div>
            <p class="text-sm leading-6 text-muted-foreground">
              {{ t("admin.smartConnectSettings.syncedDomainsHint") }}
            </p>
          </div>

          <div class="rounded-xl bg-muted/20 px-4 py-4">
            <div
              v-if="details.domains.length === 0"
              class="text-sm leading-6 text-muted-foreground"
            >
              {{ t("admin.smartConnectSettings.noSyncedDomains") }}
            </div>
            <div v-else class="flex flex-wrap gap-2">
              <Badge
                v-for="domain in details.domains"
                :key="domain"
                variant="secondary"
                class="max-w-full break-all"
              >
                {{ domain }}
              </Badge>
            </div>
          </div>
        </section>

        <section class="space-y-4 border-t border-border/60 p-5">
          <div class="space-y-1">
            <div class="text-base font-medium">
              {{ t("admin.smartConnectSettings.notes") }}
            </div>
            <p class="text-sm leading-6 text-muted-foreground">
              {{
                t("admin.smartConnectSettings.dnsInstruction", {
                  ip:
                    form.selected_ipv4 ||
                    t("admin.smartConnectSettings.localLanIpFallback"),
                })
              }}
              <span>{{ t("admin.smartConnectSettings.androidWarning") }}</span>
            </p>
          </div>
        </section>
      </template>
    </template>

    <FloatingActionDock
      :active="isDirty"
      :inline-class="[
        'space-y-4 p-5',
        showDnsmasqCard ? 'border-t border-border/60' : '',
      ]"
    >
      <template #inline>
        <div
          class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between"
        >
          <p class="text-sm leading-6 text-muted-foreground">
            {{
              saveBlockedReason ||
              t("admin.smartConnectSettings.saveSyncHint")
            }}
          </p>

          <div class="flex gap-3 sm:ml-auto">
            <Button
              variant="outline"
              :disabled="isSaving"
              @click="emit('cancel')"
            >
              {{ t("common.cancel") }}
            </Button>
            <Button
              :disabled="
                !isDirty ||
                isSaving ||
                isStartingInstall ||
                Boolean(saveBlockedReason)
              "
              @click="emit('save')"
            >
              <span
                v-if="isSaving"
                class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
              ></span>
              {{
                isSaving
                  ? t("admin.smartConnectSettings.saving")
                  : t("admin.smartConnectSettings.saveAndSync")
              }}
            </Button>
          </div>
        </div>
      </template>

      <template #floating>
        <Button
          variant="outline"
          :disabled="isSaving"
          @click="emit('cancel')"
        >
          {{ t("common.cancel") }}
        </Button>
        <Button
          :disabled="
            !isDirty ||
            isSaving ||
            isStartingInstall ||
            Boolean(saveBlockedReason)
          "
          @click="emit('save')"
        >
          <span
            v-if="isSaving"
            class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
          ></span>
          {{
            isSaving
              ? t("admin.smartConnectSettings.saving")
              : t("admin.smartConnectSettings.saveAndSync")
          }}
        </Button>
      </template>
    </FloatingActionDock>
  </div>
</template>
