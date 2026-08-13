<script setup lang="ts">
import { useId } from "vue";
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
import { Switch } from "@/components/ui/switch";
import { type WOLRelayInput } from "@/lib/api/wol";

defineProps<{
  open: boolean;
  mode: "create" | "edit";
  model: WOLRelayInput;
  saving: boolean;
}>();

const emit = defineEmits<{
  confirm: [];
  "update:open": [value: boolean];
}>();

const { t } = useI18n();
const id = useId();
</script>

<template>
  <Dialog :open="open" @update:open="emit('update:open', $event)">
    <DialogContent class="sm:max-w-lg">
      <DialogHeader>
        <DialogTitle>
          {{
            mode === "create"
              ? t("admin.wol.relayDialog.createTitle")
              : t("admin.wol.relayDialog.editTitle")
          }}
        </DialogTitle>
        <DialogDescription>
          {{ t("admin.wol.relayDialog.description") }}
        </DialogDescription>
      </DialogHeader>
      <form
        class="space-y-4"
        autocomplete="off"
        @submit.prevent="emit('confirm')"
      >
        <div class="space-y-2">
          <Label :for="`${id}-name`">{{ t("admin.wol.name") }}</Label>
          <Input
            :id="`${id}-name`"
            v-model="model.name"
            :placeholder="t('admin.wol.relayDialog.namePlaceholder')"
            maxlength="64"
          />
        </div>
        <div class="space-y-2">
          <Label :for="`${id}-address`">{{
            t("admin.wol.relayDialog.remoteAddress")
          }}</Label>
          <Input
            :id="`${id}-address`"
            v-model="model.address"
            inputmode="decimal"
            spellcheck="false"
            autocomplete="off"
            :placeholder="t('admin.wol.relayDialog.addressPlaceholder')"
          />
          <p class="text-xs text-muted-foreground">
            {{ t("admin.wol.relayDialog.addressHint") }}
          </p>
        </div>

        <details class="rounded-lg border px-3 py-2">
          <summary class="cursor-pointer text-sm text-muted-foreground">
            {{ t("admin.wol.advancedSettings") }}
          </summary>
          <div class="mt-4 space-y-4 border-t pt-4">
            <div class="space-y-2">
              <Label :for="`${id}-port`">{{ t("admin.wol.port") }}</Label>
              <Input
                :id="`${id}-port`"
                v-model.number="model.port"
                type="number"
                min="1"
                max="65535"
                inputmode="numeric"
              />
            </div>
            <div
              class="flex items-center justify-between rounded-lg bg-muted/30 px-3 py-3"
            >
              <div>
                <Label :for="`${id}-enabled`">{{
                  t("admin.wol.enabled")
                }}</Label>
                <p class="mt-0.5 text-xs text-muted-foreground">
                  {{ t("admin.wol.relayDialog.enabledHint") }}
                </p>
              </div>
              <Switch :id="`${id}-enabled`" v-model="model.enabled" />
            </div>
          </div>
        </details>
        <DialogFooter>
          <Button
            type="button"
            variant="outline"
            @click="emit('update:open', false)"
          >
            {{ t("common.cancel") }}
          </Button>
          <Button type="submit" :disabled="saving">
            {{ saving ? t("admin.wol.saving") : t("common.save") }}
          </Button>
        </DialogFooter>
      </form>
    </DialogContent>
  </Dialog>
</template>
