<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";

defineProps<{
  open: boolean;
  showWol: boolean;
  saving: boolean;
}>();

const emit = defineEmits<{
  "update:open": [value: boolean];
  "update:showWol": [value: boolean];
  save: [];
}>();

const { t } = useI18n();
</script>

<template>
  <Dialog :open="open" @update:open="emit('update:open', $event)">
    <DialogContent class="sm:max-w-md">
      <DialogHeader>
        <DialogTitle>{{ t("admin.wol.portal.title") }}</DialogTitle>
        <DialogDescription>{{
          t("admin.wol.portal.description")
        }}</DialogDescription>
      </DialogHeader>
      <div class="flex items-center justify-between gap-4 rounded-lg border p-4">
        <Label for="wol-portal-shortcut" class="leading-6">
          {{ t("admin.wol.portal.showShortcut") }}
        </Label>
        <Switch
          id="wol-portal-shortcut"
          :model-value="showWol"
          @update:model-value="emit('update:showWol', $event)"
        />
      </div>
      <DialogFooter>
        <Button variant="outline" @click="emit('update:open', false)">
          {{ t("common.cancel") }}
        </Button>
        <Button :disabled="saving" @click="emit('save')">
          {{ saving ? t("admin.wol.saving") : t("common.save") }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
