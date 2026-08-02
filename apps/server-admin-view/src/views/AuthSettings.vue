<template>
  <div
    class="dynamic-white-page-card dynamic-white-settings-surface h-full flex flex-col gap-4"
  >
    <div
      class="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between"
    >
      <div class="min-w-0 space-y-1">
        <div class="flex items-center justify-between gap-3">
          <h2 class="text-lg font-semibold tracking-tight">
            {{ authSettingsTitle }}
          </h2>
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
    :selected-count="selectedAccessCount"
    :selected-keys="selectedAccessKeys"
    :target-name="
      editingSubdomainAccessAccount?.username ||
      editingSubdomainAccessTotp?.comment ||
      t('admin.authSettings.tokenFallback')
    "
    @clear-selected="clearSelectedAccessOptions"
    @close="closeSubdomainAccessDialog"
    @save="handleSaveSubdomainAccess"
    @select-all-filtered="selectAllFilteredAccessOptions"
    @toggle-option="toggleAccessOption"
  />

  <AuthAccountEditDialog
    v-model:open="showAuthAccountDialog"
    v-model:username="authAccountUsernameInput"
    :is-saving="isSavingAuthAccount"
    :username-security-warning="usernameSecurityWarning"
    @close="closeAuthAccountDialog"
    @save="handleSaveAuthAccount"
  />

  <AuthAccountPasswordDialog
    v-model:is-password-visible="isAccountPasswordVisible"
    v-model:open="showAccountPasswordDialog"
    v-model:password="accountPasswordInput"
    v-model:username="accountPasswordUsernameInput"
    :description="accountPasswordDialogDescription"
    :is-saving="isSavingAccountPassword"
    :is-setup-mode="isAccountPasswordSetupMode"
    :password-security-warning="passwordSecurityWarning"
    :title="accountPasswordDialogTitle"
    :username-security-warning="usernameSecurityWarning"
    @close="closeAccountPasswordDialog"
    @save="handleSaveAccountPassword"
  />

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
import { ref, computed } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import { Card } from "@/components/ui/card";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Button } from "@/components/ui/button";
import {
  ChevronDown,
  FileKey2,
  Plus,
  RefreshCw,
  Settings2,
} from "lucide-vue-next";
import DocsLinkButton from "@/components/DocsLinkButton.vue";
import AuthAccountTable from "./auth-settings/AuthAccountTable.vue";
import AuthAccountEditDialog from "./auth-settings/AuthAccountEditDialog.vue";
import AuthAccountPasswordDialog from "./auth-settings/AuthAccountPasswordDialog.vue";
import AuthModeSwitchDialog from "./auth-settings/AuthModeSwitchDialog.vue";
import CredentialTransferDialogs from "./auth-settings/CredentialTransferDialogs.vue";
import SubdomainAccessDialog from "./auth-settings/SubdomainAccessDialog.vue";
import TotpCredentialTable from "./auth-settings/TotpCredentialTable.vue";
import TotpSetupDialog from "./auth-settings/TotpSetupDialog.vue";
import { useAuthCredentialTransfer } from "./auth-settings/useAuthCredentialTransfer";
import { useAuthAccountWorkflow } from "./auth-settings/useAuthAccountWorkflow";
import {
  normalizeAuthSubdomainAccess,
  useAuthSubdomainAccess,
} from "./auth-settings/useAuthSubdomainAccess";
import { useDockerAdminAccessScopes } from "./auth-settings/useDockerAdminAccessScopes";
import { useTotpSetupWorkflow } from "./auth-settings/useTotpSetupWorkflow";
import { useMediaQueryMatch } from "@admin-shared/composables/useMediaQueryMatch";
import { docsUrls } from "../lib/docs";
import { isProtectedAdminPanelDeploymentTarget } from "../lib/admin-panel-runtime";
import { useDockerAdminAuthStore } from "../store/dockerAdminAuth";
import type {
  AuthAccount,
  AuthLoginMode,
  AuthLoginModeStatus,
  HostMapping,
  StreamMapping,
  TOTPCredential,
} from "../types";
import { useAuthModeSwitch } from "./auth-settings/useAuthModeSwitch";
import { useAuthSettingsResource } from "./auth-settings/useAuthSettingsResource";

const { t } = useI18n();
const router = useRouter();
const dockerAdminAuthStore = useDockerAdminAuthStore();

