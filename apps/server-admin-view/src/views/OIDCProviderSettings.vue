<template>
  <div class="space-y-4">
    <Breadcrumb>
      <BreadcrumbList>
        <BreadcrumbItem>
          <BreadcrumbLink href="#/auth">{{
            t("admin.oidcProviders.breadcrumbTotp")
          }}</BreadcrumbLink>
        </BreadcrumbItem>
        <BreadcrumbSeparator />
        <BreadcrumbItem>
          <BreadcrumbPage>{{
            t("admin.oidcProviders.breadcrumbExternalLogin")
          }}</BreadcrumbPage>
        </BreadcrumbItem>
      </BreadcrumbList>
    </Breadcrumb>

    <Card>
      <CardHeader
        class="gap-4 sm:flex sm:flex-row sm:items-start sm:justify-between"
      >
        <div class="space-y-1.5">
          <CardTitle>{{ t("admin.oidcProviders.title") }}</CardTitle>
          <CardDescription>
            {{ t("admin.oidcProviders.description") }}
          </CardDescription>
        </div>
        <Button
          class="w-full sm:w-auto"
          :disabled="isLoading"
          @click="openCreateDialog"
        >
          <Plus class="h-4 w-4" />
          {{ t("admin.oidcProviders.addProvider") }}
        </Button>
      </CardHeader>
      <CardContent class="space-y-4">
        <div
          v-if="isLoading"
          class="py-10 text-center text-sm text-muted-foreground"
        >
          {{ t("admin.oidcProviders.loading") }}
        </div>
        <Table v-else class="table-fixed" container-class="overflow-hidden">
          <colgroup>
            <col class="w-[24%] sm:w-[18%]" />
            <col class="hidden sm:table-column sm:w-[12%]" />
            <col class="hidden md:table-column md:w-[10%]" />
            <col />
            <col class="w-[86px] sm:w-[184px] 2xl:w-[350px]" />
          </colgroup>
          <TableHeader>
            <TableRow>
              <TableHead class="whitespace-normal">{{
                t("admin.oidcProviders.columns.name")
              }}</TableHead>
              <TableHead class="hidden whitespace-normal sm:table-cell"
                >{{ t("admin.oidcProviders.columns.type") }}</TableHead
              >
              <TableHead class="hidden whitespace-normal md:table-cell"
                >{{ t("admin.oidcProviders.columns.status") }}</TableHead
              >
              <TableHead class="min-w-0 whitespace-nowrap">
                Callback URL
              </TableHead>
              <TableHead class="text-right">{{
                t("admin.oidcProviders.columns.actions")
              }}</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            <TableRow v-for="provider in providers" :key="provider.id">
              <TableCell class="whitespace-normal font-medium">
                {{ provider.name }}
              </TableCell>
              <TableCell class="hidden whitespace-normal sm:table-cell">
                {{ providerLabel(provider.type) }}
              </TableCell>
              <TableCell class="hidden whitespace-normal md:table-cell">
                <Badge variant="outline">{{ providerStatus(provider) }}</Badge>
              </TableCell>
              <TableCell class="min-w-0 max-w-[48vw] sm:max-w-none">
                <div
                  v-if="provider.callback_url"
                  class="group/callback flex min-w-0 max-w-full items-center gap-2 rounded-md border bg-muted/30 px-2.5 py-2"
                >
                  <span
                    class="block min-w-0 flex-1 truncate font-mono text-xs leading-5 text-muted-foreground"
                  >
                    {{ provider.callback_url }}
                  </span>
                  <Button
                    variant="ghost"
                    size="icon-sm"
                    class="size-7 shrink-0 opacity-100 transition-opacity sm:opacity-0 sm:group-hover/callback:opacity-100 sm:focus-visible:opacity-100"
                    :title="
                      t('admin.oidcProviders.copyCallbackUrl', {
                        provider: provider.name,
                      })
                    "
                    :aria-label="
                      t('admin.oidcProviders.copyCallbackUrl', {
                        provider: provider.name,
                      })
                    "
                    @click="copyCallbackUrl(provider.callback_url)"
                  >
                    <Copy class="h-4 w-4" />
                  </Button>
                </div>
                <span v-else class="text-muted-foreground">-</span>
              </TableCell>
              <TableCell class="text-right">
                <div
                  class="inline-flex flex-nowrap items-center justify-end gap-1.5 2xl:gap-2"
                >
                  <Button
                    variant="outline"
                    size="sm"
                    class="gap-1.5 px-2 2xl:px-2.5"
                    :disabled="isMutating"
                    :title="t('admin.oidcProviders.editProvider')"
                    :aria-label="t('admin.oidcProviders.editProvider')"
                    @click="openEditDialog(provider)"
                  >
                    <Pencil class="h-4 w-4" />
                    <span class="hidden 2xl:inline">{{
                      t("admin.oidcProviders.edit")
                    }}</span>
                  </Button>
                  <ConfirmDangerPopover
                    :title="t('admin.oidcProviders.deleteProvider')"
                    :description="t('admin.oidcProviders.deleteDescription')"
                    :loading="isMutating"
                    :disabled="isMutating"
                    :on-confirm="() => deleteProvider(provider.id)"
                  >
                    <template #trigger>
                      <Button
                        variant="destructive"
                        size="sm"
                        class="gap-1.5 px-2 2xl:px-2.5"
                        :disabled="isMutating"
                        :title="t('admin.oidcProviders.deleteProvider')"
                        :aria-label="t('admin.oidcProviders.deleteProvider')"
                      >
                        <Trash2 class="h-4 w-4" />
                        <span class="hidden 2xl:inline">{{
                          t("admin.oidcProviders.delete")
                        }}</span>
                      </Button>
                    </template>
                  </ConfirmDangerPopover>
                </div>
              </TableCell>
            </TableRow>
            <TableEmpty v-if="providers.length === 0" :colspan="5">
              {{ t("admin.oidcProviders.empty") }}
            </TableEmpty>
          </TableBody>
        </Table>
      </CardContent>
    </Card>

    <Dialog :open="showCreateDialog" @update:open="showCreateDialog = $event">
      <DialogContent class="max-h-[88vh] overflow-y-auto sm:max-w-[640px]">
        <DialogHeader>
          <DialogTitle>{{ t("admin.oidcProviders.createTitle") }}</DialogTitle>
          <DialogDescription>
            {{ t("admin.oidcProviders.createDescription") }}
          </DialogDescription>
        </DialogHeader>
        <div class="overflow-hidden rounded-lg border divide-y divide-border">
          <div class="space-y-2 p-4 transition-colors hover:bg-muted/10 sm:p-5">
            <Label for="oidc-provider-type">{{
              t("admin.oidcProviders.provider")
            }}</Label>
            <Select
              :model-value="form.type"
              @update:model-value="handleCreateProviderTypeChange"
            >
              <SelectTrigger id="oidc-provider-type" class="w-full">
                <SelectValue
                  :placeholder="t('admin.oidcProviders.selectProvider')"
                />
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
          </div>
          <div class="space-y-2 p-4 transition-colors hover:bg-muted/10 sm:p-5">
            <Label for="oidc-provider-name">{{
              t("admin.oidcProviders.displayName")
            }}</Label>
            <Input
              id="oidc-provider-name"
              v-model="form.name"
              :placeholder="t('admin.oidcProviders.displayNamePlaceholder')"
            />
          </div>
          <div
            v-if="form.type === 'microsoft'"
            class="space-y-2 p-4 transition-colors hover:bg-muted/10 sm:p-5"
          >
            <Label for="oidc-provider-tenant">Tenant</Label>
            <Input
              id="oidc-provider-tenant"
              v-model="form.tenant"
              placeholder="common / organizations / tenant id"
            />
          </div>
          <div class="space-y-2 p-4 transition-colors hover:bg-muted/10 sm:p-5">
            <Label for="oidc-provider-client-id">Client ID</Label>
            <Input
              id="oidc-provider-client-id"
              v-model="form.clientId"
              autocomplete="off"
            />
          </div>
          <div class="space-y-2 p-4 transition-colors hover:bg-muted/10 sm:p-5">
            <Label for="oidc-provider-client-secret">Client Secret</Label>
            <Input
              id="oidc-provider-client-secret"
              v-model="form.clientSecret"
              type="password"
              autocomplete="new-password"
            />
          </div>
          <div
            v-if="form.type === 'custom_oidc'"
            class="space-y-2 p-4 transition-colors hover:bg-muted/10 sm:p-5"
          >
            <Label for="oidc-provider-issuer">Issuer</Label>
            <Input
              id="oidc-provider-issuer"
              v-model="form.issuer"
              placeholder="https://idp.example.com"
            />
          </div>
          <div class="space-y-2 p-4 transition-colors hover:bg-muted/10 sm:p-5">
            <Label for="oidc-provider-scopes">Scopes</Label>
            <Input
              id="oidc-provider-scopes"
              v-model="form.scopes"
              placeholder="openid profile email"
            />
          </div>
        </div>
        <DialogFooter class="gap-2">
          <Button
            variant="outline"
            :disabled="isSaving"
            @click="showCreateDialog = false"
          >
            {{ t("admin.oidcProviders.cancel") }}
          </Button>
          <Button :disabled="isSaving" @click="handleCreateProvider">
            <LoaderCircle v-if="isSaving" class="h-4 w-4 animate-spin" />
            <Plus v-else class="h-4 w-4" />
            {{
              isSaving
                ? t("admin.oidcProviders.adding")
                : t("admin.oidcProviders.addProvider")
            }}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>

    <Dialog :open="showEditDialog" @update:open="showEditDialog = $event">
      <DialogContent class="max-h-[88vh] overflow-y-auto sm:max-w-[640px]">
        <DialogHeader>
          <DialogTitle>{{ t("admin.oidcProviders.editTitle") }}</DialogTitle>
          <DialogDescription>
            {{ t("admin.oidcProviders.editDescription") }}
          </DialogDescription>
        </DialogHeader>
        <div class="overflow-hidden rounded-lg border divide-y divide-border">
          <div class="space-y-2 p-4 transition-colors hover:bg-muted/10 sm:p-5">
            <Label for="oidc-edit-provider-type">{{
              t("admin.oidcProviders.columns.type")
            }}</Label>
            <Input
              id="oidc-edit-provider-type"
              :model-value="providerLabel(editForm.type)"
              disabled
            />
          </div>
          <div class="space-y-2 p-4 transition-colors hover:bg-muted/10 sm:p-5">
            <Label for="oidc-edit-provider-name">{{
              t("admin.oidcProviders.displayName")
            }}</Label>
            <Input id="oidc-edit-provider-name" v-model="editForm.name" />
          </div>
          <div
            v-if="editForm.type === 'microsoft'"
            class="space-y-2 p-4 transition-colors hover:bg-muted/10 sm:p-5"
          >
            <Label for="oidc-edit-provider-tenant">Tenant</Label>
            <Input
              id="oidc-edit-provider-tenant"
              v-model="editForm.tenant"
              placeholder="common / organizations / tenant id"
            />
          </div>
          <div class="space-y-2 p-4 transition-colors hover:bg-muted/10 sm:p-5">
            <Label for="oidc-edit-provider-client-id">Client ID</Label>
            <Input
              id="oidc-edit-provider-client-id"
              v-model="editForm.clientId"
              autocomplete="off"
            />
          </div>
          <div class="space-y-2 p-4 transition-colors hover:bg-muted/10 sm:p-5">
            <Label for="oidc-edit-provider-client-secret">
              Client Secret
            </Label>
            <Input
              id="oidc-edit-provider-client-secret"
              v-model="editForm.clientSecret"
              type="password"
              autocomplete="new-password"
              :placeholder="t('admin.oidcProviders.keepSecretPlaceholder')"
            />
          </div>
          <div
            v-if="editForm.type === 'custom_oidc'"
            class="space-y-2 p-4 transition-colors hover:bg-muted/10 sm:p-5"
          >
            <Label for="oidc-edit-provider-issuer">Issuer</Label>
            <Input
              id="oidc-edit-provider-issuer"
              v-model="editForm.issuer"
              placeholder="https://idp.example.com"
            />
          </div>
          <div class="space-y-2 p-4 transition-colors hover:bg-muted/10 sm:p-5">
            <Label for="oidc-edit-provider-scopes">Scopes</Label>
            <Input
              id="oidc-edit-provider-scopes"
              v-model="editForm.scopes"
              placeholder="openid profile email"
            />
          </div>
          <div
            class="flex items-center justify-between gap-3 p-4 transition-colors hover:bg-muted/10 sm:p-5"
          >
            <Label class="text-sm font-medium">{{
              t("admin.oidcProviders.enabledStatus")
            }}</Label>
            <div class="flex items-center gap-3">
              <Switch v-model="editForm.enabled" />
              <span class="text-sm text-muted-foreground">
                {{
                  editForm.enabled
                    ? t("admin.oidcProviders.enabled")
                    : t("admin.oidcProviders.disabled")
                }}
              </span>
            </div>
          </div>
        </div>
        <DialogFooter class="gap-2">
          <Button variant="outline" @click="showEditDialog = false">
            {{ t("admin.oidcProviders.cancel") }}
          </Button>
          <Button :disabled="isMutating" @click="saveProviderEdit">
            <LoaderCircle v-if="isMutating" class="h-4 w-4 animate-spin" />
            {{ t("admin.oidcProviders.saveProvider") }}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, reactive, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import {
  Breadcrumb,
  BreadcrumbItem,
  BreadcrumbLink,
  BreadcrumbList,
  BreadcrumbPage,
  BreadcrumbSeparator,
} from "@/components/ui/breadcrumb";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  Table,
  TableBody,
  TableCell,
  TableEmpty,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import {
  Copy,
  LoaderCircle,
  Pencil,
  Plus,
  Trash2,
} from "lucide-vue-next";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { copyTextToClipboard } from "@admin-shared/utils/copyTextToClipboard";
import { toast } from "@admin-shared/utils/toast";
import { ConfigAPI } from "../lib/api";
import type {
  ExternalAuthProviderType,
  OIDCProviderCatalogItem,
  OIDCProviderView,
} from "../types";

