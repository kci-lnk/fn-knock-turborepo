<template>
  <div class="space-y-4">
    <Breadcrumb>
      <BreadcrumbList>
        <BreadcrumbItem>
          <BreadcrumbLink href="#/auth">{{
            t("admin.oidcProviders.breadcrumbTotp")
          }}</BreadcrumbLink>
        </BreadcrumbItem>
        <BreadcrumbSeparator />
        <BreadcrumbItem>
          <BreadcrumbPage>{{
            t("admin.oidcProviders.breadcrumbExternalLogin")
          }}</BreadcrumbPage>
        </BreadcrumbItem>
      </BreadcrumbList>
    </Breadcrumb>

    <Card>
      <CardHeader
        class="gap-4 sm:flex sm:flex-row sm:items-start sm:justify-between"
      >
        <div class="space-y-1.5">
          <CardTitle>{{ t("admin.oidcProviders.title") }}</CardTitle>
          <CardDescription>
            {{ t("admin.oidcProviders.description") }}
          </CardDescription>
        </div>
        <Button
          class="w-full sm:w-auto"
          :disabled="isLoading"
          @click="openCreateDialog"
        >
          <Plus class="h-4 w-4" />
          {{ t("admin.oidcProviders.addProvider") }}
        </Button>
      </CardHeader>
      <CardContent class="space-y-4">
        <div
          v-if="isLoading"
          class="py-10 text-center text-sm text-muted-foreground"
        >
          {{ t("admin.oidcProviders.loading") }}
        </div>
        <Table v-else class="table-fixed" container-class="overflow-hidden">
          <colgroup>
            <col class="w-[24%] sm:w-[18%]" />
            <col class="hidden sm:table-column sm:w-[12%]" />
            <col class="hidden md:table-column md:w-[10%]" />
            <col />
            <col class="w-[86px] sm:w-[184px] 2xl:w-[350px]" />
          </colgroup>
          <TableHeader>
            <TableRow>
              <TableHead class="whitespace-normal">{{
                t("admin.oidcProviders.columns.name")
              }}</TableHead>
              <TableHead class="hidden whitespace-normal sm:table-cell">{{
                t("admin.oidcProviders.columns.type")
              }}</TableHead>
              <TableHead class="hidden whitespace-normal md:table-cell">{{
                t("admin.oidcProviders.columns.status")
              }}</TableHead>
              <TableHead class="min-w-0 whitespace-nowrap">
                Callback URL
              </TableHead>
              <TableHead class="text-right">{{
                t("admin.oidcProviders.columns.actions")
              }}</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            <TableRow v-for="provider in providers" :key="provider.id">
              <TableCell class="whitespace-normal font-medium">
                {{ provider.name }}
              </TableCell>
              <TableCell class="hidden whitespace-normal sm:table-cell">
                {{ providerLabel(provider.type) }}
              </TableCell>
              <TableCell class="hidden whitespace-normal md:table-cell">
                <Badge variant="outline">{{ providerStatus(provider) }}</Badge>
              </TableCell>
              <TableCell class="min-w-0 max-w-[48vw] sm:max-w-none">
                <div
                  v-if="provider.callback_url"
                  class="group/callback flex min-w-0 max-w-full items-center gap-2 rounded-md border bg-muted/30 px-2.5 py-2"
                >
                  <span
                    class="block min-w-0 flex-1 truncate font-mono text-xs leading-5 text-muted-foreground"
                  >
                    {{ provider.callback_url }}
                  </span>
                  <Button
                    variant="ghost"
                    size="icon-sm"
                    class="size-7 shrink-0 opacity-100 transition-opacity sm:opacity-0 sm:group-hover/callback:opacity-100 sm:focus-visible:opacity-100"
                    :title="
                      t('admin.oidcProviders.copyCallbackUrl', {
                        provider: provider.name,
                      })
                    "
                    :aria-label="
                      t('admin.oidcProviders.copyCallbackUrl', {
                        provider: provider.name,
                      })
                    "
                    @click="copyCallbackUrl(provider.callback_url)"
                  >
                    <Copy class="h-4 w-4" />
                  </Button>
                </div>
                <span v-else class="text-muted-foreground">-</span>
              </TableCell>
              <TableCell class="text-right">
                <div
                  class="inline-flex flex-nowrap items-center justify-end gap-1.5 2xl:gap-2"
                >
                  <Button
                    variant="outline"
                    size="sm"
                    class="gap-1.5 px-2 2xl:px-2.5"
                    :disabled="isMutating"
                    :title="t('admin.oidcProviders.editProvider')"
                    :aria-label="t('admin.oidcProviders.editProvider')"
                    @click="openEditDialog(provider)"
                  >
                    <Pencil class="h-4 w-4" />
                    <span class="hidden 2xl:inline">{{
                      t("admin.oidcProviders.edit")
                    }}</span>
                  </Button>
                  <ConfirmDangerPopover
                    :title="t('admin.oidcProviders.deleteProvider')"
                    :description="t('admin.oidcProviders.deleteDescription')"
                    :loading="isMutating"
                    :disabled="isMutating"
                    :on-confirm="() => deleteProvider(provider.id)"
                  >
                    <template #trigger>
                      <Button
                        variant="destructive"
                        size="sm"
                        class="gap-1.5 px-2 2xl:px-2.5"
                        :disabled="isMutating"
                        :title="t('admin.oidcProviders.deleteProvider')"
                        :aria-label="t('admin.oidcProviders.deleteProvider')"
                      >
                        <Trash2 class="h-4 w-4" />
                        <span class="hidden 2xl:inline">{{
                          t("admin.oidcProviders.delete")
                        }}</span>
                      </Button>
                    </template>
                  </ConfirmDangerPopover>
                </div>
              </TableCell>
            </TableRow>
            <TableEmpty v-if="providers.length === 0" :colspan="5">
              {{ t("admin.oidcProviders.empty") }}
            </TableEmpty>
          </TableBody>
        </Table>
      </CardContent>
    </Card>

    <Dialog :open="showCreateDialog" @update:open="showCreateDialog = $event">
      <DialogContent class="max-h-[88vh] overflow-y-auto sm:max-w-[640px]">
        <DialogHeader>
          <DialogTitle>{{ t("admin.oidcProviders.createTitle") }}</DialogTitle>
          <DialogDescription>
            {{ t("admin.oidcProviders.createDescription") }}
          </DialogDescription>
        </DialogHeader>
        <OIDCProviderFormFields
          :catalog="catalog"
          :form="form"
          mode="create"
          :provider-label="providerLabel"
          @type-change="handleCreateProviderTypeChange"
        />
        <DialogFooter class="gap-2">
          <Button
            variant="outline"
            :disabled="isSaving"
            @click="showCreateDialog = false"
          >
            {{ t("admin.oidcProviders.cancel") }}
          </Button>
          <Button :disabled="isSaving" @click="handleCreateProvider">
            <LoaderCircle v-if="isSaving" class="h-4 w-4 animate-spin" />
            <Plus v-else class="h-4 w-4" />
            {{
              isSaving
                ? t("admin.oidcProviders.adding")
                : t("admin.oidcProviders.addProvider")
            }}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>

    <Dialog
      :open="showQqBindingAlert"
      @update:open="showQqBindingAlert = $event"
    >
      <DialogContent class="sm:max-w-[520px]">
        <DialogHeader>
          <DialogTitle>{{
            t("admin.oidcProviders.qqBindingTitle")
          }}</DialogTitle>
          <DialogDescription>
            {{ t("admin.oidcProviders.qqBindingSummary") }}
          </DialogDescription>
        </DialogHeader>
        <Alert class="border-amber-200 bg-amber-50 text-amber-950">
          <CircleAlert class="h-4 w-4" />
          <AlertTitle>{{
            t("admin.oidcProviders.qqBindingAlertTitle")
          }}</AlertTitle>
          <AlertDescription class="leading-6 text-amber-900">
            {{ t("admin.oidcProviders.qqBindingInstructions") }}
          </AlertDescription>
        </Alert>
        <DialogFooter class="gap-2">
          <Button variant="outline" @click="showQqBindingAlert = false">
            {{ t("admin.oidcProviders.qqBindingLater") }}
          </Button>
          <Button @click="returnToTotpManagement">
            {{ t("admin.oidcProviders.returnToTotpManagement") }}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>

    <Dialog :open="showEditDialog" @update:open="showEditDialog = $event">
      <DialogContent class="max-h-[88vh] overflow-y-auto sm:max-w-[640px]">
        <DialogHeader>
          <DialogTitle>{{ t("admin.oidcProviders.editTitle") }}</DialogTitle>
          <DialogDescription>
            {{ t("admin.oidcProviders.editDescription") }}
          </DialogDescription>
        </DialogHeader>
        <OIDCProviderFormFields
          :catalog="catalog"
          :form="editForm"
          mode="edit"
          :provider-label="providerLabel"
        />
        <DialogFooter class="gap-2">
          <Button variant="outline" @click="showEditDialog = false">
            {{ t("admin.oidcProviders.cancel") }}
          </Button>
          <Button :disabled="isMutating" @click="saveProviderEdit">
            <LoaderCircle v-if="isMutating" class="h-4 w-4 animate-spin" />
            {{ t("admin.oidcProviders.saveProvider") }}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </div>
</template>

<script setup lang="ts">
import { useI18n } from "vue-i18n";
import {
  Breadcrumb,
  BreadcrumbItem,
  BreadcrumbLink,
  BreadcrumbList,
  BreadcrumbPage,
  BreadcrumbSeparator,
} from "@/components/ui/breadcrumb";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
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
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Badge } from "@/components/ui/badge";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import {
  CircleAlert,
  Copy,
  LoaderCircle,
  Pencil,
  Plus,
  Trash2,
} from "lucide-vue-next";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import OIDCProviderFormFields from "./oidc-provider-settings/OIDCProviderFormFields.vue";
import { useOIDCProviderManagement } from "./oidc-provider-settings/useOIDCProviderManagement";

const { t } = useI18n();
const {
  catalog,
  copyCallbackUrl,
  deleteProvider,
  editForm,
  form,
  handleCreateProvider,
  handleCreateProviderTypeChange,
  isLoading,
  isMutating,
  isSaving,
  openCreateDialog,
  openEditDialog,
  providerLabel,
  providers,
  providerStatus,
  returnToTotpManagement,
  saveProviderEdit,
  showCreateDialog,
  showEditDialog,
  showQqBindingAlert,
} = useOIDCProviderManagement();
</script>
