<script setup lang="ts">
import { Plus, Trash2 } from "lucide-vue-next";
import { useI18n } from "vue-i18n";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import type { NotificationRuleEditorController } from "./notification-rule-editor-contract";
import SchemaFieldsEditor from "./SchemaFieldsEditor.vue";

const props = defineProps<{ controller: NotificationRuleEditorController }>();
const {
  addTarget,
  availableProvidersForAdd,
  hasAvailableProvidersForAdd,
  hasProviders,
  removeTarget,
  previewWebhookTarget,
  resolveProviderDefinitionById,
  resolveProviderName,
  resolveProviderTypeLabel,
  ruleForm,
  testWebhookTarget,
} = props.controller;
const { t } = useI18n();
</script>

<template>
  <section class="min-w-0 space-y-4">
    <div
      class="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between"
    >
      <div class="text-sm font-semibold">
        {{ t("admin.notifications.rules.notificationTargets") }}
      </div>
      <DropdownMenu>
        <DropdownMenuTrigger as-child>
          <Button
            variant="outline"
            size="sm"
            class="self-start"
            :disabled="!hasProviders || !hasAvailableProvidersForAdd"
          >
            <Plus class="mr-2 h-4 w-4" />
            {{ t("admin.notifications.rules.addTarget") }}
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end" class="w-64">
          <DropdownMenuItem
            v-for="provider in availableProvidersForAdd"
            :key="provider.id"
            @click="addTarget(provider.id)"
          >
            <div class="flex min-w-0 flex-col">
              <span class="truncate font-medium">{{ provider.name }}</span>
              <span class="text-xs text-muted-foreground">
                {{ resolveProviderTypeLabel(provider.id) }}
              </span>
            </div>
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
    </div>

    <div v-if="!ruleForm.targets.length" class="text-sm text-muted-foreground">
      {{ t("admin.notifications.rules.targetEmpty") }}
    </div>
    <div
      v-else
      class="min-w-0 overflow-hidden rounded-lg border border-border/70 bg-background"
    >
      <div
        class="hidden grid-cols-[minmax(0,1fr)_180px_auto] gap-4 border-b bg-muted/20 px-4 py-3 text-xs font-medium text-muted-foreground sm:grid"
      >
        <div>{{ t("admin.notifications.rules.targetName") }}</div>
        <div>{{ t("admin.notifications.rules.providerType") }}</div>
        <div class="text-right">
          {{ t("admin.notifications.rules.actions") }}
        </div>
      </div>

      <div
        v-for="(target, index) in ruleForm.targets"
        :key="target.id || index"
        class="min-w-0 grid grid-cols-[minmax(0,1fr)_auto] gap-x-3 gap-y-3 border-b border-border/60 px-4 py-4 last:border-b-0 sm:grid-cols-[minmax(0,1fr)_180px_auto] sm:gap-4"
      >
        <div class="min-w-0 pr-2 sm:pr-0">
          <div
            class="mb-1 text-[11px] font-medium tracking-wide text-muted-foreground sm:hidden"
          >
            {{ t("admin.notifications.rules.targetName") }}
          </div>
          <div class="break-words text-sm font-medium sm:truncate">
            {{ resolveProviderName(target.provider_id) }}
          </div>
        </div>
        <div class="col-span-2 min-w-0 sm:col-span-1 sm:pt-0.5">
          <div
            class="mb-1 text-[11px] font-medium tracking-wide text-muted-foreground sm:hidden"
          >
            {{ t("admin.notifications.rules.providerType") }}
          </div>
          <div class="text-sm text-muted-foreground">
            {{ resolveProviderTypeLabel(target.provider_id) }}
          </div>
        </div>
        <div
          class="col-start-2 row-start-1 flex items-start justify-end sm:col-start-auto sm:row-start-auto"
        >
          <Button
            variant="ghost"
            size="icon"
            :aria-label="t('common.confirmDelete')"
            class="text-destructive"
            :disabled="ruleForm.targets.length <= 1"
            @click="removeTarget(index)"
          >
            <Trash2 class="h-4 w-4" />
          </Button>
        </div>
        <div
          v-if="
            resolveProviderDefinitionById(target.provider_id)?.target_schema
              .length
          "
          class="col-span-2 min-w-0 rounded-md border border-dashed bg-muted/10 p-3 sm:col-span-3"
        >
          <div class="mb-3 text-xs font-medium text-muted-foreground">
            {{ t("admin.notifications.rules.targetConfig") }}
          </div>
          <SchemaFieldsEditor
            :fields="
              resolveProviderDefinitionById(target.provider_id)!.target_schema
            "
            :model-value="target.target_config"
            @update:model-value="
              ruleForm.targets[index]!.target_config = $event
            "
            @webhook-body-preview="previewWebhookTarget(index)"
            @webhook-body-test="testWebhookTarget(index)"
          />
        </div>
      </div>
    </div>
  </section>
</template>
