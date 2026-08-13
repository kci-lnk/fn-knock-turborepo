<script setup lang="ts">
import { Loader2 } from "lucide-vue-next";
import { useI18n } from "vue-i18n";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogFooter,
} from "@/components/ui/dialog";
import type { NotificationRuleEditorController } from "./notification-rule-editor-contract";
import NotificationRuleConditions from "./NotificationRuleConditions.vue";
import NotificationRuleDialogHeader from "./NotificationRuleDialogHeader.vue";
import NotificationRuleEventTypes from "./NotificationRuleEventTypes.vue";
import NotificationRuleTargets from "./NotificationRuleTargets.vue";

const props = defineProps<{ controller: NotificationRuleEditorController }>();
const { dialogOpen, isEditMode, saveRule, saving } = props.controller;
const { t } = useI18n();
</script>

<template>
  <Dialog v-model:open="dialogOpen">
    <DialogContent
      class="flex max-h-[92vh] flex-col gap-0 overflow-hidden p-0 sm:max-w-[1040px]"
    >
      <NotificationRuleDialogHeader :controller="controller" />
      <div
        class="flex-1 space-y-6 overflow-y-auto bg-background px-4 py-5 sm:px-6"
      >
        <NotificationRuleEventTypes
          v-if="!isEditMode"
          :controller="controller"
        />
        <NotificationRuleConditions v-else :controller="controller" />
        <NotificationRuleTargets :controller="controller" />
      </div>
      <div
        class="flex flex-col-reverse gap-2 border-t bg-background px-4 py-4 sm:flex-row sm:items-center sm:justify-between sm:px-6"
      >
        <div class="text-xs text-muted-foreground">
          {{
            isEditMode
              ? t("admin.notifications.rules.saveEditHint")
              : t("admin.notifications.rules.saveCreateHint")
          }}
        </div>
        <DialogFooter class="gap-2 sm:flex-row">
          <Button variant="outline" @click="dialogOpen = false">
            {{ t("common.cancel") }}
          </Button>
          <Button :disabled="saving" @click="saveRule">
            <Loader2 v-if="saving" class="mr-2 h-4 w-4 animate-spin" />
            {{ t("common.save") }}
          </Button>
        </DialogFooter>
      </div>
    </DialogContent>
  </Dialog>
</template>
