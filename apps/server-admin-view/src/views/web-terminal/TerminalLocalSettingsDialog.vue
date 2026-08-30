<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";
import {
  Laptop,
  LoaderCircle,
  LockKeyhole,
  ShieldAlert,
  TriangleAlert,
} from "lucide-vue-next";
import type { WebTerminalPageController } from "./useWebTerminalPage";

const props = defineProps<{ controller: WebTerminalPageController }>();
const {
  closeLocalSettings,
  localActiveSessionCount,
  localConfirmationRequired,
  localConflictingSessionCount,
  localRiskAcknowledged,
  localSettingsError,
  localSettingsOpen,
  localStatus,
  localUpdating,
  submitLocalSettings,
} = props.controller;
const { t } = useI18n();

const enabling = computed(() => !localStatus.value?.enabled);
const confirmCount = computed(
  () =>
    localConflictingSessionCount.value || localActiveSessionCount.value || 0,
);
const setRiskAcknowledged = (value: boolean | "indeterminate") => {
  localRiskAcknowledged.value = value === true;
};
</script>

<template>
  <Dialog
    :open="localSettingsOpen"
    @update:open="$event ? undefined : closeLocalSettings()"
  >
    <DialogContent class="sm:max-w-[540px]">
      <DialogHeader>
        <DialogTitle class="flex items-center gap-2">
          <Laptop class="h-5 w-5" />
          {{ t("admin.webTerminal.localSettingsTitle") }}
        </DialogTitle>
        <DialogDescription>
          {{ t("admin.webTerminal.localSettingsDescription") }}
        </DialogDescription>
      </DialogHeader>

      <template v-if="localStatus">
        <div class="grid gap-2 rounded-xl border border-border/70 p-3 text-sm">
          <div class="flex items-center justify-between gap-3">
            <span class="text-muted-foreground">
              {{ t("admin.webTerminal.executionIdentity") }}
            </span>
            <span class="flex items-center gap-2 font-mono text-xs">
              {{ localStatus.executionIdentity }}
            </span>
          </div>
          <div class="flex items-center justify-between gap-3">
            <span class="text-muted-foreground">Shell</span>
            <span class="min-w-0 truncate font-mono text-xs">
              {{ localStatus.shell || t("admin.webTerminal.unavailable") }}
            </span>
          </div>
          <div class="flex items-center justify-between gap-3">
            <span class="text-muted-foreground">
              {{ t("admin.webTerminal.initialDirectory") }}
            </span>
            <span class="min-w-0 truncate font-mono text-xs">
              {{
                localStatus.workingDirectory ||
                t("admin.webTerminal.unavailable")
              }}
            </span>
          </div>
        </div>

        <Alert :variant="localStatus.privileged ? 'destructive' : 'default'">
          <ShieldAlert class="h-4 w-4" />
          <AlertTitle>
            {{
              localStatus.privileged
                ? t("admin.webTerminal.localRootRiskTitle")
                : t("admin.webTerminal.localRiskTitle")
            }}
          </AlertTitle>
          <AlertDescription>
            {{
              localStatus.privileged
                ? t("admin.webTerminal.localRootRiskDescription")
                : t("admin.webTerminal.localRiskDescription")
            }}
          </AlertDescription>
        </Alert>

        <div
          v-if="enabling"
          class="flex items-start gap-3 rounded-lg border border-border/70 p-3"
        >
          <Checkbox
            id="local-terminal-risk-acknowledgement"
            :model-value="localRiskAcknowledged"
            class="mt-0.5"
            @update:model-value="setRiskAcknowledged"
          />
          <Label
            for="local-terminal-risk-acknowledgement"
            class="cursor-pointer text-sm leading-5"
          >
            {{ t("admin.webTerminal.localRiskAcknowledgement") }}
          </Label>
        </div>

        <Alert v-if="localConfirmationRequired" variant="destructive">
          <TriangleAlert class="h-4 w-4" />
          <AlertTitle>
            {{ t("admin.webTerminal.localDisableConfirmationTitle") }}
          </AlertTitle>
          <AlertDescription>
            {{
              t("admin.webTerminal.localDisableConfirmationDescription", {
                count: confirmCount,
              })
            }}
          </AlertDescription>
        </Alert>

        <p
          v-if="localSettingsError && !localConfirmationRequired"
          class="rounded-lg border border-destructive/25 bg-destructive/5 px-3 py-2 text-sm text-destructive"
        >
          {{ localSettingsError }}
        </p>
      </template>

      <DialogFooter>
        <Button
          variant="outline"
          :disabled="localUpdating"
          @click="closeLocalSettings"
        >
          {{ t("common.cancel") }}
        </Button>
        <Button
          :variant="enabling ? 'default' : 'destructive'"
          :disabled="
            localUpdating ||
            !localStatus ||
            (enabling && (!localStatus.ready || !localRiskAcknowledged))
          "
          @click="submitLocalSettings(localConfirmationRequired)"
        >
          <LoaderCircle
            v-if="localUpdating"
            class="mr-1.5 h-4 w-4 animate-spin"
          />
          <LockKeyhole v-else class="mr-1.5 h-4 w-4" />
          {{
            localConfirmationRequired
              ? t("admin.webTerminal.localTerminateAndDisable")
              : enabling
                ? t("admin.webTerminal.enableLocal")
                : t("admin.webTerminal.disableLocal")
          }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
