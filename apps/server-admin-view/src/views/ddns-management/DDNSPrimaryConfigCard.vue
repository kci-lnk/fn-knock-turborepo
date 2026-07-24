<script setup lang="ts">
import { useI18n } from "vue-i18n";
import {
  ChevronDown,
  ChevronUp,
  Ellipsis,
  RefreshCw,
  Save,
  Trash2,
  Undo2,
} from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import ConfigCollapsibleCard from "@admin-shared/components/ConfigCollapsibleCard.vue";
import FloatingActionDock from "@admin-shared/components/common/FloatingActionDock.vue";
import type { DDNSNetworkInterfacePayload } from "@/lib/api";
import type { DnsCredentialTransferSuggestion } from "@/lib/dns-credential-bridge";
import type {
  DDNSAddressOption,
  DDNSIpSource,
  DDNSUpdateScope,
  Provider,
  ProviderField,
  LastIP,
} from "./model";
import DDNSAddressSourceFields from "./DDNSAddressSourceFields.vue";
import DDNSProviderCredentialsFields from "./DDNSProviderCredentialsFields.vue";

defineProps<{
  configured: boolean;
  configuredNetworkInterface: string;
  configuredNetworkInterfaceLabel: string;
  credentialTransferDescription: string;
  credentialTransferSuggestion: DnsCredentialTransferSuggestion | null;
  enableFieldEditing: (key: string) => void;
  fieldVisibility: Record<string, boolean>;
  formatDomainField: () => void;
  formatOptionLabel: (option: { labelKey: string }) => string;
  getFieldAutocomplete: (field: ProviderField) => string;
  getFieldDescription: (field: ProviderField) => string;
  getFieldDomId: (index: number) => string;
  getFieldInputName: (index: number) => string;
  hasSavedProviderConfig: boolean;
  interfaceIPv4Options: DDNSAddressOption[];
  interfaceIPv6Options: DDNSAddressOption[];
  lastIp: LastIP;
  selectionAnchor: LastIP;
  isClearingPrimaryConfig: boolean;
  isDirty: boolean;
  isFieldEditReady: (key: string) => boolean;
  isIpSourceOptionDisabled: (
    providerName: string,
    option: DDNSIpSource,
  ) => boolean;
  isProviderSelectDisabled: boolean;
  isSaving: boolean;
  isTesting: boolean;
  isTransferSourceLoading: boolean;
  isUpdateScopeOptionDisabled: (
    providerName: string,
    option: DDNSUpdateScope,
  ) => boolean;
  providerConfig: Record<string, string>;
  providerDef: Provider | null;
  providers: Provider[];
  ready: boolean;
  resolvedNetworkInterfaces: DDNSNetworkInterfacePayload[];
  selectedNetworkInterfaceDetail: string;
  selectedProvider: string;
  setFieldValue: (key: string, value: string) => void;
  showInterfaceAddressBlock: boolean;
  showInterfaceIPv4Select: boolean;
  showInterfaceIPv6Select: boolean;
  showSourceDomainBlock: boolean;
  showStaticIPv4Input: boolean;
  showStaticIPv6Input: boolean;
  toggleFieldVisibility: (key: string) => void;
  transferSourceScopeLabel: string;
  updateIpSource: (value: string) => void;
  updateNetworkInterface: (value: string) => void;
}>();

const emit = defineEmits<{
  applyCredentialTransfer: [];
  cancel: [];
  clearPrimaryConfig: [collapse: () => void];
  providerChange: [provider: string];
  save: [];
  test: [];
}>();

const { t } = useI18n();
</script>

