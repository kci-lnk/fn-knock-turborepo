<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { Copy, Link2, LoaderCircle } from "lucide-vue-next";
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
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import type { OIDCProviderView } from "@/types";

defineProps<{
  expiresAt: string;
  inviteUrl: string;
  isCreating: boolean;
  open: boolean;
  providerId: string;
  providers: OIDCProviderView[];
}>();

const emit = defineEmits<{
  copy: [];
  create: [];
  "provider-change": [value: unknown];
  "update:open": [open: boolean];
}>();

const { t } = useI18n();
</script>

<template>
  <Dialog :open="open" @update:open="emit('update:open', $event)">
    <DialogContent class="max-h-[88vh] overflow-y-auto sm:max-w-[560px]">
      <DialogHeader>
        <DialogTitle>{{ t("admin.passkeySettings.inviteTitle") }}</DialogTitle>
        <DialogDescription>
          {{ t("admin.passkeySettings.inviteDescription") }}
        </DialogDescription>
      </DialogHeader>
      <div class="overflow-hidden rounded-lg border divide-y divide-border">
        <div class="space-y-2 p-4 transition-colors hover:bg-muted/10 sm:p-5">
          <Label for="oidc-invite-provider">
            {{ t("admin.passkeySettings.provider") }}
          </Label>
          <Select
            :model-value="providerId"
            @update:model-value="emit('provider-change', $event)"
          >
            <SelectTrigger id="oidc-invite-provider" class="w-full">
              <SelectValue
                :placeholder="t('admin.passkeySettings.providerPlaceholder')"
              />
            </SelectTrigger>
            <SelectContent>
              <SelectItem
                v-for="provider in providers"
                :key="provider.id"
                :value="provider.id"
              >
                {{ provider.name }}
              </SelectItem>
            </SelectContent>
          </Select>
          <p class="text-[11px] text-muted-foreground">
            {{ t("admin.passkeySettings.inviteExpiresIn") }}
          </p>
        </div>
        <div
          v-if="inviteUrl"
          class="space-y-3 p-4 transition-colors hover:bg-muted/10 sm:p-5"
        >
          <Label>{{ t("admin.passkeySettings.inviteLink") }}</Label>
          <div
            class="flex items-start gap-2 rounded-md border bg-muted/30 px-2.5 py-2"
          >
            <p
              class="min-w-0 flex-1 whitespace-normal break-all font-mono text-xs leading-5 text-muted-foreground"
            >
              {{ inviteUrl }}
            </p>
            <Button
              variant="ghost"
              size="icon-sm"
              class="size-7 shrink-0"
              :title="t('admin.passkeySettings.copyInviteLink')"
              :aria-label="t('admin.passkeySettings.copyInviteLink')"
              @click="emit('copy')"
            >
              <Copy class="h-4 w-4" />
            </Button>
          </div>
          <p class="text-xs text-muted-foreground">
            {{
              t("admin.passkeySettings.expiresAt", {
                time: expiresAt || "-",
              })
            }}
          </p>
        </div>
      </div>
      <DialogFooter class="gap-2">
        <Button variant="outline" @click="emit('update:open', false)">
          {{ t("admin.passkeySettings.close") }}
        </Button>
        <Button v-if="inviteUrl" variant="outline" @click="emit('copy')">
          <Copy class="h-4 w-4" />
          {{ t("admin.passkeySettings.copyLink") }}
        </Button>
        <Button :disabled="isCreating || !providerId" @click="emit('create')">
          <LoaderCircle v-if="isCreating" class="h-4 w-4 animate-spin" />
          <Link2 v-else class="h-4 w-4" />
          {{ t("admin.passkeySettings.generate") }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
