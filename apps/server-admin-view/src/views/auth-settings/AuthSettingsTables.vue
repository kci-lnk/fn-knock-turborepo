<script setup lang="ts">
import { Card } from "@/components/ui/card";
import AuthAccountTable from "./AuthAccountTable.vue";
import TotpCredentialTable from "./TotpCredentialTable.vue";
import type { AuthSettingsPageController } from "./useAuthSettingsPage";

const props = defineProps<{ controller: AuthSettingsPageController }>();
const {
  authAccountTableClass,
  authAccountTableColspan,
  authAccounts,
  authLoginMode,
  credentials,
  getSubdomainAccessPreview,
  getSubdomainAccessSummary,
  goToPasskeys,
  handleAccountDockerAdminPanelAccessChange,
  handleAdminPanelAccessTooltipClick,
  handleAdminPanelAccessTooltipOpenChange,
  handleDelete,
  handleDeleteAccount,
  handleDockerAdminPanelAccessChange,
  hasDockerAdminPanelAccess,
  isAccessScopeUpdating,
  isAdminPanelAccessTooltipOpen,
  isDeleting,
  isLoading,
  isSubdomainAccessUpdating,
  openAccountPasswordDialog,
  openAccountSubdomainAccessDialog,
  openCreateAuthAccountDialog,
  openSubdomainAccessDialog,
  saveAccountUsername,
  saveComment,
  showAdminPanelAccessColumn,
  showLoadingSkeleton,
  totpTableClass,
  totpTableColspan,
  usernameSecurityWarning,
  validateAccountUsername,
  validateComment,
} = props.controller;
</script>

<template>
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
</template>
