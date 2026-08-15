<script setup lang="ts">
import { Save, ShieldCheck } from "lucide-vue-next";
import { useI18n } from "vue-i18n";
import { Button } from "@/components/ui/button";
import { CardContent } from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import FloatingActionDock from "@admin-shared/components/common/FloatingActionDock.vue";
import StreamBypassRuleGroups from "./StreamBypassRuleGroups.vue";
import type { StreamBypassPolicyPageModel } from "./useStreamBypassPolicyPage";

defineProps<{ model: StreamBypassPolicyPageModel }>();
const { t } = useI18n();
</script>

<template>
  <CardContent class="space-y-6 px-3 sm:px-6">
    <section class="rounded-xl bg-muted/30 p-4 sm:p-5">
      <div class="flex items-start justify-between gap-4">
        <div>
          <Label for="stream-bypass-enabled" class="text-base">
            {{ t("admin.streamMappings.policyEnabled") }}
          </Label>
          <p class="mt-1 text-sm leading-6 text-muted-foreground">
            {{ t("admin.streamMappings.policyEnabledDescription") }}
          </p>
        </div>
        <Switch
          id="stream-bypass-enabled"
          :model-value="model.form.enabled"
          :disabled="model.saving || !model.authEnabled"
          @update:model-value="model.setEnabled"
        />
      </div>
      <div
        v-if="!model.authEnabled"
        class="mt-4 rounded-lg border border-amber-300/60 bg-amber-50 p-3 text-sm leading-6 text-amber-900 dark:border-amber-800/60 dark:bg-amber-950/30 dark:text-amber-200"
      >
        {{ t("admin.streamMappings.policyAuthDisabledNotice") }}
      </div>
      <div
        v-if="model.form.enabled"
        class="mt-4 flex items-start gap-2 rounded-lg border border-primary/15 bg-primary/5 p-3 text-xs leading-5 text-muted-foreground"
      >
        <ShieldCheck class="mt-0.5 h-4 w-4 shrink-0 text-primary" />
        <span>{{ t("admin.streamMappings.policyValidationNotice") }}</span>
      </div>
    </section>

    <StreamBypassRuleGroups
      v-if="model.form.enabled"
      :form="model.form"
      :saving="model.saving"
      :value-drafts="model.valueDrafts"
    />
  </CardContent>

  <FloatingActionDock
    :active="model.isDirty"
    inline-class="border-t border-border/60 p-5"
  >
    <template #inline>
      <div
        class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between"
      >
        <p class="text-sm text-muted-foreground">
          {{
            !model.form.enabled
              ? t("admin.streamMappings.policyDisabledSaveHint")
              : model.isBroadRule
                ? t("admin.streamMappings.policyBroadRuleWarning")
                : t("admin.streamMappings.policySaveHint")
          }}
        </p>
        <div class="flex gap-3 sm:ml-auto">
          <Button
            variant="outline"
            :disabled="model.saving"
            @click="model.cancel"
          >
            {{ t("common.cancel") }}
          </Button>
          <Button
            :disabled="!model.isDirty || model.saving"
            @click="model.save"
          >
            <Save class="mr-2 h-4 w-4" />
            {{
              model.saving
                ? t("admin.streamMappings.savingPolicy")
                : t("common.save")
            }}
          </Button>
        </div>
      </div>
    </template>
    <template #floating>
      <Button variant="outline" :disabled="model.saving" @click="model.cancel">
        {{ t("common.cancel") }}
      </Button>
      <Button :disabled="!model.isDirty || model.saving" @click="model.save">
        <Save class="mr-2 h-4 w-4" />{{ t("common.save") }}
      </Button>
    </template>
  </FloatingActionDock>
</template>
