<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { Eye, EyeOff, RefreshCw } from "lucide-vue-next";
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
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import OverflowTooltipText from "@admin-shared/components/common/OverflowTooltipText.vue";
import type { DDNSNetworkInterfacePayload } from "@/lib/api";
import {
  DEFAULT_DDNS_IP_SOURCE,
  DEFAULT_DDNS_UPDATE_SCOPE,
  INTERFACE_IPV4_INDEX_KEY,
  INTERFACE_IPV6_INDEX_KEY,
  IP_SOURCE_KEY,
  IP_SOURCE_OPTIONS,
  NETWORK_INTERFACE_AUTO_VALUE,
  NETWORK_INTERFACE_KEY,
  SOURCE_DOMAIN_KEY,
  STATIC_IPV4_KEY,
  STATIC_IPV6_KEY,
  UPDATE_SCOPE_KEY,
  UPDATE_SCOPE_OPTIONS,
  normalizeInterfaceAddressIndex,
  normalizeIpSource,
  normalizeUpdateScope,
  toNetworkInterfaceSelectValue,
  type DDNSIpSource,
  type DDNSUpdateScope,
  type Provider,
  type ProviderField,
  type TargetDialogState,
} from "./model";

type LabelKeyOption<TValue extends string> = {
  labelKey: string;
  value: TValue;
};

type AddressOption = {
  label: string;
  value: string;
};

