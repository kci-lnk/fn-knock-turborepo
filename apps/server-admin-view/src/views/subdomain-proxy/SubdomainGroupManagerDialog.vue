<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { GripVertical, Plus, Trash2 } from "lucide-vue-next";
import { VueDraggable } from "vue-draggable-plus";
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
import type { HostMapping, HostMappingGroup } from "@/types";
import {
  createHostMappingGroupId,
  isHostMappingGroupNameLengthValid,
  normalizeHostMappingGroupNameKey,
} from "./host-mapping-groups";

const props = defineProps<{
  groups: HostMappingGroup[];
  mappings: HostMapping[];
  open: boolean;
  saving: boolean;
}>();

const emit = defineEmits<{
  save: [groups: HostMappingGroup[]];
  "update:open": [open: boolean];
}>();

const { t } = useI18n();
const draft = ref<HostMappingGroup[]>([]);
const pendingDeleteId = ref<string | null>(null);

watch(
  () => [props.open, props.groups] as const,
  ([open, groups]) => {
    if (!open) return;
    draft.value = groups.map((group) => ({ ...group }));
    pendingDeleteId.value = null;
  },
  { immediate: true, deep: true },
);

const normalizedNames = computed(() =>
  draft.value.map((group) => normalizeHostMappingGroupNameKey(group.name)),
);
const isValid = computed(() => {
  if (draft.value.length > 32) return false;
  return draft.value.every((group, index) => {
    return (
      isHostMappingGroupNameLengthValid(group.name) &&
      normalizedNames.value.indexOf(
        normalizeHostMappingGroupNameKey(group.name),
      ) === index
    );
  });
});

const mappingCount = (groupId: string) =>
  props.mappings.filter((mapping) => mapping.group_id === groupId).length;

const addGroup = () => {
  if (draft.value.length >= 32) return;
  draft.value.push({
    id: createHostMappingGroupId(),
    name: t("admin.subdomainProxy.newGroupDefault", {
      number: draft.value.length + 1,
    }),
  });
};

const requestDelete = (group: HostMappingGroup) => {
  if (mappingCount(group.id) > 0 && pendingDeleteId.value !== group.id) {
    pendingDeleteId.value = group.id;
    return;
  }
  draft.value = draft.value.filter((item) => item.id !== group.id);
  pendingDeleteId.value = null;
};

const save = () => {
  if (!isValid.value) return;
  emit(
    "save",
    draft.value.map((group) => ({ ...group, name: group.name.trim() })),
  );
};
</script>

<template>
  <Dialog :open="open" @update:open="emit('update:open', $event)">
    <DialogContent class="sm:max-w-[560px]">
      <DialogHeader>
        <DialogTitle>{{ t("admin.subdomainProxy.manageGroups") }}</DialogTitle>
        <DialogDescription>
          {{ t("admin.subdomainProxy.manageGroupsDescription") }}
        </DialogDescription>
      </DialogHeader>

      <div class="space-y-3 py-2">
        <VueDraggable
          v-model="draft"
          handle=".group-drag-handle"
          :animation="180"
          :disabled="saving"
          class="max-h-[50vh] space-y-2 overflow-y-auto pr-1"
        >
          <div
            v-for="group in draft"
            :key="group.id"
            class="rounded-md border bg-background p-3"
          >
            <div class="flex items-center gap-2">
              <button
                type="button"
                class="group-drag-handle inline-flex h-9 w-8 shrink-0 items-center justify-center rounded-md text-muted-foreground hover:bg-muted"
                :aria-label="t('admin.subdomainProxy.dragGroupAria')"
              >
                <GripVertical class="h-4 w-4" />
              </button>
              <Input
                v-model="group.name"
                :aria-label="t('admin.subdomainProxy.groupName')"
              />
              <span class="shrink-0 text-xs text-muted-foreground">
                {{
                  t("admin.subdomainProxy.groupMappingsCount", {
                    count: mappingCount(group.id),
                  })
                }}
              </span>
              <Button
                type="button"
                variant="ghost"
                size="icon"
                :disabled="saving"
                :aria-label="t('admin.subdomainProxy.deleteGroup')"
                @click="requestDelete(group)"
              >
                <Trash2 class="h-4 w-4" />
              </Button>
            </div>
            <div
              v-if="pendingDeleteId === group.id"
              class="mt-3 flex items-center justify-between gap-3 rounded-md bg-destructive/10 px-3 py-2 text-xs text-destructive"
              role="alert"
            >
              <span>{{ t("admin.subdomainProxy.deleteGroupConfirm") }}</span>
              <div class="flex shrink-0 gap-2">
                <Button
                  type="button"
                  size="sm"
                  variant="ghost"
                  @click="pendingDeleteId = null"
                >
                  {{ t("admin.subdomainProxy.cancel") }}
                </Button>
                <Button
                  type="button"
                  size="sm"
                  variant="destructive"
                  @click="requestDelete(group)"
                >
                  {{ t("admin.subdomainProxy.confirmDeleteGroup") }}
                </Button>
              </div>
            </div>
          </div>
        </VueDraggable>

        <div
          v-if="draft.length === 0"
          class="rounded-md border border-dashed px-4 py-8 text-center text-sm text-muted-foreground"
        >
          {{ t("admin.subdomainProxy.noGroups") }}
        </div>

        <p v-if="!isValid" class="text-xs text-destructive">
          {{ t("admin.subdomainProxy.groupValidation") }}
        </p>

        <Button
          type="button"
          variant="outline"
          :disabled="saving || draft.length >= 32"
          @click="addGroup"
        >
          <Plus class="mr-2 h-4 w-4" />
          {{ t("admin.subdomainProxy.createGroup") }}
        </Button>
      </div>

      <DialogFooter>
        <Button
          type="button"
          variant="outline"
          :disabled="saving"
          @click="emit('update:open', false)"
        >
          {{ t("admin.subdomainProxy.cancel") }}
        </Button>
        <Button type="button" :disabled="saving || !isValid" @click="save">
          {{ t("admin.subdomainProxy.saveGroups") }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
