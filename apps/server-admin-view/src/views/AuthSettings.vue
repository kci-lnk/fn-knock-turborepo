<template>
  <Card class="min-h-[600px]">
    <CardHeader
      class="gap-4 sm:flex sm:flex-row sm:items-center sm:justify-between"
    >
      <div class="min-w-0 space-y-1.5">
        <div class="flex items-center justify-between gap-3">
          <CardTitle>{{ t("admin.authSettings.title") }}</CardTitle>
          <DocsLinkButton class="sm:hidden" :href="docsUrls.guides.auth" />
        </div>
        <CardDescription>{{
          t("admin.authSettings.description")
        }}</CardDescription>
      </div>
      <div
        class="grid w-full grid-cols-2 gap-2 sm:flex sm:w-auto sm:items-center"
      >
        <DocsLinkButton
          class="hidden sm:inline-flex"
          :href="docsUrls.guides.auth"
          size="default"
        />
        <Button
          class="order-3 h-11 w-full justify-center px-3 sm:order-none sm:size-9 sm:w-auto sm:px-0"
          variant="outline"
          :aria-label="t('admin.authSettings.credentialTransfer')"
          :title="t('admin.authSettings.credentialTransfer')"
          :disabled="isCredentialTransferBusy"
          @click="showCredentialTransferDialog = true"
        >
          <FileKey2 class="h-4 w-4" />
          <span class="sm:hidden">
            {{ t("admin.authSettings.credentialTransferShort") }}
          </span>
        </Button>
        <Button
          class="order-2 h-11 min-w-0 w-full sm:order-none sm:h-9 sm:w-auto"
          variant="outline"
          @click="goToOidcProviders"
        >
          {{ t("admin.authSettings.oidcLogin") }}
        </Button>
        <Button
          class="order-1 col-span-2 h-11 w-full sm:order-none sm:h-9 sm:w-auto"
          @click="openSetupDialog"
        >
          {{ t("admin.authSettings.bindNewToken") }}
        </Button>
      </div>
    </CardHeader>
    <CardContent v-if="isLoading && showLoadingSkeleton && !credentials.length">
      <div class="border rounded-md overflow-hidden">
        <Table :class="totpTableClass" container-class="overflow-x-auto">
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
              <TableHead class="whitespace-normal">{{
                t("admin.authSettings.boundAt")
              }}</TableHead>
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
              <TableCell class="text-right"
                ><Skeleton class="h-8 w-16 rounded-md ml-auto sm:w-24"
              /></TableCell>
            </TableRow>
          </TableBody>
        </Table>
      </div>
    </CardContent>
    <CardContent v-else-if="!isLoading || credentials.length">
      <Table :class="totpTableClass" container-class="overflow-x-auto">
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
            <TableHead class="whitespace-normal">{{
              t("admin.authSettings.boundAt")
            }}</TableHead>
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
                  <Button
                    variant="destructive"
                    size="sm"
                    :disabled="isDeleting"
                  >
                    {{ t("admin.authSettings.delete") }}
                  </Button>
                </template>
              </ConfirmDangerPopover>
            </TableCell>
          </TableRow>
          <TableEmpty
            v-if="credentials.length === 0"
            :colspan="totpTableColspan"
          >
            {{ t("admin.authSettings.empty") }}
          </TableEmpty>
        </TableBody>
      </Table>
    </CardContent>
    <CardContent v-else class="min-h-[180px]" aria-hidden="true"></CardContent>
  </Card>

  <input
    ref="credentialImportInputRef"
    type="file"
    accept=".json,application/json"
    class="hidden"
    @change="handleCredentialImportFileChange"
  />

  <Dialog
    :open="showCredentialTransferDialog"
    @update:open="showCredentialTransferDialog = $event"
  >
    <DialogContent class="max-h-[88vh] overflow-y-auto sm:max-w-[520px]">
      <DialogHeader>
        <DialogTitle>{{
          t("admin.authSettings.credentialTransfer")
        }}</DialogTitle>
        <DialogDescription>
          {{ t("admin.authSettings.credentialTransferDescription") }}
        </DialogDescription>
      </DialogHeader>
      <div class="grid gap-3 sm:grid-cols-2">
        <Button
          variant="outline"
          class="h-auto justify-start gap-3 px-4 py-3 text-left"
          :disabled="credentials.length === 0 || isCredentialTransferBusy"
          @click="openExportDialogFromCredentialTransferDialog"
        >
          <Download class="h-4 w-4 shrink-0" />
          <span class="min-w-0 whitespace-normal font-medium">
            {{ t("admin.authSettings.exportCredentials") }}
          </span>
        </Button>
        <Button
          variant="outline"
          class="h-auto justify-start gap-3 px-4 py-3 text-left"
          :disabled="isCredentialTransferBusy"
          @click="triggerImportFilePickerFromCredentialTransferDialog"
        >
          <Upload class="h-4 w-4 shrink-0" />
          <span class="min-w-0 whitespace-normal font-medium">
            {{ t("admin.authSettings.importCredentials") }}
          </span>
        </Button>
      </div>
      <DialogFooter>
        <Button
          variant="outline"
          :disabled="isCredentialTransferBusy"
          @click="showCredentialTransferDialog = false"
        >
          {{ t("admin.authSettings.cancel") }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>

  <Dialog :open="showExportDialog" @update:open="showExportDialog = $event">
    <DialogContent class="max-h-[88vh] overflow-y-auto sm:max-w-[520px]">
      <DialogHeader>
        <DialogTitle>{{
          t("admin.authSettings.exportCredentialsTitle")
        }}</DialogTitle>
        <DialogDescription>
          {{ t("admin.authSettings.exportCredentialsDescription") }}
        </DialogDescription>
      </DialogHeader>
      <DialogFooter class="gap-2">
        <Button
          variant="outline"
          :disabled="isExportingCredentials"
          @click="showExportDialog = false"
        >
          {{ t("admin.authSettings.cancel") }}
        </Button>
        <Button
          :disabled="isExportingCredentials"
          @click="handleExportCredentials"
        >
          <span
            v-if="isExportingCredentials"
            class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
          ></span>
          {{ t("admin.authSettings.confirmExportCredentials") }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>

  <Dialog
    :open="showImportDialog"
    @update:open="
      showImportDialog = $event;
      if (!$event) resetPendingCredentialImport();
    "
  >
    <DialogContent class="max-h-[88vh] overflow-y-auto sm:max-w-[520px]">
      <DialogHeader>
        <DialogTitle>{{
          t("admin.authSettings.importCredentialsTitle")
        }}</DialogTitle>
        <DialogDescription>
          {{ t("admin.authSettings.importCredentialsDescription") }}
        </DialogDescription>
      </DialogHeader>
      <div class="rounded-md border bg-muted/20 px-3 py-2 text-sm">
        <p class="break-all font-medium">
          {{ pendingCredentialImportFilename }}
        </p>
      </div>
      <DialogFooter class="gap-2">
        <Button
          variant="outline"
          :disabled="isImportingCredentials"
          @click="
            showImportDialog = false;
            resetPendingCredentialImport();
          "
        >
          {{ t("admin.authSettings.cancel") }}
        </Button>
        <Button
          :disabled="isImportingCredentials"
          @click="handleImportCredentials"
        >
          <span
            v-if="isImportingCredentials"
            class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
          ></span>
          {{ t("admin.authSettings.confirmImportCredentials") }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>

  <Dialog
    :open="showSubdomainAccessDialog"
    @update:open="handleSubdomainAccessDialogOpenChange"
  >
    <DialogContent class="max-h-[88vh] overflow-y-auto sm:max-w-[640px]">
      <DialogHeader>
        <DialogTitle>
          {{ t("admin.authSettings.permissionDialogTitle") }}
        </DialogTitle>
        <DialogDescription>
          {{
            t("admin.authSettings.permissionDialogDescription", {
              name:
                editingSubdomainAccessTotp?.comment ||
                t("admin.authSettings.tokenFallback"),
            })
          }}
        </DialogDescription>
      </DialogHeader>

      <div class="space-y-4">
        <div class="grid grid-cols-2 gap-2">
          <Button
            type="button"
            :variant="subdomainAccessMode === 'all' ? 'default' : 'outline'"
            class="h-auto justify-start px-4 py-3 text-left"
            @click="subdomainAccessMode = 'all'"
          >
            <span class="min-w-0 whitespace-normal">
              {{ t("admin.authSettings.permissionAll") }}
            </span>
          </Button>
          <Button
            type="button"
            :variant="subdomainAccessMode === 'custom' ? 'default' : 'outline'"
            class="h-auto justify-start px-4 py-3 text-left"
            @click="subdomainAccessMode = 'custom'"
          >
            <span class="min-w-0 whitespace-normal">
              {{ t("admin.authSettings.permissionCustom") }}
            </span>
          </Button>
        </div>

        <div v-if="subdomainAccessMode === 'custom'" class="space-y-3">
          <Input
            v-model.trim="subdomainAccessSearch"
            :placeholder="t('admin.authSettings.permissionSearchPlaceholder')"
          />
          <div class="flex flex-wrap items-center justify-between gap-2">
            <p class="text-sm text-muted-foreground">
              {{
                t("admin.authSettings.permissionSelectedCount", {
                  count: selectedSubdomainHostCount,
                })
              }}
            </p>
            <div class="flex gap-2">
              <Button
                type="button"
                variant="outline"
                size="sm"
                :disabled="filteredSubdomainAccessOptions.length === 0"
                @click="selectAllFilteredSubdomainHosts"
              >
                {{ t("admin.authSettings.permissionSelectAll") }}
              </Button>
              <Button
                type="button"
                variant="outline"
                size="sm"
                :disabled="selectedSubdomainHostCount === 0"
                @click="clearSelectedSubdomainHosts"
              >
                {{ t("admin.authSettings.permissionClear") }}
              </Button>
            </div>
          </div>

          <div
            class="max-h-72 overflow-y-auto rounded-md border"
            role="group"
            :aria-label="t('admin.authSettings.permissionCustom')"
          >
            <label
              v-for="option in filteredSubdomainAccessOptions"
              :key="option.host"
              class="flex cursor-pointer items-start gap-3 border-b px-3 py-3 last:border-b-0 hover:bg-muted/40"
            >
              <Checkbox
                class="mt-0.5"
                :model-value="isSubdomainHostSelected(option.host)"
                @update:model-value="
                  toggleSubdomainHost(option.host, $event === true)
                "
              />
              <span class="min-w-0 flex-1">
                <span class="block truncate text-sm font-medium">
                  {{ option.label }}
                </span>
                <span class="block truncate text-xs text-muted-foreground">
                  {{ option.description }}
                </span>
              </span>
              <span
                v-if="option.builtin"
                class="shrink-0 rounded border px-1.5 py-0.5 text-xs text-muted-foreground"
              >
                {{ t("admin.authSettings.permissionBuiltin") }}
              </span>
              <span
                v-else-if="option.stale"
                class="shrink-0 rounded border px-1.5 py-0.5 text-xs text-muted-foreground"
              >
                {{ t("admin.authSettings.permissionStaleHost") }}
              </span>
            </label>
            <div
              v-if="filteredSubdomainAccessOptions.length === 0"
              class="px-3 py-8 text-center text-sm text-muted-foreground"
            >
              {{
                subdomainAccessOptions.length === 0
                  ? t("admin.authSettings.permissionNoHosts")
                  : t("admin.authSettings.permissionNoSearchResults")
              }}
            </div>
          </div>
        </div>
      </div>

      <DialogFooter class="gap-2">
        <Button
          variant="outline"
          :disabled="isSavingSubdomainAccess"
          @click="closeSubdomainAccessDialog"
        >
          {{ t("admin.authSettings.cancel") }}
        </Button>
        <Button
          :disabled="isSavingSubdomainAccess || !editingSubdomainAccessTotp"
          @click="handleSaveSubdomainAccess"
        >
          <span
            v-if="isSavingSubdomainAccess"
            class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
          ></span>
          {{ t("common.save") }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>

  <Dialog
    :open="showSetupDialog"
    @update:open="
      showSetupDialog = $event;
      if (!$event) handleCancelSetup();
    "
  >
    <DialogContent
      class="max-w-md !top-[5vh] !translate-y-0 max-h-[85vh] overflow-y-auto overscroll-contain max-sm:!inset-x-0 max-sm:!top-auto max-sm:!bottom-0 max-sm:!translate-x-0 max-sm:!translate-y-0 max-sm:!max-w-none max-sm:max-h-[100dvh] max-sm:rounded-b-none max-sm:border-b-0 max-sm:pb-[calc(env(safe-area-inset-bottom)+1rem)]"
      @focusin="handleDialogFocusIn"
    >
      <DialogHeader>
        <DialogTitle>{{ t("admin.authSettings.bindDialogTitle") }}</DialogTitle>
        <DialogDescription>
          {{ t("admin.authSettings.bindDialogDescription") }}
        </DialogDescription>
      </DialogHeader>
      <div
        v-if="setupData && setupStep === 'BIND'"
        class="w-full py-4 max-sm:py-2"
      >
        <Transition
          mode="out-in"
          enter-active-class="transition duration-150 ease-out"
          leave-active-class="transition duration-100 ease-in"
          :enter-from-class="setupBindTransitionEnterFromClass"
          enter-to-class="translate-x-0 opacity-100"
          leave-from-class="translate-x-0 opacity-100"
          :leave-to-class="setupBindTransitionLeaveToClass"
        >
          <div
            v-if="setupBindView === 'qr'"
            key="setup-qr"
            class="flex flex-col items-center gap-4"
          >
            <div class="rounded-xl border bg-white p-4">
              <QrcodeVue :value="setupData.uri" :size="200" level="M" />
            </div>
            <Button
              type="button"
              variant="link"
              class="h-auto gap-1 px-0 text-sm"
              @click="openManualSetupView"
            >
              {{ t("admin.authSettings.manualSetupEntry") }}
              <ChevronRight class="h-4 w-4" />
            </Button>
          </div>

          <div v-else key="setup-manual" class="w-full space-y-4">
            <button
              type="button"
              class="-mx-2 inline-flex w-[calc(100%+1rem)] items-center gap-3 rounded-md px-2 py-1.5 text-left transition-colors hover:bg-accent hover:text-accent-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
              :aria-label="t('admin.authSettings.backToQRCodeSetupAria')"
              @click="returnQRCodeSetupView"
            >
              <ChevronLeft class="h-4 w-4 shrink-0" />
              <span class="text-sm font-semibold">
                {{ t("admin.authSettings.manualSetupTitle") }}
              </span>
            </button>
            <div class="space-y-3 rounded-md border bg-muted/30 p-3">
              <p class="text-xs leading-5 text-muted-foreground">
                {{ t("admin.authSettings.manualSetupDescription") }}
              </p>
              <div
                class="flex items-start gap-2 rounded-md border bg-background px-2.5 py-2"
              >
                <div class="min-w-0 flex-1 space-y-1">
                  <Label class="text-xs text-muted-foreground">
                    {{ t("admin.authSettings.manualSetupSecretLabel") }}
                  </Label>
                  <p
                    class="break-all font-mono text-xs leading-5 text-muted-foreground"
                  >
                    {{ setupSecretDisplay }}
                  </p>
                </div>
                <Button
                  variant="ghost"
                  size="icon-sm"
                  class="size-8 shrink-0"
                  :title="t('admin.authSettings.copySetupSecret')"
                  :aria-label="t('admin.authSettings.copySetupSecret')"
                  @click="copySetupSecret"
                >
                  <Copy class="h-4 w-4" />
                </Button>
              </div>
            </div>
          </div>
        </Transition>

        <div class="mt-6 w-full space-y-4 max-sm:mt-4">
          <div
            ref="otpInputAreaRef"
            class="space-y-2 flex flex-col items-center scroll-mt-24"
          >
            <Label class="text-sm text-muted-foreground self-center">{{
              t("admin.authSettings.otpLabel")
            }}</Label>
            <div class="w-full flex justify-center py-2">
              <InputOTP
                inputmode="numeric"
                :maxlength="6"
                v-model="verifyToken"
                @complete="handleBind"
                :disabled="isBinding"
                :autofocus="true"
                autocomplete="off"
                data-form-type="other"
                data-1p-ignore="true"
                data-lpignore="true"
                data-bwignore="true"
              >
                <InputOTPGroup>
                  <InputOTPSlot v-for="i in 6" :key="i - 1" :index="i - 1" />
                </InputOTPGroup>
              </InputOTP>
            </div>
            <p v-if="isBinding" class="text-sm text-muted-foreground">
              {{ t("admin.authSettings.verifying") }}
            </p>
            <p v-if="bindErrorMessage" class="text-sm text-destructive">
              {{ bindErrorMessage }}
            </p>
          </div>
        </div>
      </div>
      <div v-else-if="setupStep === 'NAME'" class="flex flex-col gap-4 py-4">
        <div class="space-y-2">
          <Label>{{ t("admin.authSettings.nameSuccessLabel") }}</Label>
          <Input
            v-model="newTotpComment"
            :placeholder="t('admin.authSettings.namePlaceholder')"
            @keyup.enter="handleSaveSetupName"
          />
          <p class="text-xs text-muted-foreground">
            {{ t("admin.authSettings.nameHelp") }}
          </p>
        </div>
        <p v-if="bindErrorMessage" class="text-sm text-destructive">
          {{ bindErrorMessage }}
        </p>
        <div class="flex justify-end gap-2 mt-4">
          <Button @click="handleSaveSetupName" :disabled="isBinding">
            <span
              v-if="isBinding"
              class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
            ></span>
            {{ t("common.save") }}
          </Button>
        </div>
      </div>
      <div v-else class="flex items-center justify-center py-12">
        <span
          class="animate-spin h-5 w-5 border-2 border-primary border-t-transparent rounded-full mr-2"
        ></span
        >{{ t("admin.authSettings.generating") }}
      </div>
    </DialogContent>
  </Dialog>
</template>

<script setup lang="ts">
import {
  ref,
  onMounted,
  onBeforeUnmount,
  watch,
  nextTick,
  computed,
} from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import {
  Card,
  CardHeader,
  CardTitle,
  CardDescription,
  CardContent,
} from "@/components/ui/card";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
  TableEmpty,
} from "@/components/ui/table";
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
  ChevronLeft,
  ChevronRight,
  Copy,
  Download,
  FileKey2,
  Upload,
} from "lucide-vue-next";
import DocsLinkButton from "@/components/DocsLinkButton.vue";
import InlineCommentEditor from "@admin-shared/components/InlineCommentEditor.vue";
import HumanFriendlyTime from "@admin-shared/components/common/HumanFriendlyTime.vue";
import { Label } from "@/components/ui/label";
import { Input } from "@/components/ui/input";
import {
  InputOTP,
  InputOTPGroup,
  InputOTPSlot,
} from "@/components/ui/input-otp";
import { Skeleton } from "@/components/ui/skeleton";
import { Switch } from "@/components/ui/switch";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { useDelayedLoading } from "@admin-shared/composables/useDelayedLoading";
import { copyTextToClipboard } from "@admin-shared/utils/copyTextToClipboard";
import { downloadBlob } from "@admin-shared/utils/downloadBlob";
import { ConfigAPI } from "../lib/api";
import { docsUrls } from "../lib/docs";
import { useDockerAdminAuthStore } from "../store/dockerAdminAuth";
import QrcodeVue from "qrcode.vue";
import { toast } from "@admin-shared/utils/toast";
import type {
  HostMapping,
  TOTPCredential,
  TOTPCredentialImportSummary,
  TOTPSubdomainAccess,
  TOTPSubdomainAccessMode,
  TOTPAccessScope,
} from "../types";

const DOCKER_ADMIN_PANEL_ACCESS_SCOPE: TOTPAccessScope = "docker_admin_panel";
const BUILTIN_SELECT_PAGE_ACCESS_HOST = "__builtin_select__";
const BUILTIN_SELECT_PAGE_PATH = "/__select__";
const DEFAULT_SUBDOMAIN_ACCESS: TOTPSubdomainAccess = {
  mode: "all",
  hosts: [],
};
const MAX_TOTP_CREDENTIAL_IMPORT_FILE_SIZE = 256 * 1024;

type SubdomainAccessOption = {
  host: string;
  label: string;
  description: string;
  stale?: boolean;
  builtin?: boolean;
};

const { t } = useI18n();
const router = useRouter();
const dockerAdminAuthStore = useDockerAdminAuthStore();

const credentials = ref<TOTPCredential[]>([]);
const hostMappings = ref<HostMapping[]>([]);
const updatingAccessScopeIds = ref<Set<string>>(new Set());
const updatingSubdomainAccessIds = ref<Set<string>>(new Set());
const openAdminPanelAccessTooltipId = ref<string | null>(null);
const isTouchInteraction = ref(false);
const credentialImportInputRef = ref<HTMLInputElement | null>(null);
const showCredentialTransferDialog = ref(false);
const showExportDialog = ref(false);
const showImportDialog = ref(false);
const showSubdomainAccessDialog = ref(false);
const editingSubdomainAccessTotp = ref<TOTPCredential | null>(null);
const subdomainAccessMode = ref<TOTPSubdomainAccessMode>("all");
const selectedSubdomainHosts = ref<Set<string>>(new Set());
const subdomainAccessSearch = ref("");
const pendingCredentialImportPayload = ref<unknown>(null);
const pendingCredentialImportFilename = ref("");
let adminPanelAccessTooltipMediaQuery: MediaQueryList | null = null;
const { isPending: isLoading, run: runLoadStatus } = useAsyncAction({
  onError: (error) => {
    console.error("Failed to get TOTP status:", error);
  },
});
const showLoadingSkeleton = useDelayedLoading(isLoading);
const { isPending: isExportingCredentials, run: runExportCredentials } =
  useAsyncAction({
    onError: (error) => {
      toast.error(
        extractErrorMessage(
          error,
          t("admin.authSettings.exportCredentialsFailed"),
        ),
      );
    },
  });
const { isPending: isImportingCredentials, run: runImportCredentials } =
  useAsyncAction({
    onError: (error) => {
      toast.error(
        extractErrorMessage(
          error,
          t("admin.authSettings.importCredentialsFailed"),
        ),
      );
    },
  });

// Setup state
const showSetupDialog = ref(false);
const setupData = ref<{ secret: string; uri: string } | null>(null);
const verifyToken = ref("");
const newTotpComment = ref("");
const bindErrorMessage = ref("");
const setupStep = ref<"BIND" | "NAME">("BIND");
const setupBindView = ref<"qr" | "manual">("qr");
const setupBindMotionDirection = ref<"forward" | "back">("forward");
const boundTotpId = ref<string | null>(null);
const bindingMode = ref<"bind" | "rename">("bind");
const otpInputAreaRef = ref<HTMLElement | null>(null);
let viewportResizeTimer: ReturnType<typeof window.setTimeout> | null = null;
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
const {
  isPending: isSavingSubdomainAccess,
  run: runSaveSubdomainAccess,
} = useAsyncAction({
  onError: (error) => {
    toast.error(
      extractErrorMessage(
        error,
        t("admin.authSettings.permissionUpdateFailed"),
      ),
    );
  },
});

// Delete state
const { isPending: isDeleting, run: runDeleteTotp } = useAsyncAction({
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
const isCredentialTransferBusy = computed(
  () => isExportingCredentials.value || isImportingCredentials.value,
);
const setupSecretDisplay = computed(() => {
  const secret = setupData.value?.secret || "";
  return formatTOTPSecretForDisplay(secret);
});
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
  setupAdminPanelAccessTooltipInteraction();
  window.visualViewport?.addEventListener("resize", handleVisualViewportResize);
  await fetchStatus();
});

onBeforeUnmount(() => {
  teardownAdminPanelAccessTooltipInteraction();
  window.visualViewport?.removeEventListener(
    "resize",
    handleVisualViewportResize,
  );
  if (viewportResizeTimer) {
    window.clearTimeout(viewportResizeTimer);
    viewportResizeTimer = null;
  }
});

function updateInteractionMode() {
  if (typeof window === "undefined") return;
  isTouchInteraction.value = window.matchMedia(
    "(hover: none), (pointer: coarse)",
  ).matches;
}

function setupAdminPanelAccessTooltipInteraction() {
  if (typeof window === "undefined") return;

  adminPanelAccessTooltipMediaQuery = window.matchMedia(
    "(hover: none), (pointer: coarse)",
  );
  updateInteractionMode();

  if (
    typeof adminPanelAccessTooltipMediaQuery.addEventListener === "function"
  ) {
    adminPanelAccessTooltipMediaQuery.addEventListener(
      "change",
      updateInteractionMode,
    );
    return;
  }

  adminPanelAccessTooltipMediaQuery.addListener(updateInteractionMode);
}

function teardownAdminPanelAccessTooltipInteraction() {
  if (!adminPanelAccessTooltipMediaQuery) return;

  if (
    typeof adminPanelAccessTooltipMediaQuery.removeEventListener === "function"
  ) {
    adminPanelAccessTooltipMediaQuery.removeEventListener(
      "change",
      updateInteractionMode,
    );
    adminPanelAccessTooltipMediaQuery = null;
    return;
  }

  adminPanelAccessTooltipMediaQuery.removeListener(updateInteractionMode);
  adminPanelAccessTooltipMediaQuery = null;
}

watch(
  () => [showSetupDialog.value, setupStep.value, setupData.value] as const,
  async ([isOpen, step, setup]) => {
    if (!isOpen || step !== "BIND" || !setup) return;
    await nextTick();
    scrollOtpIntoView("auto");
  },
);

async function fetchStatus() {
  await runLoadStatus(async () => {
    const [res, mappings] = await Promise.all([
      ConfigAPI.getTOTPStatus(),
      ConfigAPI.getHostMappings().catch((error) => {
        console.error("Failed to get host mappings:", error);
        return [] as HostMapping[];
      }),
    ]);
    hostMappings.value = mappings;
    credentials.value = (res.credentials || []).map(normalizeCredential);
  });
}

function normalizeSubdomainHost(value: unknown) {
  const raw = String(value ?? "").trim().toLowerCase();
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

function normalizeTOTPSubdomainAccess(
  value: unknown,
): TOTPSubdomainAccess {
  if (
    typeof value !== "object" ||
    value === null ||
    (value as { mode?: unknown }).mode !== "custom"
  ) {
    return { ...DEFAULT_SUBDOMAIN_ACCESS };
  }

  const hostsValue = (value as { hosts?: unknown }).hosts;
  const hosts = Array.isArray(hostsValue)
    ? [...new Set(hostsValue.map(normalizeSubdomainHost).filter(Boolean))]
        .sort(compareSubdomainAccessHosts)
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
    subdomain_access: normalizeTOTPSubdomainAccess(
      credential.subdomain_access,
    ),
  };
}

function buildTOTPCredentialExportFilename() {
  return `fn-knock-totp-credentials-${new Date()
    .toISOString()
    .replace(/[:.]/g, "-")}.json`;
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

function openExportDialog() {
  if (credentials.value.length === 0 || isCredentialTransferBusy.value) return;
  showExportDialog.value = true;
}

function openExportDialogFromCredentialTransferDialog() {
  showCredentialTransferDialog.value = false;
  openExportDialog();
}

async function handleExportCredentials() {
  await runExportCredentials(async () => {
    const blob = await ConfigAPI.downloadTOTPCredentials();
    downloadBlob(blob, buildTOTPCredentialExportFilename());
    showExportDialog.value = false;
    toast.success(t("admin.authSettings.exportCredentialsStarted"));
  });
}

function resetPendingCredentialImport() {
  pendingCredentialImportPayload.value = null;
  pendingCredentialImportFilename.value = "";
}

function resetCredentialImportInput() {
  if (credentialImportInputRef.value) {
    credentialImportInputRef.value.value = "";
  }
}

function triggerImportFilePicker() {
  if (isCredentialTransferBusy.value) return;
  resetCredentialImportInput();
  credentialImportInputRef.value?.click();
}

function triggerImportFilePickerFromCredentialTransferDialog() {
  showCredentialTransferDialog.value = false;
  triggerImportFilePicker();
}

async function handleCredentialImportFileChange(event: Event) {
  const input = event.target as HTMLInputElement | null;
  const file = input?.files?.[0] ?? null;
  resetPendingCredentialImport();

  if (!file) return;

  if (
    !file.name.toLowerCase().endsWith(".json") &&
    file.type !== "application/json"
  ) {
    toast.error(t("admin.authSettings.importCredentialsInvalidFile"));
    resetCredentialImportInput();
    return;
  }

  if (file.size > MAX_TOTP_CREDENTIAL_IMPORT_FILE_SIZE) {
    toast.error(t("admin.authSettings.importCredentialsFileTooLarge"), {
      description: t("admin.authSettings.importCredentialsFileTooLargeDetail", {
        size: Math.floor(MAX_TOTP_CREDENTIAL_IMPORT_FILE_SIZE / 1024),
      }),
    });
    resetCredentialImportInput();
    return;
  }

  try {
    pendingCredentialImportPayload.value = JSON.parse(await file.text());
    pendingCredentialImportFilename.value = file.name;
    showImportDialog.value = true;
  } catch {
    toast.error(t("admin.authSettings.importCredentialsParseFailed"));
  } finally {
    resetCredentialImportInput();
  }
}

function buildImportSummaryDescription(summary: TOTPCredentialImportSummary) {
  return t("admin.authSettings.importCredentialsSummary", {
    imported: summary.imported,
    skippedExistingId: summary.skipped_existing_id,
    skippedExistingSecret: summary.skipped_existing_secret,
    skippedFileDuplicate: summary.skipped_file_duplicate,
    invalid: summary.invalid,
    total: summary.total,
  });
}

async function handleImportCredentials() {
  const payload = pendingCredentialImportPayload.value;
  if (!payload) {
    toast.error(t("admin.authSettings.importCredentialsChooseFileFirst"));
    return;
  }

  await runImportCredentials(async () => {
    const summary = await ConfigAPI.importTOTPCredentials(payload);
    showImportDialog.value = false;
    resetPendingCredentialImport();
    await fetchStatus();
    toast.success(t("admin.authSettings.importCredentialsCompleted"), {
      description: buildImportSummaryDescription(summary),
    });
  });
}

function hasDockerAdminPanelAccess(totp: TOTPCredential) {
  return (totp.access_scopes || []).includes(DOCKER_ADMIN_PANEL_ACCESS_SCOPE);
}

function getSubdomainAccess(totp: TOTPCredential) {
  return normalizeTOTPSubdomainAccess(totp.subdomain_access);
}

function getSubdomainAccessSummary(totp: TOTPCredential) {
  const access = getSubdomainAccess(totp);
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

function getSubdomainAccessPreview(totp: TOTPCredential) {
  const access = getSubdomainAccess(totp);
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

function openSubdomainAccessDialog(totp: TOTPCredential) {
  const access = getSubdomainAccess(totp);
  editingSubdomainAccessTotp.value = totp;
  subdomainAccessMode.value = access.mode;
  selectedSubdomainHosts.value = new Set(access.hosts);
  subdomainAccessSearch.value = "";
  showSubdomainAccessDialog.value = true;
}

function handleSubdomainAccessDialogOpenChange(open: boolean) {
  if (open) {
    showSubdomainAccessDialog.value = true;
    return;
  }
  closeSubdomainAccessDialog();
}

function closeSubdomainAccessDialog() {
  showSubdomainAccessDialog.value = false;
  editingSubdomainAccessTotp.value = null;
  subdomainAccessMode.value = "all";
  selectedSubdomainHosts.value = new Set();
  subdomainAccessSearch.value = "";
}

function isSubdomainHostSelected(host: string) {
  return selectedSubdomainHosts.value.has(host);
}

function toggleSubdomainHost(host: string, checked: boolean) {
  const normalizedHost = normalizeSubdomainHost(host);
  if (!normalizedHost) return;
  const next = new Set(selectedSubdomainHosts.value);
  if (checked) {
    next.add(normalizedHost);
  } else {
    next.delete(normalizedHost);
  }
  selectedSubdomainHosts.value = next;
}

function selectAllFilteredSubdomainHosts() {
  const next = new Set(selectedSubdomainHosts.value);
  for (const option of filteredSubdomainAccessOptions.value) {
    next.add(option.host);
  }
  selectedSubdomainHosts.value = next;
}

function clearSelectedSubdomainHosts() {
  selectedSubdomainHosts.value = new Set();
}

function isSubdomainAccessUpdating(totpId: string) {
  return updatingSubdomainAccessIds.value.has(totpId);
}

function setSubdomainAccessUpdating(totpId: string, pending: boolean) {
  const next = new Set(updatingSubdomainAccessIds.value);
  if (pending) {
    next.add(totpId);
  } else {
    next.delete(totpId);
  }
  updatingSubdomainAccessIds.value = next;
}

async function handleSaveSubdomainAccess() {
  const target = editingSubdomainAccessTotp.value;
  if (!target) return;

  const subdomainAccess: TOTPSubdomainAccess =
    subdomainAccessMode.value === "custom"
      ? {
          mode: "custom",
          hosts: [...selectedSubdomainHosts.value].sort(
            compareSubdomainAccessHosts,
          ),
        }
      : { ...DEFAULT_SUBDOMAIN_ACCESS };

  setSubdomainAccessUpdating(target.id, true);
  try {
    await runSaveSubdomainAccess(async () => {
      const updated = normalizeCredential(
        await ConfigAPI.updateTOTPSubdomainAccess(target.id, subdomainAccess),
      );
      const existing = credentials.value.find((item) => item.id === target.id);
      if (existing) {
        Object.assign(existing, updated);
      }
      toast.success(t("admin.authSettings.permissionUpdated"));
      closeSubdomainAccessDialog();
    });
  } finally {
    setSubdomainAccessUpdating(target.id, false);
  }
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

function isAccessScopeUpdating(totpId: string) {
  return updatingAccessScopeIds.value.has(totpId);
}

function setAccessScopeUpdating(totpId: string, pending: boolean) {
  const next = new Set(updatingAccessScopeIds.value);
  if (pending) {
    next.add(totpId);
  } else {
    next.delete(totpId);
  }
  updatingAccessScopeIds.value = next;
}

async function handleDockerAdminPanelAccessChange(
  totp: TOTPCredential,
  enabled: boolean,
) {
  const previousScopes = [...(totp.access_scopes || [])];
  const nextScopeSet = new Set<TOTPAccessScope>(previousScopes);
  if (enabled) {
    nextScopeSet.add(DOCKER_ADMIN_PANEL_ACCESS_SCOPE);
  } else {
    nextScopeSet.delete(DOCKER_ADMIN_PANEL_ACCESS_SCOPE);
  }

  const nextScopes = [...nextScopeSet];
  totp.access_scopes = nextScopes;
  setAccessScopeUpdating(totp.id, true);

  try {
    const updated = await ConfigAPI.updateTOTPAccessScopes(totp.id, nextScopes);
    const target = credentials.value.find((item) => item.id === totp.id);
    if (target) {
      target.access_scopes = updated.access_scopes || [];
    }
    toast.success(t("admin.authSettings.adminPanelAccessUpdated"));
  } catch (error) {
    totp.access_scopes = previousScopes;
    toast.error(
      extractErrorMessage(
        error,
        t("admin.authSettings.adminPanelAccessUpdateFailed"),
      ),
    );
  } finally {
    setAccessScopeUpdating(totp.id, false);
  }
}

function scrollOtpIntoView(behavior: ScrollBehavior = "smooth") {
  otpInputAreaRef.value?.scrollIntoView({
    block: "center",
    inline: "nearest",
    behavior,
  });
}

function handleDialogFocusIn(event: FocusEvent) {
  if (setupStep.value !== "BIND") return;
  const target = event.target as HTMLElement | null;
  if (!target || !otpInputAreaRef.value?.contains(target)) return;
  window.setTimeout(() => {
    scrollOtpIntoView();
  }, 120);
}

function handleVisualViewportResize() {
  if (!showSetupDialog.value || setupStep.value !== "BIND") return;
  const viewport = window.visualViewport;
  if (!viewport) return;

  const keyboardHeight = window.innerHeight - viewport.height;
  if (keyboardHeight < 120) return;

  if (viewportResizeTimer) {
    window.clearTimeout(viewportResizeTimer);
  }

  viewportResizeTimer = window.setTimeout(() => {
    scrollOtpIntoView();
  }, 80);
}

async function openSetupDialog() {
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

function handleCancelSetup() {
  setupData.value = null;
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
  await runDeleteTotp(async () => {
    await ConfigAPI.deleteTOTP(totpId);
    await fetchStatus();
    toast.success(t("admin.authSettings.tokenDeleted"));
  });
}

function goToPasskeys(totpId: string) {
  router.push(`/auth/passkeys/${encodeURIComponent(totpId)}`);
}

function goToOidcProviders() {
  router.push("/auth/oidc-providers");
}
</script>
