<template>
  <Dialog :open="props.open" @update:open="handleOpenChange">
    <DialogContent class="sm:max-w-[720px] max-h-[88vh] overflow-y-auto">
      <DialogHeader>
        <DialogTitle>{{ dialogTitle }}</DialogTitle>
        <DialogDescription>
          {{ t("admin.acmeApplicationDialog.description") }}
        </DialogDescription>
      </DialogHeader>

      <div class="grid gap-6 py-1">
        <div class="grid gap-2">
          <label :for="`${a11yId}-acmeapplicationdialog-1`" class="text-sm text-muted-foreground">
            {{ t("admin.acmeApplicationDialog.name") }}
          </label>
          <Input
            :id="`${a11yId}-acmeapplicationdialog-1`"
            v-model.trim="name"
            :disabled="props.pending"
            :placeholder="t('admin.acmeApplicationDialog.namePlaceholder')"
          />
        </div>

        <div class="grid gap-2">
          <div class="flex items-center justify-between gap-3">
            <label
              :for="`${a11yId}-acmeapplicationdialog-2`"
              class="text-sm text-muted-foreground"
            >
              {{ t("admin.acmeApplicationDialog.domains") }}
            </label>
            <span class="text-xs text-muted-foreground">{{
              t("admin.acmeApplicationDialog.domainsHint")
            }}</span>
          </div>
          <TagsInput
            v-model="domains"
            add-on-blur
            class="min-h-[65px]"
            :disabled="props.pending"
          >
            <TagsInputItem v-for="item in domains" :key="item" :value="item">
              <TagsInputItemText />
              <TagsInputItemDelete />
            </TagsInputItem>
            <TagsInputInput
              :id="`${a11yId}-acmeapplicationdialog-2`"
              :disabled="props.pending"
              :placeholder="t('admin.acmeApplicationDialog.domainsPlaceholder')"
            />
          </TagsInput>
        </div>

        <div class="grid gap-2">
          <div class="flex items-center justify-between gap-3">
            <label
              :for="`${a11yId}-acmeapplicationdialog-3`"
              class="text-sm text-muted-foreground"
            >
              {{ t("admin.acmeApplicationDialog.dnsProvider") }}
            </label>
            <span
              v-if="activeDnsType"
              class="text-xs font-mono text-muted-foreground"
            >
              {{ activeDnsType }}
            </span>
          </div>
          <Select v-model="dnsType" :disabled="props.pending">
            <SelectTrigger
              :id="`${a11yId}-acmeapplicationdialog-3`"
              class="w-full"
            >
              <SelectValue
                :placeholder="
                  t('admin.acmeApplicationDialog.selectDnsProvider')
                "
              />
            </SelectTrigger>
            <SelectContent class="max-h-[320px]">
              <SelectGroup
                v-for="group in groupedProviders"
                :key="group.groupKey"
              >
                <SelectLabel>{{ group.group }}</SelectLabel>
                <SelectItem
                  v-for="provider in group.items"
                  :key="provider.dnsType"
                  :value="provider.dnsType"
                >
                  <div class="flex w-full items-center justify-between gap-3">
                    <span class="truncate">{{ provider.label }}</span>
                    <span
                      class="shrink-0 font-mono text-xs text-muted-foreground"
                    >
                      {{ provider.dnsType }}
                    </span>
                  </div>
                </SelectItem>
              </SelectGroup>
            </SelectContent>
          </Select>
        </div>

        <div
          v-if="activeCredentialFields.length"
          data-acme-dialog="credentials"
          class="grid gap-4 rounded-xl border bg-muted/15 p-4 sm:p-5"
        >
          <div class="flex items-start justify-between gap-3">
            <div class="grid gap-0.5">
              <div class="flex flex-wrap items-center gap-2">
                <div class="text-sm font-medium">
                  {{ t("admin.acmeApplicationDialog.dnsApiCredentials") }}
                </div>
                <span
                  class="rounded-full border bg-background px-2 py-0.5 text-[11px] text-muted-foreground"
                >
                  {{ credentialSummary }}
                </span>
              </div>
              <p class="text-xs text-muted-foreground">
                {{ t("admin.acmeApplicationDialog.credentialsDescription") }}
              </p>
              <p
                v-if="hasMultipleCredentialSchemes"
                class="text-xs text-muted-foreground"
              >
                {{
                  t("admin.acmeApplicationDialog.multipleSchemesDescription")
                }}
              </p>
            </div>
            <Button
              type="button"
              variant="ghost"
              size="icon-sm"
              class="text-muted-foreground hover:text-foreground"
              :title="
                isCredentialsVisible
                  ? t('admin.acmeApplicationDialog.hide')
                  : t('admin.acmeApplicationDialog.show')
              "
              :aria-label="
                isCredentialsVisible
                  ? t('admin.acmeApplicationDialog.hideCredentials')
                  : t('admin.acmeApplicationDialog.showCredentials')
              "
              @click="isCredentialsVisible = !isCredentialsVisible"
            >
              <component
                :is="isCredentialsVisible ? EyeOff : Eye"
                class="h-4 w-4"
              />
            </Button>
          </div>

          <div class="grid gap-3">
            <CredentialTransferHint
              v-if="credentialTransferSuggestion"
              :action-label="
                t('admin.acmeApplicationDialog.fillFromSource', {
                  source: transferSourceScopeLabel,
                })
              "
              :description="credentialTransferDescription"
              :fields="
                credentialTransferSuggestion.fillableFields.map(
                  (field) => field.targetKey,
                )
              "
              :loading="isTransferSourceLoading"
              :source-label="`${transferSourceScopeLabel} · ${credentialTransferSuggestion.bridgeLabel}`"
              @apply="applyCredentialTransfer"
            />

            <div class="grid gap-3">
              <div
                v-for="(scheme, schemeIndex) in activeCredentialSchemes"
                :key="scheme.id"
                :class="
                  hasMultipleCredentialSchemes
                    ? 'grid gap-3 rounded-lg border bg-background/60 p-3'
                    : 'grid gap-3'
                "
              >
                <div
                  v-if="hasMultipleCredentialSchemes || scheme.description"
                  class="grid gap-1"
                >
                  <div
                    v-if="hasMultipleCredentialSchemes"
                    class="text-xs font-medium text-foreground"
                  >
                    {{ scheme.label }}
                  </div>
                  <p
                    v-if="scheme.description"
                    class="text-[11px] leading-5 text-muted-foreground"
                  >
                    {{ scheme.description }}
                  </p>
                </div>

                <div class="grid gap-3">
                  <div
                    v-for="(field, fieldIndex) in scheme.fields"
                    :key="field.key"
                    class="grid gap-2"
                  >
                    <div class="flex items-center justify-between gap-2">
                      <span class="text-sm font-mono text-muted-foreground">
                        {{ field.key }}
                      </span>
                      <span
                        v-if="field.required === false"
                        class="text-[11px] text-muted-foreground"
                      >
                        {{ t("admin.acmeApplicationDialog.optional") }}
                      </span>
                    </div>
                    <Input
                      v-model.trim="credentials[field.key]"
                      :aria-label="field.key"
                      :type="isCredentialsVisible ? 'text' : 'password'"
                      class="font-mono"
                      :name="`acme-credential-${schemeIndex}-${fieldIndex}`"
                      autocomplete="new-password"
                      :readonly="!isCredentialEditReady(field.key)"
                      :disabled="props.pending"
                      @focus="enableCredentialEditing(field.key)"
                      @pointerdown="enableCredentialEditing(field.key)"
                    />
                    <p
                      v-if="field.description"
                      class="text-[11px] leading-5 text-muted-foreground"
                    >
                      {{ field.description }}
                    </p>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>

        <div
          class="flex items-center justify-between rounded-lg border bg-muted/20 px-4 py-3"
        >
          <div class="grid gap-0.5">
            <div class="text-sm font-medium">
              {{ t("admin.acmeApplicationDialog.autoRenew") }}
            </div>
            <div class="text-xs text-muted-foreground">
              {{ t("admin.acmeApplicationDialog.autoRenewDescription") }}
            </div>
          </div>
          <Switch v-model="renewEnabled" :aria-label="t('admin.acmeApplicationDialog.autoRenew')" :disabled="props.pending" />
        </div>
      </div>

      <DialogFooter class="gap-2 sm:justify-end">
        <Button
          type="button"
          variant="outline"
          :disabled="props.pending"
          @click="handleOpenChange(false)"
        >
          {{ t("common.cancel") }}
        </Button>
        <Button
          type="button"
          variant="secondary"
          :disabled="!canSubmit || props.pending"
          @click="submit(false)"
        >
          {{ t("common.save") }}
        </Button>
        <Button
          type="button"
          :disabled="!canSubmit || props.pending"
          @click="submit(true)"
        >
          {{ t("admin.acmeApplicationDialog.saveAndApply") }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>

<script setup lang="ts">
import { useId } from "vue";
import { Eye, EyeOff } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectLabel,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import {
  TagsInput,
  TagsInputInput,
  TagsInputItem,
  TagsInputItemDelete,
  TagsInputItemText,
} from "@/components/ui/tags-input";
import CredentialTransferHint from "@/components/CredentialTransferHint.vue";
import {
  useAcmeApplicationForm,
  type AcmeApplicationDialogEmit,
  type AcmeApplicationDialogProps,
} from "./acme-application/useAcmeApplicationForm";

const a11yId = useId();

const props = defineProps<AcmeApplicationDialogProps>();
const emit = defineEmits<AcmeApplicationDialogEmit>();

const {
  activeCredentialFields,
  activeCredentialSchemes,
  activeDnsType,
  applyCredentialTransfer,
  canSubmit,
  credentialSummary,
  credentialTransferDescription,
  credentialTransferSuggestion,
  credentials,
  dialogTitle,
  dnsType,
  domains,
  enableCredentialEditing,
  groupedProviders,
  handleOpenChange,
  hasMultipleCredentialSchemes,
  isCredentialEditReady,
  isCredentialsVisible,
  isTransferSourceLoading,
  name,
  renewEnabled,
  submit,
  t,
  transferSourceScopeLabel,
} = useAcmeApplicationForm(props, emit);
</script>
