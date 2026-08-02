<template>
  <div class="w-full space-y-3">
    <div class="grid grid-cols-2 gap-2">
      <Button
        type="button"
        :variant="credentialKind === 'totp' ? 'default' : 'outline'"
        :disabled="disabled"
        @click="credentialKind = 'totp'"
      >
        {{ t("auth.totpLogin") }}
      </Button>
      <Button
        type="button"
        :variant="credentialKind === 'ldap' ? 'default' : 'outline'"
        :disabled="disabled"
        @click="credentialKind = 'ldap'"
      >
        {{ t("auth.ldapLogin") }}
      </Button>
    </div>

    <template v-if="credentialKind === 'ldap'">
      <div class="space-y-2">
        <Label for="ldap-provider">{{ t("auth.ldapProvider") }}</Label>
        <select
          v-if="providers.length > 1"
          id="ldap-provider"
          v-model="providerId"
          class="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
          :disabled="disabled"
        >
          <option
            v-for="provider in providers"
            :key="provider.id"
            :value="provider.id"
          >
            {{ provider.name }}
          </option>
        </select>
        <div
          v-else
          class="rounded-md border border-input bg-muted/40 px-3 py-2 text-sm"
        >
          {{ providers[0]?.name }}
        </div>
      </div>
      <div class="space-y-2">
        <Label for="ldap-username">{{ t("auth.ldapUsername") }}</Label>
        <Input
          id="ldap-username"
          v-model="username"
          autocomplete="username"
          :disabled="disabled"
        />
      </div>
      <div class="space-y-2">
        <Label for="ldap-password">{{ t("auth.ldapPassword") }}</Label>
        <Input
          id="ldap-password"
          v-model="password"
          type="password"
          autocomplete="current-password"
          :disabled="disabled"
        />
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import { useI18n } from "vue-i18n";
import type { AuthLdapProvider } from "@frontend-core/auth/types";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";

defineProps<{ disabled: boolean; providers: AuthLdapProvider[] }>();
const credentialKind = defineModel<"totp" | "ldap">("credentialKind", {
  required: true,
});
const providerId = defineModel<string>("providerId", { required: true });
const username = defineModel<string>("username", { required: true });
const password = defineModel<string>("password", { required: true });
const { t } = useI18n();
</script>