const { t } = useI18n();
const catalog = ref<OIDCProviderCatalogItem[]>([]);
const providers = ref<OIDCProviderView[]>([]);
const form = reactive({
  type: "google" as ExternalAuthProviderType,
  name: "",
  clientId: "",
  clientSecret: "",
  issuer: "",
  tenant: "common",
  scopes: "",
});
const showCreateDialog = ref(false);
const showEditDialog = ref(false);
const editForm = reactive({
  id: "",
  type: "google" as ExternalAuthProviderType,
  name: "",
  enabled: false,
  clientId: "",
  clientSecret: "",
  issuer: "",
  tenant: "common",
  scopes: "",
});

const selectedDefinition = computed(() =>
  catalog.value.find((item) => item.type === form.type),
);

const { isPending: isLoading, run: runLoad } = useAsyncAction({
  onError: (error) => {
    toast.error(extractErrorMessage(error, t("admin.oidcProviders.loadFailed")));
  },
});
const { isPending: isSaving, run: runSave } = useAsyncAction({
  onError: (error) => {
    toast.error(extractErrorMessage(error, t("admin.oidcProviders.saveFailed")));
  },
});
const { isPending: isMutating, run: runMutate } = useAsyncAction({
  onError: (error) => {
    toast.error(
      extractErrorMessage(error, t("admin.oidcProviders.operationFailed")),
    );
  },
});

