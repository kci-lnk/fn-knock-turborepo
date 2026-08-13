<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { LoaderCircle } from "lucide-vue-next";
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
import { Textarea } from "@/components/ui/textarea";
import type { LdapProviderCatalogItem, LdapProviderType } from "@/types";
import type { LdapProviderForm } from "./useLdapProviderManagement";

defineProps<{
  applyPreset: (type: LdapProviderType) => void;
  catalog: LdapProviderCatalogItem[];
  editing: boolean;
  form: LdapProviderForm;
  isSaving: boolean;
  open: boolean;
  save: () => Promise<void> | void;
}>();
const emit = defineEmits<{ "update:open": [value: boolean] }>();
const { t } = useI18n();
</script>

<template>
  <Dialog :open="open" @update:open="emit('update:open', $event)">
    <DialogContent class="max-h-[90vh] overflow-y-auto sm:max-w-[720px]">
      <DialogHeader>
        <DialogTitle>
          {{
            editing
              ? t("admin.ldapProviders.editTitle")
              : t("admin.ldapProviders.createTitle")
          }}
        </DialogTitle>
        <DialogDescription>{{ t("admin.ldapProviders.formDescription") }}</DialogDescription>
      </DialogHeader>
      <div class="grid gap-4 sm:grid-cols-2">
        <div class="space-y-2">
          <Label for="ldap-provider-type">{{ t("admin.ldapProviders.type") }}</Label>
          <Select
            :model-value="form.type"
            :disabled="editing"
            @update:model-value="applyPreset($event as LdapProviderType)"
          >
            <SelectTrigger id="ldap-provider-type"><SelectValue /></SelectTrigger>
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
        </div>
        <div class="space-y-2">
          <Label for="ldap-provider-name">{{ t("admin.ldapProviders.name") }}</Label>
          <Input id="ldap-provider-name" v-model="form.name" />
        </div>
        <div class="space-y-2 sm:col-span-2">
          <Label for="ldap-provider-servers">{{ t("admin.ldapProviders.servers") }}</Label>
          <Textarea
            id="ldap-provider-servers"
            v-model="form.servers"
            rows="3"
            placeholder="ldaps://ldap1.example.com:636"
          />
        </div>
        <div class="space-y-2">
          <Label for="ldap-provider-transport">{{ t("admin.ldapProviders.transport") }}</Label>
          <Select v-model="form.transport">
            <SelectTrigger id="ldap-provider-transport"><SelectValue /></SelectTrigger>
            <SelectContent>
              <SelectItem value="ldaps">LDAPS</SelectItem>
              <SelectItem value="starttls">StartTLS</SelectItem>
            </SelectContent>
          </Select>
        </div>
        <div class="space-y-2">
          <Label for="ldap-provider-bind-mode">{{ t("admin.ldapProviders.bindMode") }}</Label>
          <Select v-model="form.bindMode">
            <SelectTrigger id="ldap-provider-bind-mode"><SelectValue /></SelectTrigger>
            <SelectContent>
              <SelectItem value="search">{{ t("admin.ldapProviders.searchBind") }}</SelectItem>
              <SelectItem value="direct">{{ t("admin.ldapProviders.directBind") }}</SelectItem>
            </SelectContent>
          </Select>
        </div>
        <div class="space-y-2 sm:col-span-2">
          <Label for="ldap-provider-base-dn">Base DN</Label>
          <Input
            id="ldap-provider-base-dn"
            v-model="form.baseDn"
            placeholder="dc=example,dc=com"
          />
        </div>
        <div class="space-y-2 sm:col-span-2">
          <Label for="ldap-provider-user-filter">{{ t("admin.ldapProviders.userFilter") }}</Label>
          <Input id="ldap-provider-user-filter" v-model="form.userFilter" />
        </div>
        <template v-if="form.bindMode === 'search'">
          <div class="space-y-2">
            <Label for="ldap-provider-service-dn">{{ t("admin.ldapProviders.serviceDn") }}</Label>
            <Input
              id="ldap-provider-service-dn"
              v-model="form.serviceBindDn"
              autocomplete="off"
            />
          </div>
          <div class="space-y-2">
            <Label for="ldap-provider-service-password">
              {{ t("admin.ldapProviders.servicePassword") }}
            </Label>
            <Input
              id="ldap-provider-service-password"
              v-model="form.serviceBindPassword"
              type="password"
              autocomplete="new-password"
              :placeholder="editing ? t('admin.ldapProviders.keepSecret') : ''"
            />
          </div>
        </template>
        <div v-else class="space-y-2 sm:col-span-2">
          <Label for="ldap-provider-direct-template">
            {{ t("admin.ldapProviders.directTemplate") }}
          </Label>
          <Input
            id="ldap-provider-direct-template"
            v-model="form.directBindTemplate"
            placeholder="{username}@example.com"
          />
        </div>
        <div class="space-y-2">
          <Label for="ldap-provider-subject-attribute">
            {{ t("admin.ldapProviders.subjectAttribute") }}
          </Label>
          <Input id="ldap-provider-subject-attribute" v-model="form.subjectAttribute" />
        </div>
        <div class="space-y-2">
          <Label for="ldap-provider-username-attribute">
            {{ t("admin.ldapProviders.usernameAttribute") }}
          </Label>
          <Input id="ldap-provider-username-attribute" v-model="form.usernameAttribute" />
        </div>
        <div class="space-y-2">
          <Label for="ldap-provider-display-attribute">
            {{ t("admin.ldapProviders.displayAttribute") }}
          </Label>
          <Input id="ldap-provider-display-attribute" v-model="form.displayNameAttribute" />
        </div>
        <div class="space-y-2">
          <Label for="ldap-provider-email-attribute">
            {{ t("admin.ldapProviders.emailAttribute") }}
          </Label>
          <Input id="ldap-provider-email-attribute" v-model="form.emailAttribute" />
        </div>
        <div class="space-y-2 sm:col-span-2">
          <Label for="ldap-provider-ca-pem">{{ t("admin.ldapProviders.caPem") }}</Label>
          <Textarea
            id="ldap-provider-ca-pem"
            v-model="form.caPem"
            rows="4"
            placeholder="-----BEGIN CERTIFICATE-----"
          />
        </div>
        <div
          class="flex items-center justify-between rounded-md border p-3 sm:col-span-2"
        >
          <Label for="ldap-provider-enabled">{{ t("admin.ldapProviders.enabled") }}</Label>
          <Switch id="ldap-provider-enabled" v-model="form.enabled" />
        </div>
      </div>
      <DialogFooter>
        <Button variant="outline" @click="emit('update:open', false)">
          {{ t("admin.ldapProviders.cancel") }}
        </Button>
        <Button :disabled="isSaving" @click="save">
          <LoaderCircle v-if="isSaving" class="h-4 w-4 animate-spin" />
          {{ t("admin.ldapProviders.save") }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
