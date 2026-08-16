<script setup lang="ts">
import { useI18n } from "vue-i18n";
import type { WOLTarget } from "@/lib/api/wol";

defineProps<{
  target: WOLTarget;
  hasRelays: boolean;
  statusLabel: string;
  checkedAtLabel: string;
}>();

const { t } = useI18n();
</script>

<template>
  <div class="grid gap-4 sm:grid-cols-2 sm:gap-6">
    <div>
      <p class="text-xs text-muted-foreground">{{ t("admin.wol.mac") }}</p>
      <p class="mt-1 break-all font-mono text-sm">{{ target.mac }}</p>
    </div>
    <div>
      <p class="text-xs text-muted-foreground">
        {{ t("admin.wol.status.label") }}
      </p>
      <div
        class="mt-1 grid gap-1 text-sm sm:flex sm:flex-wrap sm:items-center sm:gap-x-3"
      >
        <span class="font-medium">{{ statusLabel }}</span>
        <span class="text-xs text-muted-foreground">{{ checkedAtLabel }}</span>
        <span
          v-if="target.status.observedIp || target.ipAddress"
          class="font-mono text-xs"
        >
          {{ target.status.observedIp || target.ipAddress }}
        </span>
      </div>
    </div>
    <div v-if="hasRelays" class="sm:col-span-2">
      <p class="text-xs text-muted-foreground">
        {{ t("admin.wol.deliveryPath") }}
      </p>
      <div class="mt-1 text-sm">
        <template v-if="target.deliveryMode === 'local'">
          <p>{{ t("admin.wol.localDelivery") }}</p>
          <p
            v-if="target.broadcastAddress"
            class="mt-0.5 break-all font-mono text-xs text-muted-foreground"
          >
            {{ target.broadcastAddress }}:9
          </p>
        </template>
        <p v-else-if="target.relay">{{ target.relay.name }}</p>
        <p v-else class="text-destructive">
          {{ t("admin.wol.relayMissing") }}
        </p>
      </div>
    </div>
  </div>
</template>
