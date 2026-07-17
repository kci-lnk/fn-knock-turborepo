<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { Button } from "@/components/ui/button";
import { CardContent } from "@/components/ui/card";
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
import HumanFriendlyTime from "@admin-shared/components/common/HumanFriendlyTime.vue";
import InlineCommentEditor from "@admin-shared/components/InlineCommentEditor.vue";
import type { TOTPCredential } from "../../types";

defineProps<{
  credentials: TOTPCredential[];
  getSubdomainAccessPreview: (totp: TOTPCredential) => string;
  getSubdomainAccessSummary: (totp: TOTPCredential) => string;
  goToPasskeys: (totpId: string) => void;
  handleAdminPanelAccessTooltipClick: (totpId: string) => void;
  handleAdminPanelAccessTooltipOpenChange: (
    totpId: string,
    open: boolean,
  ) => void;
  handleDelete: (totpId: string) => void | Promise<void>;
  handleDockerAdminPanelAccessChange: (
    totp: TOTPCredential,
    enabled: boolean,
  ) => void | Promise<void>;
  hasDockerAdminPanelAccess: (totp: TOTPCredential) => boolean;
  isAccessScopeUpdating: (totpId: string) => boolean;
  isAdminPanelAccessTooltipOpen: (totpId: string) => boolean;
  isDeleting: boolean;
  isLoading: boolean;
  isSubdomainAccessUpdating: (totpId: string) => boolean;
  openSubdomainAccessDialog: (totp: TOTPCredential) => void;
  saveComment: (id: string, value: string) => Promise<void>;
  showAdminPanelAccessColumn: boolean;
  showLoadingSkeleton: boolean;
  tableClass: string;
  tableColspan: number;
  validateComment: (value: string, id: string) => string | undefined;
}>();

const { t } = useI18n();
</script>

