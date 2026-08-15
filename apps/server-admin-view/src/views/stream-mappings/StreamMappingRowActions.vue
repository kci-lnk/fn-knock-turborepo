<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import {
  BadgeCheck,
  ChevronDown,
  Loader2,
  RefreshCw,
  ShieldOff,
  Trash2,
} from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import type { StreamMapping } from "../../types";
import {
  formatMappingLabel,
  formatProtocolLabel,
  getMappingKey,
} from "./streamMappingModel";

const props = defineProps<{
  mapping: StreamMapping;
  probingMappingKey: string | null;
  removingMappingKey: string | null;
  onRemove: (mapping: StreamMapping) => Promise<boolean>;
}>();

const emit = defineEmits<{
  edit: [mapping: StreamMapping];
  probe: [mapping: StreamMapping];
  policy: [mapping: StreamMapping];
  service: [mapping: StreamMapping];
}>();

const { t } = useI18n();
const deleteDialogOpen = ref(false);
const mappingKey = computed(() => getMappingKey(props.mapping));
const isProbing = computed(() => props.probingMappingKey === mappingKey.value);
const isRemoving = computed(
  () => props.removingMappingKey === mappingKey.value,
);

function openDeleteDialog() {
  deleteDialogOpen.value = true;
}

function handleDeleteDialogOpenChange(open: boolean) {
  if (!isRemoving.value) deleteDialogOpen.value = open;
}

async function removeMapping() {
  if (await props.onRemove(props.mapping)) {
    deleteDialogOpen.value = false;
  }
}
</script>

<template>
  <div class="flex justify-end">
    <Button
      variant="outline"
      size="sm"
      class="rounded-r-none"
      @click="emit('edit', mapping)"
    >
      {{ t("admin.streamMappings.edit") }}
    </Button>
    <DropdownMenu>
      <DropdownMenuTrigger as-child>
        <Button
          variant="outline"
          size="icon"
          :aria-label="t('common.moreActions')"
          class="h-8 w-8 rounded-l-none border-l-0"
        >
          <ChevronDown class="h-4 w-4" />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" class="w-44">
        <DropdownMenuItem
          :disabled="isProbing"
          @select="emit('probe', mapping)"
        >
          <RefreshCw
            class="mr-2 h-4 w-4"
            :class="{ 'animate-spin': isProbing }"
          />
          {{
            isProbing
              ? t("admin.streamMappings.probing")
              : t("admin.streamMappings.probe")
          }}
        </DropdownMenuItem>
        <DropdownMenuItem @select="emit('policy', mapping)">
          <ShieldOff class="mr-2 h-4 w-4" />
          {{ t("admin.streamMappings.bypassPolicy") }}
        </DropdownMenuItem>
        <DropdownMenuItem @select="emit('service', mapping)">
          <BadgeCheck class="mr-2 h-4 w-4" />
          {{ t("admin.streamMappings.selectService") }}
        </DropdownMenuItem>
        <DropdownMenuSeparator />
        <DropdownMenuItem
          variant="destructive"
          :disabled="isRemoving"
          @select="openDeleteDialog"
        >
          <Trash2 class="mr-2 h-4 w-4" />
          {{ t("admin.streamMappings.delete") }}
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  </div>

  <Dialog :open="deleteDialogOpen" @update:open="handleDeleteDialogOpenChange">
    <DialogContent class="sm:max-w-[440px]" :show-close-button="!isRemoving">
      <DialogHeader>
        <DialogTitle>
          {{
            t("admin.streamMappings.deleteTitle", {
              protocol: formatProtocolLabel(mapping.protocol),
            })
          }}
        </DialogTitle>
        <DialogDescription>
          {{
            t("admin.streamMappings.deleteDescription", {
              mapping: formatMappingLabel(mapping),
              target: mapping.target,
            })
          }}
        </DialogDescription>
      </DialogHeader>
      <DialogFooter>
        <Button
          variant="outline"
          :disabled="isRemoving"
          @click="deleteDialogOpen = false"
        >
          {{ t("common.cancel") }}
        </Button>
        <Button
          variant="destructive"
          :disabled="isRemoving"
          @click="removeMapping"
        >
          <Loader2 v-if="isRemoving" class="mr-2 h-4 w-4 animate-spin" />
          {{ t("common.confirmDelete") }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
