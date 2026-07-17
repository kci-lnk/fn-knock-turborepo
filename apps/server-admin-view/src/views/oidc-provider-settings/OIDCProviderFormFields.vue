<script setup lang="ts">
import { useI18n } from "vue-i18n";
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
import type { OIDCProviderCatalogItem } from "@/types";
import type { OIDCProviderForm } from "./oidcProviderForm";

defineProps<{
  catalog: OIDCProviderCatalogItem[];
  form: OIDCProviderForm;
  mode: "create" | "edit";
  providerLabel: (type: string) => string;
}>();

const emit = defineEmits<{
  "type-change": [value: unknown];
}>();

const { t } = useI18n();
</script>

<template>
  <div class="overflow-hidden rounded-lg border divide-y divide-border">
    <div class="space-y-2 p-4 transition-colors hover:bg-muted/10 sm:p-5">
      <Label :for="`oidc-${mode}-provider-type`">
        {{
          mode === "create"
            ? t("admin.oidcProviders.provider")
            : t("admin.oidcProviders.columns.type")
        }}
      </Label>
      <Select
        v-if="mode === 'create'"
        :model-value="form.type"
        @update:model-value="emit('type-change', $event)"
      >
        <SelectTrigger :id="`oidc-${mode}-provider-type`" class="w-full">
          <SelectValue :placeholder="t('admin.oidcProviders.selectProvider')" />
        </SelectTrigger>
        <SelectContent>
          <SelectItem
            v-for="item in catalog"
            :key="item.type"
            :value="item.type"
          >
            {{ item.label }}
          </SelectItem>
        </SelectContent>
      </Select>
      <Input
        v-else
        :id="`oidc-${mode}-provider-type`"
        :model-value="providerLabel(form.type)"
        disabled
      />
    </div>

    <div
      v-if="mode === 'edit' || form.type !== 'fnknock_qq'"
      class="space-y-2 p-4 transition-colors hover:bg-muted/10 sm:p-5"
    >
      <Label :for="`oidc-${mode}-provider-name`">
        {{ t("admin.oidcProviders.displayName") }}
      </Label>
      <Input
        :id="`oidc-${mode}-provider-name`"
        v-model="form.name"
        :placeholder="t('admin.oidcProviders.displayNamePlaceholder')"
      />
    </div>

    <div
      v-if="form.type === 'microsoft'"
      class="space-y-2 p-4 transition-colors hover:bg-muted/10 sm:p-5"
    >
      <Label :for="`oidc-${mode}-provider-tenant`">Tenant</Label>
      <Input
        :id="`oidc-${mode}-provider-tenant`"
        v-model="form.tenant"
        placeholder="common / organizations / tenant id"
      />
    </div>

    <div
      v-if="form.type !== 'fnknock_qq'"
      class="space-y-2 p-4 transition-colors hover:bg-muted/10 sm:p-5"
    >
      <Label :for="`oidc-${mode}-provider-client-id`">Client ID</Label>
      <Input
        :id="`oidc-${mode}-provider-client-id`"
        v-model="form.clientId"
        autocomplete="off"
      />
    </div>

    <div
      v-if="form.type !== 'fnknock_qq'"
      class="space-y-2 p-4 transition-colors hover:bg-muted/10 sm:p-5"
    >
      <Label :for="`oidc-${mode}-provider-client-secret`">
        Client Secret
      </Label>
      <Input
        :id="`oidc-${mode}-provider-client-secret`"
        v-model="form.clientSecret"
        type="password"
        autocomplete="new-password"
        :placeholder="
          mode === 'edit'
            ? t('admin.oidcProviders.keepSecretPlaceholder')
            : undefined
        "
      />
    </div>

    <div
      v-if="form.type === 'custom_oidc'"
      class="space-y-2 p-4 transition-colors hover:bg-muted/10 sm:p-5"
    >
      <Label :for="`oidc-${mode}-provider-issuer`">Issuer</Label>
      <Input
        :id="`oidc-${mode}-provider-issuer`"
        v-model="form.issuer"
        placeholder="https://idp.example.com"
      />
    </div>

    <div
      v-if="form.type !== 'fnknock_qq'"
      class="space-y-2 p-4 transition-colors hover:bg-muted/10 sm:p-5"
    >
      <Label :for="`oidc-${mode}-provider-scopes`">Scopes</Label>
      <Input
        :id="`oidc-${mode}-provider-scopes`"
        v-model="form.scopes"
        placeholder="openid profile email"
      />
    </div>

    <div
      v-if="mode === 'edit'"
      class="flex items-center justify-between gap-3 p-4 transition-colors hover:bg-muted/10 sm:p-5"
    >
      <Label class="text-sm font-medium">
        {{ t("admin.oidcProviders.enabledStatus") }}
      </Label>
      <div class="flex items-center gap-3">
        <Switch v-model="form.enabled" />
        <span class="text-sm text-muted-foreground">
          {{
            form.enabled
              ? t("admin.oidcProviders.enabled")
              : t("admin.oidcProviders.disabled")
          }}
        </span>
      </div>
    </div>
  </div>
</template>