<template>
  <CardContent v-if="isLoading && showLoadingSkeleton && !credentials.length">
    <Table :class="tableClass" container-class="overflow-x-auto">
      <colgroup>
        <col :class="showAdminPanelAccessColumn ? 'w-[24%]' : 'w-[27%]'" />
        <col :class="showAdminPanelAccessColumn ? 'w-[16%]' : 'w-[18%]'" />
        <col :class="showAdminPanelAccessColumn ? 'w-[16%]' : 'w-[19%]'" />
        <col :class="showAdminPanelAccessColumn ? 'w-[18%]' : 'w-[22%]'" />
        <col v-if="showAdminPanelAccessColumn" class="w-[14%]" />
        <col :class="showAdminPanelAccessColumn ? 'w-[12%]' : 'w-[14%]'" />
      </colgroup>
      <TableHeader>
        <TableRow>
          <TableHead class="whitespace-normal">
            {{ t("admin.authSettings.comment") }}
          </TableHead>
          <TableHead class="whitespace-normal">
            {{ t("admin.authSettings.boundAt") }}
          </TableHead>
          <TableHead class="whitespace-normal">
            {{ t("admin.authSettings.deviceAssociation") }}
          </TableHead>
          <TableHead class="whitespace-normal">
            {{ t("admin.authSettings.permission") }}
          </TableHead>
          <TableHead
            v-if="showAdminPanelAccessColumn"
            class="whitespace-normal"
          >
            {{ t("admin.authSettings.adminPanelAccess") }}
          </TableHead>
          <TableHead class="text-right">
            {{ t("admin.authSettings.actions") }}
          </TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        <TableRow v-for="n in 4" :key="n">
          <TableCell><Skeleton class="h-4 w-40 max-w-full" /></TableCell>
          <TableCell><Skeleton class="h-4 w-36 max-w-full" /></TableCell>
          <TableCell><Skeleton class="h-4 w-52 max-w-full" /></TableCell>
          <TableCell><Skeleton class="h-8 w-40 max-w-full" /></TableCell>
          <TableCell v-if="showAdminPanelAccessColumn">
            <Skeleton class="h-6 w-24 max-w-full" />
          </TableCell>
          <TableCell class="text-right">
            <Skeleton class="ml-auto h-8 w-16 rounded-md sm:w-24" />
          </TableCell>
        </TableRow>
      </TableBody>
    </Table>
  </CardContent>
  <CardContent v-else-if="!isLoading || credentials.length">
    <Table :class="tableClass" container-class="overflow-x-auto">
      <colgroup>
        <col :class="showAdminPanelAccessColumn ? 'w-[24%]' : 'w-[27%]'" />
        <col :class="showAdminPanelAccessColumn ? 'w-[16%]' : 'w-[18%]'" />
        <col :class="showAdminPanelAccessColumn ? 'w-[16%]' : 'w-[19%]'" />
        <col :class="showAdminPanelAccessColumn ? 'w-[18%]' : 'w-[22%]'" />
        <col v-if="showAdminPanelAccessColumn" class="w-[14%]" />
        <col :class="showAdminPanelAccessColumn ? 'w-[12%]' : 'w-[14%]'" />
      </colgroup>
      <TableHeader>
        <TableRow>
          <TableHead class="whitespace-normal">
            {{ t("admin.authSettings.comment") }}
          </TableHead>
          <TableHead class="whitespace-normal">
            {{ t("admin.authSettings.boundAt") }}
          </TableHead>
          <TableHead class="whitespace-normal">
            {{ t("admin.authSettings.deviceAssociation") }}
          </TableHead>
          <TableHead class="whitespace-normal">
            {{ t("admin.authSettings.permission") }}
          </TableHead>
          <TableHead
            v-if="showAdminPanelAccessColumn"
            class="whitespace-normal"
          >
            {{ t("admin.authSettings.adminPanelAccess") }}
          </TableHead>
          <TableHead class="text-right">
            {{ t("admin.authSettings.actions") }}
          </TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        <TableRow v-for="totp in credentials" :key="totp.id">
          <TableCell class="min-w-0 whitespace-normal">
            <InlineCommentEditor
              :text="totp.comment"
              :allow-empty="false"
              :validate="(value) => validateComment(value, totp.id)"
              :save="(value) => saveComment(totp.id, value)"
            />
          </TableCell>
          <TableCell><HumanFriendlyTime :value="totp.createdAt" /></TableCell>
          <TableCell class="whitespace-normal">
            <Button
              variant="link"
              class="h-auto whitespace-normal p-0 text-left"
              @click="goToPasskeys(totp.id)"
            >
              {{ t("admin.authSettings.managePasskey") }}
            </Button>
          </TableCell>
          <TableCell class="min-w-0 whitespace-normal">
            <div class="flex min-w-0 flex-col gap-1">
              <button
                type="button"
                class="min-w-0 text-left text-sm font-medium text-primary underline-offset-4 hover:underline disabled:pointer-events-none disabled:opacity-60"
                :disabled="isSubdomainAccessUpdating(totp.id)"
                @click="openSubdomainAccessDialog(totp)"
              >
                {{ getSubdomainAccessSummary(totp) }}
              </button>
              <span
                v-if="getSubdomainAccessPreview(totp)"
                class="truncate text-xs text-muted-foreground"
                :title="getSubdomainAccessPreview(totp)"
              >
                {{ getSubdomainAccessPreview(totp) }}
              </span>
            </div>
          </TableCell>
          <TableCell v-if="showAdminPanelAccessColumn">
            <TooltipProvider>
              <Tooltip
                :open="isAdminPanelAccessTooltipOpen(totp.id)"
                @update:open="
                  handleAdminPanelAccessTooltipOpenChange(totp.id, $event)
                "
              >
                <TooltipTrigger as-child>
                  <div
                    class="inline-flex cursor-help items-center gap-2"
                    tabindex="0"
                    @click="handleAdminPanelAccessTooltipClick(totp.id)"
                  >
                    <Switch
                      :model-value="hasDockerAdminPanelAccess(totp)"
                      :disabled="isAccessScopeUpdating(totp.id)"
                      :aria-label="t('admin.authSettings.adminPanelAccess')"
                      @update:model-value="
                        handleDockerAdminPanelAccessChange(
                          totp,
                          $event === true,
                        )
                      "
                    />
                    <span class="text-xs text-muted-foreground">
                      {{
                        hasDockerAdminPanelAccess(totp)
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
          <TableCell class="text-right">
            <ConfirmDangerPopover
              :title="t('admin.authSettings.deleteTitle')"
              :description="
                t('admin.authSettings.deleteDescription', {
                  name: totp.comment || t('admin.authSettings.tokenFallback'),
                })
              "
              :loading="isDeleting"
              :disabled="isDeleting"
              :on-confirm="() => handleDelete(totp.id)"
            >
              <template #trigger>
                <Button variant="destructive" size="sm" :disabled="isDeleting">
                  {{ t("admin.authSettings.delete") }}
                </Button>
              </template>
            </ConfirmDangerPopover>
          </TableCell>
        </TableRow>
        <TableEmpty v-if="credentials.length === 0" :colspan="tableColspan">
          {{ t("admin.authSettings.empty") }}
        </TableEmpty>
      </TableBody>
    </Table>
  </CardContent>
  <CardContent v-else class="min-h-[180px]" aria-hidden="true"></CardContent>
</template>
