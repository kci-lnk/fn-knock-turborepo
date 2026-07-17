<script setup lang="ts">
import { computed } from "vue";
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
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";

const props = defineProps<{
  isSaving: boolean;
  open: boolean;
  username: string;
  usernameSecurityWarning: (value: string) => string;
}>();

const emit = defineEmits<{
  close: [];
  save: [];
  "update:open": [value: boolean];
  "update:username": [value: string];
}>();

const { t } = useI18n();
const warning = computed(() => props.usernameSecurityWarning(props.username));

const handleOpenChange = (open: boolean) => {
  emit("update:open", open);
  if (!open) emit("close");
};
</script>

<template>
  <Dialog :open="open" @update:open="handleOpenChange">
    <DialogContent class="max-h-[88vh] overflow-y-auto sm:max-w-[440px]">
      <DialogHeader>
        <DialogTitle>{{ t("admin.authSettings.editAccount") }}</DialogTitle>
        <DialogDescription>
          {{ t("admin.authSettings.editAccountDescription") }}
        </DialogDescription>
      </DialogHeader>

      <div class="space-y-2">
        <Label for="auth-account-username">
          {{ t("admin.authSettings.accountUsername") }}
        </Label>
        <Input
          id="auth-account-username"
          :model-value="username"
          autocomplete="off"
          :disabled="isSaving"
          @update:model-value="emit('update:username', String($event))"
          @keyup.enter="emit('save')"
        />
        <p
          v-if="warning"
          class="text-xs text-amber-600 dark:text-amber-400"
          role="status"
        >
          {{ warning }}
        </p>
      </div>

      <DialogFooter class="gap-2">
        <Button variant="outline" :disabled="isSaving" @click="emit('close')">
          {{ t("admin.authSettings.cancel") }}
        </Button>
        <Button :disabled="isSaving" @click="emit('save')">
          <span
            v-if="isSaving"
            class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
          />
          {{ t("common.save") }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
