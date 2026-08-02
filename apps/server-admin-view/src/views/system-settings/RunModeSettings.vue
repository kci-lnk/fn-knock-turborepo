<template>
  <Card>
    <CardHeader>
      <CardTitle>{{ t("admin.runModeSettings.title") }}</CardTitle>
      <CardDescription>{{
        t("admin.runModeSettings.description")
      }}</CardDescription>
    </CardHeader>
    <CardContent class="grid gap-6">
      <Alert
        class="items-start rounded-xl border-border/70 bg-muted/30 text-foreground"
      >
        <Info class="mt-0.5 h-4 w-4" />
        <AlertTitle>{{ accessAlertTitle }}</AlertTitle>
        <AlertDescription>
          <div class="space-y-2 text-sm leading-6">
            <p>{{ accessAlertDescription }}</p>
          </div>
        </AlertDescription>
      </Alert>

      <Alert
        v-if="showHostFirewallUnavailableAlert"
        class="items-start rounded-xl border-border/70 bg-muted/30 text-foreground"
      >
        <Info class="mt-0.5 h-4 w-4" />
        <AlertTitle>{{
          t("admin.runModeSettings.hostFirewallUnavailableTitle")
        }}</AlertTitle>
        <AlertDescription>
          <div class="space-y-2 text-sm leading-6">
            <p>{{ hostFirewallUnavailableDescription }}</p>
          </div>
        </AlertDescription>
      </Alert>

      <div
        v-if="canUseDirectMode"
        class="group rounded-lg border transition-all hover:border-primary/50"
        :class="
          mode === 0
            ? 'border-primary/70 bg-primary/5 ring-1 ring-primary/20 shadow-sm'
            : 'border-border bg-background hover:border-primary/40 hover:bg-muted/30'
        "
      >
        <label class="flex cursor-pointer items-start space-x-4 rounded-lg p-4">
          <div
            class="mt-1 flex h-5 w-5 shrink-0 items-center justify-center rounded-full border transition-colors"
            :class="
              mode === 0
                ? 'border-primary'
                : 'border-muted-foreground/40 group-hover:border-primary/60'
            "
            aria-hidden="true"
          >
            <div
              v-show="mode === 0"
              class="h-2.5 w-2.5 rounded-full bg-primary"
            />
          </div>
          <input
            v-model="mode"
            type="radio"
            name="run-mode"
            :value="0"
            class="sr-only"
          />
          <span class="flex-1 space-y-2">
            <span class="flex items-center gap-2">
              <span class="text-base font-semibold leading-none">
                {{ t("admin.runModeSettings.directModeTitle") }}
              </span>
              <span
                class="inline-flex items-center rounded-md border border-border bg-muted/40 px-2 py-0.5 text-xs font-medium text-muted-foreground"
              >
                {{ t("admin.runModeSettings.directModeBadge") }}
              </span>
            </span>
            <span class="block text-sm text-muted-foreground">
              {{ t("admin.runModeSettings.directModeDescription") }}
            </span>
          </span>
        </label>
        <div class="px-4 pb-4 pl-12">
          <DocsLinkButton :href="docsUrls.runModes.direct" />
        </div>
      </div>

      <div
        class="group rounded-lg border transition-all hover:border-primary/50"
        :class="
          mode === 1
            ? 'border-primary/70 bg-primary/5 ring-1 ring-primary/20 shadow-sm'
            : 'border-border bg-background hover:border-primary/40 hover:bg-muted/30'
        "
      >
        <label class="flex cursor-pointer items-start space-x-4 rounded-lg p-4">
          <div
            class="mt-1 flex h-5 w-5 shrink-0 items-center justify-center rounded-full border transition-colors"
            :class="
              mode === 1
                ? 'border-primary'
                : 'border-muted-foreground/40 group-hover:border-primary/60'
            "
            aria-hidden="true"
          >
            <div
              v-show="mode === 1"
              class="h-2.5 w-2.5 rounded-full bg-primary"
            />
          </div>
          <input
            v-model="mode"
            type="radio"
            name="run-mode"
            :value="1"
            class="sr-only"
            @change="selectReverseProxyMode"
          />
          <span class="flex-1 space-y-2">
            <span class="block text-base font-semibold leading-none">
              {{ t("admin.runModeSettings.reverseModeTitle") }}
            </span>
            <span class="block text-sm text-muted-foreground">
              {{ t("admin.runModeSettings.reverseModeDescription") }}
            </span>
          </span>
        </label>
        <div class="space-y-2 px-4 pb-4 pl-12">
          <DocsLinkButton :href="docsUrls.runModes.reverse" />
          <div v-if="mode === 1" class="grid gap-3 sm:grid-cols-2">
            <button
              type="button"
              class="rounded-lg border px-3 py-3 text-left transition-colors"
              :class="
                reverseProxySubmode === 'subdomain'
                  ? 'border-primary/70 bg-primary/5 shadow-sm'
                  : 'border-border bg-background hover:border-primary/40 hover:bg-muted/30'
              "
              @click="reverseProxySubmode = 'subdomain'"
            >
              <p class="text-sm font-medium text-foreground">
                {{ t("admin.runModeSettings.subdomainMapping") }}
              </p>
              <p class="mt-1 text-xs leading-5 text-muted-foreground">
                {{ t("admin.runModeSettings.subdomainSubmodeDescription") }}
              </p>
            </button>
            <button
              type="button"
              class="rounded-lg border px-3 py-3 text-left transition-colors"
              :class="
                reverseProxySubmode === 'path'
                  ? 'border-primary/70 bg-primary/5 shadow-sm'
                  : 'border-border bg-background hover:border-primary/40 hover:bg-muted/30'
              "
              @click="reverseProxySubmode = 'path'"
            >
              <p
                class="flex flex-wrap items-center gap-2 text-sm font-medium text-foreground"
              >
                <span>{{ t("admin.runModeSettings.pathMapping") }}</span>
                <span
                  class="inline-flex items-center rounded border border-border bg-muted/40 px-1.5 py-0.5 text-[10px] font-medium leading-none text-muted-foreground"
                >
                  {{ t("admin.runModeSettings.pathSubmodeDeprecatedBadge") }}
                </span>
              </p>
              <p class="mt-1 text-xs leading-5 text-muted-foreground">
                {{ t("admin.runModeSettings.pathSubmodeDescription") }}
              </p>
            </button>
          </div>
        </div>
      </div>

      <div
        class="group rounded-lg border transition-all hover:border-primary/50"
        :class="
          mode === 3
            ? 'border-primary/70 bg-primary/5 ring-1 ring-primary/20 shadow-sm'
            : 'border-border bg-background hover:border-primary/40 hover:bg-muted/30'
        "
      >
        <label class="flex cursor-pointer items-start space-x-4 rounded-lg p-4">
          <div
            class="mt-1 flex h-5 w-5 shrink-0 items-center justify-center rounded-full border transition-colors"
            :class="
              mode === 3
                ? 'border-primary'
                : 'border-muted-foreground/40 group-hover:border-primary/60'
            "
            aria-hidden="true"
          >
            <div
              v-show="mode === 3"
              class="h-2.5 w-2.5 rounded-full bg-primary"
            />
          </div>
          <input
            v-model="mode"
            type="radio"
            name="run-mode"
            :value="3"
            class="sr-only"
          />
          <span class="flex-1 space-y-2">
            <span class="flex items-center gap-2">
              <span class="text-base font-semibold leading-none">
                {{ t("admin.runModeSettings.subdomainModeTitle") }}
              </span>
              <span
                class="inline-flex items-center rounded-md border border-border bg-muted/40 px-2 py-0.5 text-xs font-medium text-muted-foreground"
              >
                {{ t("admin.runModeSettings.subdomainModeBadge") }}
              </span>
            </span>
            <span class="block text-sm text-muted-foreground">
              {{ t("admin.runModeSettings.subdomainModeDescription") }}
            </span>
          </span>
        </label>
        <div class="px-4 pb-4 pl-12">
          <DocsLinkButton :href="docsUrls.runModes.subdomain" />
        </div>
      </div>
    </CardContent>
    <CardFooter
      class="flex flex-col gap-4 border-t border-border pt-6 sm:flex-row sm:items-center sm:justify-between"
    >
      <label
        v-if="canManageHostFirewall"
        class="flex items-start gap-3 text-sm text-muted-foreground"
      >
        <Checkbox
          class="mt-0.5"
          :model-value="autoManageFirewall"
          :disabled="isBusy"
          @update:model-value="handleAutoManageFirewallChange"
        />
        <span class="space-y-1">
          <span class="block font-medium text-foreground">
            {{ t("admin.runModeSettings.autoFirewallTitle") }}
          </span>
          <span class="block text-xs leading-5 text-muted-foreground">
            {{ t("admin.runModeSettings.autoFirewallDescription") }}
          </span>
        </span>
        <Loader2
          v-if="isAutoManageFirewallPending"
          class="mt-0.5 h-4 w-4 animate-spin text-muted-foreground"
        />
      </label>
      <div
        v-else-if="!isDockerDeployment && !isFpkLiteDeployment"
        class="w-full text-sm leading-6 text-muted-foreground sm:max-w-xl"
      >
        {{ t("admin.runModeSettings.hostFirewallDisabled") }}
      </div>

      <FloatingActionDock
        :active="!isModeUnchanged"
        inline-class="flex w-full justify-end gap-2 sm:w-auto"
      >
        <template #inline>
          <DropdownMenu v-if="canManageHostFirewall">
            <DropdownMenuTrigger as-child>
              <Button variant="outline" class="w-24 gap-2" :disabled="isBusy">
                <Loader2
                  v-if="isFirewallActionPending"
                  class="h-4 w-4 animate-spin"
                />
                <span>{{ t("admin.runModeSettings.actions") }}</span>
                <ChevronDown class="h-4 w-4 text-muted-foreground" />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end" class="w-56">
              <DropdownMenuItem
                :disabled="isBusy"
                @select="resetFirewallBySelectedMode"
              >
                <RefreshCw class="h-4 w-4" />
                {{ t("admin.runModeSettings.resetFirewallByMode") }}
              </DropdownMenuItem>
              <DropdownMenuItem
                :disabled="isBusy"
                @select="openFirewallAdditionalPortsDialog"
              >
                <ShieldPlus class="h-4 w-4" />
                {{ t("admin.runModeSettings.additionalPorts.menu") }}
              </DropdownMenuItem>
              <DropdownMenuSeparator />
              <DropdownMenuItem
                variant="destructive"
                :disabled="isBusy"
                @select="clearFirewallRules"
              >
                <Trash2 class="h-4 w-4" />
                {{ t("admin.runModeSettings.clearFirewall") }}
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
          <Button
            variant="outline"
            class="w-24"
            @click="reset"
            :disabled="isBusy"
          >
            {{ t("admin.runModeSettings.discardChanges") }}
          </Button>
          <Button @click="save" :disabled="isBusy || isModeUnchanged">
            <span
              v-if="isSaving"
              class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
            ></span>
            {{ t("admin.runModeSettings.saveChanges") }}
          </Button>
        </template>

        <template #floating>
          <Button
            variant="outline"
            class="w-24"
            @click="reset"
            :disabled="isBusy"
          >
            {{ t("admin.runModeSettings.discardChanges") }}
          </Button>
          <Button @click="save" :disabled="isBusy || isModeUnchanged">
            <span
              v-if="isSaving"
              class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
            ></span>
            {{ t("admin.runModeSettings.saveChanges") }}
          </Button>
        </template>
      </FloatingActionDock>
    </CardFooter>
  </Card>

  <RunModeConfirmationDialog
    :open="isConfirmDialogOpen"
    v-model:dont-show-again="dontShowAgainChecked"
    :content="confirmDialogContent"
    :saving="isSaving"
    @close="closeConfirmation"
    @confirm="confirmSave"
    @update:open="handleConfirmDialogOpenChange"
  />

  <FirewallAdditionalPortsDialog
    :open="isFirewallAdditionalPortsDialogOpen"
    :auto-manage-firewall-enabled="firewallAdditionalPortsAutoManageEnabled"
    :details="firewallAdditionalPortsDetails"
    :has-unsaved-mode-changes="hasUnsavedFirewallModeChanges"
    :load-failed="firewallAdditionalPortsLoadFailed"
    :loading="isFirewallAdditionalPortsLoading"
    :mode-label="firewallAdditionalPortsModeLabel"
    :saving="isFirewallAdditionalPortsSaving"
    @retry="loadFirewallAdditionalPorts"
    @save="saveFirewallAdditionalPorts"
    @update:open="handleFirewallAdditionalPortsDialogOpenChange"
  />
