<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { RefreshCw } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import type { DDNSNetworkInterfacePayload } from "@/lib/api/ddns";
import type {
  DDNSAddressOption,
  DDNSLabelKeyOption,
} from "./ddns-target-dialog-contract";
import type {
  DDNSIpSource,
  DDNSUpdateScope,
  Provider,
  ProviderField,
  TargetDialogState,
} from "./model";
import DDNSTargetAddressFields from "./DDNSTargetAddressFields.vue";
import DDNSTargetBasicFields from "./DDNSTargetBasicFields.vue";
import DDNSTargetProviderFields from "./DDNSTargetProviderFields.vue";

defineProps<{
  description: string;
  formatDomainField: () => void;
  formatOptionLabel: (
    option: DDNSLabelKeyOption<DDNSIpSource | DDNSUpdateScope>,
  ) => string;
  getFieldAutocomplete: (field: ProviderField) => string;
  getFieldDescription: (field: ProviderField) => string;
  isFieldVisible: (key: string) => boolean;
  isIpSourceOptionDisabled: (
    providerName: string,
    option: DDNSIpSource,
  ) => boolean;
  isSaving: boolean;
  isUpdateScopeOptionDisabled: (
    providerName: string,
    option: DDNSUpdateScope,
  ) => boolean;
  networkInterfaceLabel: string;
  open: boolean;
  providers: Provider[];
  providerDef: Provider | null;
  resolvedNetworkInterfaces: DDNSNetworkInterfacePayload[];
  shouldShowDomainBlock: boolean;
  shouldShowInterfaceBlock: boolean;
  shouldShowStaticBlock: boolean;
  state: TargetDialogState;
  title: string;
  toggleFieldVisibility: (key: string) => void;
  updateScope: DDNSUpdateScope;
  ipv4Options: DDNSAddressOption[];
  ipv6Options: DDNSAddressOption[];
}>();

const emit = defineEmits<{
  confirm: [];
  "update:networkInterface": [value: string];
  "update:open": [value: boolean];
  "update:provider": [value: string];
}>();
const { t } = useI18n();
</script>

<template>
  <Dialog :open="open" @update:open="emit('update:open', $event)">
    <DialogContent class="max-h-[88vh] overflow-y-auto sm:max-w-[760px]">
      <DialogHeader>
        <DialogTitle>{{ title }}</DialogTitle>
        <DialogDescription>{{ description }}</DialogDescription>
      </DialogHeader>

      <div class="divide-y divide-border overflow-hidden rounded-lg border">
        <DDNSTargetBasicFields
          :providers="providers"
          :state="state"
          @update:provider="emit('update:provider', $event)"
        />

        <template v-if="state.provider">
          <DDNSTargetAddressFields
            :format-option-label="formatOptionLabel"
            :ipv4-options="ipv4Options"
            :ipv6-options="ipv6Options"
            :is-ip-source-option-disabled="isIpSourceOptionDisabled"
            :is-update-scope-option-disabled="isUpdateScopeOptionDisabled"
            :network-interface-label="networkInterfaceLabel"
            :resolved-network-interfaces="resolvedNetworkInterfaces"
            :should-show-domain-block="shouldShowDomainBlock"
            :should-show-interface-block="shouldShowInterfaceBlock"
            :should-show-static-block="shouldShowStaticBlock"
            :state="state"
            :update-scope="updateScope"
            @update:network-interface="
              emit('update:networkInterface', $event)
            "
          />
          <DDNSTargetProviderFields
            v-if="providerDef"
            :format-domain-field="formatDomainField"
            :get-field-autocomplete="getFieldAutocomplete"
            :get-field-description="getFieldDescription"
            :is-field-visible="isFieldVisible"
            :provider-def="providerDef"
            :state="state"
            :toggle-field-visibility="toggleFieldVisibility"
          />
        </template>
      </div>

      <DialogFooter class="gap-2">
        <Button
          variant="outline"
          :disabled="isSaving"
          @click="emit('update:open', false)"
        >
          {{ t("common.cancel") }}
        </Button>
        <Button :disabled="isSaving" @click="emit('confirm')">
          <RefreshCw v-if="isSaving" class="mr-1.5 h-4 w-4 animate-spin" />
          {{ isSaving ? t("admin.ddns.saving") : t("common.save") }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