watch(
  selectedDefinition,
  (definition) => {
    if (!definition) return;
    if (!form.name.trim()) form.name = definition.default_name;
    form.scopes = definition.default_scopes.join(" ");
    if (definition.type === "microsoft" && !form.tenant.trim()) {
      form.tenant = "common";
    }
  },
  { immediate: true },
);

onMounted(loadAll);

function resetCreateForm() {
  const definition =
    catalog.value.find((item) => item.type === form.type) || catalog.value[0];
  form.type = (definition?.type || "google") as ExternalAuthProviderType;
  form.name = definition?.default_name || "";
  form.clientId = "";
  form.clientSecret = "";
  form.issuer = "";
  form.tenant = "common";
  form.scopes = definition?.default_scopes.join(" ") || "";
}

function openCreateDialog() {
  resetCreateForm();
  showCreateDialog.value = true;
}

function handleCreateProviderTypeChange(value: unknown) {
  form.type = String(value ?? "") as ExternalAuthProviderType;
  const definition = catalog.value.find((item) => item.type === form.type);
  form.name = definition?.default_name || "";
  form.scopes = definition?.default_scopes.join(" ") || "";
  form.issuer = "";
  form.tenant = "common";
}

function providerLabel(type: string) {
  return catalog.value.find((item) => item.type === type)?.label || type;
}