<template>
  <ConfigCollapsibleCard
    :title="t('admin.ddns.mainConfigTitle')"
    :configured="configured"
    :ready="ready"
    expanded-content-class="p-0 sm:p-0"
  >
    <template #summary>
      {{
        t("admin.ddns.currentProvider", {
          provider: providerDef?.label || t("admin.ddns.notConfigured"),
        })
      }}
    </template>

    <template v-if="hasSavedProviderConfig" #collapsed-actions>
      <Button
        variant="outline"
        :disabled="isTesting || isSaving || !selectedProvider"
        @click="emit('test')"
      >
        <RefreshCw v-if="isTesting" class="w-4 h-4 mr-2 animate-spin" />
        {{ isTesting ? t("admin.ddns.updating") : t("admin.ddns.refreshNow") }}
      </Button>
    </template>

    <template #default>
      <div class="divide-y divide-border">
        <div
          class="p-4 sm:p-6 grid gap-2 sm:grid-cols-[200px_1fr] md:grid-cols-[240px_1fr] items-start"
        >
          <div class="space-y-1 mt-1.5">
            <Label for="ddns-provider" class="text-sm font-medium">
              {{ t("admin.ddns.providerLabel") }}
            </Label>
            <p class="text-xs text-muted-foreground hidden sm:block pr-4">
              {{ t("admin.ddns.providerHint") }}
            </p>
          </div>
          <div class="w-full max-w-md">
            <Select
              :modelValue="selectedProvider"
              :disabled="isProviderSelectDisabled"
              @update:modelValue="
                (val: any) => emit('providerChange', String(val ?? ''))
              "
            >
              <SelectTrigger class="w-full" id="ddns-provider">
                <SelectValue :placeholder="t('admin.ddns.selectProvider')" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem
                  v-for="provider in providers"
                  :key="provider.name"
                  :value="provider.name"
                >
                  {{ provider.label }}
                </SelectItem>
              </SelectContent>
            </Select>
          </div>
        </div>

        <DDNSAddressSourceFields
          :configured-network-interface="configuredNetworkInterface"
          :configured-network-interface-label="configuredNetworkInterfaceLabel"
          :format-option-label="formatOptionLabel"
          :interface-i-pv4-options="interfaceIPv4Options"
          :interface-i-pv6-options="interfaceIPv6Options"
          :last-ip="lastIp"
          :selection-anchor="selectionAnchor"
          :is-ip-source-option-disabled="isIpSourceOptionDisabled"
          :is-update-scope-option-disabled="isUpdateScopeOptionDisabled"
          :provider-config="providerConfig"
          :resolved-network-interfaces="resolvedNetworkInterfaces"
          :selected-network-interface-detail="selectedNetworkInterfaceDetail"
          :selected-provider="selectedProvider"
          :set-field-value="setFieldValue"
          :show-interface-address-block="showInterfaceAddressBlock"
          :show-interface-i-pv4-select="showInterfaceIPv4Select"
          :show-interface-i-pv6-select="showInterfaceIPv6Select"
          :show-source-domain-block="showSourceDomainBlock"
          :show-static-i-pv4-input="showStaticIPv4Input"
          :show-static-i-pv6-input="showStaticIPv6Input"
          :update-ip-source="updateIpSource"
          :update-network-interface="updateNetworkInterface"
        />

        <DDNSProviderCredentialsFields
          :credential-transfer-description="credentialTransferDescription"
          :credential-transfer-suggestion="credentialTransferSuggestion"
          :enable-field-editing="enableFieldEditing"
          :field-visibility="fieldVisibility"
          :format-domain-field="formatDomainField"
          :get-field-autocomplete="getFieldAutocomplete"
          :get-field-description="getFieldDescription"
          :get-field-dom-id="getFieldDomId"
          :get-field-input-name="getFieldInputName"
          :is-field-edit-ready="isFieldEditReady"
          :is-transfer-source-loading="isTransferSourceLoading"
          :provider-config="providerConfig"
          :provider-def="providerDef"
          :set-field-value="setFieldValue"
          :toggle-field-visibility="toggleFieldVisibility"
          :transfer-source-scope-label="transferSourceScopeLabel"
          @apply-credential-transfer="emit('applyCredentialTransfer')"
        />
      </div>
    </template>

    <template #actions="{ collapse }">
      <FloatingActionDock
        :active="isDirty"
        inline-class="p-4 sm:px-6 sm:py-4 bg-muted/30 border-t flex items-center justify-between gap-2 sm:justify-end sm:gap-3 rounded-b-lg"
      >
        <template #inline>
          <Button
            variant="outline"
            class="h-10 w-10 shrink-0 gap-0 px-0 sm:h-9 sm:w-auto sm:gap-2 sm:px-4"
            :aria-label="t('admin.ddns.collapse')"
            :title="t('admin.ddns.collapse')"
            @click="collapse"
          >
            <ChevronUp class="h-4 w-4 sm:hidden" />
            <span class="hidden sm:inline">
              {{ t("admin.ddns.collapse") }}
            </span>
          </Button>
          <DropdownMenu>
            <DropdownMenuTrigger as-child>
              <Button
                variant="outline"
                class="h-10 w-10 shrink-0 gap-0 px-0 sm:h-9 sm:w-24 sm:gap-2 sm:px-4"
                :aria-label="t('admin.ddns.actions')"
                :title="t('admin.ddns.actions')"
                :disabled="
                  isClearingPrimaryConfig ||
                  !selectedProvider ||
                  !hasSavedProviderConfig
                "
              >
                <Ellipsis class="h-4 w-4 sm:hidden" />
                <span class="hidden sm:inline">
                  {{ t("admin.ddns.actions") }}
                </span>
                <ChevronDown
                  class="hidden h-4 w-4 text-muted-foreground sm:block"
                />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end" class="w-48">
              <DropdownMenuItem
                variant="destructive"
                :disabled="isClearingPrimaryConfig || !hasSavedProviderConfig"
                @click="emit('clearPrimaryConfig', collapse)"
              >
                <Trash2 class="mr-2 h-4 w-4" />
                {{ t("admin.ddns.clearPrimaryConfig") }}
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
          <Button
            v-if="isDirty"
            variant="outline"
            :disabled="isSaving || isTesting"
            :aria-label="t('common.cancel')"
            :title="t('common.cancel')"
            @click="emit('cancel')"
            class="h-10 w-10 shrink-0 gap-0 px-0 sm:h-9 sm:w-auto sm:gap-2 sm:px-4"
          >
            <Undo2 class="h-4 w-4" />
            <span class="hidden sm:inline">{{ t("common.cancel") }}</span>
          </Button>
          <Button
            v-if="isDirty"
            variant="outline"
            :disabled="isSaving || isTesting || !selectedProvider"
            :aria-label="isSaving ? t('admin.ddns.saving') : t('common.save')"
            :title="isSaving ? t('admin.ddns.saving') : t('common.save')"
            @click="emit('save')"
            class="h-10 w-10 min-w-10 shrink-0 gap-0 px-0 sm:h-9 sm:w-auto sm:min-w-[88px] sm:gap-2 sm:px-4"
          >
            <RefreshCw v-if="isSaving" class="h-4 w-4 animate-spin" />
            <Save v-else class="h-4 w-4" />
            <span class="hidden sm:inline">
              {{ isSaving ? t("admin.ddns.saving") : t("common.save") }}
            </span>
          </Button>
          <Button
            :disabled="isTesting || isSaving || !selectedProvider"
            :aria-label="
              isTesting
                ? t('admin.ddns.updating')
                : t('admin.ddns.saveAndUpdate')
            "
            :title="
              isTesting
                ? t('admin.ddns.updating')
                : t('admin.ddns.saveAndUpdate')
            "
            @click="emit('test')"
            class="h-10 w-10 min-w-10 shrink-0 gap-0 px-0 shadow-sm sm:h-9 sm:w-auto sm:min-w-[100px] sm:gap-2 sm:px-4"
          >
            <RefreshCw v-if="isTesting" class="h-4 w-4 animate-spin" />
            <RefreshCw v-else class="h-4 w-4 sm:hidden" />
            <span class="hidden sm:inline">
              {{
                isTesting
                  ? t("admin.ddns.updating")
                  : t("admin.ddns.saveAndUpdate")
              }}
            </span>
          </Button>
        </template>

        <template #floating>
          <Button
            v-if="isDirty"
            variant="outline"
            :disabled="isSaving || isTesting"
            :aria-label="t('common.cancel')"
            :title="t('common.cancel')"
            @click="emit('cancel')"
            class="!w-10 !min-w-10 shrink-0 !gap-0 !px-0 border-white/20 bg-transparent text-white hover:bg-white/10 hover:text-white sm:!w-auto sm:!min-w-[5.65rem] sm:!gap-2 sm:!px-[1.15rem]"
          >
            <Undo2 class="h-4 w-4" />
            <span class="hidden sm:inline">{{ t("common.cancel") }}</span>
          </Button>
          <Button
            v-if="isDirty"
            variant="outline"
            :disabled="isSaving || isTesting || !selectedProvider"
            :aria-label="isSaving ? t('admin.ddns.saving') : t('common.save')"
            :title="isSaving ? t('admin.ddns.saving') : t('common.save')"
            @click="emit('save')"
            class="!w-10 !min-w-10 shrink-0 !gap-0 !px-0 border-white/20 bg-transparent text-white hover:bg-white/10 hover:text-white sm:!w-auto sm:!min-w-[5.65rem] sm:!gap-2 sm:!px-[1.15rem]"
          >
            <RefreshCw v-if="isSaving" class="h-4 w-4 animate-spin" />
            <Save v-else class="h-4 w-4" />
            <span class="hidden sm:inline">
              {{ isSaving ? t("admin.ddns.saving") : t("common.save") }}
            </span>
          </Button>
          <Button
            :disabled="isTesting || isSaving || !selectedProvider"
            :aria-label="
              isTesting
                ? t('admin.ddns.updating')
                : t('admin.ddns.saveAndUpdate')
            "
            :title="
              isTesting
                ? t('admin.ddns.updating')
                : t('admin.ddns.saveAndUpdate')
            "
            @click="emit('test')"
            class="!w-10 !min-w-10 shrink-0 !gap-0 !px-0 shadow-sm sm:!w-auto sm:!min-w-[5.65rem] sm:!gap-2 sm:!px-[1.15rem]"
          >
            <RefreshCw v-if="isTesting" class="h-4 w-4 animate-spin" />
            <RefreshCw v-else class="h-4 w-4 sm:hidden" />
            <span class="hidden sm:inline">
              {{
                isTesting
                  ? t("admin.ddns.updating")
                  : t("admin.ddns.saveAndUpdate")
              }}
            </span>
          </Button>
        </template>
      </FloatingActionDock>
    </template>
  </ConfigCollapsibleCard>
</template>
