<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import type { SubdomainMappingDialogProps } from "./subdomain-mapping-dialog-contract";

const { dialog } = defineProps<{ dialog: SubdomainMappingDialogProps }>();
const { t } = useI18n();
const mappingUseAuthModel = computed({
  get: () => dialog.mappingUseAuth,
  set: (value: boolean) => dialog.setMappingUseAuth(value),
});
const showToolbarModel = computed({
  get: () => dialog.showToolbar,
  set: (value: boolean) => dialog.setShowToolbar(value),
});
const basicAuthInjectionModel = computed({
  get: () => dialog.basicAuthInjection,
  set: (value: boolean) => dialog.setBasicAuthInjection(value),
});
const basicAuthUsernameModel = computed({
  get: () => dialog.mappingForm.basic_auth.username,
  set: (value: string) => dialog.updateMappingBasicAuth({ username: value }),
});
const basicAuthPasswordModel = computed({
  get: () => dialog.mappingForm.basic_auth.password,
  set: (value: string) => dialog.updateMappingBasicAuth({ password: value }),
});
</script>

<template>
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
      :disabled="dialog.isMappingAuthService"
    />
  </div>

  <div
    v-if="
      dialog.mappingForm.target_type === 'proxy' &&
      !dialog.isMappingWebSocketTarget
    "
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
    <TooltipProvider v-if="dialog.shouldShowPortalDisabledTooltip">
      <Tooltip
        :open="dialog.isPortalDisabledTooltipOpen"
        @update:open="dialog.handlePortalDisabledTooltipOpenChange"
      >
        <TooltipTrigger as-child>
          <Switch
            id="mapping-toolbar"
            class="cursor-help"
            :model-value="dialog.showToolbar"
            aria-disabled="true"
            @click="dialog.handlePortalDisabledTooltipTriggerClick"
            @keydown.enter.prevent="
              dialog.handlePortalDisabledTooltipTriggerClick
            "
            @keydown.space.prevent="
              dialog.handlePortalDisabledTooltipTriggerClick
            "
          />
        </TooltipTrigger>
        <TooltipContent side="top" align="end" class="max-w-xs">
          <p>{{ t("admin.subdomainProxy.portalDisabledDescription") }}</p>
        </TooltipContent>
      </Tooltip>
    </TooltipProvider>
    <Switch v-else id="mapping-toolbar" v-model="showToolbarModel" />
  </div>

  <div
    v-if="dialog.canShowBasicAuthInjection"
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
        :disabled="dialog.isMappingAuthService"
      />
    </div>
    <div v-if="basicAuthInjectionModel" class="grid gap-3 sm:grid-cols-2">
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
        v-if="dialog.basicAuthValidationMessage"
        class="text-xs text-destructive sm:col-span-2"
      >
        {{ dialog.basicAuthValidationMessage }}
      </p>
    </div>
  </div>
</template>
