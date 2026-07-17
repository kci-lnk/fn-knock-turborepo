<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import type { TOTPSubdomainAccessMode } from "../../types";

type SubdomainAccessOption = {
  host: string;
  label: string;
  description: string;
  stale?: boolean;
  builtin?: boolean;
};

defineProps<{
  hasTarget: boolean;
  isSaving: boolean;
  mode: TOTPSubdomainAccessMode;
  open: boolean;
  optionCount: number;
  options: SubdomainAccessOption[];
  search: string;
  selectedCount: number;
  selectedHosts: ReadonlySet<string>;
  targetName: string;
}>();

const emit = defineEmits<{
  "update:mode": [value: TOTPSubdomainAccessMode];
  "update:open": [value: boolean];
  "update:search": [value: string];
  clearSelected: [];
  close: [];
  save: [];
  selectAllFiltered: [];
  toggleHost: [host: string, checked: boolean];
}>();

const { t } = useI18n();

const handleOpenChange = (open: boolean) => {
  if (open) {
    emit("update:open", true);
    return;
  }
  emit("close");
};

const updateSearch = (value: string | number) => {
  emit("update:search", String(value).trim());
};
</script>

<template>
  <Dialog :open="open" @update:open="handleOpenChange">
    <DialogContent class="max-h-[88vh] overflow-y-auto sm:max-w-[640px]">
      <DialogHeader>
        <DialogTitle>
          {{ t("admin.authSettings.permissionDialogTitle") }}
        </DialogTitle>
        <DialogDescription>
          {{
            t("admin.authSettings.permissionDialogDescription", {
              name: targetName,
            })
          }}
        </DialogDescription>
      </DialogHeader>

      <div class="space-y-4">
        <div class="grid grid-cols-2 gap-2">
          <Button
            type="button"
            :variant="mode === 'all' ? 'default' : 'outline'"
            class="h-auto justify-start px-4 py-3 text-left"
            @click="emit('update:mode', 'all')"
          >
            <span class="min-w-0 whitespace-normal">
              {{ t("admin.authSettings.permissionAll") }}
            </span>
          </Button>
          <Button
            type="button"
            :variant="mode === 'custom' ? 'default' : 'outline'"
            class="h-auto justify-start px-4 py-3 text-left"
            @click="emit('update:mode', 'custom')"
          >
            <span class="min-w-0 whitespace-normal">
              {{ t("admin.authSettings.permissionCustom") }}
            </span>
          </Button>
        </div>

        <div v-if="mode === 'custom'" class="space-y-3">
          <Input
            :model-value="search"
            :placeholder="t('admin.authSettings.permissionSearchPlaceholder')"
            @update:model-value="updateSearch"
          />
          <div class="flex flex-wrap items-center justify-between gap-2">
            <p class="text-sm text-muted-foreground">
              {{
                t("admin.authSettings.permissionSelectedCount", {
                  count: selectedCount,
                })
              }}
            </p>
            <div class="flex gap-2">
              <Button
                type="button"
                variant="outline"
                size="sm"
                :disabled="options.length === 0"
                @click="emit('selectAllFiltered')"
              >
                {{ t("admin.authSettings.permissionSelectAll") }}
              </Button>
              <Button
                type="button"
                variant="outline"
                size="sm"
                :disabled="selectedCount === 0"
                @click="emit('clearSelected')"
              >
                {{ t("admin.authSettings.permissionClear") }}
              </Button>
            </div>
          </div>

          <div
            class="max-h-72 overflow-y-auto rounded-md border"
            role="group"
            :aria-label="t('admin.authSettings.permissionCustom')"
          >
            <label
              v-for="option in options"
              :key="option.host"
              class="flex cursor-pointer items-start gap-3 border-b px-3 py-3 last:border-b-0 hover:bg-muted/40"
            >
              <Checkbox
                class="mt-0.5"
                :model-value="selectedHosts.has(option.host)"
                @update:model-value="
                  emit('toggleHost', option.host, $event === true)
                "
              />
              <span class="min-w-0 flex-1">
                <span class="block truncate text-sm font-medium">
                  {{ option.label }}
                </span>
                <span class="block truncate text-xs text-muted-foreground">
                  {{ option.description }}
                </span>
              </span>
              <span
                v-if="option.builtin"
                class="shrink-0 rounded border px-1.5 py-0.5 text-xs text-muted-foreground"
              >
                {{ t("admin.authSettings.permissionBuiltin") }}
              </span>
              <span
                v-else-if="option.stale"
                class="shrink-0 rounded border px-1.5 py-0.5 text-xs text-muted-foreground"
              >
                {{ t("admin.authSettings.permissionStaleHost") }}
              </span>
            </label>
            <div
              v-if="options.length === 0"
              class="px-3 py-8 text-center text-sm text-muted-foreground"
            >
              {{
                optionCount === 0
                  ? t("admin.authSettings.permissionNoHosts")
                  : t("admin.authSettings.permissionNoSearchResults")
              }}
            </div>
          </div>
        </div>
      </div>

      <DialogFooter class="gap-2">
        <Button variant="outline" :disabled="isSaving" @click="emit('close')">
          {{ t("admin.authSettings.cancel") }}
        </Button>
        <Button :disabled="isSaving || !hasTarget" @click="emit('save')">
          <span
            v-if="isSaving"
            class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
          ></span>
          {{ t("common.save") }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