const credentials = ref<TOTPCredential[]>([]);
const authAccounts = ref<AuthAccount[]>([]);
const authLoginMode = ref<AuthLoginMode>("totp");
const authModeStatus = ref<AuthLoginModeStatus | null>(null);
const hostMappings = ref<HostMapping[]>([]);
const streamMappings = ref<StreamMapping[]>([]);
const openAdminPanelAccessTooltipId = ref<string | null>(null);
const isTouchInteraction = useMediaQueryMatch(
  "(hover: none), (pointer: coarse)",
);
const {
  authModePreview,
  handleSwitchAuthMode,
  isAuthModeBusy,
  isPreviewingAuthMode,
  isSwitchingAuthMode,
  openAuthModeSwitchDialog,
  refreshAuthModePreview,
  showAuthModeSwitchDialog,
} = useAuthModeSwitch({
  authLoginMode,
  authModeStatus,
  refreshStatus: fetchStatus,
  translate: (key) => t(key),
});
const {
  accountPasswordDialogDescription,
  accountPasswordDialogTitle,
  accountPasswordInput,
  accountPasswordUsernameInput,
  authAccountUsernameInput,
  closeAccountPasswordDialog,
  closeAuthAccountDialog,
  handleSaveAccountPassword,
  handleSaveAuthAccount,
  isAccountPasswordSetupMode,
  isAccountPasswordVisible,
  isSavingAccountPassword,
  isSavingAuthAccount,
  normalizeAuthAccount,
  openAccountPasswordDialog,
  openAccountPasswordDialogFromSwitch,
  openAuthAccountDialog,
  openCreateAuthAccountDialog,
  passwordSecurityWarning,
  replaceAuthAccount,
  saveAccountUsername,
  showAccountPasswordDialog,
  showAuthAccountDialog,
  usernameSecurityWarning,
  validateAccountUsername,
} = useAuthAccountWorkflow({
  authAccounts,
  normalizeSubdomainAccess: normalizeAuthSubdomainAccess,
  refreshAuthModePreview,
  showAuthModeSwitchDialog,
});
const {
  clearSelectedAccessOptions,
  closeSubdomainAccessDialog,
  editingSubdomainAccessAccount,
  editingSubdomainAccessTotp,
  filteredSubdomainAccessOptions,
  getSubdomainAccessPreview,
  getSubdomainAccessSummary,
  handleSaveSubdomainAccess,
  isSavingSubdomainAccess,
  isSubdomainAccessUpdating,
  normalizeCredential,
  openAccountSubdomainAccessDialog,
  openSubdomainAccessDialog,
  selectedAccessKeys,
  selectedAccessCount,
  selectAllFilteredAccessOptions,
  showSubdomainAccessDialog,
  subdomainAccessMode,
  subdomainAccessOptions,
  subdomainAccessSearch,
  toggleAccessOption,
} = useAuthSubdomainAccess({
  credentials,
  hostMappings,
  streamMappings,
  replaceAuthAccount,
  translate: (key, params) => (params ? t(key, params) : t(key)),
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
  bindErrorMessage,
  copySetupSecret,
  handleBind,
  handleCancelSetup,
  handleSaveSetupName,
  isBinding,
  newTotpComment,
  openAccountTotpSetupDialog,
  openManualSetupView,
  openSetupDialog,
  returnQRCodeSetupView,
  setupBindTransitionEnterFromClass,
  setupBindTransitionLeaveToClass,
  setupBindView,
  setupData,
  setupDialogDescription,
  setupDialogTitle,
  setupSecretDisplay,
  setupStep,
  showSetupDialog,
  verifyToken,
} = useTotpSetupWorkflow({
  credentials,
  onReopenAuthModeSwitch: async () => {
    showAuthModeSwitchDialog.value = true;
    await refreshAuthModePreview();
  },
  refreshStatus: fetchStatus,
  replaceAuthAccount,
  translate: (key, params) => (params ? t(key, params) : t(key)),
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
const authSettingsResource = useAuthSettingsResource({
  authAccounts,
  authLoginMode,
  authModeStatus,
  credentials,
  hostMappings,
  streamMappings,
  normalizeAuthAccount,
  normalizeCredential,
  translate: (key) => t(key),
});
const {
  handleDelete,
  handleDeleteAccount,
  isDeleting,
  isLoading,
  saveComment,
  showLoadingSkeleton,
} = authSettingsResource;

const showAdminPanelAccessColumn = computed(() => {
  return isProtectedAdminPanelDeploymentTarget(
    dockerAdminAuthStore.state?.deployment_target,
  );
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
async function fetchStatus() {
  await authSettingsResource.fetchStatus();
}

function handlePrimaryAuthAction() {
  if (authLoginMode.value === "password") {
    openCreateAuthAccountDialog();
    return;
  }

  void openSetupDialog();
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

async function openAccountTotpSetupDialogFromSwitch(account: AuthAccount) {
  showAuthModeSwitchDialog.value = false;
  await openAccountTotpSetupDialog(account, true);
}

function validateComment(newText: string, id: string) {
  if (credentials.value.some((t) => t.comment === newText && t.id !== id)) {
    return t("admin.authSettings.commentDuplicate");
  }
}

function goToPasskeys(totpId: string) {
  router.push(`/auth/passkeys/${encodeURIComponent(totpId)}`);
}

function goToOidcProviders() {
  router.push("/auth/oidc-providers");
}
</script>
