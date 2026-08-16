<template>
  <Dialog :open="open" @update:open="emit('update:open', $event)">
    <DialogContent class="sm:max-w-[520px]">
      <DialogHeader>
        <DialogTitle>{{
          t("admin.streamMappings.selectServiceTitle")
        }}</DialogTitle>
        <DialogDescription>
          {{ t("admin.streamMappings.selectServiceDescription") }}
        </DialogDescription>
      </DialogHeader>
      <div class="space-y-2">
        <Label for="stream-service-select">{{
          t("admin.streamMappings.serviceProfile")
        }}</Label>
        <select
          id="stream-service-select"
          v-model="selected"
          class="h-10 w-full rounded-md border border-input bg-background px-3 text-sm"
          :disabled="loading"
        >
          <option value="" disabled>
            {{ t("admin.streamMappings.selectServicePlaceholder") }}
          </option>
          <option
            v-for="item in compatibleItems"
            :key="item.service_id"
            :value="item.service_id"
          >
            {{ serviceOptionLabel(item) }}
          </option>
        </select>
        <p
          class="rounded-lg border border-primary/15 bg-primary/5 p-3 text-xs leading-5 text-muted-foreground"
        >
          {{
            selectedItem && !selectedItem.strict_capable
              ? t("admin.streamMappings.selectServiceIdentificationOnlyWarning")
              : t("admin.streamMappings.selectServiceWarning")
          }}
        </p>
      </div>
      <DialogFooter>
        <ConfirmDangerPopover
          v-if="canClear"
          :title="t('admin.streamMappings.clearServiceTitle')"
          :description="t('admin.streamMappings.clearServiceDescription')"
          :confirm-text="t('admin.streamMappings.clearServiceConfirm')"
          :loading="loading"
          :disabled="loading"
          :on-confirm="() => emit('clear')"
          content-class="w-80 text-left"
        >
          <template #trigger>
            <Button
              variant="destructive-outline"
              class="sm:mr-auto"
              :disabled="loading"
            >
              {{ t("admin.streamMappings.clearService") }}
            </Button>
          </template>
        </ConfirmDangerPopover>
        <Button
          variant="outline"
          :disabled="loading"
          @click="emit('update:open', false)"
        >
          {{ t("admin.streamMappings.cancel") }}
        </Button>
        <Button
          :disabled="loading || !canConfirm"
          @click="emit('confirm', selected)"
        >
          {{
            loading
              ? t("admin.streamMappings.savingPolicy")
              : selectedItem && !selectedItem.strict_capable
                ? t("admin.streamMappings.confirmServiceIdentificationOnly")
                : t("admin.streamMappings.confirmService")
          }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
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
import type { StreamMapping } from "@/types";
import type {
  StreamServiceCatalog,
  StreamServiceDescriptor,
} from "@/lib/api/config";

const props = defineProps<{
  open: boolean;
  loading: boolean;
  mapping: StreamMapping | null;
  catalog: StreamServiceCatalog | null;
  initialServiceId: string;
}>();
const emit = defineEmits<{
  clear: [];
  confirm: [serviceId: string];
  "update:open": [open: boolean];
}>();
const { t } = useI18n();
const selected = ref("");
const canClear = computed(
  () =>
    props.mapping?.service_profile?.source === "manual" &&
    Boolean(props.initialServiceId),
);
const compatibleItems = computed(() =>
  (props.catalog?.items ?? []).filter(
    (item) =>
      Boolean(props.mapping) &&
      item.transports.includes(props.mapping!.protocol),
  ),
);
const selectedItem = computed(() =>
  compatibleItems.value.find((item) => item.service_id === selected.value),
);
const canConfirm = computed(() =>
  compatibleItems.value.some((item) => item.service_id === selected.value),
);
function serviceOptionLabel(item: StreamServiceDescriptor) {
  const label = `${item.display_name} · ${item.service_family}`;
  return item.strict_capable
    ? label
    : `${label} · ${t("admin.streamMappings.serviceIdentificationOnly")}`;
}
watch(
  () => [props.open, props.mapping, props.initialServiceId] as const,
  () => {
    if (props.open) selected.value = props.initialServiceId;
  },
);
</script>
