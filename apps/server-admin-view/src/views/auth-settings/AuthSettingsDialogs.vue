<script setup lang="ts">
import { useI18n } from "vue-i18n";
import AuthAccountEditDialog from "./AuthAccountEditDialog.vue";
import AuthAccountPasswordDialog from "./AuthAccountPasswordDialog.vue";
import AuthModeSwitchDialog from "./AuthModeSwitchDialog.vue";
import CredentialTransferDialogs from "./CredentialTransferDialogs.vue";
import SubdomainAccessDialog from "./SubdomainAccessDialog.vue";
import TotpSetupDialog from "./TotpSetupDialog.vue";
import type { AuthSettingsPageController } from "./useAuthSettingsPage";

const props = defineProps<{ controller: AuthSettingsPageController }>();
const { t } = useI18n();
const {
  accountPasswordDialogDescription,
  accountPasswordDialogTitle,
  accountPasswordInput,
  accountPasswordUsernameInput,
  authAccountUsernameInput,
  authAccounts,
  authLoginMode,
  authModePreview,
  bindErrorMessage,
  clearSelectedAccessOptions,
  closeAccountPasswordDialog,
  closeAuthAccountDialog,
  closeSubdomainAccessDialog,
  copySetupSecret,
  editingSubdomainAccessAccount,
  editingSubdomainAccessTotp,
  exportableCredentialCount,
  filteredSubdomainAccessOptions,
  handleBind,
  handleCancelSetup,
  handleCredentialImportFileChange,
  handleExportCredentials,
  handleImportCredentials,
  handleSaveAccountPassword,
  handleSaveAuthAccount,
  handleSaveSetupName,
  handleSaveSubdomainAccess,
  handleSwitchAuthMode,
  isAccountPasswordSetupMode,
  isAccountPasswordVisible,
  isBinding,
  isCredentialTransferBusy,
  isExportingCredentials,
  isImportingCredentials,
  isPreviewingAuthMode,
  isSavingAccountPassword,
  isSavingAuthAccount,
  isSavingSubdomainAccess,
  isSwitchingAuthMode,
  newTotpComment,
  openAccountPasswordDialogFromSwitch,
  openAccountTotpSetupDialogFromSwitch,
  openAuthAccountDialog,
  openExportDialogFromCredentialTransferDialog,
  openManualSetupView,
  pendingCredentialImportFilename,
  passwordSecurityWarning,
  resetPendingCredentialImport,
  returnQRCodeSetupView,
  selectedAccessCount,
  selectedAccessKeys,
  selectAllFilteredAccessOptions,
  setCredentialImportInput,
  setupBindTransitionEnterFromClass,
  setupBindTransitionLeaveToClass,
  setupBindView,
  setupData,
  setupDialogDescription,
  setupDialogTitle,
  setupSecretDisplay,
  setupStep,
  showAccountPasswordDialog,
  showAuthAccountDialog,
  showAuthModeSwitchDialog,
  showCredentialTransferDialog,
  showExportDialog,
  showImportDialog,
  showSetupDialog,
  showSubdomainAccessDialog,
  subdomainAccessMode,
  subdomainAccessOptions,
  subdomainAccessSearch,
  toggleAccessOption,
  triggerImportFilePickerFromCredentialTransferDialog,
  usernameSecurityWarning,
  verifyToken,
} = props.controller;
</script>

<template>
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
    :has-target="Boolean(editingSubdomainAccessTotp || editingSubdomainAccessAccount)"
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
