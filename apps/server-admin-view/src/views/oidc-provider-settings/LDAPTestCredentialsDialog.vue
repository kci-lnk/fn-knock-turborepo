<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { TestTube2 } from "lucide-vue-next";
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

defineProps<{
  open: boolean;
  password: string;
  submit: () => Promise<void> | void;
  username: string;
}>();
const emit = defineEmits<{
  "update:open": [value: boolean];
  "update:password": [value: string];
  "update:username": [value: string];
}>();
const { t } = useI18n();
</script>

<template>
  <Dialog :open="open" @update:open="emit('update:open', $event)">
    <DialogContent class="sm:max-w-[460px]">
      <DialogHeader>
        <DialogTitle>{{ t("admin.ldapProviders.testCredentialsTitle") }}</DialogTitle>
        <DialogDescription>
          {{ t("admin.ldapProviders.testCredentialsDescription") }}
        </DialogDescription>
      </DialogHeader>
      <form class="space-y-4" @submit.prevent="submit">
        <div class="space-y-2">
          <Label for="ldap-test-username">{{ t("admin.ldapProviders.testUsername") }}</Label>
          <Input
            id="ldap-test-username"
            :model-value="username"
            autocomplete="username"
            @update:model-value="emit('update:username', String($event))"
          />
        </div>
        <div class="space-y-2">
          <Label for="ldap-test-password">{{ t("admin.ldapProviders.testPassword") }}</Label>
          <Input
            id="ldap-test-password"
            :model-value="password"
            type="password"
            autocomplete="current-password"
            @update:model-value="emit('update:password', String($event))"
          />
        </div>
        <DialogFooter>
          <Button type="button" variant="outline" @click="emit('update:open', false)">
            {{ t("admin.ldapProviders.cancel") }}
          </Button>
          <Button type="submit">
            <TestTube2 class="h-4 w-4" />
            {{ t("admin.ldapProviders.test") }}
          </Button>
        </DialogFooter>
      </form>
    </DialogContent>
  </Dialog>
</template>
