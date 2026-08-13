<script setup lang="ts">
import { useId } from "vue";
import { useI18n } from "vue-i18n";
import { Save } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { CardContent } from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import FloatingActionDock from "@admin-shared/components/common/FloatingActionDock.vue";
import AdvancedAuthDurationSettings from "./AdvancedAuthDurationSettings.vue";
import AdvancedAuthRuleGroups from "./AdvancedAuthRuleGroups.vue";
import type { SubdomainAdvancedAuthPageModel } from "./useSubdomainAdvancedAuthPage";

defineProps<{ model: SubdomainAdvancedAuthPageModel }>();
const { t } = useI18n();
const a11yId = useId();
</script>

<template>
  <CardContent class="space-y-6 px-3 sm:px-6">
    <section class="rounded-xl bg-muted/30 p-4 sm:p-5">
      <div class="flex items-start justify-between gap-4">
        <div>
          <Label :for="`${a11yId}-enabled`" class="text-base">
            {{ t("admin.advancedAuth.enabled") }}
          </Label>
          <p class="mt-1 text-sm leading-6 text-muted-foreground">
            {{ t("admin.advancedAuth.enabledDescription") }}
          </p>
        </div>
        <Switch
          :id="`${a11yId}-enabled`"
          v-model:model-value="model.form.enabled"
          :disabled="model.saving"
        />
      </div>
      <p class="mt-3 text-xs leading-5 text-amber-700 dark:text-amber-300">
        {{ t("admin.advancedAuth.temporaryGrantNotice") }}
      </p>
    </section>

    <div v-if="model.form.enabled" class="space-y-6">
      <AdvancedAuthRuleGroups
        :form="model.form"
        :saving="model.saving"
        :value-drafts="model.valueDrafts"
      />
      <AdvancedAuthDurationSettings
        :form="model.form"
        :saving="model.saving"
      />
    </div>
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
            model.form.enabled && model.isBroadRule
              ? t("admin.advancedAuth.broadRuleWarning")
              : t("admin.advancedAuth.saveHint")
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
              model.saving ? t("admin.advancedAuth.saving") : t("common.save")
            }}
          </Button>
        </div>
      </div>
    </template>
    <template #floating>
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
        <Save class="mr-2 h-4 w-4" />{{ t("common.save") }}
      </Button>
    </template>
  </FloatingActionDock>
</template>