defineProps<{
  description: string;
  formatDomainField: () => void;
  formatOptionLabel: (
    option: LabelKeyOption<DDNSIpSource | DDNSUpdateScope>,
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
  ipv4Options: AddressOption[];
  ipv6Options: AddressOption[];
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
    <DialogContent class="sm:max-w-[760px] max-h-[88vh] overflow-y-auto">
      <DialogHeader>
        <DialogTitle>{{ title }}</DialogTitle>
        <DialogDescription>{{ description }}</DialogDescription>
      </DialogHeader>

      <div class="overflow-hidden rounded-lg border divide-y divide-border">
        <div
          class="p-4 sm:p-5 grid gap-3 sm:grid-cols-[180px_1fr] md:grid-cols-[220px_1fr] items-start transition-colors hover:bg-muted/10"
        >
          <div class="space-y-1 sm:mt-1.5">
            <Label class="text-sm font-medium">
              {{ t("admin.ddns.targetEnabledLabel") }}
            </Label>
            <p class="text-xs text-muted-foreground hidden sm:block pr-4">
              {{ t("admin.ddns.targetEnabledHint") }}
            </p>
          </div>
          <div class="w-full max-w-md space-y-2 sm:justify-self-end">
            <div
              class="flex min-h-10 w-full items-center justify-start gap-3 sm:justify-end sm:px-3"
            >
              <Switch v-model="state.enabled" />
              <span class="text-sm text-muted-foreground">
                {{
                  state.enabled
                    ? t("admin.ddns.activeLabel")
                    : t("admin.ddns.stoppedLabel")
                }}
              </span>
            </div>
            <p class="text-[11px] text-muted-foreground sm:hidden">
              {{ t("admin.ddns.targetEnabledHint") }}
            </p>
          </div>
        </div>

        <div
          class="p-4 sm:p-5 grid gap-2 sm:grid-cols-[180px_1fr] md:grid-cols-[220px_1fr] items-start transition-colors hover:bg-muted/10"
        >
          <div class="space-y-1 mt-1.5">
            <Label for="ddns-target-name" class="text-sm font-medium">
              {{ t("admin.ddns.name") }}
            </Label>
            <p class="text-xs text-muted-foreground hidden sm:block pr-4">
              {{ t("admin.ddns.targetNameHint") }}
            </p>
          </div>
          <div class="w-full max-w-md space-y-2">
            <Input
              id="ddns-target-name"
              v-model="state.name"
              :placeholder="t('admin.ddns.targetNamePlaceholder')"
            />
            <p class="text-[11px] text-muted-foreground sm:hidden">
              {{ t("admin.ddns.targetNameHint") }}
            </p>
          </div>
        </div>

        <div
          class="p-4 sm:p-5 grid gap-2 sm:grid-cols-[180px_1fr] md:grid-cols-[220px_1fr] items-start transition-colors hover:bg-muted/10"
        >
          <div class="space-y-1 mt-1.5">
            <Label for="ddns-target-provider" class="text-sm font-medium">
              {{ t("admin.ddns.providerLabel") }}
            </Label>
            <p class="text-xs text-muted-foreground hidden sm:block pr-4">
              {{ t("admin.ddns.targetProviderHint") }}
            </p>
          </div>
          <div class="w-full max-w-md space-y-2">
            <Select
              :modelValue="state.provider"
              @update:modelValue="
                (val: any) => emit('update:provider', String(val ?? ''))
              "
            >
              <SelectTrigger id="ddns-target-provider">
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
            <p class="text-[11px] text-muted-foreground sm:hidden">
              {{ t("admin.ddns.targetProviderHint") }}
            </p>
          </div>
        </div>

        <template v-if="state.provider">
          <div
            v-if="shouldShowStaticBlock && updateScope !== 'ipv6_only'"
            class="p-4 sm:p-5 grid gap-2 sm:grid-cols-[180px_1fr] md:grid-cols-[220px_1fr] items-start transition-colors hover:bg-muted/10"
          >
            <div class="space-y-1 mt-1.5">
              <Label for="ddns-target-static-ipv4" class="text-sm font-medium">
                {{ t("admin.ddns.staticIpv4Label") }}
              </Label>
              <p class="text-xs text-muted-foreground hidden sm:block pr-4">
                {{ t("admin.ddns.staticIpv4Hint") }}
              </p>
            </div>
            <div class="w-full max-w-md space-y-2">
              <Input
                id="ddns-target-static-ipv4"
                v-model="state.config[STATIC_IPV4_KEY]"
                placeholder="203.0.113.10"
                inputmode="decimal"
                autocomplete="off"
              />
              <p class="text-[11px] text-muted-foreground sm:hidden">
                {{ t("admin.ddns.staticIpv4Hint") }}
              </p>
            </div>
          </div>

          <div
            v-if="shouldShowStaticBlock && updateScope !== 'ipv4_only'"
            class="p-4 sm:p-5 grid gap-2 sm:grid-cols-[180px_1fr] md:grid-cols-[220px_1fr] items-start transition-colors hover:bg-muted/10"
          >
            <div class="space-y-1 mt-1.5">
              <Label for="ddns-target-static-ipv6" class="text-sm font-medium">
                {{ t("admin.ddns.staticIpv6Label") }}
              </Label>
              <p class="text-xs text-muted-foreground hidden sm:block pr-4">
                {{ t("admin.ddns.staticIpv6Hint") }}
              </p>
            </div>
            <div class="w-full max-w-md space-y-2">
              <Input
                id="ddns-target-static-ipv6"
                v-model="state.config[STATIC_IPV6_KEY]"
                placeholder="2001:db8::10"
                autocomplete="off"
              />
              <p class="text-[11px] text-muted-foreground sm:hidden">
                {{ t("admin.ddns.staticIpv6Hint") }}
              </p>
            </div>
          </div>

          <div
            v-if="shouldShowDomainBlock"
            class="p-4 sm:p-5 grid gap-2 sm:grid-cols-[180px_1fr] md:grid-cols-[220px_1fr] items-start transition-colors hover:bg-muted/10"
          >
            <div class="space-y-1 mt-1.5">
              <Label
                for="ddns-target-source-domain"
                class="text-sm font-medium"
              >
                {{ t("admin.ddns.sourceDomainLabel") }}
              </Label>
              <p class="text-xs text-muted-foreground hidden sm:block pr-4">
                {{ t("admin.ddns.sourceDomainHint") }}
              </p>
            </div>
            <div class="w-full max-w-md space-y-2">
              <Input
                id="ddns-target-source-domain"
                v-model="state.config[SOURCE_DOMAIN_KEY]"
                placeholder="origin.example.com"
                autocomplete="off"
              />
              <p class="text-[11px] text-muted-foreground sm:hidden">
                {{ t("admin.ddns.sourceDomainHint") }}
              </p>
            </div>
          </div>

          <div
            class="p-4 sm:p-5 grid gap-2 sm:grid-cols-[180px_1fr] md:grid-cols-[220px_1fr] items-start transition-colors hover:bg-muted/10"
          >
            <div class="space-y-1 mt-1.5">
              <Label for="ddns-target-update-scope" class="text-sm font-medium">
                {{ t("admin.ddns.updateScopeLabel") }}
              </Label>
              <p class="text-xs text-muted-foreground hidden sm:block pr-4">
                {{ t("admin.ddns.updateScopeHint") }}
              </p>
            </div>
            <div class="w-full max-w-md space-y-2">
              <Select
                :modelValue="
                  state.config[UPDATE_SCOPE_KEY] || DEFAULT_DDNS_UPDATE_SCOPE
                "
                @update:modelValue="
                  (val: any) =>
                    (state.config[UPDATE_SCOPE_KEY] = normalizeUpdateScope(
                      String(val ?? ''),
                    ))
                "
              >
                <SelectTrigger id="ddns-target-update-scope">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem
                    v-for="option in UPDATE_SCOPE_OPTIONS"
                    :key="option.value"
                    :value="option.value"
                    :disabled="
                      isUpdateScopeOptionDisabled(state.provider, option.value)
                    "
                  >
                    {{ formatOptionLabel(option) }}
                  </SelectItem>
                </SelectContent>
              </Select>
              <p class="text-[11px] text-muted-foreground sm:hidden">
                {{ t("admin.ddns.updateScopeHint") }}
              </p>
            </div>
          </div>

          <div
            class="p-4 sm:p-5 grid gap-2 sm:grid-cols-[180px_1fr] md:grid-cols-[220px_1fr] items-start transition-colors hover:bg-muted/10"
          >
            <div class="space-y-1 mt-1.5">
              <Label for="ddns-target-ip-source" class="text-sm font-medium">
                {{ t("admin.ddns.ipSourceLabel") }}
              </Label>
              <p class="text-xs text-muted-foreground hidden sm:block pr-4">
                {{ t("admin.ddns.ipSourceHint") }}
              </p>
            </div>
            <div class="w-full max-w-md space-y-2">
              <Select
                :modelValue="
                  state.config[IP_SOURCE_KEY] || DEFAULT_DDNS_IP_SOURCE
                "
                @update:modelValue="
                  (val: any) =>
                    (state.config[IP_SOURCE_KEY] = normalizeIpSource(
                      String(val ?? ''),
                    ))
                "
              >
                <SelectTrigger id="ddns-target-ip-source">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem
                    v-for="option in IP_SOURCE_OPTIONS"
                    :key="option.value"
                    :value="option.value"
                    :disabled="
                      isIpSourceOptionDisabled(state.provider, option.value)
                    "
                  >
                    {{ formatOptionLabel(option) }}
                  </SelectItem>
                </SelectContent>
              </Select>
              <p class="text-[11px] text-muted-foreground">
                {{ t("admin.ddns.interfaceOnlyFiltered") }}
              </p>
            </div>
          </div>

          <div
            class="p-4 sm:p-5 grid gap-2 sm:grid-cols-[180px_1fr] md:grid-cols-[220px_1fr] items-start transition-colors hover:bg-muted/10"
          >
            <div class="space-y-1 mt-1.5">
              <Label
                for="ddns-target-network-interface"
                class="text-sm font-medium"
              >
                {{ t("admin.ddns.outboundInterface") }}
              </Label>
              <p class="text-xs text-muted-foreground hidden sm:block pr-4">
                {{ t("admin.ddns.interfaceHint") }}
              </p>
            </div>
            <div class="w-full max-w-md space-y-2">
              <Select
                :modelValue="
                  toNetworkInterfaceSelectValue(
                    state.config[NETWORK_INTERFACE_KEY],
                  )
                "
                @update:modelValue="
                  (val: any) =>
                    emit(
                      'update:networkInterface',
                      val === NETWORK_INTERFACE_AUTO_VALUE
                        ? ''
                        : String(val ?? ''),
                    )
                "
              >
                <SelectTrigger id="ddns-target-network-interface">
                  <SelectValue :placeholder="t('admin.ddns.autoSelect')">
                    <span class="block min-w-0 max-w-full truncate">
                      {{ networkInterfaceLabel }}
                    </span>
                  </SelectValue>
                </SelectTrigger>
                <SelectContent
                  class="w-[var(--reka-select-trigger-width)] max-w-[min(32rem,calc(100vw-2rem))]"
                >
                  <SelectItem :value="NETWORK_INTERFACE_AUTO_VALUE">
                    {{ t("admin.ddns.autoSelect") }}
                  </SelectItem>
                  <SelectItem
                    v-for="networkInterface in resolvedNetworkInterfaces"
                    :key="networkInterface.name"
                    :value="networkInterface.name"
                  >
                    <div class="min-w-0 flex-1 pr-5">
                      <OverflowTooltipText
                        :text="networkInterface.label"
                        class="text-sm"
                        tooltip-align="start"
                        tooltip-side="right"
                      />
                    </div>
                  </SelectItem>
                </SelectContent>
              </Select>
              <p class="text-[11px] text-muted-foreground sm:hidden">
                {{ t("admin.ddns.interfaceHint") }}
              </p>
            </div>
          </div>

          <div
            v-if="shouldShowInterfaceBlock"
            class="p-4 sm:p-5 grid gap-2 sm:grid-cols-[180px_1fr] md:grid-cols-[220px_1fr] items-start transition-colors hover:bg-muted/10"
          >
            <div class="space-y-1 mt-1.5">
              <Label class="text-sm font-medium">
                {{ t("admin.ddns.interfaceAddressHelpTitle") }}
              </Label>
              <p class="text-xs text-muted-foreground hidden sm:block pr-4">
                {{ t("admin.ddns.interfaceAddressHelp") }}
              </p>
            </div>
            <div
              class="w-full max-w-md space-y-2 text-[11px] leading-5 text-muted-foreground"
            >
              <p>{{ t("admin.ddns.addressOrderHelp") }}</p>
              <p>{{ t("admin.ddns.filteredAddressHelp") }}</p>
            </div>
          </div>

          <div
            v-if="updateScope !== 'ipv6_only' && shouldShowInterfaceBlock"
            class="p-4 sm:p-5 grid gap-2 sm:grid-cols-[180px_1fr] md:grid-cols-[220px_1fr] items-start transition-colors hover:bg-muted/10"
          >
            <div class="space-y-1 mt-1.5">
              <Label for="ddns-target-ipv4" class="text-sm font-medium">
                {{ t("admin.ddns.selectIpv4Label") }}
              </Label>
              <p class="text-xs text-muted-foreground hidden sm:block pr-4">
                {{ t("admin.ddns.selectIpv4Hint") }}
              </p>
            </div>
            <div class="w-full max-w-md space-y-2">
              <Select
                :modelValue="
                  normalizeInterfaceAddressIndex(
                    state.config[INTERFACE_IPV4_INDEX_KEY],
                  ) || undefined
                "
                :disabled="ipv4Options.length === 0"
                @update:modelValue="
                  (val: any) =>
                    (state.config[INTERFACE_IPV4_INDEX_KEY] =
                      normalizeInterfaceAddressIndex(String(val ?? '')))
                "
              >
                <SelectTrigger id="ddns-target-ipv4">
                  <SelectValue
                    :placeholder="t('admin.ddns.selectIpv4Placeholder')"
                  />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem
                    v-for="option in ipv4Options"
                    :key="option.value"
                    :value="option.value"
                  >
                    {{ option.label }}
                  </SelectItem>
                </SelectContent>
              </Select>
              <p class="text-[11px] text-muted-foreground sm:hidden">
                {{ t("admin.ddns.selectIpv4Hint") }}
              </p>
            </div>
          </div>

          <div
            v-if="updateScope !== 'ipv4_only' && shouldShowInterfaceBlock"
            class="p-4 sm:p-5 grid gap-2 sm:grid-cols-[180px_1fr] md:grid-cols-[220px_1fr] items-start transition-colors hover:bg-muted/10"
          >
            <div class="space-y-1 mt-1.5">
              <Label for="ddns-target-ipv6" class="text-sm font-medium">
                {{ t("admin.ddns.selectIpv6Label") }}
              </Label>
              <p class="text-xs text-muted-foreground hidden sm:block pr-4">
                {{ t("admin.ddns.selectIpv6Hint") }}
              </p>
            </div>
            <div class="w-full max-w-md space-y-2">
              <Select
                :modelValue="
                  normalizeInterfaceAddressIndex(
                    state.config[INTERFACE_IPV6_INDEX_KEY],
                  ) || undefined
                "
                :disabled="ipv6Options.length === 0"
                @update:modelValue="
                  (val: any) =>
                    (state.config[INTERFACE_IPV6_INDEX_KEY] =
                      normalizeInterfaceAddressIndex(String(val ?? '')))
                "
              >
                <SelectTrigger id="ddns-target-ipv6">
                  <SelectValue
                    :placeholder="t('admin.ddns.selectIpv6Placeholder')"
                  />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem
                    v-for="option in ipv6Options"
                    :key="option.value"
                    :value="option.value"
                  >
                    {{ option.label }}
                  </SelectItem>
                </SelectContent>
              </Select>
              <p class="text-[11px] text-muted-foreground sm:hidden">
                {{ t("admin.ddns.selectIpv6Hint") }}
              </p>
            </div>
          </div>

          <template v-if="providerDef">
            <div
              v-for="field in providerDef.fields"
              :key="`target-${field.key}`"
              class="p-4 sm:p-5 grid gap-2 sm:grid-cols-[180px_1fr] md:grid-cols-[220px_1fr] items-start transition-colors hover:bg-muted/10"
            >
              <div class="space-y-1 mt-1.5">
                <Label
                  :for="`ddns-target-field-${field.key}`"
                  class="text-sm font-medium flex items-center gap-1"
                >
                  {{ field.label }}
                  <span
                    v-if="field.required !== false"
                    class="text-destructive"
                  >
                    *
                  </span>
                </Label>
                <p
                  v-if="getFieldDescription(field)"
                  class="text-xs text-muted-foreground hidden sm:block pr-4"
                >
                  {{ getFieldDescription(field) }}
                </p>
              </div>

              <div class="w-full max-w-md space-y-2">
                <Select
                  v-if="field.type === 'select' && field.options"
                  :modelValue="
                    state.config[field.key] || field.options[0]?.value || ''
                  "
                  @update:modelValue="
                    (val: any) => (state.config[field.key] = String(val ?? ''))
                  "
                >
                  <SelectTrigger :id="`ddns-target-field-${field.key}`">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem
                      v-for="option in field.options"
                      :key="option.value"
                      :value="option.value"
                    >
                      {{ option.label }}
                    </SelectItem>
                  </SelectContent>
                </Select>

                <div v-else-if="field.type === 'password'" class="relative">
                  <Input
                    :id="`ddns-target-field-${field.key}`"
                    v-model="state.config[field.key]"
                    :type="isFieldVisible(field.key) ? 'text' : 'password'"
                    :placeholder="field.placeholder"
                    :autocomplete="getFieldAutocomplete(field)"
                    class="pr-10"
                  />
                  <button
                    type="button"
                    class="absolute right-3 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground transition-colors"
                    @click="toggleFieldVisibility(field.key)"
                  >
                    <component
                      :is="isFieldVisible(field.key) ? EyeOff : Eye"
                      class="h-4 w-4"
                    />
                  </button>
                </div>

                <Input
                  v-else
                  :id="`ddns-target-field-${field.key}`"
                  v-model="state.config[field.key]"
                  :type="field.type"
                  :placeholder="field.placeholder"
                  :autocomplete="getFieldAutocomplete(field)"
                  @blur="field.key === 'domain' && formatDomainField()"
                />

                <p
                  v-if="getFieldDescription(field)"
                  class="text-[11px] text-muted-foreground sm:hidden"
                >
                  {{ getFieldDescription(field) }}
                </p>
              </div>
            </div>
          </template>
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
