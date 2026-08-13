<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { LoaderCircle, Pencil, Plus, TestTube2, Trash2 } from "lucide-vue-next";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import LDAPProviderEditorDialog from "./LDAPProviderEditorDialog.vue";
import LDAPTestCredentialsDialog from "./LDAPTestCredentialsDialog.vue";
import { useLdapProviderManagement } from "./useLdapProviderManagement";

const { t } = useI18n();
const {
  applyPreset,
  catalog,
  editingId,
  form,
  isLoading,
  isSaving,
  mutatingId,
  openCreate,
  openEdit,
  providers,
  readServers,
  removeProvider,
  runDirectProviderTest,
  save,
  setEditorDialogOpen,
  setTestCredentialsDialogOpen,
  showDialog,
  showTestCredentialsDialog,
  testPassword,
  testProvider,
  testUsername,
} = useLdapProviderManagement();
</script>

<template>
  <Card>
    <CardHeader
      class="gap-4 sm:flex sm:flex-row sm:items-start sm:justify-between"
    >
      <div class="space-y-1.5">
        <CardTitle>{{ t("admin.ldapProviders.title") }}</CardTitle>
        <CardDescription>{{ t("admin.ldapProviders.description") }}</CardDescription>
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
              >
                <Trash2 class="h-4 w-4" />
              </Button>
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

  <LDAPProviderEditorDialog
    :open="showDialog"
    :apply-preset="applyPreset"
    :catalog="catalog"
    :editing="Boolean(editingId)"
    :form="form"
    :is-saving="isSaving"
    :save="save"
    @update:open="setEditorDialogOpen"
  />
  <LDAPTestCredentialsDialog
    v-model:password="testPassword"
    v-model:username="testUsername"
    :open="showTestCredentialsDialog"
    :submit="runDirectProviderTest"
    @update:open="setTestCredentialsDialogOpen"
  />
</template>
