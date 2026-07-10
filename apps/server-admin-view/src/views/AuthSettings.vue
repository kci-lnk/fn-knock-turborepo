<template>
  <div
    class="dynamic-white-page-card dynamic-white-settings-surface h-full flex flex-col gap-4"
  >
    <div
      class="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between"
    >
      <div class="min-w-0 space-y-1">
        <div class="flex items-center justify-between gap-3">
          <h1 class="text-lg font-semibold tracking-tight">
            {{ authSettingsTitle }}
          </h1>
          <DocsLinkButton class="sm:hidden" :href="docsUrls.guides.auth" />
        </div>
        <p class="text-sm text-muted-foreground">
          {{ authSettingsDescription }}
        </p>
      </div>
      <div class="flex w-full items-center gap-2 sm:w-auto">
        <DocsLinkButton
          class="hidden sm:inline-flex"
          :href="docsUrls.guides.auth"
          size="default"
        />
        <div
          class="grid flex-1 grid-cols-[minmax(0,1fr)_auto] gap-0 sm:flex-none"
        >
          <Button
            class="h-11 min-w-0 rounded-r-none sm:h-9"
            @click="handlePrimaryAuthAction"
          >
            <Plus class="h-4 w-4" aria-hidden="true" />
            {{ primaryAuthActionLabel }}
          </Button>
          <DropdownMenu>
            <DropdownMenuTrigger as-child>
              <Button
                size="icon"
                class="h-11 rounded-l-none border-l border-primary-foreground/25 px-2 sm:h-9"
                :aria-label="t('admin.authSettings.moreActions')"
                :title="t('admin.authSettings.moreActions')"
              >
                <ChevronDown class="h-4 w-4" />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end" class="w-56">
              <DropdownMenuItem
                :disabled="isCredentialTransferBusy"
                @select="showCredentialTransferDialog = true"
              >
                <FileKey2 class="mr-2 h-4 w-4" />
                {{ t("admin.authSettings.credentialTransfer") }}
              </DropdownMenuItem>
              <DropdownMenuItem
                :disabled="isAuthModeBusy"
                @select="openAuthModeSwitchDialog"
              >
                <RefreshCw
                  class="mr-2 h-4 w-4"
                  :class="{ 'animate-spin': isAuthModeBusy }"
                />
                {{ t("admin.authSettings.switchAuthMode") }}
              </DropdownMenuItem>
              <DropdownMenuItem @select="goToOidcProviders">
                <Settings2 class="mr-2 h-4 w-4" />
                {{ t("admin.authSettings.oidcLogin") }}
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
        </div>
      </div>
    </div>

    <Card>
      <TotpCredentialTable
        v-if="authLoginMode === 'totp'"
        :credentials="credentials"
        :get-subdomain-access-preview="getSubdomainAccessPreview"
        :get-subdomain-access-summary="getSubdomainAccessSummary"
        :go-to-passkeys="goToPasskeys"
        :handle-admin-panel-access-tooltip-click="
          handleAdminPanelAccessTooltipClick
        "
        :handle-admin-panel-access-tooltip-open-change="
          handleAdminPanelAccessTooltipOpenChange
        "
        :handle-delete="handleDelete"
        :handle-docker-admin-panel-access-change="
          handleDockerAdminPanelAccessChange
        "
        :has-docker-admin-panel-access="hasDockerAdminPanelAccess"
        :is-access-scope-updating="isAccessScopeUpdating"
        :is-admin-panel-access-tooltip-open="isAdminPanelAccessTooltipOpen"
        :is-deleting="isDeleting"
        :is-loading="isLoading"
        :is-subdomain-access-updating="isSubdomainAccessUpdating"
        :open-subdomain-access-dialog="openSubdomainAccessDialog"
        :save-comment="saveComment"
        :show-admin-panel-access-column="showAdminPanelAccessColumn"
        :show-loading-skeleton="showLoadingSkeleton"
        :table-class="totpTableClass"
        :table-colspan="totpTableColspan"
        :validate-comment="validateComment"
      />
      <AuthAccountTable
        v-else
        :accounts="authAccounts"
        :get-subdomain-access-preview="getSubdomainAccessPreview"
        :get-subdomain-access-summary="getSubdomainAccessSummary"
        :handle-admin-panel-access-tooltip-click="
          handleAdminPanelAccessTooltipClick
        "
        :handle-admin-panel-access-tooltip-open-change="
          handleAdminPanelAccessTooltipOpenChange
        "
        :handle-delete="handleDeleteAccount"
        :handle-docker-admin-panel-access-change="
          handleAccountDockerAdminPanelAccessChange
        "
        :has-docker-admin-panel-access="hasDockerAdminPanelAccess"
        :is-access-scope-updating="isAccessScopeUpdating"
        :is-admin-panel-access-tooltip-open="isAdminPanelAccessTooltipOpen"
        :is-deleting="isDeleting"
        :is-loading="isLoading"
        :is-subdomain-access-updating="isSubdomainAccessUpdating"
        :open-create-account-dialog="openCreateAuthAccountDialog"
        :open-password-dialog="openAccountPasswordDialog"
        :open-subdomain-access-dialog="openAccountSubdomainAccessDialog"
        :save-username="saveAccountUsername"
        :show-admin-panel-access-column="showAdminPanelAccessColumn"
        :show-loading-skeleton="showLoadingSkeleton"
        :table-class="authAccountTableClass"
        :table-colspan="authAccountTableColspan"
        :username-security-warning="usernameSecurityWarning"
        :validate-username="validateAccountUsername"
      />
    </Card>
  </div>

  <AuthModeSwitchDialog
    v-model:open="showAuthModeSwitchDialog"
    :current-mode="authLoginMode"
    :accounts="authAccounts"
    :preview="authModePreview"
    :is-previewing="isPreviewingAuthMode"
    :is-switching="isSwitchingAuthMode"
    @bind-totp="openAccountTotpSetupDialogFromSwitch"
    @confirm="handleSwitchAuthMode"
    @edit-account="openAuthAccountDialog"
    @set-password="openAccountPasswordDialogFromSwitch"
  />

  <input
    :ref="setCredentialImportInput"
    type="file"
    accept=".json,application/json"
    class="hidden"
    @change="handleCredentialImportFileChange"
  />

  <CredentialTransferDialogs
    v-model:credential-transfer-open="showCredentialTransferDialog"
    v-model:export-open="showExportDialog"
    v-model:import-open="showImportDialog"
    :credential-count="exportableCredentialCount"
    :is-credential-transfer-busy="isCredentialTransferBusy"
    :is-exporting-credentials="isExportingCredentials"
    :is-importing-credentials="isImportingCredentials"
    :pending-credential-import-filename="pendingCredentialImportFilename"
    @export-from-transfer="openExportDialogFromCredentialTransferDialog"
    @import-from-transfer="triggerImportFilePickerFromCredentialTransferDialog"
    @confirm-export="handleExportCredentials"
    @confirm-import="handleImportCredentials"
    @reset-import="resetPendingCredentialImport"
  />

  <SubdomainAccessDialog
    v-model:open="showSubdomainAccessDialog"
    v-model:mode="subdomainAccessMode"
    v-model:search="subdomainAccessSearch"
    :has-target="
      Boolean(editingSubdomainAccessTotp || editingSubdomainAccessAccount)
    "
    :is-saving="isSavingSubdomainAccess"
    :option-count="subdomainAccessOptions.length"
    :options="filteredSubdomainAccessOptions"
    :selected-count="selectedSubdomainHostCount"
    :selected-hosts="selectedSubdomainHosts"
    :target-name="
      editingSubdomainAccessAccount?.username ||
      editingSubdomainAccessTotp?.comment ||
      t('admin.authSettings.tokenFallback')
    "
    @clear-selected="clearSelectedSubdomainHosts"
    @close="closeSubdomainAccessDialog"
    @save="handleSaveSubdomainAccess"
    @select-all-filtered="selectAllFilteredSubdomainHosts"
    @toggle-host="toggleSubdomainHost"
  />

  <Dialog
    :open="showAuthAccountDialog"
    @update:open="
      showAuthAccountDialog = $event;
      if (!$event) closeAuthAccountDialog();
    "
  >
    <DialogContent class="max-h-[88vh] overflow-y-auto sm:max-w-[440px]">
      <DialogHeader>
        <DialogTitle>
          {{ t("admin.authSettings.editAccount") }}
        </DialogTitle>
        <DialogDescription>
          {{ t("admin.authSettings.editAccountDescription") }}
        </DialogDescription>
      </DialogHeader>
      <div class="space-y-4">
        <div class="space-y-2">
          <Label for="auth-account-username">
            {{ t("admin.authSettings.accountUsername") }}
          </Label>
          <Input
            id="auth-account-username"
            v-model="authAccountUsernameInput"
            autocomplete="off"
            :disabled="isSavingAuthAccount"
            @keyup.enter="handleSaveAuthAccount"
          />
          <p
            v-if="usernameSecurityWarning(authAccountUsernameInput)"
            class="text-xs text-amber-600 dark:text-amber-400"
            role="status"
          >
            {{ usernameSecurityWarning(authAccountUsernameInput) }}
          </p>
        </div>
      </div>
      <DialogFooter class="gap-2">
        <Button
          variant="outline"
          :disabled="isSavingAuthAccount"
          @click="closeAuthAccountDialog"
        >
          {{ t("admin.authSettings.cancel") }}
        </Button>
        <Button :disabled="isSavingAuthAccount" @click="handleSaveAuthAccount">
          <span
            v-if="isSavingAuthAccount"
            class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
          ></span>
          {{ t("common.save") }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>

  <Dialog
    :open="showAccountPasswordDialog"
    @update:open="
      showAccountPasswordDialog = $event;
      if (!$event) closeAccountPasswordDialog();
    "
  >
    <DialogContent class="max-h-[88vh] overflow-y-auto sm:max-w-[440px]">
      <DialogHeader>
        <DialogTitle>
          {{ accountPasswordDialogTitle }}
        </DialogTitle>
        <DialogDescription>
          {{ accountPasswordDialogDescription }}
        </DialogDescription>
      </DialogHeader>
      <div class="space-y-4">
        <template v-if="isAccountPasswordSetupMode">
          <div class="space-y-2">
            <Label for="auth-account-setup-username">
              {{ t("admin.authSettings.accountUsername") }}
            </Label>
            <Input
              id="auth-account-setup-username"
              v-model="accountPasswordUsernameInput"
              autocomplete="off"
              :disabled="isSavingAccountPassword"
              @keyup.enter="handleSaveAccountPassword"
            />
            <p
              v-if="usernameSecurityWarning(accountPasswordUsernameInput)"
              class="text-xs text-amber-600 dark:text-amber-400"
              role="status"
            >
              {{ usernameSecurityWarning(accountPasswordUsernameInput) }}
            </p>
          </div>
        </template>
        <div class="space-y-2">
          <Label>{{ t("admin.authSettings.password") }}</Label>
          <div class="relative">
            <Input
              v-model="accountPasswordInput"
              :type="isAccountPasswordVisible ? 'text' : 'password'"
              autocomplete="new-password"
              class="pr-10"
              :disabled="isSavingAccountPassword"
              @keyup.enter="handleSaveAccountPassword"
            />
            <Button
              type="button"
              variant="ghost"
              size="icon-sm"
              class="absolute right-1 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground"
              :disabled="isSavingAccountPassword"
              :title="
                isAccountPasswordVisible
                  ? t('admin.authSettings.hidePassword')
                  : t('admin.authSettings.showPassword')
              "
              :aria-label="
                isAccountPasswordVisible
                  ? t('admin.authSettings.hidePassword')
                  : t('admin.authSettings.showPassword')
              "
              @click="isAccountPasswordVisible = !isAccountPasswordVisible"
            >
              <component
                :is="isAccountPasswordVisible ? EyeOff : Eye"
                class="h-4 w-4"
              />
            </Button>
          </div>
          <p class="text-xs text-muted-foreground">
            {{ t("admin.authSettings.passwordRuleHint") }}
          </p>
          <p
            v-if="passwordSecurityWarning(accountPasswordInput)"
            class="text-xs text-amber-600 dark:text-amber-400"
            role="status"
          >
            {{ passwordSecurityWarning(accountPasswordInput) }}
          </p>
        </div>
      </div>
      <DialogFooter class="gap-2">
        <Button
          variant="outline"
          :disabled="isSavingAccountPassword"
          @click="closeAccountPasswordDialog"
        >
          {{ t("admin.authSettings.cancel") }}
        </Button>
        <Button
          :disabled="isSavingAccountPassword"
          @click="handleSaveAccountPassword"
        >
          <span
            v-if="isSavingAccountPassword"
            class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
          ></span>
          {{ t("common.save") }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>

  <TotpSetupDialog
    v-model:comment="newTotpComment"
    v-model:open="showSetupDialog"
    v-model:verify-token="verifyToken"
    :bind-error-message="bindErrorMessage"
    :bind-view="setupBindView"
    :description="setupDialogDescription"
    :enter-from-class="setupBindTransitionEnterFromClass"
    :is-binding="isBinding"
    :leave-to-class="setupBindTransitionLeaveToClass"
    :secret-display="setupSecretDisplay"
    :setup-data="setupData"
    :step="setupStep"
    :title="setupDialogTitle"
    @bind="handleBind"
    @cancel="handleCancelSetup"
    @copy-secret="copySetupSecret"
    @open-manual="openManualSetupView"
    @return-to-qr="returnQRCodeSetupView"
    @save-name="handleSaveSetupName"
  />
</template>

<script setup lang="ts">
import {
  ref,
  onMounted,
  computed,
} from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import { Card } from "@/components/ui/card";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import {
  ChevronDown,
  Eye,
  EyeOff,
  FileKey2,
  Plus,
  RefreshCw,
  Settings2,
} from "lucide-vue-next";
import DocsLinkButton from "@/components/DocsLinkButton.vue";
import AuthAccountTable from "./auth-settings/AuthAccountTable.vue";
import AuthModeSwitchDialog from "./auth-settings/AuthModeSwitchDialog.vue";
import CredentialTransferDialogs from "./auth-settings/CredentialTransferDialogs.vue";
import SubdomainAccessDialog from "./auth-settings/SubdomainAccessDialog.vue";
import TotpCredentialTable from "./auth-settings/TotpCredentialTable.vue";
import TotpSetupDialog from "./auth-settings/TotpSetupDialog.vue";
import { useAuthCredentialTransfer } from "./auth-settings/useAuthCredentialTransfer";
import { useAuthSubdomainAccess } from "./auth-settings/useAuthSubdomainAccess";
import { useDockerAdminAccessScopes } from "./auth-settings/useDockerAdminAccessScopes";
import { Label } from "@/components/ui/label";
import { Input } from "@/components/ui/input";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { useDelayedLoading } from "@admin-shared/composables/useDelayedLoading";
import { useMediaQueryMatch } from "@admin-shared/composables/useMediaQueryMatch";
import { copyTextToClipboard } from "@admin-shared/utils/copyTextToClipboard";
import { ConfigAPI } from "../lib/api";
import { docsUrls } from "../lib/docs";
import { useDockerAdminAuthStore } from "../store/dockerAdminAuth";
import { toast } from "@admin-shared/utils/toast";
import type {
  AuthAccount,
  AuthLoginMode,
  AuthLoginModePreview,
  AuthLoginModeStatus,
  HostMapping,
  TOTPCredential,
  TOTPSubdomainAccess,
} from "../types";

const BUILTIN_SELECT_PAGE_ACCESS_HOST = "__builtin_select__";
const BUILTIN_SELECT_PAGE_PATH = "/__select__";
const DEFAULT_SUBDOMAIN_ACCESS: TOTPSubdomainAccess = {
  mode: "all",
  hosts: [],
};

type SubdomainAccessOption = {
  host: string;
  label: string;
  description: string;
  stale?: boolean;
  builtin?: boolean;
};

type AuthPermissionRecord = Pick<
  TOTPCredential,
  "id" | "access_scopes" | "subdomain_access"
>;

const { t } = useI18n();
const router = useRouter();
const dockerAdminAuthStore = useDockerAdminAuthStore();

const credentials = ref<TOTPCredential[]>([]);
const authAccounts = ref<AuthAccount[]>([]);
const authLoginMode = ref<AuthLoginMode>("totp");
const authModeStatus = ref<AuthLoginModeStatus | null>(null);
const authModePreview = ref<AuthLoginModePreview | null>(null);
const hostMappings = ref<HostMapping[]>([]);
const openAdminPanelAccessTooltipId = ref<string | null>(null);
const isTouchInteraction = useMediaQueryMatch(
  "(hover: none), (pointer: coarse)",
);
const showAuthModeSwitchDialog = ref(false);
const showAuthAccountDialog = ref(false);
const showAccountPasswordDialog = ref(false);
const editingAuthAccount = ref<AuthAccount | null>(null);
const authAccountUsernameInput = ref("");
const isCreatingAuthAccount = ref(false);
const editingPasswordAccount = ref<AuthAccount | null>(null);
const accountPasswordUsernameInput = ref("");
const accountPasswordInput = ref("");
const isAccountPasswordVisible = ref(false);
const { isPending: isLoading, run: runLoadStatus } = useAsyncAction({
  onError: (error) => {
    console.error("Failed to get TOTP status:", error);
  },
});
const showLoadingSkeleton = useDelayedLoading(isLoading);
const {
  clearSelectedSubdomainHosts,
  closeSubdomainAccessDialog,
  editingSubdomainAccessAccount,
  editingSubdomainAccessTotp,
  getSubdomainAccess,
  handleSaveSubdomainAccess,
  isSavingSubdomainAccess,
  isSubdomainAccessUpdating,
  openAccountSubdomainAccessDialog,
  openSubdomainAccessDialog,
  selectHosts,
  selectedSubdomainHosts,
  showSubdomainAccessDialog,
  subdomainAccessMode,
  subdomainAccessSearch,
  toggleSubdomainHost,
} = useAuthSubdomainAccess({
  compareHosts: compareSubdomainAccessHosts,
  credentials,
  normalizeAccess: normalizeTOTPSubdomainAccess,
  normalizeCredential,
  normalizeHost: normalizeSubdomainHost,
  replaceAuthAccount,
  translate: (key) => t(key),
});
const {
  handleAccountDockerAdminPanelAccessChange,
  handleDockerAdminPanelAccessChange,
  hasDockerAdminPanelAccess,
  isAccessScopeUpdating,
} = useDockerAdminAccessScopes({
  credentials,
  replaceAuthAccount,
  translate: (key) => t(key),
});
const {
  exportableCredentialCount,
  handleCredentialImportFileChange,
  handleExportCredentials,
  handleImportCredentials,
  isCredentialTransferBusy,
  isExportingCredentials,
  isImportingCredentials,
  openExportDialogFromCredentialTransferDialog,
  pendingCredentialImportFilename,
  resetPendingCredentialImport,
  setCredentialImportInput,
  showCredentialTransferDialog,
  showExportDialog,
  showImportDialog,
  triggerImportFilePickerFromCredentialTransferDialog,
} = useAuthCredentialTransfer({
  authAccounts,
  authLoginMode,
  credentials,
  refreshStatus: fetchStatus,
  translate: (key, params) => (params ? t(key, params) : t(key)),
});
const isAccountPasswordSetupMode = computed(
  () =>
    isCreatingAuthAccount.value ||
    editingPasswordAccount.value?.passwordConfigured === false,
);
const accountPasswordDialogTitle = computed(() => {
  if (isCreatingAuthAccount.value) {
    return t("admin.authSettings.createAccount");
  }
  if (isAccountPasswordSetupMode.value) {
    return t("admin.authSettings.setupAccountPassword");
  }
  return editingPasswordAccount.value?.passwordConfigured
    ? t("admin.authSettings.changePassword")
    : t("admin.authSettings.setPassword");
});
const accountPasswordDialogDescription = computed(() => {
  if (isCreatingAuthAccount.value) {
    return t("admin.authSettings.createAccountDescription");
  }
  if (isAccountPasswordSetupMode.value) {
    return t("admin.authSettings.setupAccountPasswordDescription");
  }
  return t("admin.authSettings.accountPasswordDescription", {
    username: editingPasswordAccount.value?.username || "",
  });
});
const { isPending: isPreviewingAuthMode, run: runPreviewAuthMode } =
  useAsyncAction({
    onError: (error) => {
      toast.error(
        extractErrorMessage(
          error,
          t("admin.authSettings.previewAuthModeFailed"),
        ),
      );
    },
  });
const { isPending: isSwitchingAuthMode, run: runSwitchAuthMode } =
  useAsyncAction({
    onError: (error) => {
      toast.error(
        extractErrorMessage(
          error,
          t("admin.authSettings.switchAuthModeFailed"),
        ),
      );
    },
  });
const { isPending: isSavingAccountPassword, run: runSaveAccountPassword } =
  useAsyncAction({
    onError: (error) => {
      toast.error(
        extractErrorMessage(
          error,
          t("admin.authSettings.accountPasswordSaveFailed"),
        ),
      );
    },
  });
const { isPending: isSavingAuthAccount, run: runSaveAuthAccount } =
  useAsyncAction({
    onError: (error) => {
      toast.error(
        extractErrorMessage(error, t("admin.authSettings.accountSaveFailed")),
      );
    },
  });

// Setup state
const showSetupDialog = ref(false);
const setupData = ref<{ secret: string; uri: string } | null>(null);
const bindingTotpAccount = ref<AuthAccount | null>(null);
const reopenAuthModeSwitchAfterTotpBind = ref(false);
const reopenAuthModeSwitchAfterPasswordSave = ref(false);
const verifyToken = ref("");
const newTotpComment = ref("");
const bindErrorMessage = ref("");
const setupStep = ref<"BIND" | "NAME">("BIND");
const setupBindView = ref<"qr" | "manual">("qr");
const setupBindMotionDirection = ref<"forward" | "back">("forward");
const boundTotpId = ref<string | null>(null);
const bindingMode = ref<"bind" | "rename">("bind");
const { isPending: isBinding, run: runBindingAction } = useAsyncAction({
  onError: (error) => {
    const fallback =
      bindingMode.value === "bind"
        ? t("admin.authSettings.bindError")
        : t("admin.authSettings.renameError");
    bindErrorMessage.value = extractErrorMessage(error, fallback);
    if (bindingMode.value === "bind") {
      verifyToken.value = "";
    }
  },
});
const { run: runSetupInit } = useAsyncAction({
  onError: (error) => {
    console.error("Failed to setup TOTP:", error);
    bindErrorMessage.value = t("admin.authSettings.setupFailed");
    setupData.value = null;
  },
});
const { run: runSaveComment } = useAsyncAction({
  rethrow: true,
});
// Delete state
const { isPending: isDeleting, run: runDeleteCredential } = useAsyncAction({
  onError: (error) => {
    toast.error(
      extractErrorMessage(error, t("admin.authSettings.deleteFailed")),
    );
  },
});

const showAdminPanelAccessColumn = computed(() => {
  const target = dockerAdminAuthStore.state?.deployment_target;
  return target === "docker" || target === "openwrt";
});
const totpTableClass = computed(() =>
  showAdminPanelAccessColumn.value
    ? "min-w-[920px] table-fixed"
    : "min-w-[780px] table-fixed",
);
const totpTableColspan = computed(() =>
  showAdminPanelAccessColumn.value ? 6 : 5,
);
const authAccountTableClass = computed(() =>
  showAdminPanelAccessColumn.value
    ? "min-w-[840px] table-fixed"
    : "min-w-[680px] table-fixed",
);
const authAccountTableColspan = computed(() =>
  showAdminPanelAccessColumn.value ? 4 : 3,
);
const isAuthModeBusy = computed(
  () => isPreviewingAuthMode.value || isSwitchingAuthMode.value,
);
const authSettingsTitle = computed(() =>
  authLoginMode.value === "password"
    ? t("admin.authSettings.passwordAccountsTitle")
    : t("admin.authSettings.title"),
);
const authSettingsDescription = computed(() =>
  authLoginMode.value === "password"
    ? t("admin.authSettings.passwordAccountsDescription")
    : t("admin.authSettings.description"),
);
const primaryAuthActionLabel = computed(() =>
  authLoginMode.value === "password"
    ? t("admin.authSettings.createAccount")
    : t("admin.authSettings.bindNewToken"),
);
const targetAuthLoginMode = computed<AuthLoginMode>(() =>
  authLoginMode.value === "totp" ? "password" : "totp",
);
const setupSecretDisplay = computed(() => {
  const secret = setupData.value?.secret || "";
  return formatTOTPSecretForDisplay(secret);
});
const setupDialogTitle = computed(() =>
  bindingTotpAccount.value
    ? t("admin.authSettings.accountTotpBindDialogTitle")
    : t("admin.authSettings.bindDialogTitle"),
);
const setupDialogDescription = computed(() =>
  bindingTotpAccount.value
    ? t("admin.authSettings.accountTotpBindDialogDescription", {
        username: bindingTotpAccount.value.username,
      })
    : t("admin.authSettings.bindDialogDescription"),
);
const setupBindTransitionEnterFromClass = computed(() => {
  return setupBindMotionDirection.value === "forward"
    ? "translate-x-4 opacity-0"
    : "-translate-x-4 opacity-0";
});
const setupBindTransitionLeaveToClass = computed(() => {
  return setupBindMotionDirection.value === "forward"
    ? "-translate-x-4 opacity-0"
    : "translate-x-4 opacity-0";
});
const selectedSubdomainHostCount = computed(
  () => selectedSubdomainHosts.value.size,
);
const subdomainAccessOptions = computed<SubdomainAccessOption[]>(() => {
  const byHost = new Map<string, SubdomainAccessOption>();
  byHost.set(BUILTIN_SELECT_PAGE_ACCESS_HOST, {
    host: BUILTIN_SELECT_PAGE_ACCESS_HOST,
    label: t("admin.authSettings.permissionBuiltinSelectLabel"),
    description: BUILTIN_SELECT_PAGE_PATH,
    builtin: true,
  });

  for (const mapping of hostMappings.value) {
    if (mapping.service_role === "auth" || mapping.use_auth !== true) {
      continue;
    }
    const host = normalizeSubdomainHost(mapping.host);
    if (!host || byHost.has(host)) continue;
    const label =
      mapping.title_override.trim() || mapping.title.trim() || mapping.host;
    byHost.set(host, {
      host,
      label,
      description: host,
      stale: false,
    });
  }

  for (const host of selectedSubdomainHosts.value) {
    if (byHost.has(host)) continue;
    byHost.set(host, {
      host,
      label: host,
      description: host,
      stale: true,
    });
  }

  const options = [...byHost.values()];
  return [
    ...options.filter((option) => option.builtin),
    ...options
      .filter((option) => !option.builtin)
      .sort((left, right) => left.host.localeCompare(right.host)),
  ];
});
const filteredSubdomainAccessOptions = computed(() => {
  const keyword = subdomainAccessSearch.value.trim().toLowerCase();
  if (!keyword) return subdomainAccessOptions.value;
  return subdomainAccessOptions.value.filter(
    (option) =>
      option.host.includes(keyword) ||
      option.description.toLowerCase().includes(keyword) ||
      option.label.toLowerCase().includes(keyword),
  );
});

onMounted(async () => {
  await fetchStatus();
});

async function fetchStatus() {
  await runLoadStatus(async () => {
    const [res, mappings, modeStatus, accounts] = await Promise.all([
      ConfigAPI.getTOTPStatus(),
      ConfigAPI.getHostMappings().catch((error) => {
        console.error("Failed to get host mappings:", error);
        return [] as HostMapping[];
      }),
      ConfigAPI.getAuthLoginMode(),
      ConfigAPI.getAuthAccounts().catch((error) => {
        console.error("Failed to get auth accounts:", error);
        return [] as AuthAccount[];
      }),
    ]);
    hostMappings.value = mappings;
    credentials.value = (res.credentials || []).map(normalizeCredential);
    authModeStatus.value = modeStatus;
    authLoginMode.value = modeStatus.mode || "totp";
    authAccounts.value = (accounts || []).map(normalizeAuthAccount);
  });
}

function normalizeSubdomainHost(value: unknown) {
  const raw = String(value ?? "")
    .trim()
    .toLowerCase();
  if (!raw) return "";
  if (
    raw === BUILTIN_SELECT_PAGE_ACCESS_HOST ||
    raw === BUILTIN_SELECT_PAGE_PATH
  ) {
    return BUILTIN_SELECT_PAGE_ACCESS_HOST;
  }

  let host = raw;
  try {
    const parsed = new URL(raw.includes("://") ? raw : `https://${raw}`);
    host = parsed.hostname;
  } catch {
    const hostCandidate =
      raw
        .replace(/^[a-z][a-z0-9+.-]*:\/\//i, "")
        .replace(/^[^@/\s]+@/, "")
        .split(/[/?#]/, 1)[0] ?? "";
    host = hostCandidate.replace(/:\d+$/, "");
  }

  host = host.trim().toLowerCase().replace(/\.+$/, "");
  if (!host || host.includes("*") || /\s/.test(host)) return "";
  return host;
}

function compareSubdomainAccessHosts(left: string, right: string) {
  if (left === BUILTIN_SELECT_PAGE_ACCESS_HOST) return -1;
  if (right === BUILTIN_SELECT_PAGE_ACCESS_HOST) return 1;
  return left.localeCompare(right);
}

function formatSubdomainAccessHostLabel(host: string) {
  return host === BUILTIN_SELECT_PAGE_ACCESS_HOST
    ? t("admin.authSettings.permissionBuiltinSelectLabel")
    : host;
}

function normalizeTOTPSubdomainAccess(value: unknown): TOTPSubdomainAccess {
  if (
    typeof value !== "object" ||
    value === null ||
    (value as { mode?: unknown }).mode !== "custom"
  ) {
    return { ...DEFAULT_SUBDOMAIN_ACCESS };
  }

  const hostsValue = (value as { hosts?: unknown }).hosts;
  const hosts = Array.isArray(hostsValue)
    ? [...new Set(hostsValue.map(normalizeSubdomainHost).filter(Boolean))].sort(
        compareSubdomainAccessHosts,
      )
    : [];
  return {
    mode: "custom",
    hosts,
  };
}

function normalizeCredential(credential: TOTPCredential): TOTPCredential {
  return {
    ...credential,
    access_scopes: credential.access_scopes || [],
    subdomain_access: normalizeTOTPSubdomainAccess(credential.subdomain_access),
  };
}

function normalizeAuthAccount(account: AuthAccount): AuthAccount {
  return {
    ...account,
    displayName: account.username,
    access_scopes: account.access_scopes || [],
    subdomain_access: normalizeTOTPSubdomainAccess(account.subdomain_access),
    passwordConfigured: account.passwordConfigured === true,
    totpConfigured: account.totpConfigured === true,
  };
}

function replaceAuthAccount(account: AuthAccount) {
  const normalized = normalizeAuthAccount(account);
  const index = authAccounts.value.findIndex((item) => item.id === account.id);
  if (index >= 0) {
    authAccounts.value.splice(index, 1, normalized);
    return;
  }
  authAccounts.value.push(normalized);
}

function openAuthAccountDialog(account: AuthAccount) {
  editingAuthAccount.value = account;
  authAccountUsernameInput.value = account.username;
  showAuthAccountDialog.value = true;
}

function closeAuthAccountDialog() {
  showAuthAccountDialog.value = false;
  editingAuthAccount.value = null;
  authAccountUsernameInput.value = "";
}

function openCreateAuthAccountDialog() {
  isCreatingAuthAccount.value = true;
  editingPasswordAccount.value = null;
  reopenAuthModeSwitchAfterPasswordSave.value = false;
  accountPasswordUsernameInput.value = "";
  accountPasswordInput.value = "";
  isAccountPasswordVisible.value = false;
  showAccountPasswordDialog.value = true;
}

function handlePrimaryAuthAction() {
  if (authLoginMode.value === "password") {
    openCreateAuthAccountDialog();
    return;
  }

  void openSetupDialog();
}

function normalizeTOTPSecret(secret: string) {
  return secret.replace(/\s+/g, "").toUpperCase();
}

function splitTOTPSecretGroups(secret: string) {
  return normalizeTOTPSecret(secret).match(/.{1,4}/g) || [];
}

function formatTOTPSecretForDisplay(secret: string) {
  return splitTOTPSecretGroups(secret).join(" ");
}

async function openAuthModeSwitchDialog() {
  if (isAuthModeBusy.value) return;
  authModePreview.value = null;
  showAuthModeSwitchDialog.value = true;
  await refreshAuthModePreview();
}

async function refreshAuthModePreview() {
  await runPreviewAuthMode(async () => {
    authModePreview.value = await ConfigAPI.previewAuthLoginMode(
      targetAuthLoginMode.value,
    );
  });
}

async function handleSwitchAuthMode() {
  await runSwitchAuthMode(async () => {
    authModeStatus.value = await ConfigAPI.switchAuthLoginMode(
      targetAuthLoginMode.value,
    );
    showAuthModeSwitchDialog.value = false;
    authModePreview.value = null;
    await fetchStatus();
    toast.success(t("admin.authSettings.switchAuthModeCompleted"));
  });
}

async function handleSaveAuthAccount() {
  const account = editingAuthAccount.value;
  if (!account) return;
  const username = authAccountUsernameInput.value.trim();
  const validationMessage = validateAccountUsername(username, account);
  if (validationMessage) {
    toast.error(validationMessage);
    return;
  }
  await runSaveAuthAccount(async () => {
    const updated = await ConfigAPI.updateAuthAccount(account.id, {
      username,
    });
    replaceAuthAccount(updated);
    closeAuthAccountDialog();
    if (showAuthModeSwitchDialog.value) {
      await refreshAuthModePreview();
    }
    toast.success(t("admin.authSettings.accountSaved"));
  });
}

function validateAccountUsername(value: string, account: AuthAccount) {
  const username = value.trim();
  if (!username) {
    return t("admin.authSettings.accountUsernameRequired");
  }
  const isDuplicate = authAccounts.value.some(
    (item) => item.id !== account.id && item.username === username,
  );
  if (isDuplicate) {
    return t("admin.authSettings.accountUsernameDuplicate");
  }
}

function usernameSecurityWarning(value: string) {
  const username = value.trim();
  return username && username.length < 3
    ? t("admin.authSettings.shortUsernameWarning")
    : "";
}

function passwordSecurityWarning(password: string) {
  if (!password) return "";
  const hasLetters = /[A-Za-z]/.test(password);
  const hasNumbers = /\d/.test(password);
  return password.length < 6 ||
    /\s/.test(password) ||
    !hasLetters ||
    !hasNumbers
    ? t("admin.authSettings.weakPasswordWarning")
    : "";
}

async function saveAccountUsername(account: AuthAccount, value: string) {
  const username = value.trim();
  try {
    const updated = await ConfigAPI.updateAuthAccount(account.id, {
      username,
    });
    replaceAuthAccount(updated);
    if (showAuthModeSwitchDialog.value) {
      await refreshAuthModePreview();
    }
    toast.success(t("admin.authSettings.accountSaved"));
  } catch (error) {
    throw new Error(
      extractErrorMessage(error, t("admin.authSettings.accountSaveFailed")),
    );
  }
}

function openAccountPasswordDialog(
  account: AuthAccount,
  reopenSwitchAfterSave = false,
) {
  isCreatingAuthAccount.value = false;
  editingPasswordAccount.value = account;
  reopenAuthModeSwitchAfterPasswordSave.value = reopenSwitchAfterSave;
  accountPasswordUsernameInput.value = account.username;
  accountPasswordInput.value = "";
  isAccountPasswordVisible.value = false;
  showAccountPasswordDialog.value = true;
}

function openAccountPasswordDialogFromSwitch(account: AuthAccount) {
  showAuthModeSwitchDialog.value = false;
  openAccountPasswordDialog(account, true);
}

function closeAccountPasswordDialog() {
  showAccountPasswordDialog.value = false;
  isCreatingAuthAccount.value = false;
  editingPasswordAccount.value = null;
  reopenAuthModeSwitchAfterPasswordSave.value = false;
  accountPasswordUsernameInput.value = "";
  accountPasswordInput.value = "";
  isAccountPasswordVisible.value = false;
}

async function handleSaveAccountPassword() {
  const account = editingPasswordAccount.value;
  if (!isCreatingAuthAccount.value && !account) return;
  const password = accountPasswordInput.value;
  const username = accountPasswordUsernameInput.value.trim();
  if (isAccountPasswordSetupMode.value && !username) {
    toast.error(t("admin.authSettings.accountUsernameRequired"));
    return;
  }
  if (!password) {
    toast.error(t("admin.authSettings.accountPasswordRequired"));
    return;
  }
  await runSaveAccountPassword(async () => {
    let updated: AuthAccount | null = null;
    const wasCreating = isCreatingAuthAccount.value;
    if (isCreatingAuthAccount.value) {
      updated = await ConfigAPI.createAuthAccount({
        username,
        password,
      });
    } else if (isAccountPasswordSetupMode.value && account) {
      updated = await ConfigAPI.setupAuthAccount(account.id, {
        username,
        password,
      });
    } else if (account) {
      updated = await ConfigAPI.setAuthAccountPassword(account.id, password);
    }
    if (!updated) return;
    const shouldReopenSwitch = reopenAuthModeSwitchAfterPasswordSave.value;
    replaceAuthAccount(updated);
    closeAccountPasswordDialog();
    if (shouldReopenSwitch) {
      showAuthModeSwitchDialog.value = true;
      await refreshAuthModePreview();
    } else if (showAuthModeSwitchDialog.value) {
      await refreshAuthModePreview();
    }
    toast.success(
      t(
        wasCreating
          ? "admin.authSettings.accountCreated"
          : "admin.authSettings.accountPasswordSaved",
      ),
    );
  });
}

async function copySetupSecret() {
  const secret = setupData.value?.secret;
  if (!secret) return;

  try {
    const result = await copyTextToClipboard(secret);
    if (result.verified) {
      toast.success(t("admin.authSettings.setupSecretCopied"));
      return;
    }

    toast.info(t("admin.authSettings.setupSecretCopyUnverified"), {
      description: t("admin.authSettings.setupSecretCopyUnverifiedDescription"),
    });
  } catch (error) {
    console.error("copySetupSecret:", error);
    toast.error(t("admin.authSettings.setupSecretCopyFailed"), {
      description: t("admin.authSettings.setupSecretManualCopyHint"),
    });
  }
}

function openManualSetupView() {
  setupBindMotionDirection.value = "forward";
  setupBindView.value = "manual";
}

function returnQRCodeSetupView() {
  setupBindMotionDirection.value = "back";
  setupBindView.value = "qr";
}

function getSubdomainAccessSummary(record: AuthPermissionRecord) {
  const access = getSubdomainAccess(record);
  if (access.mode !== "custom") {
    return t("admin.authSettings.permissionAll");
  }
  if (access.hosts.length === 0) {
    return t("admin.authSettings.permissionCustomEmpty");
  }
  return t("admin.authSettings.permissionCustomSummary", {
    count: access.hosts.length,
  });
}

function getSubdomainAccessPreview(record: AuthPermissionRecord) {
  const access = getSubdomainAccess(record);
  if (access.mode !== "custom") return "";
  if (access.hosts.length === 0) {
    return t("admin.authSettings.permissionNoAllowedHosts");
  }
  const previewHosts = access.hosts
    .slice(0, 2)
    .map(formatSubdomainAccessHostLabel)
    .join(", ");
  if (access.hosts.length <= 2) return previewHosts;
  return t("admin.authSettings.permissionPreviewMore", {
    hosts: previewHosts,
    count: access.hosts.length,
  });
}

function selectAllFilteredSubdomainHosts() {
  selectHosts(
    filteredSubdomainAccessOptions.value.map((option) => option.host),
  );
}

function isAdminPanelAccessTooltipOpen(totpId: string) {
  return openAdminPanelAccessTooltipId.value === totpId;
}

function handleAdminPanelAccessTooltipOpenChange(
  totpId: string,
  nextOpen: boolean,
) {
  openAdminPanelAccessTooltipId.value = nextOpen ? totpId : null;
}

function handleAdminPanelAccessTooltipClick(totpId: string) {
  if (!isTouchInteraction.value) return;
  openAdminPanelAccessTooltipId.value =
    openAdminPanelAccessTooltipId.value === totpId ? null : totpId;
}

async function openSetupDialog() {
  bindingTotpAccount.value = null;
  reopenAuthModeSwitchAfterTotpBind.value = false;
  showSetupDialog.value = true;
  bindErrorMessage.value = "";
  verifyToken.value = "";
  newTotpComment.value = "";
  setupData.value = null;
  setupStep.value = "BIND";
  setupBindView.value = "qr";
  setupBindMotionDirection.value = "forward";
  boundTotpId.value = null;
  await runSetupInit(async () => {
    setupData.value = await ConfigAPI.setupTOTP();
  });
}

async function openAccountTotpSetupDialog(
  account: AuthAccount,
  reopenSwitchAfterBind = false,
) {
  bindingTotpAccount.value = account;
  reopenAuthModeSwitchAfterTotpBind.value = reopenSwitchAfterBind;
  showSetupDialog.value = true;
  bindErrorMessage.value = "";
  verifyToken.value = "";
  newTotpComment.value = account.username;
  setupData.value = null;
  setupStep.value = "BIND";
  setupBindView.value = "qr";
  setupBindMotionDirection.value = "forward";
  boundTotpId.value = null;
  await runSetupInit(async () => {
    setupData.value = await ConfigAPI.setupAuthAccountTOTP(account.id);
  });
}

async function openAccountTotpSetupDialogFromSwitch(account: AuthAccount) {
  showAuthModeSwitchDialog.value = false;
  await openAccountTotpSetupDialog(account, true);
}

function handleCancelSetup() {
  setupData.value = null;
  bindingTotpAccount.value = null;
  reopenAuthModeSwitchAfterTotpBind.value = false;
  verifyToken.value = "";
  bindErrorMessage.value = "";
  setupStep.value = "BIND";
  setupBindView.value = "qr";
  setupBindMotionDirection.value = "forward";
  boundTotpId.value = null;
}

async function handleBind() {
  const setup = setupData.value;
  if (!setup || verifyToken.value.length !== 6) return;
  bindingMode.value = "bind";
  bindErrorMessage.value = "";
  await runBindingAction(async () => {
    const account = bindingTotpAccount.value;
    if (account) {
      const updated = await ConfigAPI.bindAuthAccountTOTP(
        account.id,
        setup.secret,
        verifyToken.value,
      );
      const shouldReopenSwitch = reopenAuthModeSwitchAfterTotpBind.value;
      replaceAuthAccount(updated);
      await fetchStatus();
      showSetupDialog.value = false;
      bindingTotpAccount.value = null;
      reopenAuthModeSwitchAfterTotpBind.value = false;
      toast.success(t("admin.authSettings.accountTotpBound"));
      if (shouldReopenSwitch) {
        showAuthModeSwitchDialog.value = true;
        await refreshAuthModePreview();
      }
      return;
    }
    const randomSuffix = Math.random().toString(36).substring(2, 8);
    const randomName =
      t("admin.authSettings.randomDevicePrefix") + randomSuffix;
    await ConfigAPI.bindTOTP(setup.secret, verifyToken.value, randomName);
    await fetchStatus();

    const newCred = credentials.value.find((c) => c.comment === randomName);
    if (newCred) {
      boundTotpId.value = newCred.id;
      newTotpComment.value = randomName;
      setupStep.value = "NAME";
    } else {
      showSetupDialog.value = false;
    }
  });
}

async function handleSaveSetupName() {
  if (!newTotpComment.value.trim()) {
    bindErrorMessage.value = t("admin.authSettings.commentRequired");
    return;
  }
  if (
    credentials.value.some(
      (t) => t.comment === newTotpComment.value && t.id !== boundTotpId.value,
    )
  ) {
    bindErrorMessage.value = t("admin.authSettings.commentDuplicateDetailed");
    return;
  }
  const totpId = boundTotpId.value;
  if (!totpId) return;

  bindingMode.value = "rename";
  bindErrorMessage.value = "";
  await runBindingAction(async () => {
    await ConfigAPI.updateTOTPComment(totpId, newTotpComment.value);
    showSetupDialog.value = false;
    await fetchStatus();
    toast.success(t("admin.authSettings.deviceSaved"));
  });
}

function validateComment(newText: string, id: string) {
  if (credentials.value.some((t) => t.comment === newText && t.id !== id)) {
    return t("admin.authSettings.commentDuplicate");
  }
}

async function saveComment(id: string, newText: string) {
  await runSaveComment(() => ConfigAPI.updateTOTPComment(id, newText), {
    onSuccess: () => {
      const target = credentials.value.find((t) => t.id === id);
      if (target) {
        target.comment = newText;
      }
      toast.success(t("admin.authSettings.commentUpdated"));
    },
    onError: (error) => {
      throw new Error(
        extractErrorMessage(error, t("admin.authSettings.renameError")),
      );
    },
  });
}

async function handleDelete(totpId: string) {
  await runDeleteCredential(async () => {
    await ConfigAPI.deleteTOTP(totpId);
    await fetchStatus();
    toast.success(t("admin.authSettings.tokenDeleted"));
  });
}

async function handleDeleteAccount(accountId: string) {
  await runDeleteCredential(async () => {
    await ConfigAPI.deleteAuthAccount(accountId);
    await fetchStatus();
    toast.success(t("admin.authSettings.accountDeleted"));
  });
}

function goToPasskeys(totpId: string) {
  router.push(`/auth/passkeys/${encodeURIComponent(totpId)}`);
}

function goToOidcProviders() {
  router.push("/auth/oidc-providers");
}
</script>
