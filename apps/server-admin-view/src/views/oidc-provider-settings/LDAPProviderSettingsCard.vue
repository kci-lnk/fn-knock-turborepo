<script setup lang="ts">
import { onMounted, reactive, ref } from "vue";
import { useI18n } from "vue-i18n";
import { LoaderCircle, Pencil, Plus, TestTube2, Trash2 } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
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
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import { toast } from "@admin-shared/utils/toast";
import { extractErrorMessage } from "@frontend-core/errors/extractErrorMessage";
import { ConfigAPI } from "@/lib/api/config";
import type {
  LdapProviderCatalogItem,
  LdapProviderType,
  LdapProviderView,
} from "@/types";

const { t } = useI18n();
const catalog = ref<LdapProviderCatalogItem[]>([]);
const providers = ref<LdapProviderView[]>([]);
const isLoading = ref(false);
const isSaving = ref(false);
const mutatingId = ref("");
const showDialog = ref(false);
const editingId = ref("");
const showTestCredentialsDialog = ref(false);
const testingProvider = ref<LdapProviderView | null>(null);
const testUsername = ref("");
const testPassword = ref("");

const form = reactive({
  type: "openldap" as LdapProviderType,
  name: "OpenLDAP",
  enabled: true,
  servers: "ldaps://ldap.example.com:636",
  transport: "ldaps" as "ldaps" | "starttls",
  bindMode: "search" as "search" | "direct",
  baseDn: "",
  userFilter: "(&(objectClass=person)(uid={username}))",
  serviceBindDn: "",
  serviceBindPassword: "",
  directBindTemplate: "uid={username},ou=people,dc=example,dc=com",
  subjectAttribute: "entryUUID",
  usernameAttribute: "uid",
  displayNameAttribute: "cn",
  emailAttribute: "mail",
  caPem: "",
});

const readText = (config: Record<string, unknown>, key: string) => {
  const value = config[key];
  return typeof value === "string" ? value : "";
};

const readServers = (config: Record<string, unknown>) =>
  Array.isArray(config.servers)
    ? config.servers.map(String).join("\n")
    : readText(config, "servers");

const applyPreset = (type: LdapProviderType) => {
  form.type = type;
  const preset = catalog.value.find((item) => item.type === type);
  if (!preset) return;
  form.name = preset.label;
  form.transport = preset.defaults.transport;
  form.bindMode = preset.defaults.bind_mode;
  form.userFilter = preset.defaults.user_filter;
  form.subjectAttribute = preset.defaults.subject_attribute;
  form.usernameAttribute = preset.defaults.username_attribute;
  form.displayNameAttribute = preset.defaults.display_name_attribute;
  form.emailAttribute = preset.defaults.email_attribute;
  form.directBindTemplate = type === "active_directory" ? "{username}" : "";
};

const resetForm = () => {
  editingId.value = "";
  form.baseDn = "";
  form.servers = "ldaps://ldap.example.com:636";
  form.serviceBindDn = "";
  form.serviceBindPassword = "";
  form.caPem = "";
  form.enabled = true;
  applyPreset(catalog.value[0]?.type || "openldap");
};

const load = async () => {
  isLoading.value = true;
  try {
    const [definitions, items] = await Promise.all([
      ConfigAPI.getLdapProviderCatalog(),
      ConfigAPI.getLdapProviders(),
    ]);
    catalog.value = definitions;
    providers.value = items;
  } catch (error) {
    toast.error(
      extractErrorMessage(error, t("admin.ldapProviders.loadFailed")),
    );
  } finally {
    isLoading.value = false;
  }
};

const openCreate = () => {
  resetForm();
  showDialog.value = true;
};

