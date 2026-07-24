<script setup lang="ts">
import { computed, useId } from "vue";
import { useI18n } from "vue-i18n";
import { Eye, EyeOff } from "lucide-vue-next";
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

const a11yId = useId();

const props = defineProps<{
  description: string;
  isPasswordVisible: boolean;
  isSaving: boolean;
  isSetupMode: boolean;
  open: boolean;
  password: string;
  passwordSecurityWarning: (value: string) => string;
  title: string;
  username: string;
  usernameSecurityWarning: (value: string) => string;
}>();

const emit = defineEmits<{
  close: [];
  save: [];
  "update:isPasswordVisible": [value: boolean];
  "update:open": [value: boolean];
  "update:password": [value: string];
  "update:username": [value: string];
}>();

const { t } = useI18n();
const usernameWarning = computed(() =>
  props.usernameSecurityWarning(props.username),
);
const passwordWarning = computed(() =>
  props.passwordSecurityWarning(props.password),
);

const handleOpenChange = (open: boolean) => {
  emit("update:open", open);
  if (!open) emit("close");
};
</script>

<template>
  <Dialog :open="open" @update:open="handleOpenChange">
    <DialogContent class="max-h-[88vh] overflow-y-auto sm:max-w-[440px]">
      <DialogHeader>
        <DialogTitle>{{ title }}</DialogTitle>
        <DialogDescription>{{ description }}</DialogDescription>
      </DialogHeader>

      <div class="space-y-4">
        <div v-if="isSetupMode" class="space-y-2">
          <Label for="auth-account-setup-username">
            {{ t("admin.authSettings.accountUsername") }}
          </Label>
          <Input
            id="auth-account-setup-username"
            :model-value="username"
            autocomplete="off"
            :disabled="isSaving"
            @update:model-value="emit('update:username', String($event))"
            @keyup.enter="emit('save')"
          />
          <p
            v-if="usernameWarning"
            class="text-xs text-amber-600 dark:text-amber-400"
            role="status"
          >
            {{ usernameWarning }}
          </p>
        </div>

        <div class="space-y-2">
          <Label :for="`${a11yId}-authaccountpassworddialog-1`">{{
            t("admin.authSettings.password")
          }}</Label>
          <div class="relative">
            <Input
              :id="`${a11yId}-authaccountpassworddialog-1`"
              :model-value="password"
              :type="isPasswordVisible ? 'text' : 'password'"
              autocomplete="new-password"
              class="pr-10"
              :disabled="isSaving"
              @update:model-value="emit('update:password', String($event))"
              @keyup.enter="emit('save')"
            />
            <Button
              type="button"
              variant="ghost"
              size="icon-sm"
              class="absolute right-1 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground"
              :disabled="isSaving"
              :title="
                isPasswordVisible
                  ? t('admin.authSettings.hidePassword')
                  : t('admin.authSettings.showPassword')
              "
              :aria-label="
                isPasswordVisible
                  ? t('admin.authSettings.hidePassword')
                  : t('admin.authSettings.showPassword')
              "
              @click="emit('update:isPasswordVisible', !isPasswordVisible)"
            >
              <component
                :is="isPasswordVisible ? EyeOff : Eye"
                class="h-4 w-4"
              />
            </Button>
          </div>
          <p class="text-xs text-muted-foreground">
            {{ t("admin.authSettings.passwordRuleHint") }}
          </p>
          <p
            v-if="passwordWarning"
            class="text-xs text-amber-600 dark:text-amber-400"
            role="status"
          >
            {{ passwordWarning }}
          </p>
        </div>
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
