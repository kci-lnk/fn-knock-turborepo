<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { Plus } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import type { LanCertificateDeployment } from "@/types";

defineProps<{
  settings: LanCertificateDeployment | null;
  addressDraft: string;
  saving: boolean;
}>();

const emit = defineEmits<{
  "update:addressDraft": [value: string];
  addAddress: [address: string];
  save: [enabled: boolean];
}>();

const { t } = useI18n();

function listenerLabel(scope?: string) {
  if (scope === "all") return t("admin.certConfig.externalLanListenerAll");
  if (scope === "loopback") {
    return t("admin.certConfig.externalLanListenerLoopback");
  }
  return scope || "—";
}
</script>

<template>
  <div class="grid min-w-0 gap-3">
    <p class="text-xs leading-5 text-muted-foreground">
      {{ t("admin.certConfig.externalLanSecurityDescription") }}
    </p>

    <div class="grid min-w-0 gap-1.5">
      <Label for="external-lan-addresses" class="text-xs">
        {{ t("admin.certConfig.externalLanAddressesLabel") }}
      </Label>
      <Textarea
        id="external-lan-addresses"
        :model-value="addressDraft"
        class="min-h-16 resize-y text-sm"
        rows="2"
        placeholder="192.168.31.98"
        :disabled="saving"
        @update:model-value="emit('update:addressDraft', String($event))"
      />
      <p class="text-xs leading-5 text-muted-foreground">
        {{ t("admin.certConfig.externalLanAddressesHelp") }}
      </p>
    </div>

    <div class="grid min-w-0 gap-2">
      <span class="text-xs font-medium text-muted-foreground">
        {{ t("admin.certConfig.externalLanDetectedLabel") }}
      </span>
      <div class="flex min-w-0 flex-wrap gap-1.5">
        <Button
          v-for="address in settings?.detected_addresses ?? []"
          :key="address"
          type="button"
          size="sm"
          variant="outline"
          class="h-7 min-w-0 max-w-full px-2 text-xs"
          :disabled="saving"
          @click="emit('addAddress', address)"
        >
          <Plus class="mr-1 size-3 shrink-0" />
          <span class="truncate">{{ address }}</span>
        </Button>
        <span
          v-if="!settings?.detected_addresses.length"
          class="text-xs leading-7 text-muted-foreground"
        >
          {{ t("admin.certConfig.externalLanNoneDetected") }}
        </span>
      </div>
    </div>

    <div
      class="flex min-w-0 flex-col gap-3 border-t pt-3 sm:flex-row sm:items-center sm:justify-between"
    >
      <div
        class="flex min-w-0 items-center gap-2 text-xs text-muted-foreground"
      >
        <span>{{ t("admin.certConfig.externalLanListener") }}</span>
        <span
          class="min-w-0 break-words rounded bg-muted px-1.5 py-0.5 font-medium text-foreground"
        >
          {{ listenerLabel(settings?.listener_scope) }}
        </span>
      </div>
      <div class="flex flex-col-reverse gap-2 sm:flex-row">
        <Button
          v-if="settings?.enabled"
          size="sm"
          variant="outline"
          class="w-full sm:w-auto"
          :disabled="saving"
          @click="emit('save', false)"
        >
          {{ t("admin.certConfig.externalLanDisable") }}
        </Button>
        <Button
          size="sm"
          class="w-full sm:w-auto"
          :disabled="saving || !addressDraft.trim()"
          @click="emit('save', true)"
        >
          {{
            settings?.enabled
              ? t("admin.certConfig.externalLanSaveAddresses")
              : t("admin.certConfig.externalLanEnable")
          }}
        </Button>
      </div>
    </div>
  </div>
</template>