const openEdit = (provider: LdapProviderView) => {
  const config = provider.connection_config || {};
  editingId.value = provider.id;
  form.type = provider.type;
  form.name = provider.name;
  form.enabled = provider.enabled;
  form.servers = readServers(config);
  form.transport =
    readText(config, "transport") === "starttls" ? "starttls" : "ldaps";
  form.bindMode =
    readText(config, "bind_mode") === "direct" ? "direct" : "search";
  form.baseDn = readText(config, "base_dn");
  form.userFilter = readText(config, "user_filter");
  form.serviceBindDn = readText(config, "service_bind_dn");
  form.serviceBindPassword = "";
  form.directBindTemplate = readText(config, "direct_bind_template");
  form.subjectAttribute = readText(config, "subject_attribute");
  form.usernameAttribute = readText(config, "username_attribute");
  form.displayNameAttribute = readText(config, "display_name_attribute");
  form.emailAttribute = readText(config, "email_attribute");
  form.caPem = readText(config, "ca_pem");
  showDialog.value = true;
};

const payload = () => ({
  name: form.name.trim(),
  type: form.type,
  enabled: form.enabled,
  connection_config: {
    servers: form.servers
      .split(/\r?\n|,/u)
      .map((item) => item.trim())
      .filter(Boolean),
    transport: form.transport,
    bind_mode: form.bindMode,
    base_dn: form.baseDn.trim(),
    user_filter: form.userFilter.trim(),
    service_bind_dn: form.serviceBindDn.trim(),
    service_bind_password: form.serviceBindPassword,
    direct_bind_template: form.directBindTemplate.trim(),
    subject_attribute: form.subjectAttribute.trim(),
    username_attribute: form.usernameAttribute.trim(),
    display_name_attribute: form.displayNameAttribute.trim(),
    email_attribute: form.emailAttribute.trim(),
    ca_pem: form.caPem.trim(),
  },
});

const save = async () => {
  isSaving.value = true;
  try {
    if (editingId.value) {
      await ConfigAPI.updateLdapProvider(editingId.value, payload());
    } else {
      await ConfigAPI.createLdapProvider(payload());
    }
    toast.success(t("admin.ldapProviders.saved"));
    showDialog.value = false;
    await load();
  } catch (error) {
    toast.error(
      extractErrorMessage(error, t("admin.ldapProviders.saveFailed")),
    );
  } finally {
    isSaving.value = false;
  }
};

const runProviderTest = async (
  provider: LdapProviderView,
  credentials?: { username: string; password: string },
) => {
  mutatingId.value = provider.id;
  try {
    const result = await ConfigAPI.testLdapProvider(provider.id, credentials);
    if (!result.success) throw new Error(result.message);
    toast.success(result.message || t("admin.ldapProviders.testSucceeded"));
    await load();
  } catch (error) {
    toast.error(
      extractErrorMessage(error, t("admin.ldapProviders.testFailed")),
    );
  } finally {
    mutatingId.value = "";
  }
};

const testProvider = async (provider: LdapProviderView) => {
  if (readText(provider.connection_config, "bind_mode") === "direct") {
    testingProvider.value = provider;
    testUsername.value = "";
    testPassword.value = "";
    showTestCredentialsDialog.value = true;
    return;
  }
  await runProviderTest(provider);
};

const runDirectProviderTest = async () => {
  const provider = testingProvider.value;
  if (!provider || !testUsername.value.trim() || !testPassword.value) {
    toast.error(t("admin.ldapProviders.testCredentialsRequired"));
    return;
  }
  showTestCredentialsDialog.value = false;
  await runProviderTest(provider, {
    username: testUsername.value.trim(),
    password: testPassword.value,
  });
  testPassword.value = "";
  testingProvider.value = null;
};

const setTestCredentialsDialogOpen = (open: boolean) => {
  showTestCredentialsDialog.value = open;
  if (!open) {
    testPassword.value = "";
    testingProvider.value = null;
  }
};

const removeProvider = async (id: string) => {
  mutatingId.value = id;
  try {
    await ConfigAPI.deleteLdapProvider(id);
    toast.success(t("admin.ldapProviders.deleted"));
    await load();
  } catch (error) {
    toast.error(
      extractErrorMessage(error, t("admin.ldapProviders.deleteFailed")),
    );
  } finally {
    mutatingId.value = "";
  }
};

