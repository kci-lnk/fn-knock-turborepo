<script setup lang="ts">
import {
  computed,
  type ComponentPublicInstance,
  type StyleValue,
  type UnwrapNestedRefs,
} from "vue";
import { useI18n } from "vue-i18n";
import {
  AlertTriangle,
  ChevronLeft,
  ChevronRight,
  RefreshCw,
  ShieldCheck,
} from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Dialog, DialogContent, DialogFooter } from "@/components/ui/dialog";
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
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import SubdomainMappingVisibilityPanel from "./SubdomainMappingVisibilityPanel.vue";
import SubdomainMappingIconPanel from "./SubdomainMappingIconPanel.vue";
import SubdomainMappingIconEntry from "./SubdomainMappingIconEntry.vue";
import SubdomainMappingTargetField from "./SubdomainMappingTargetField.vue";
import type { HostMapping } from "@/types";
import type { MappingInputMode } from "./model";
import { useMappingVisibility } from "./useMappingVisibility";
import type { useMappingIcon } from "./useMappingIcon";
const props = defineProps<{
  basicAuthInjection: boolean;
  basicAuthValidationMessage: string;
  canRefreshMappingMetadata: boolean;
  canShowBasicAuthInjection: boolean;
  canUseRootDomainSuffix: boolean;
  composedPreviewHost: string;
  contentStyle: StyleValue;
  fullHostInputHint: string;
  gatewayHostResponseBlockedReason: string;
  gatewayProxyHeadersBlockedReason: string;
  globalWafEnabled: boolean;
  handleFocusIn: (event: FocusEvent) => void;
  handleInputModeChange: (mode: MappingInputMode) => void;
  handlePortalDisabledTooltipOpenChange: (open: boolean) => void;
  handlePortalDisabledTooltipTriggerClick: () => void;
  isGatewayAdvancedLoading: boolean;
  iconEditor: UnwrapNestedRefs<ReturnType<typeof useMappingIcon>>;
  isMappingAuthService: boolean;
  isMappingValid: boolean;
  isMappingWebSocketTarget: boolean;
  isPortalDisabledTooltipOpen: boolean;
  isRefreshingMappingMetadata: boolean;
  isSavingMappings: boolean;
  mappingForm: HostMapping;
  mappingInputLabel: string;
  mappingInputMode: MappingInputMode;
  mappingModeDescription: string;
  mappingResolvedTitle: string;
  mappingSubdomain: string;
  mappingUseAuth: boolean;
  open: boolean;
  preserveHost: boolean;
  refreshMappingMetadata: () => void | Promise<unknown>;
  savedRootDomain: string;
  scrollStyle: StyleValue;
  sendProxyHeaders: boolean;
  setBasicAuthInjection: (value: boolean) => void;
  setMappingSubdomain: (value: string) => void;
  setMappingUseAuth: (value: boolean) => void;
  setPreserveHost: (value: boolean) => void;
  setScrollElement: (element: Element | ComponentPublicInstance | null) => void;
  setSendProxyHeaders: (value: boolean) => void;
  setShowToolbar: (value: boolean) => void;
  shouldShowPortalDisabledTooltip: boolean;
  showToolbar: boolean;
  updateMappingBasicAuth: (patch: Partial<HostMapping["basic_auth"]>) => void;
  updateMappingForm: (patch: Partial<HostMapping>) => void;
  visibilityEditor: UnwrapNestedRefs<ReturnType<typeof useMappingVisibility>>;
}>();

const emit = defineEmits<{
  close: [];
  save: [];
  "update:open": [value: boolean];
}>();

const { t } = useI18n();