function normalizeScopes(value: string) {
  return value
    .split(/[,\s]+/)
    .map((item) => item.trim())
    .filter(Boolean);
}

function connectionValueText(value: unknown) {
  if (Array.isArray(value)) return value.join(" ");
  return typeof value === "string" ? value : "";
}

function hasConnectionValue(value: unknown) {
  if (Array.isArray(value)) return value.length > 0;
  return typeof value === "string" ? value.trim().length > 0 : !!value;
}

function isCreateConfigComplete() {
  const definition = selectedDefinition.value;
  if (!definition) return false;
  const values: Record<string, unknown> = {
    client_id: form.clientId.trim(),
    client_secret: form.clientSecret.trim(),
    issuer:
      form.type === "custom_oidc"
        ? form.issuer.trim()
        : form.type === "microsoft" && form.tenant.trim()
          ? `https://login.microsoftonline.com/${form.tenant.trim()}/v2.0`
          : undefined,
  };
  return definition.required_fields.every((field) =>
    hasConnectionValue(values[field]),
  );
}

function providerHasRequiredConfig(provider: OIDCProviderView) {
  const definition = catalog.value.find((item) => item.type === provider.type);
  if (!definition) return false;
  return definition.required_fields.every((field) =>
    hasConnectionValue(provider.connection_config_masked[field]),
  );
}

