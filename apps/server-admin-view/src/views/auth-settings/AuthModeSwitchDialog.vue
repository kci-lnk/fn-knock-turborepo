<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import type {
  AuthAccount,
  AuthLoginMode,
  AuthLoginModePreview,
} from "../../types";

const props = defineProps<{
  open: boolean;
  currentMode: AuthLoginMode;
  accounts: AuthAccount[];
  preview: AuthLoginModePreview | null;
  isPreviewing: boolean;
  isSwitching: boolean;
}>();

const emit = defineEmits<{
  "update:open": [value: boolean];
  confirm: [];
  "bind-totp": [account: AuthAccount];
  "edit-account": [account: AuthAccount];
  "set-password": [account: AuthAccount];
}>();

const { t } = useI18n();

const dialogOpen = computed({
  get: () => props.open,
  set: (value) => emit("update:open", value),
});

const targetModeLabel = computed(() =>
  props.currentMode === "totp"
    ? t("admin.authSettings.passwordLoginMode")
    : t("admin.authSettings.totpLoginMode"),
);

const isSwitchingToPassword = computed(() => props.currentMode === "totp");
const isSwitchingToTotp = computed(() => props.currentMode === "password");
const accountsNeedingPassword = computed(() =>
  props.accounts.filter((account) => !account.passwordConfigured),
);
const accountsNeedingTotp = computed(() =>
  props.accounts.filter((account) => !account.totpConfigured),
);
const shouldShowPasswordPreparation = computed(
  () => isSwitchingToPassword.value && accountsNeedingPassword.value.length > 0,
);
const shouldShowTotpPreparation = computed(
  () => isSwitchingToTotp.value && accountsNeedingTotp.value.length > 0,
);
</script>

<template>
  <Dialog v-model:open="dialogOpen">
    <DialogContent class="max-h-[88vh] overflow-y-auto sm:max-w-[520px]">
      <DialogHeader>
        <DialogTitle>
          {{ t("admin.authSettings.switchAuthModeTitle") }}
        </DialogTitle>
        <DialogDescription>
          {{
            t("admin.authSettings.switchAuthModeDescription", {
              mode: targetModeLabel,
            })
          }}
        </DialogDescription>
      </DialogHeader>

      <div v-if="isPreviewing" class="space-y-3">
        <div class="h-10 animate-pulse rounded-md bg-muted"></div>
        <div class="h-20 animate-pulse rounded-md bg-muted"></div>
      </div>
      <div v-else-if="preview" class="space-y-3 text-sm">
        <p
          v-if="preview.passwordRequiredBeforeSwitch"
          class="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-destructive"
        >
          {{ t("admin.authSettings.previewPasswordRequiredBeforeSwitch") }}
        </p>
        <p
          v-else-if="preview.missingSourceTotpCount"
          class="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-destructive"
        >
          {{
            t("admin.authSettings.previewTotpRequiredBeforeSwitch", {
              count: preview.missingSourceTotpCount,
            })
          }}
        </p>
        <p
          v-else-if="preview.blockingIssueCount"
          class="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-destructive"
        >
          {{
            t("admin.authSettings.previewBlockingIssues", {
              count: preview.blockingIssueCount,
            })
          }}
        </p>
        <div
          v-if="shouldShowPasswordPreparation"
          class="overflow-hidden rounded-md border"
        >
          <div class="border-b bg-muted/20 px-3 py-2">
            <div>
              <p class="font-medium">
                {{ t("admin.authSettings.passwordAccountsPreparationTitle") }}
              </p>
              <p class="mt-1 text-xs text-muted-foreground">
                {{
                  t("admin.authSettings.passwordAccountsPreparationDescription")
                }}
              </p>
            </div>
          </div>
          <div class="divide-y">
            <div
              v-for="account in accountsNeedingPassword"
              :key="account.id"
              class="flex flex-col gap-3 px-3 py-3 sm:flex-row sm:items-center sm:justify-between"
            >
              <div class="min-w-0">
                <p class="truncate font-medium">
                  {{ account.username }}
                </p>
                <p class="truncate text-xs text-muted-foreground">
                  {{
                    account.sourceTotpName ||
                    account.sourceTotpId
                  }}
                </p>
              </div>
              <div class="flex shrink-0 items-center gap-2">
                <Badge variant="destructive">
                  {{ t("admin.authSettings.passwordUnset") }}
                </Badge>
                <Button
                  size="sm"
                  variant="outline"
                  :disabled="isSwitching"
                  @click="emit('set-password', account)"
                >
                  {{ t("admin.authSettings.setPassword") }}
                </Button>
              </div>
            </div>
          </div>
        </div>
        <div v-if="shouldShowTotpPreparation" class="overflow-hidden rounded-md border">
          <div class="border-b bg-muted/20 px-3 py-2">
            <div>
              <p class="font-medium">
                {{ t("admin.authSettings.totpAccountsPreparationTitle") }}
              </p>
              <p class="mt-1 text-xs text-muted-foreground">
                {{ t("admin.authSettings.totpAccountsPreparationDescription") }}
              </p>
            </div>
          </div>
          <div class="divide-y">
            <div
              v-for="account in accountsNeedingTotp"
              :key="account.id"
              class="flex flex-col gap-3 px-3 py-3 sm:flex-row sm:items-center sm:justify-between"
            >
              <div class="min-w-0">
                <p class="truncate font-medium">
                  {{ account.username }}
                </p>
                <p class="truncate text-xs text-muted-foreground">
                  {{ t("admin.authSettings.totpUnavailableHint") }}
                </p>
              </div>
              <div class="flex shrink-0 items-center gap-2">
                <Badge variant="destructive">
                  {{ t("admin.authSettings.totpMissing") }}
                </Badge>
                <Button
                  size="sm"
                  variant="outline"
                  :disabled="isSwitching"
                  @click="emit('bind-totp', account)"
                >
                  {{ t("admin.authSettings.bindTotp") }}
                </Button>
              </div>
            </div>
          </div>
        </div>
      </div>

      <DialogFooter class="gap-2">
        <Button
          variant="outline"
          :disabled="isPreviewing || isSwitching"
          @click="dialogOpen = false"
        >
          {{ t("admin.authSettings.cancel") }}
        </Button>
        <Button
          :disabled="
            isPreviewing ||
            isSwitching ||
            !preview ||
            preview.blockingIssueCount > 0
          "
          @click="emit('confirm')"
        >
          <span
            v-if="isSwitching"
            class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
          ></span>
          {{ t("admin.authSettings.confirmSwitchAuthMode") }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