const titleOverrideModel = computed({
  get: () => props.mappingForm.title_override,
  set: (value: string) => props.updateMappingForm({ title_override: value }),
});
const mappingSubdomainModel = computed({
  get: () => props.mappingSubdomain,
  set: (value: string) => props.setMappingSubdomain(value),
});
const mappingUseAuthModel = computed({
  get: () => props.mappingUseAuth,
  set: (value: boolean) => props.setMappingUseAuth(value),
});
const showToolbarModel = computed({
  get: () => props.showToolbar,
  set: (value: boolean) => props.setShowToolbar(value),
});
const basicAuthInjectionModel = computed({
  get: () => props.basicAuthInjection,
  set: (value: boolean) => props.setBasicAuthInjection(value),
});
const basicAuthUsernameModel = computed({
  get: () => props.mappingForm.basic_auth.username,
  set: (value: string) => props.updateMappingBasicAuth({ username: value }),
});

const basicAuthPasswordModel = computed({
  get: () => props.mappingForm.basic_auth.password,
  set: (value: string) => props.updateMappingBasicAuth({ password: value }),
});

const sendProxyHeadersModel = computed({
  get: () => props.sendProxyHeaders,
  set: (value: boolean) => props.setSendProxyHeaders(value),
});

const preserveHostModel = computed({
  get: () => props.preserveHost,
  set: (value: boolean) => props.setPreserveHost(value),
});

const protocolModeModel = computed({
  get: () => props.mappingForm.protocol_mode || "auto",
  set: (value) =>
    props.updateMappingForm({
      protocol_mode: value === "http1" || value === "http2" ? value : "auto",
    }),
});

const mappingWafEnabledModel = computed({
  get: () => props.mappingForm.waf_enabled !== false,
  set: (value: boolean) => props.updateMappingForm({ waf_enabled: value }),
});
</script>

