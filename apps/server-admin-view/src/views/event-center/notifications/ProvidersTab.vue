<script setup lang="ts">
import { toRef } from "vue";
import { useI18n } from "vue-i18n";
import { Loader2, Pencil, Plus, Send, Trash2 } from "lucide-vue-next";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import RefreshButton from "@/components/RefreshButton.vue";
import HumanFriendlyTime from "@admin-shared/components/common/HumanFriendlyTime.vue";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import ProviderEditorDialog from "./ProviderEditorDialog.vue";
import { useNotificationProviders } from "./useNotificationProviders";

const props = withDefaults(
  defineProps<{
    active?: boolean;
  }>(),
  { active: false },
);

const { t } = useI18n();
const {
  catalog,
  configuredSensitiveFields,
  deleteProvider,
  deletingId,
  dialogMode,
  dialogOpen,
  editingId,
  generatedProviderName,
  handleTypeChange,
  loadData,
  loading,
  openCreateDialog,
  openEditDialog,
  providerForm,
  providers,
  resolveProviderTypeLabel,
  saveProvider,
  saving,
  selectedDefinition,
  showWxPusherAlert,
  testProvider,
  testProviderDraft,
  testingDraft,
  testingId,
} = useNotificationProviders(toRef(props, "active"));
</script>

<template>
  <div class="space-y-4 p-4 sm:p-6">
    <div class="flex flex-wrap items-center gap-2">
      <div class="text-sm text-muted-foreground">
        {{ t("admin.notifications.providers.intro") }}
      </div>
      <div class="ml-auto flex items-center gap-2">
        <RefreshButton
          :loading="loading"
          :disabled="loading"
          @click="loadData"
        />
        <Button @click="openCreateDialog">
          <Plus class="mr-2 h-4 w-4" />
          {{ t("admin.notifications.providers.addProvider") }}
        </Button>
      </div>
    </div>

    <div class="overflow-hidden rounded-md border bg-background">
      <Table>
        <TableHeader>
          <TableRow>
            <TableHead>{{ t("admin.notifications.providers.name") }}</TableHead>
            <TableHead>{{ t("admin.notifications.providers.type") }}</TableHead>
            <TableHead>
              {{ t("admin.notifications.providers.status") }}
            </TableHead>
            <TableHead>
              {{ t("admin.notifications.providers.updatedAt") }}
            </TableHead>
            <TableHead class="w-[180px] text-right">
              {{ t("admin.notifications.providers.actions") }}
            </TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          <TableRow v-if="loading && providers.length === 0">
            <TableCell colspan="6" class="py-10 text-center">
              <Loader2
                class="mx-auto h-5 w-5 animate-spin text-muted-foreground"
              />
            </TableCell>
          </TableRow>
          <TableRow v-else-if="providers.length === 0">
            <TableCell
              colspan="6"
              class="py-10 text-center text-muted-foreground"
            >
              {{ t("admin.notifications.providers.empty") }}
            </TableCell>
          </TableRow>
          <TableRow v-for="provider in providers" :key="provider.id">
            <TableCell>
              <div class="space-y-1">
                <div class="font-medium">{{ provider.name }}</div>
                <div
                  v-if="provider.last_error"
                  class="line-clamp-2 text-xs text-muted-foreground"
                >
                  {{ t("admin.notifications.providers.lastErrorPrefix")
                  }}{{ provider.last_error }}
                </div>
              </div>
            </TableCell>
            <TableCell>{{ resolveProviderTypeLabel(provider.type) }}</TableCell>
            <TableCell>
              <Badge
                variant="outline"
                :class="
                  provider.enabled
                    ? 'border-emerald-500/25 bg-emerald-500/10 text-emerald-700'
                    : 'border-muted-foreground/20 bg-muted text-muted-foreground'
                "
              >
                {{
                  provider.enabled
                    ? t("admin.notifications.providers.enabled")
                    : t("admin.notifications.providers.disabled")
                }}
              </Badge>
            </TableCell>
            <TableCell class="text-sm text-muted-foreground">
              <HumanFriendlyTime :value="provider.updated_at" />
            </TableCell>
            <TableCell class="text-right">
              <div class="inline-flex items-center gap-1">
                <Button
                  variant="ghost"
                  size="icon"
                  :aria-label="t('common.test')"
                  :disabled="testingId === provider.id"
                  @click="testProvider(provider)"
                >
                  <Send class="h-4 w-4" />
                </Button>
                <Button
                  variant="ghost"
                  size="icon"
                  :aria-label="t('common.edit')"
                  :disabled="editingId === provider.id"
                  @click="openEditDialog(provider)"
                >
                  <Loader2
                    v-if="editingId === provider.id"
                    class="h-4 w-4 animate-spin"
                  />
                  <Pencil v-else class="h-4 w-4" />
                </Button>
                <ConfirmDangerPopover
                  :title="t('admin.notifications.providers.deleteTitle')"
                  :description="
                    t('admin.notifications.providers.deleteDescription')
                  "
                  :loading="deletingId === provider.id"
                  :disabled="deletingId === provider.id"
                  :on-confirm="() => deleteProvider(provider)"
                >
                  <template #trigger>
                    <Button
                      variant="ghost"
                      size="icon"
                      :aria-label="t('common.confirmDelete')"
                      class="text-destructive"
                      :disabled="deletingId === provider.id"
                    >
                      <Trash2 class="h-4 w-4" />
                    </Button>
                  </template>
                </ConfirmDangerPopover>
              </div>
            </TableCell>
          </TableRow>
        </TableBody>
      </Table>
    </div>
  </div>

  <ProviderEditorDialog
    v-model:open="dialogOpen"
    :catalog="catalog"
    :configured-sensitive-fields="configuredSensitiveFields"
    :form="providerForm"
    :generated-provider-name="generatedProviderName"
    :mode="dialogMode"
    :saving="saving"
    :selected-definition="selectedDefinition"
    :show-wx-pusher-alert="showWxPusherAlert"
    :testing-draft="testingDraft"
    @save="saveProvider"
    @test="testProviderDraft"
    @type-change="handleTypeChange"
  />
</template>