</template>

<script setup lang="ts">
import {
  ChevronDown,
  Info,
  Loader2,
  RefreshCw,
  ShieldPlus,
  Trash2,
} from "lucide-vue-next";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import {
  Card,
  CardHeader,
  CardTitle,
  CardDescription,
  CardContent,
  CardFooter,
} from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import DocsLinkButton from "@/components/DocsLinkButton.vue";
import FloatingActionDock from "@admin-shared/components/common/FloatingActionDock.vue";
import { docsUrls } from "../../lib/docs";
import FirewallAdditionalPortsDialog from "./FirewallAdditionalPortsDialog.vue";
import RunModeConfirmationDialog from "./RunModeConfirmationDialog.vue";
import { useRunModeSettingsController } from "./useRunModeSettingsController";

const {
  accessAlertDescription,
  accessAlertTitle,
  autoManageFirewall,
  canManageHostFirewall,
  canUseDirectMode,
  clearFirewallRules,
  closeConfirmation,
  confirmDialogContent,
  confirmSave,
  dontShowAgainChecked,
  firewallAdditionalPortsAutoManageEnabled,
  firewallAdditionalPortsDetails,
  firewallAdditionalPortsLoadFailed,
  firewallAdditionalPortsModeLabel,
  handleAutoManageFirewallChange,
  handleConfirmDialogOpenChange,
  handleFirewallAdditionalPortsDialogOpenChange,
  hasUnsavedFirewallModeChanges,
  hostFirewallUnavailableDescription,
  isAutoManageFirewallPending,
  isBusy,
  isConfirmDialogOpen,
  isDockerDeployment,
  isFpkLiteDeployment,
  isFirewallActionPending,
  isFirewallAdditionalPortsDialogOpen,
  isFirewallAdditionalPortsLoading,
  isFirewallAdditionalPortsSaving,
  isModeUnchanged,
  isSaving,
  mode,
  loadFirewallAdditionalPorts,
  openFirewallAdditionalPortsDialog,
  reset,
  resetFirewallBySelectedMode,
  reverseProxySubmode,
  save,
  saveFirewallAdditionalPorts,
  selectReverseProxyMode,
  showHostFirewallUnavailableAlert,
  t,
} = useRunModeSettingsController();
</script>