<template>
  <Dialog :open="open" @update:open="emit('update:open', $event)">
    <DialogContent
      class="flex max-h-[85vh] flex-col gap-0 overflow-hidden overscroll-contain p-0 sm:max-w-[520px] max-sm:!inset-x-0 max-sm:!bottom-[var(--mapping-dialog-keyboard-inset)] max-sm:!top-auto max-sm:!max-w-none max-sm:!translate-x-0 max-sm:!translate-y-0 max-sm:max-h-[var(--mapping-dialog-mobile-max-height)] max-sm:rounded-b-none max-sm:border-b-0"
      :style="contentStyle"
      :show-close-button="false"
    >
      <div
        v-if="visibilityEditor.mappingDialogView !== 'basic'"
        class="shrink-0 border-b bg-background px-6 pb-3 pt-8"
      >
        <button
          type="button"
          class="-mx-2 inline-flex w-[calc(100%+1rem)] items-center gap-3 rounded-md px-2 py-1.5 text-left transition-colors hover:bg-accent hover:text-accent-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
          :aria-label="t('admin.subdomainProxy.backToBasicAria')"
          @click="visibilityEditor.returnBasicView"
        >
          <ChevronLeft class="h-4 w-4 shrink-0" />
          <span class="text-sm font-semibold">
            {{
              visibilityEditor.mappingDialogView === "icon"
                ? t("admin.subdomainProxy.iconTitle")
                : t("admin.subdomainProxy.visibilityTitle")
            }}
          </span>
        </button>
      </div>
      <div
        :ref="setScrollElement"
        class="relative min-h-0 flex-1 overscroll-contain overflow-x-hidden overflow-y-auto px-6 [overflow-anchor:none]"
        :style="scrollStyle"
        @focusin="handleFocusIn"
      >
        <Transition
          enter-active-class="motion-safe:transition-[opacity,transform] motion-safe:duration-200 motion-safe:ease-out motion-safe:will-change-transform motion-reduce:transition-none"
          leave-active-class="absolute inset-x-6 top-0 motion-safe:transition-[opacity,transform] motion-safe:duration-200 motion-safe:ease-out motion-safe:will-change-transform motion-reduce:hidden"
          :enter-from-class="visibilityEditor.transitionEnterFromClass"
          enter-to-class="translate-x-0 opacity-100"
          leave-from-class="translate-x-0 opacity-100"
          :leave-to-class="visibilityEditor.transitionLeaveToClass"
        >
          <div
            v-if="visibilityEditor.mappingDialogView === 'basic'"
            key="mapping-basic"
            class="grid gap-4 pb-4 pt-6"
          >
            <div class="space-y-2">
              <div class="flex items-center justify-between gap-3">
                <Label for="mapping-display-title">
                  {{ t("admin.subdomainProxy.displayTitle") }}
                </Label>
                <Button
                  variant="link"
                  size="sm"
                  class="h-auto p-0 text-xs"
                  :disabled="
                    !canRefreshMappingMetadata || isRefreshingMappingMetadata
                  "
                  @click="refreshMappingMetadata"
                >
                  <RefreshCw
                    v-if="isRefreshingMappingMetadata"
                    class="mr-1 h-3.5 w-3.5 animate-spin"
                  />
                  {{
                    isRefreshingMappingMetadata
                      ? t("admin.subdomainProxy.refreshing")
                      : t("admin.subdomainProxy.refreshTitle")
                  }}
                </Button>
              </div>
              <Input
                id="mapping-display-title"
                v-model="titleOverrideModel"
                :placeholder="t('admin.subdomainProxy.titleAutoPlaceholder')"
              />
              <p class="text-xs text-muted-foreground">
                {{ t("admin.subdomainProxy.titleHelp") }}
                <span v-if="mappingResolvedTitle">
                  {{
                    t("admin.subdomainProxy.fetchedTitle", {
                      title: mappingResolvedTitle,
                    })
                  }}
                </span>
                <span v-else-if="mappingForm.target.trim()">
                  {{ t("admin.subdomainProxy.noFetchedTitle") }}
                </span>
              </p>
            </div>

            <SubdomainMappingIconEntry
              :icon-editor="iconEditor"
              :open-editor="visibilityEditor.openIconView"
            />

            <div class="space-y-2">
              <div
                class="flex flex-col gap-2 sm:flex-row sm:items-start sm:justify-between"
              >
                <div class="space-y-1">
                  <Label for="mapping-subdomain">
                    {{ mappingInputLabel }}
                  </Label>
                  <p class="text-xs text-muted-foreground">
                    {{ mappingModeDescription }}
                  </p>
                </div>
                <div
                  role="group"
                  :aria-label="t('admin.subdomainProxy.hostInputModeAria')"
                  class="grid w-full grid-cols-2 rounded-lg bg-muted p-[3px] text-muted-foreground sm:w-[216px]"
                >
                  <button
                    type="button"
                    :aria-pressed="mappingInputMode === 'subdomain'"
                    :disabled="!canUseRootDomainSuffix"
                    class="inline-flex h-8 items-center justify-center rounded-md px-2 text-xs font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-50"
                    :class="
                      mappingInputMode === 'subdomain'
                        ? 'bg-background text-foreground shadow-sm'
                        : 'hover:text-foreground'
                    "
                    @click="handleInputModeChange('subdomain')"
                  >
                    {{ t("admin.subdomainProxy.fixedSuffix") }}
                  </button>
                  <button
                    type="button"
                    :aria-pressed="mappingInputMode === 'full_host'"
                    class="inline-flex h-8 items-center justify-center rounded-md px-2 text-xs font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                    :class="
                      mappingInputMode === 'full_host'
                        ? 'bg-background text-foreground shadow-sm'
                        : 'hover:text-foreground'
                    "
                    @click="handleInputModeChange('full_host')"
                  >
                    {{ t("admin.subdomainProxy.fullHost") }}
                  </button>
                </div>
              </div>
              <template v-if="mappingInputMode === 'subdomain'">
                <div class="flex items-stretch rounded-md border">
                  <Input
                    id="mapping-subdomain"
                    v-model="mappingSubdomainModel"
                    placeholder="redis"
                    class="rounded-none border-0 shadow-none focus-visible:ring-0"
                  />
                  <div
                    class="flex items-center border-l bg-muted/30 px-3 text-sm text-muted-foreground"
                  >
                    .{{ savedRootDomain }}
                  </div>
                </div>
                <p class="text-xs text-muted-foreground">
                  {{
                    t("admin.subdomainProxy.finalHost", {
                      host:
                        composedPreviewHost ||
                        t("admin.subdomainProxy.notFilled"),
                    })
                  }}
                </p>
              </template>
              <template v-else>
                <Input
                  id="mapping-subdomain"
                  v-model="mappingSubdomainModel"
                  placeholder="auth.other-domain.example"
                />
                <p class="text-xs text-muted-foreground">
                  {{ fullHostInputHint }}
                </p>
              </template>
            </div>

            <SubdomainMappingTargetField
              :model-value="mappingForm.target"
              :open="open"
              @update:model-value="updateMappingForm({ target: $event })"
            />

            <div class="space-y-3 pt-2">
              <h3 class="text-sm font-semibold">
                {{ t("admin.subdomainProxy.advancedConfig") }}
              </h3>

              <div
                class="flex items-center justify-between gap-4 rounded-lg border px-4 py-3"
              >
                <div class="min-w-0 space-y-1">
                  <Label for="mapping-auth">
                    {{ t("admin.subdomainProxy.authRequired") }}
                  </Label>
                  <p class="text-xs leading-5 text-muted-foreground">
                    {{ t("admin.subdomainProxy.authRequiredDescription") }}
                  </p>
                </div>
                <Switch
                  id="mapping-auth"
                  v-model="mappingUseAuthModel"
                  :disabled="isMappingAuthService"
                />
              </div>

              <div
                v-if="!isMappingWebSocketTarget"
                class="flex items-center justify-between gap-4 rounded-lg border px-4 py-3"
              >
                <div class="min-w-0 space-y-1">
                  <Label for="mapping-toolbar">
                    {{ t("admin.subdomainProxy.toolbar") }}
                  </Label>
                  <p class="text-xs leading-5 text-muted-foreground">
                    {{ t("admin.subdomainProxy.toolbarDescription") }}
                    <a
                      href="#/system/gateway-portal"
                      class="font-medium text-foreground underline underline-offset-4 transition hover:text-primary"
                    >
                      {{ t("admin.subdomainProxy.toolbarSettingsLink") }}
                    </a>
                    {{ t("admin.subdomainProxy.toolbarSettingsSuffix") }}
                  </p>
                </div>
                <TooltipProvider v-if="shouldShowPortalDisabledTooltip">
                  <Tooltip
                    :open="isPortalDisabledTooltipOpen"
                    @update:open="handlePortalDisabledTooltipOpenChange"
                  >
                    <TooltipTrigger as-child>
                      <Switch
                        id="mapping-toolbar"
                        class="cursor-help"
                        :model-value="showToolbar"
                        aria-disabled="true"
                        @click="handlePortalDisabledTooltipTriggerClick"
                        @keydown.enter.prevent="
                          handlePortalDisabledTooltipTriggerClick
                        "
                        @keydown.space.prevent="
                          handlePortalDisabledTooltipTriggerClick
                        "
                      />
                    </TooltipTrigger>
                    <TooltipContent side="top" align="end" class="max-w-xs">
                      <p>
                        {{
                          t("admin.subdomainProxy.portalDisabledDescription")
                        }}
                      </p>
                    </TooltipContent>
                  </Tooltip>
                </TooltipProvider>
                <Switch
                  v-else
                  id="mapping-toolbar"
                  v-model="showToolbarModel"
                />
              </div>

              <div
                v-if="canShowBasicAuthInjection"
                class="space-y-3 rounded-lg border px-4 py-3"
              >
                <div class="flex items-center justify-between gap-4">
                  <div class="min-w-0 space-y-1">
                    <Label for="mapping-basic-auth">
                      {{ t("admin.subdomainProxy.basicAuthSkip") }}
                    </Label>
                    <p class="text-xs leading-5 text-muted-foreground">
                      {{ t("admin.subdomainProxy.basicAuthSkipDescription") }}
                    </p>
                  </div>
                  <Switch
                    id="mapping-basic-auth"
                    v-model="basicAuthInjectionModel"
                    :disabled="isMappingAuthService"
                  />
                </div>

                <div
                  v-if="basicAuthInjectionModel"
                  class="grid gap-3 sm:grid-cols-2"
                >
                  <div class="space-y-2">
                    <Label for="mapping-basic-auth-username">
                      {{ t("admin.subdomainProxy.username") }}
                    </Label>
                    <Input
                      id="mapping-basic-auth-username"
                      v-model="basicAuthUsernameModel"
                      autocomplete="username"
                      placeholder="admin"
                    />
                  </div>
                  <div class="space-y-2">
                    <Label for="mapping-basic-auth-password">
                      {{ t("admin.subdomainProxy.password") }}
                    </Label>
                    <Input
                      id="mapping-basic-auth-password"
                      v-model="basicAuthPasswordModel"
                      type="password"
                      autocomplete="new-password"
                    />
                  </div>
                  <p
                    v-if="basicAuthValidationMessage"
                    class="text-xs text-destructive sm:col-span-2"
                  >
                    {{ basicAuthValidationMessage }}
                  </p>
                </div>
              </div>

              <div
                class="flex items-center justify-between gap-4 rounded-lg border px-4 py-3"
              >
                <div class="min-w-0 space-y-1">
                  <Label for="mapping-proxy-headers">
                    {{ t("admin.subdomainProxy.proxyHeaders") }}
                  </Label>
                  <p class="text-xs leading-5 text-muted-foreground">
                    <template v-if="gatewayProxyHeadersBlockedReason">
                      {{ gatewayProxyHeadersBlockedReason }}
                    </template>
                    <template v-else>
                      {{ t("admin.subdomainProxy.proxyHeadersDescription") }}
                    </template>
                  </p>
                </div>
                <Switch
                  id="mapping-proxy-headers"
                  v-model="sendProxyHeadersModel"
                  :disabled="
                    isSavingMappings || !!gatewayProxyHeadersBlockedReason
                  "
                />
              </div>

              <div
                class="flex items-center justify-between gap-4 rounded-lg border px-4 py-3"
              >
                <div class="min-w-0 space-y-1">
                  <Label for="mapping-host-response">
                    {{ t("admin.subdomainProxy.hostResponse") }}
                  </Label>
                  <p class="text-xs leading-5 text-muted-foreground">
                    <template v-if="gatewayHostResponseBlockedReason">
                      {{ gatewayHostResponseBlockedReason }}
                    </template>
                    <template v-else>
                      {{ t("admin.subdomainProxy.hostResponseDescription") }}
                    </template>
                  </p>
                </div>
                <Switch
                  id="mapping-host-response"
                  v-model="preserveHostModel"
                  :disabled="
                    isSavingMappings || !!gatewayHostResponseBlockedReason
                  "
                />
              </div>

              <div class="space-y-2 rounded-lg border px-4 py-3">
                <div class="space-y-1">
                  <Label for="mapping-protocol-mode">
                    {{ t("admin.subdomainProxy.protocolMode") }}
                  </Label>
                  <p class="text-xs leading-5 text-muted-foreground">
                    {{ t("admin.subdomainProxy.protocolModeDescription") }}
                  </p>
                </div>
                <Select
                  v-model="protocolModeModel"
                  :disabled="isSavingMappings"
                >
                  <SelectTrigger
                    id="mapping-protocol-mode"
                    class="w-full"
                    :disabled="isSavingMappings"
                  >
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="auto">
                      {{ t("admin.subdomainProxy.protocolModeAuto") }}
                    </SelectItem>
                    <SelectItem value="http1">
                      {{ t("admin.subdomainProxy.protocolModeHttp1") }}
                    </SelectItem>
                    <SelectItem value="http2">
                      {{ t("admin.subdomainProxy.protocolModeHttp2") }}
                    </SelectItem>
                  </SelectContent>
                </Select>
              </div>

              <div
                v-if="globalWafEnabled && !isMappingAuthService"
                class="flex items-center justify-between gap-4 rounded-lg border px-4 py-3"
              >
                <div class="min-w-0 space-y-1">
                  <Label for="mapping-waf">
                    {{ t("admin.subdomainProxy.wafEnabled") }}
                  </Label>
                  <p class="text-xs leading-5 text-muted-foreground">
                    {{ t("admin.subdomainProxy.wafEnabledDescription") }}
                  </p>
                </div>
                <Switch
                  id="mapping-waf"
                  v-model="mappingWafEnabledModel"
                  :disabled="isSavingMappings"
                />
              </div>

              <Alert
                v-if="
                  !isMappingAuthService &&
                  visibilityEditor.globalVisibilityLoadError
                "
                variant="destructive"
                class="items-start"
              >
                <AlertTriangle class="h-4 w-4" />
                <AlertTitle>
                  {{ t("admin.subdomainProxy.visibilityLoadFailed") }}
                </AlertTitle>
                <AlertDescription class="space-y-3">
                  <p class="break-words">
                    {{ visibilityEditor.globalVisibilityLoadError }}
                  </p>
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    :disabled="visibilityEditor.isGlobalVisibilityLoading"
                    @click="visibilityEditor.loadGlobalVisibility"
                  >
                    <RefreshCw
                      class="mr-2 h-3.5 w-3.5"
                      :class="{
                        'animate-spin':
                          visibilityEditor.isGlobalVisibilityLoading,
                      }"
                    />
                    {{ t("admin.subdomainProxy.retry") }}
                  </Button>
                </AlertDescription>
              </Alert>

              <Button
                v-if="visibilityEditor.visibilityAvailable"
                type="button"
                variant="outline"
                class="h-auto w-full justify-between gap-3 px-4 py-3 text-left"
                @click="visibilityEditor.openVisibilityView"
              >
                <span class="flex min-w-0 flex-1 items-start gap-3">
                  <ShieldCheck class="mt-0.5 h-4 w-4 text-muted-foreground" />
                  <span class="min-w-0 flex-1 space-y-1">
                    <span class="block text-sm font-medium">
                      {{ t("admin.subdomainProxy.visibilityTitle") }}
                    </span>
                    <span
                      class="block whitespace-normal break-words text-xs font-normal leading-5"
                      :class="
                        visibilityEditor.visibilityValidationMessage
                          ? 'text-destructive'
                          : 'text-muted-foreground'
                      "
                    >
                      {{
                        visibilityEditor.visibilityValidationMessage ||
                        visibilityEditor.visibilitySummary
                      }}
                    </span>
                  </span>
                </span>
                <ChevronRight class="h-4 w-4 shrink-0 text-muted-foreground" />
              </Button>
            </div>
          </div>
          <SubdomainMappingIconPanel
            v-else-if="visibilityEditor.mappingDialogView === 'icon'"
            key="mapping-icon"
            :icon-editor="iconEditor"
            :is-saving-mappings="isSavingMappings"
          />
          <SubdomainMappingVisibilityPanel
            v-else
            key="mapping-visibility"
            :composed-preview-host="composedPreviewHost"
            :mapping-form="mappingForm"
            :visibility-editor="visibilityEditor"
          />
        </Transition>
      </div>
      <DialogFooter
        class="shrink-0 border-t bg-background px-6 py-4 max-sm:pb-[calc(env(safe-area-inset-bottom)+1rem)]"
      >
        <Button variant="outline" @click="emit('close')">
          {{ t("admin.subdomainProxy.cancel") }}
        </Button>
        <Button
          :disabled="
            !isMappingValid ||
            isSavingMappings ||
            isGatewayAdvancedLoading ||
            iconEditor.isIconBusy
          "
          @click="emit('save')"
        >
          {{ t("admin.subdomainProxy.saveMapping") }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
