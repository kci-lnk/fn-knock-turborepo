<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { Button } from "@/components/ui/button";
import { CardContent } from "@/components/ui/card";
import { Plus } from "lucide-vue-next";
import { Skeleton } from "@/components/ui/skeleton";
import { Switch } from "@/components/ui/switch";
import {
  Table,
  TableBody,
  TableCell,
  TableEmpty,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import InlineCommentEditor from "@admin-shared/components/InlineCommentEditor.vue";
import type { AuthAccount } from "../../types";

const props = defineProps<{
  accounts: AuthAccount[];
  title?: string;
  description?: string;
  getSubdomainAccessPreview: (account: AuthAccount) => string;
  getSubdomainAccessSummary: (account: AuthAccount) => string;
  handleAdminPanelAccessTooltipClick: (accountId: string) => void;
  handleAdminPanelAccessTooltipOpenChange: (
    accountId: string,
    open: boolean,
  ) => void;
  handleDelete: (accountId: string) => void | Promise<void>;
  handleDockerAdminPanelAccessChange: (
    account: AuthAccount,
    enabled: boolean,
  ) => void | Promise<void>;
  hasDockerAdminPanelAccess: (account: AuthAccount) => boolean;
  isAccessScopeUpdating: (accountId: string) => boolean;
  isAdminPanelAccessTooltipOpen: (accountId: string) => boolean;
  isDeleting: boolean;
  isLoading: boolean;
  isSubdomainAccessUpdating: (accountId: string) => boolean;
  openCreateAccountDialog: () => void;
  openPasswordDialog: (account: AuthAccount) => void;
  openSubdomainAccessDialog: (account: AuthAccount) => void;
  saveUsername: (account: AuthAccount, value: string) => Promise<void>;
  showAdminPanelAccessColumn: boolean;
  showLoadingSkeleton: boolean;
  tableClass: string;
  tableColspan: number;
  usernameSecurityWarning: (value: string) => string | undefined;
  validateUsername: (value: string, account: AuthAccount) => string | undefined;
}>();

const { t } = useI18n();
</script>

<template>
  <CardContent v-if="isLoading && showLoadingSkeleton && !accounts.length">
    <div
      v-if="title || description"
      class="mb-4 flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between"
    >
      <div class="space-y-1">
        <h3 v-if="title" class="text-sm font-semibold leading-6">
          {{ title }}
        </h3>
        <p v-if="description" class="text-sm leading-6 text-muted-foreground">
          {{ description }}
        </p>
      </div>
      <Button
        size="sm"
        class="w-full shrink-0 sm:w-auto"
        @click="openCreateAccountDialog"
      >
        <Plus class="mr-1 h-4 w-4" aria-hidden="true" />
        {{ t("admin.authSettings.createAccount") }}
      </Button>
    </div>
    <Table :class="tableClass" container-class="overflow-x-auto">
      <TableBody>
        <TableRow v-for="n in 4" :key="n">
          <TableCell><Skeleton class="h-4 w-32 max-w-full" /></TableCell>
          <TableCell><Skeleton class="h-8 w-40 max-w-full" /></TableCell>
          <TableCell v-if="showAdminPanelAccessColumn">
            <Skeleton class="h-6 w-24 max-w-full" />
          </TableCell>
          <TableCell class="text-right">
            <Skeleton class="ml-auto h-8 w-24 rounded-md" />
          </TableCell>
        </TableRow>
      </TableBody>
    </Table>
  </CardContent>
  <CardContent v-else-if="!isLoading || accounts.length">
    <div
      v-if="title || description"
      class="mb-4 flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between"
    >
      <div class="space-y-1">
        <h3 v-if="title" class="text-sm font-semibold leading-6">
          {{ title }}
        </h3>
        <p v-if="description" class="text-sm leading-6 text-muted-foreground">
          {{ description }}
        </p>
      </div>
      <Button
        size="sm"
        class="w-full shrink-0 sm:w-auto"
        @click="openCreateAccountDialog"
      >
        <Plus class="mr-1 h-4 w-4" aria-hidden="true" />
        {{ t("admin.authSettings.createAccount") }}
      </Button>
    </div>
    <Table :class="tableClass" container-class="overflow-x-auto">
      <colgroup>
        <col :class="showAdminPanelAccessColumn ? 'w-[28%]' : 'w-[30%]'" />
        <col :class="showAdminPanelAccessColumn ? 'w-[34%]' : 'w-[45%]'" />
        <col v-if="showAdminPanelAccessColumn" class="w-[18%]" />
        <col :class="showAdminPanelAccessColumn ? 'w-[20%]' : 'w-[25%]'" />
      </colgroup>
      <TableHeader>
        <TableRow class="hover:bg-transparent">
          <TableHead class="h-12 whitespace-normal px-5 text-sm font-semibold">
            {{ t("admin.authSettings.accountUsername") }}
          </TableHead>
          <TableHead class="h-12 whitespace-normal px-5 text-sm font-semibold">
            {{ t("admin.authSettings.permission") }}
          </TableHead>
          <TableHead
            v-if="showAdminPanelAccessColumn"
            class="h-12 whitespace-normal px-5 text-sm font-semibold"
          >
            {{ t("admin.authSettings.adminPanelAccess") }}
          </TableHead>
          <TableHead
            class="h-12 whitespace-normal px-5 text-right text-sm font-semibold"
          >
            {{ t("admin.authSettings.actions") }}
          </TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        <TableRow v-for="account in accounts" :key="account.id">
          <TableCell class="min-w-0 whitespace-normal px-5 py-4 font-medium">
            <InlineCommentEditor
              :text="account.username"
              :allow-empty="false"
              :warning="usernameSecurityWarning"
              :validate="(value) => validateUsername(value, account)"
              :save="(value) => saveUsername(account, value)"
            />
          </TableCell>
          <TableCell class="min-w-0 whitespace-normal px-5 py-4">
            <div class="flex min-w-0 flex-col gap-1.5">
              <button
                type="button"
                class="min-w-0 text-left text-sm font-medium leading-6 text-primary underline-offset-4 hover:underline disabled:pointer-events-none disabled:opacity-60"
                :disabled="isSubdomainAccessUpdating(account.id)"
                @click="openSubdomainAccessDialog(account)"
              >
                {{ getSubdomainAccessSummary(account) }}
              </button>
              <span
                v-if="getSubdomainAccessPreview(account)"
                class="truncate text-xs leading-5 text-muted-foreground"
                :title="getSubdomainAccessPreview(account)"
              >
                {{ getSubdomainAccessPreview(account) }}
              </span>
            </div>
          </TableCell>
          <TableCell v-if="showAdminPanelAccessColumn" class="px-5 py-4">
            <TooltipProvider>
              <Tooltip
                :open="isAdminPanelAccessTooltipOpen(account.id)"
                @update:open="
                  handleAdminPanelAccessTooltipOpenChange(account.id, $event)
                "
              >
                <TooltipTrigger as-child>
                  <div
                    class="inline-flex cursor-help items-center gap-3"
                    tabindex="0"
                    @click="handleAdminPanelAccessTooltipClick(account.id)"
                  >
                    <Switch
                      :model-value="hasDockerAdminPanelAccess(account)"
                      :disabled="isAccessScopeUpdating(account.id)"
                      :aria-label="t('admin.authSettings.adminPanelAccess')"
                      @update:model-value="
                        handleDockerAdminPanelAccessChange(
                          account,
                          $event === true,
                        )
                      "
                    />
                    <span class="text-xs leading-5 text-muted-foreground">
                      {{
                        hasDockerAdminPanelAccess(account)
                          ? t("admin.authSettings.adminPanelAllowed")
                          : t("admin.authSettings.adminPanelDenied")
                      }}
                    </span>
                  </div>
                </TooltipTrigger>
                <TooltipContent class="max-w-72 text-left">
                  <p>{{ t("admin.authSettings.adminPanelAccessTooltip") }}</p>
                </TooltipContent>
              </Tooltip>
            </TooltipProvider>
          </TableCell>
          <TableCell class="px-5 py-4">
            <div
              class="flex flex-wrap items-center justify-end gap-x-3 gap-y-2 whitespace-nowrap"
            >
              <Button
                size="sm"
                variant="link"
                class="h-8 px-2 text-sm font-medium"
                @click="openPasswordDialog(account)"
              >
                {{
                  account.passwordConfigured
                    ? t("admin.authSettings.edit")
                    : t("admin.authSettings.setPassword")
                }}
              </Button>
              <ConfirmDangerPopover
                :title="t('admin.authSettings.accountDeleteTitle')"
                :description="
                  t('admin.authSettings.accountDeleteDescription', {
                    name: account.username,
                  })
                "
                :loading="isDeleting"
                :disabled="isDeleting"
                :on-confirm="() => handleDelete(account.id)"
              >
                <template #trigger>
                  <Button
                    variant="link"
                    size="sm"
                    class="h-8 px-2 text-sm font-medium text-destructive hover:text-destructive"
                    :disabled="isDeleting"
                  >
                    {{ t("admin.authSettings.delete") }}
                  </Button>
                </template>
              </ConfirmDangerPopover>
            </div>
          </TableCell>
        </TableRow>
        <TableEmpty v-if="accounts.length === 0" :colspan="tableColspan">
          {{ t("admin.authSettings.emptyAccounts") }}
        </TableEmpty>
      </TableBody>
    </Table>
  </CardContent>
  <CardContent v-else class="min-h-[180px]" aria-hidden="true"></CardContent>
</template>