onMounted(load);
</script>

<template>
  <Card>
    <CardHeader
      class="gap-4 sm:flex sm:flex-row sm:items-start sm:justify-between"
    >
      <div class="space-y-1.5">
        <CardTitle>{{ t("admin.ldapProviders.title") }}</CardTitle>
        <CardDescription>{{
          t("admin.ldapProviders.description")
        }}</CardDescription>
      </div>
      <Button :disabled="isLoading" @click="openCreate">
        <Plus class="h-4 w-4" />{{ t("admin.ldapProviders.add") }}
      </Button>
    </CardHeader>
    <CardContent class="space-y-3">
      <div
        v-if="isLoading"
        class="py-8 text-center text-sm text-muted-foreground"
      >
        {{ t("admin.ldapProviders.loading") }}
      </div>
      <div
        v-for="provider in providers"
        v-else
        :key="provider.id"
        class="flex flex-col gap-3 rounded-lg border p-4 sm:flex-row sm:items-center"
      >
        <div class="min-w-0 flex-1">
          <div class="flex flex-wrap items-center gap-2">
            <span class="font-medium">{{ provider.name }}</span>
            <Badge variant="outline">{{ provider.type }}</Badge>
            <Badge :variant="provider.enabled ? 'default' : 'secondary'">
              {{
                provider.enabled
                  ? t("admin.ldapProviders.enabled")
                  : t("admin.ldapProviders.disabled")
              }}
            </Badge>
            <Badge
              v-if="provider.last_test_status === 'success'"
              variant="outline"
              class="border-emerald-500/40 text-emerald-600"
            >
              {{ t("admin.ldapProviders.testSucceeded") }}
            </Badge>
            <Badge
              v-else-if="provider.last_test_status === 'failed'"
              variant="outline"
              class="border-destructive/40 text-destructive"
              :title="provider.last_error || undefined"
            >
              {{ t("admin.ldapProviders.testFailed") }}
            </Badge>
          </div>
          <p class="mt-1 truncate text-xs text-muted-foreground">
            {{ readServers(provider.connection_config) || "-" }}
          </p>
        </div>
        <div class="flex gap-2">
          <Button
            variant="outline"
            size="sm"
            :disabled="!!mutatingId"
            @click="testProvider(provider)"
          >
            <LoaderCircle
              v-if="mutatingId === provider.id"
              class="h-4 w-4 animate-spin"
            />
            <TestTube2 v-else class="h-4 w-4" />
            {{ t("admin.ldapProviders.test") }}
          </Button>
          <Button
            variant="outline"
            size="sm"
            :disabled="!!mutatingId"
            :aria-label="t('admin.ldapProviders.editTitle')"
            @click="openEdit(provider)"
          >
            <Pencil class="h-4 w-4" />
          </Button>
          <ConfirmDangerPopover
            :title="t('admin.ldapProviders.deleteTitle')"
            :description="t('admin.ldapProviders.deleteDescription')"
            :loading="mutatingId === provider.id"
            :disabled="!!mutatingId"
            :on-confirm="() => removeProvider(provider.id)"
          >
            <template #trigger>
              <Button
                variant="destructive"
                size="sm"
                :disabled="!!mutatingId"
                :aria-label="t('admin.ldapProviders.deleteTitle')"
                ><Trash2 class="h-4 w-4"
              /></Button>
            </template>
          </ConfirmDangerPopover>
        </div>
      </div>
      <p
        v-if="!isLoading && providers.length === 0"
        class="py-8 text-center text-sm text-muted-foreground"
      >
        {{ t("admin.ldapProviders.empty") }}
      </p>
    </CardContent>
  </Card>

  <Dialog :open="showDialog" @update:open="showDialog = $event">
    <DialogContent class="max-h-[90vh] overflow-y-auto sm:max-w-[720px]">
      <DialogHeader>
        <DialogTitle>{{
          editingId
            ? t("admin.ldapProviders.editTitle")
            : t("admin.ldapProviders.createTitle")
        }}</DialogTitle>
        <DialogDescription>{{
          t("admin.ldapProviders.formDescription")
        }}</DialogDescription>
      </DialogHeader>
      <div class="grid gap-4 sm:grid-cols-2">
        <div class="space-y-2">
          <Label for="ldap-provider-type">{{
            t("admin.ldapProviders.type")
          }}</Label>
          <Select
            :model-value="form.type"
            :disabled="!!editingId"
            @update:model-value="applyPreset($event as LdapProviderType)"
          >
            <SelectTrigger id="ldap-provider-type"
              ><SelectValue
            /></SelectTrigger>
            <SelectContent
              ><SelectItem
                v-for="item in catalog"
                :key="item.type"
                :value="item.type"
                >{{ item.label }}</SelectItem
              ></SelectContent
            >
          </Select>
        </div>
        <div class="space-y-2">
          <Label for="ldap-provider-name">{{
            t("admin.ldapProviders.name")
          }}</Label
          ><Input id="ldap-provider-name" v-model="form.name" />
        </div>
        <div class="space-y-2 sm:col-span-2">
          <Label for="ldap-provider-servers">{{
            t("admin.ldapProviders.servers")
          }}</Label
          ><Textarea
            id="ldap-provider-servers"
            v-model="form.servers"
            rows="3"
            placeholder="ldaps://ldap1.example.com:636"
          />
        </div>
        <div class="space-y-2">
          <Label for="ldap-provider-transport">{{
            t("admin.ldapProviders.transport")
          }}</Label
          ><Select v-model="form.transport"
            ><SelectTrigger id="ldap-provider-transport"
              ><SelectValue /></SelectTrigger
            ><SelectContent
              ><SelectItem value="ldaps">LDAPS</SelectItem
              ><SelectItem value="starttls">StartTLS</SelectItem></SelectContent
            ></Select
          >
        </div>
        <div class="space-y-2">
          <Label for="ldap-provider-bind-mode">{{
            t("admin.ldapProviders.bindMode")
          }}</Label
          ><Select v-model="form.bindMode"
            ><SelectTrigger id="ldap-provider-bind-mode"
              ><SelectValue /></SelectTrigger
            ><SelectContent
              ><SelectItem value="search">{{
                t("admin.ldapProviders.searchBind")
              }}</SelectItem
              ><SelectItem value="direct">{{
                t("admin.ldapProviders.directBind")
              }}</SelectItem></SelectContent
            ></Select
          >
        </div>
        <div class="space-y-2 sm:col-span-2">
          <Label for="ldap-provider-base-dn">Base DN</Label
          ><Input
            id="ldap-provider-base-dn"
            v-model="form.baseDn"
            placeholder="dc=example,dc=com"
          />
        </div>
        <div class="space-y-2 sm:col-span-2">
          <Label for="ldap-provider-user-filter">{{
            t("admin.ldapProviders.userFilter")
          }}</Label
          ><Input id="ldap-provider-user-filter" v-model="form.userFilter" />
        </div>
        <template v-if="form.bindMode === 'search'">
          <div class="space-y-2">
            <Label for="ldap-provider-service-dn">{{
              t("admin.ldapProviders.serviceDn")
            }}</Label
            ><Input
              id="ldap-provider-service-dn"
              v-model="form.serviceBindDn"
              autocomplete="off"
            />
          </div>
          <div class="space-y-2">
            <Label for="ldap-provider-service-password">{{
              t("admin.ldapProviders.servicePassword")
            }}</Label
            ><Input
              id="ldap-provider-service-password"
              v-model="form.serviceBindPassword"
              type="password"
              autocomplete="new-password"
              :placeholder="
                editingId ? t('admin.ldapProviders.keepSecret') : ''
              "
            />
          </div>
        </template>
        <div v-else class="space-y-2 sm:col-span-2">
          <Label for="ldap-provider-direct-template">{{
            t("admin.ldapProviders.directTemplate")
          }}</Label
          ><Input
            id="ldap-provider-direct-template"
            v-model="form.directBindTemplate"
            placeholder="{username}@example.com"
          />
        </div>
        <div class="space-y-2">
          <Label for="ldap-provider-subject-attribute">{{
            t("admin.ldapProviders.subjectAttribute")
          }}</Label
          ><Input
            id="ldap-provider-subject-attribute"
            v-model="form.subjectAttribute"
          />
        </div>
        <div class="space-y-2">
          <Label for="ldap-provider-username-attribute">{{
            t("admin.ldapProviders.usernameAttribute")
          }}</Label
          ><Input
            id="ldap-provider-username-attribute"
            v-model="form.usernameAttribute"
          />
        </div>
        <div class="space-y-2">
          <Label for="ldap-provider-display-attribute">{{
            t("admin.ldapProviders.displayAttribute")
          }}</Label
          ><Input
            id="ldap-provider-display-attribute"
            v-model="form.displayNameAttribute"
          />
        </div>
        <div class="space-y-2">
          <Label for="ldap-provider-email-attribute">{{
            t("admin.ldapProviders.emailAttribute")
          }}</Label
          ><Input
            id="ldap-provider-email-attribute"
            v-model="form.emailAttribute"
          />
        </div>
        <div class="space-y-2 sm:col-span-2">
          <Label for="ldap-provider-ca-pem">{{
            t("admin.ldapProviders.caPem")
          }}</Label
          ><Textarea
            id="ldap-provider-ca-pem"
            v-model="form.caPem"
            rows="4"
            placeholder="-----BEGIN CERTIFICATE-----"
          />
        </div>
        <div
          class="flex items-center justify-between rounded-md border p-3 sm:col-span-2"
        >
          <Label for="ldap-provider-enabled">{{
            t("admin.ldapProviders.enabled")
          }}</Label
          ><Switch id="ldap-provider-enabled" v-model="form.enabled" />
        </div>
      </div>
      <DialogFooter
        ><Button variant="outline" @click="showDialog = false">{{
          t("admin.ldapProviders.cancel")
        }}</Button
        ><Button :disabled="isSaving" @click="save"
          ><LoaderCircle v-if="isSaving" class="h-4 w-4 animate-spin" />{{
            t("admin.ldapProviders.save")
          }}</Button
        ></DialogFooter
      >
    </DialogContent>
  </Dialog>

  <Dialog
    :open="showTestCredentialsDialog"
    @update:open="setTestCredentialsDialogOpen"
  >
    <DialogContent class="sm:max-w-[460px]">
      <DialogHeader>
        <DialogTitle>{{
          t("admin.ldapProviders.testCredentialsTitle")
        }}</DialogTitle>
        <DialogDescription>
          {{ t("admin.ldapProviders.testCredentialsDescription") }}
        </DialogDescription>
      </DialogHeader>
      <form class="space-y-4" @submit.prevent="runDirectProviderTest">
        <div class="space-y-2">
          <Label for="ldap-test-username">{{
            t("admin.ldapProviders.testUsername")
          }}</Label>
          <Input
            id="ldap-test-username"
            v-model="testUsername"
            autocomplete="username"
          />
        </div>
        <div class="space-y-2">
          <Label for="ldap-test-password">{{
            t("admin.ldapProviders.testPassword")
          }}</Label>
          <Input
            id="ldap-test-password"
            v-model="testPassword"
            type="password"
            autocomplete="current-password"
          />
        </div>
        <DialogFooter>
          <Button
            type="button"
            variant="outline"
            @click="setTestCredentialsDialogOpen(false)"
          >
            {{ t("admin.ldapProviders.cancel") }}
          </Button>
          <Button type="submit">
            <TestTube2 class="h-4 w-4" />
            {{ t("admin.ldapProviders.test") }}
          </Button>
        </DialogFooter>
      </form>
    </DialogContent>
  </Dialog>
</template>
