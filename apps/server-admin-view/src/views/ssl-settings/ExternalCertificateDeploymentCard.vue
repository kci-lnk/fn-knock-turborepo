<script setup lang="ts">
import { onMounted } from "vue";
import { useI18n } from "vue-i18n";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import ConfigCollapsibleCard from "@admin-shared/components/ConfigCollapsibleCard.vue";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import { useExternalCertificateBindings } from "./useExternalCertificateBindings";

const { t } = useI18n();
const {
  bindingName,
  bindingNameDrafts,
  bindings,
  clearCredential,
  configured,
  copyCompleteConfiguration,
  copyValue,
  createBinding,
  credential,
  credentialFields,
  formatDate,
  isCreating,
  isLoading,
  loadBindings,
  pendingBindingId,
  provider,
  providerName,
  providerOptions,
  renameBinding,
  revokeBinding,
  rotateToken,
  setBindingEnabled,
  summary,
} = useExternalCertificateBindings();

function collapseAndClear(collapse: () => void) {
  clearCredential();
  collapse();
}

onMounted(() => void loadBindings());
</script>

<template>
  <ConfigCollapsibleCard
    :title="t('admin.certConfig.externalAutomationTitle')"
    :configured="configured"
    :ready="!isLoading"
    :edit-label="t('admin.certConfig.externalManage')"
    collapsed-content-class="min-h-[76px] flex flex-col items-start gap-3 sm:h-[40px] sm:flex-row sm:items-center sm:justify-between"
    summary-class="text-xs text-muted-foreground max-w-full whitespace-normal break-words sm:truncate"
    expanded-content-class="p-0 sm:p-0"
    actions-class="border-t bg-muted/30 px-4 py-4 sm:px-6 flex justify-end rounded-b-lg"
    card-class="dynamic-white-cert-card"
  >
    <template #summary>{{ summary }}</template>

    <template #default>
      <div class="divide-y divide-border">
        <div class="grid gap-4 p-4 sm:p-6">
          <div class="grid gap-1">
            <div class="text-base font-semibold">
              {{ t("admin.certConfig.externalCreateTitle") }}
            </div>
            <p class="text-sm text-muted-foreground">
              {{ t("admin.certConfig.externalDescription") }}
            </p>
          </div>
          <Alert>
            <AlertTitle>{{
              t("admin.certConfig.externalSecurityTitle")
            }}</AlertTitle>
            <AlertDescription>
              {{ t("admin.certConfig.externalSecurityDescription") }}
            </AlertDescription>
          </Alert>
          <div
            class="grid gap-3 sm:grid-cols-[180px_minmax(0,1fr)_auto] sm:items-end"
          >
            <div class="grid gap-1.5">
              <Label for="external-certificate-provider">
                {{ t("admin.certConfig.externalProviderLabel") }}
              </Label>
              <Select v-model="provider">
                <SelectTrigger id="external-certificate-provider">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem
                    v-for="option in providerOptions"
                    :key="option.value"
                    :value="option.value"
                  >
                    {{ option.label }}
                  </SelectItem>
                </SelectContent>
              </Select>
            </div>
            <div class="grid gap-1.5">
              <Label for="external-certificate-binding-name">
                {{ t("admin.certConfig.externalNameLabel") }}
              </Label>
              <Input
                id="external-certificate-binding-name"
                v-model="bindingName"
                :placeholder="t('admin.certConfig.externalNamePlaceholder')"
                maxlength="80"
                @keyup.enter="createBinding"
              />
            </div>
            <Button
              :disabled="isCreating || !bindingName.trim()"
              @click="createBinding"
            >
              {{
                isCreating
                  ? t("admin.certConfig.externalCreating")
                  : t("admin.certConfig.externalCreate")
              }}
            </Button>
          </div>
        </div>

        <div
          v-if="credential"
          class="grid gap-4 bg-amber-50/60 p-4 dark:bg-amber-950/15 sm:p-6"
        >
          <Alert>
            <AlertTitle>{{
              t("admin.certConfig.externalCredentialTitle", {
                provider: providerName(credential.binding.provider),
              })
            }}</AlertTitle>
            <AlertDescription>
              {{ t("admin.certConfig.externalCredentialDescription") }}
            </AlertDescription>
          </Alert>
          <Alert>
            <AlertTitle>{{
              t("admin.certConfig.externalLoopbackTitle")
            }}</AlertTitle>
            <AlertDescription>
              {{
                t("admin.certConfig.externalLoopbackDescription", {
                  port: credential.binding.deploy_port,
                })
              }}
            </AlertDescription>
          </Alert>
          <div class="grid gap-3">
            <div
              v-for="item in credentialFields"
              :key="item.label"
              class="grid gap-1.5"
            >
              <div class="text-xs font-medium text-muted-foreground">
                {{ item.label }}
              </div>
              <div class="flex min-w-0 items-start gap-2">
                <code
                  class="min-w-0 flex-1 rounded-md border bg-background px-3 py-2 text-xs"
                  :class="
                    item.multiline
                      ? 'max-h-96 overflow-auto whitespace-pre-wrap break-words'
                      : 'break-all'
                  "
                  >{{ item.value }}</code
                >
                <Button
                  size="sm"
                  variant="outline"
                  @click="copyValue(item.value)"
                >
                  {{ t("admin.certConfig.externalCopy") }}
                </Button>
              </div>
            </div>
          </div>
          <div class="flex justify-end">
            <Button variant="outline" @click="copyCompleteConfiguration">
              {{ t("admin.certConfig.externalCopyAll") }}
            </Button>
          </div>
        </div>

        <div class="grid gap-3 p-4 sm:p-6">
          <div class="font-semibold">
            {{ t("admin.certConfig.externalBindingsTitle") }}
          </div>
          <div
            v-if="!bindings.length"
            class="rounded-lg border border-dashed p-8 text-center text-sm text-muted-foreground"
          >
            {{ t("admin.certConfig.externalNoBindings") }}
          </div>
          <div
            v-for="binding in bindings"
            :key="binding.id"
            class="dynamic-white-cert-subsurface grid gap-3 rounded-lg border bg-muted/15 p-4"
          >
            <div class="flex flex-wrap items-start justify-between gap-3">
              <div class="grid min-w-0 gap-1">
                <div class="flex flex-wrap items-center gap-2">
                  <span class="font-medium">{{ binding.name }}</span>
                  <Badge variant="outline">
                    {{ providerName(binding.provider) }}
                  </Badge>
                  <Badge :variant="binding.enabled ? 'default' : 'secondary'">
                    {{
                      binding.enabled
                        ? t("admin.certConfig.externalEnabled")
                        : t("admin.certConfig.externalDisabled")
                    }}
                  </Badge>
                  <Badge
                    :variant="
                      binding.last_result === 'failed'
                        ? 'destructive'
                        : 'outline'
                    "
                  >
                    {{
                      binding.last_result === "success"
                        ? t("admin.certConfig.externalLastSuccess")
                        : binding.last_result === "failed"
                          ? t("admin.certConfig.externalLastFailed")
                          : t("admin.certConfig.externalNeverDeployed")
                    }}
                  </Badge>
                </div>
              </div>
              <div class="flex flex-wrap gap-2">
                <Button
                  size="sm"
                  variant="outline"
                  :disabled="pendingBindingId === binding.id"
                  @click="setBindingEnabled(binding, !binding.enabled)"
                >
                  {{
                    binding.enabled
                      ? t("admin.certConfig.externalDisable")
                      : t("admin.certConfig.externalEnable")
                  }}
                </Button>
                <ConfirmDangerPopover
                  :title="t('admin.certConfig.externalRotateTitle')"
                  :description="t('admin.certConfig.externalRotateDescription')"
                  :confirm-text="t('admin.certConfig.externalRotate')"
                  :loading="pendingBindingId === binding.id"
                  :on-confirm="() => rotateToken(binding)"
                >
                  <template #trigger>
                    <Button size="sm" variant="outline">
                      {{ t("admin.certConfig.externalRotate") }}
                    </Button>
                  </template>
                </ConfirmDangerPopover>
                <ConfirmDangerPopover
                  :title="t('admin.certConfig.externalRevokeTitle')"
                  :description="t('admin.certConfig.externalRevokeDescription')"
                  :confirm-text="t('admin.certConfig.externalRevoke')"
                  :loading="pendingBindingId === binding.id"
                  :on-confirm="() => revokeBinding(binding)"
                >
                  <template #trigger>
                    <Button size="sm" variant="destructive">
                      {{ t("admin.certConfig.externalRevoke") }}
                    </Button>
                  </template>
                </ConfirmDangerPopover>
              </div>
            </div>
            <div class="flex flex-col gap-2 sm:flex-row">
              <Input
                v-model="bindingNameDrafts[binding.id]"
                :aria-label="t('admin.certConfig.externalRename')"
                maxlength="80"
              />
              <Button
                size="sm"
                variant="outline"
                :disabled="
                  pendingBindingId === binding.id ||
                  !bindingNameDrafts[binding.id]?.trim() ||
                  bindingNameDrafts[binding.id]?.trim() === binding.name
                "
                @click="renameBinding(binding)"
              >
                {{ t("admin.certConfig.externalRename") }}
              </Button>
            </div>
            <div
              class="grid gap-1 text-xs text-muted-foreground sm:grid-cols-2"
            >
              <div>
                {{ t("admin.certConfig.externalLastDeployment") }}:
                {{ formatDate(binding.last_deployed_at) }}
              </div>
              <div>
                {{ t("admin.certConfig.externalExpiresAt") }}:
                {{ formatDate(binding.last_valid_to) }}
              </div>
              <div
                v-if="binding.last_dns_names.length"
                class="break-all sm:col-span-2"
              >
                {{ t("admin.certConfig.externalDomains") }}:
                {{ binding.last_dns_names.join(", ") }}
              </div>
              <div
                v-if="binding.last_error"
                class="break-words text-destructive sm:col-span-2"
              >
                {{ binding.last_error }}
              </div>
            </div>
          </div>
        </div>
      </div>
    </template>

    <template #actions="{ collapse }">
      <Button variant="outline" @click="collapseAndClear(collapse)">
        {{ t("admin.certConfig.collapse") }}
      </Button>
    </template>
  </ConfigCollapsibleCard>
</template>