function providerStatus(provider: OIDCProviderView) {
  if (!providerHasRequiredConfig(provider))
    return t("admin.oidcProviders.pendingConfig");
  return provider.enabled
    ? t("admin.oidcProviders.enabled")
    : t("admin.oidcProviders.disabled");
}

async function copyCallbackUrl(url: string) {
  try {
    await copyTextToClipboard(url);
    toast.success(t("admin.oidcProviders.callbackCopied"), {
      description: url,
    });
  } catch (error) {
    console.error("copyCallbackUrl:", error);
    toast.error(t("admin.oidcProviders.callbackCopyFailed"), {
      description: t("admin.oidcProviders.copyRestricted"),
    });
  }
}

async function loadAll() {
  await runLoad(async () => {
    const [catalogData, providersData] = await Promise.all([
      ConfigAPI.getOIDCProviderCatalog(),
      ConfigAPI.getOIDCProviders(),
    ]);
    catalog.value = catalogData;
    providers.value = providersData;
    if (!catalog.value.some((item) => item.type === form.type)) {
      resetCreateForm();
    }
  });
}

async function handleCreateProvider() {
  await runSave(async () => {
    const scopes = normalizeScopes(form.scopes);
    const enabled = isCreateConfigComplete();
    await ConfigAPI.createOIDCProvider({
      type: form.type,
      name: form.name.trim(),
      enabled,
      connection_config: {
        client_id: form.clientId.trim(),
        client_secret: form.clientSecret.trim(),
        ...(form.type === "custom_oidc" ? { issuer: form.issuer.trim() } : {}),
        ...(form.type === "microsoft" ? { tenant: form.tenant.trim() } : {}),
        ...(scopes.length ? { scopes } : {}),
      },
    });
    form.clientId = "";
    form.clientSecret = "";
    showCreateDialog.value = false;
    toast.success(
      enabled
        ? t("admin.oidcProviders.providerAdded")
        : t("admin.oidcProviders.providerDraftAdded"),
    );
    await loadAll();
  });
}

function openEditDialog(provider: OIDCProviderView) {
  const config = provider.connection_config_masked || {};
  editForm.id = provider.id;
  editForm.type = provider.type;
  editForm.name = provider.name;
  editForm.enabled = provider.enabled;
  editForm.clientId = connectionValueText(config.client_id);
  editForm.clientSecret = "";
  editForm.issuer = connectionValueText(config.issuer);
  editForm.tenant = connectionValueText(config.tenant) || "common";
  editForm.scopes = connectionValueText(config.scopes);
  showEditDialog.value = true;
}

async function saveProviderEdit() {
  if (!editForm.id) return;
  await runMutate(async () => {
    const scopes = normalizeScopes(editForm.scopes);
    const connectionConfig: Record<string, unknown> = {
      client_id: editForm.clientId.trim(),
      ...(editForm.clientSecret.trim()
        ? { client_secret: editForm.clientSecret.trim() }
        : {}),
      ...(editForm.type === "custom_oidc"
        ? { issuer: editForm.issuer.trim() }
        : {}),
      ...(editForm.type === "microsoft"
        ? { tenant: editForm.tenant.trim() }
        : {}),
      ...(scopes.length ? { scopes } : {}),
    };
    await ConfigAPI.updateOIDCProvider(editForm.id, {
      name: editForm.name.trim(),
      enabled: editForm.enabled,
      connection_config: connectionConfig,
    });
    toast.success(t("admin.oidcProviders.providerSaved"));
    showEditDialog.value = false;
    await loadAll();
  });
}

async function deleteProvider(id: string) {
  await runMutate(async () => {
    await ConfigAPI.deleteOIDCProvider(id);
    toast.success(t("admin.oidcProviders.providerDeleted"));
    await loadAll();
  });
}
</script>
