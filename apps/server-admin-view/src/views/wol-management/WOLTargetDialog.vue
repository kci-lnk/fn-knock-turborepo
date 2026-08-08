<script setup lang="ts">
import { computed, useId } from "vue";
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
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { Textarea } from "@/components/ui/textarea";
import type { WOLRelay, WOLTargetInput } from "@/lib/api";

const props = defineProps<{
  open: boolean;
  mode: "create" | "edit";
  model: WOLTargetInput;
  relays: WOLRelay[];
  saving: boolean;
}>();

const emit = defineEmits<{
  confirm: [];
  "update:open": [value: boolean];
}>();

const { t } = useI18n();
const id = useId();
const localDeliveryValue = "__local__";
const deliveryValue = computed({
  get: () => props.model.relayId ?? localDeliveryValue,
  set: (value: string) => {
    props.model.relayId = value === localDeliveryValue ? null : value;
    if (props.model.relayId) props.model.broadcastAddress = null;
  },
});
const broadcastValue = computed({
  get: () => props.model.broadcastAddress ?? "",
  set: (value: string | number) => {
    const normalized = String(value).trim();
    props.model.broadcastAddress = normalized || null;
  },
});
const ipAddressValue = computed({
  get: () => props.model.ipAddress ?? "",
  set: (value: string | number) => {
    const normalized = String(value).trim();
    props.model.ipAddress = normalized || null;
  },
});
</script>

<template>
  <Dialog :open="open" @update:open="emit('update:open', $event)">
    <DialogContent class="sm:max-w-lg">
      <DialogHeader>
        <DialogTitle>
          {{
            mode === "create"
              ? t("admin.wol.targetDialog.createTitle")
              : t("admin.wol.targetDialog.editTitle")
          }}
        </DialogTitle>
        <DialogDescription>
          {{ t("admin.wol.targetDialog.description") }}
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
            :placeholder="t('admin.wol.targetDialog.namePlaceholder')"
            maxlength="64"
          />
        </div>
        <div class="space-y-2">
          <Label :for="`${id}-mac`">{{ t("admin.wol.mac") }}</Label>
          <Input
            :id="`${id}-mac`"
            v-model="model.mac"
            autocomplete="off"
            autocapitalize="characters"
            spellcheck="false"
            placeholder="AA:BB:CC:DD:EE:FF"
          />
        </div>
        <div class="space-y-2">
          <Label :for="`${id}-note`">{{ t("admin.wol.note") }}</Label>
          <Textarea
            :id="`${id}-note`"
            v-model="model.note"
            rows="2"
            maxlength="256"
            :placeholder="t('admin.wol.targetDialog.notePlaceholder')"
          />
        </div>
        <div class="space-y-2">
          <Label :for="`${id}-ip`">{{ t("admin.wol.ipAddress") }}</Label>
          <Input
            :id="`${id}-ip`"
            v-model="ipAddressValue"
            inputmode="decimal"
            placeholder="192.168.31.20"
          />
          <p class="text-xs text-muted-foreground">
            {{ t("admin.wol.targetDialog.ipAddressHint") }}
          </p>
        </div>
        <div class="space-y-2">
          <Label :for="`${id}-relay`">{{ t("admin.wol.deliveryPath") }}</Label>
          <Select v-model="deliveryValue">
            <SelectTrigger :id="`${id}-relay`">
              <SelectValue
                :placeholder="t('admin.wol.targetDialog.selectRelay')"
              />
            </SelectTrigger>
            <SelectContent>
              <SelectItem :value="localDeliveryValue">
                {{ t("admin.wol.localDelivery") }}
              </SelectItem>
              <SelectItem
                v-for="relay in relays"
                :key="relay.id"
                :value="relay.id"
              >
                {{ relay.name }} · {{ relay.address
                }}<template v-if="relay.port !== 40009"
                  >:{{ relay.port }}</template
                >
              </SelectItem>
            </SelectContent>
          </Select>
        </div>
        <div v-if="!model.relayId" class="space-y-2">
          <Label :for="`${id}-broadcast`">{{
            t("admin.wol.broadcastAddress")
          }}</Label>
          <Input
            :id="`${id}-broadcast`"
            v-model="broadcastValue"
            inputmode="decimal"
            placeholder="192.168.31.255"
          />
          <p class="text-xs text-muted-foreground">
            {{ t("admin.wol.targetDialog.broadcastHint") }}
          </p>
        </div>
        <div
          class="flex items-center justify-between rounded-lg border px-3 py-3"
        >
          <div>
            <Label :for="`${id}-enabled`">{{ t("admin.wol.enabled") }}</Label>
            <p class="mt-0.5 text-xs text-muted-foreground">
              {{ t("admin.wol.targetDialog.enabledHint") }}
            </p>
          </div>
          <Switch :id="`${id}-enabled`" v-model="model.enabled" />
        </div>
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
