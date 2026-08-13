<script setup lang="ts">
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import FloatingActionDock from "@admin-shared/components/common/FloatingActionDock.vue";
import type { GatewayHostToggleSettingsModel } from "./useGatewayHostToggleSettings";

defineProps<{
  model: GatewayHostToggleSettingsModel;
  toggleColumnLabelKey: string;
}>();
</script>

<template>
  <Alert v-if="!model.isAvailable" class="border-zinc-200 bg-zinc-50">
    <AlertTitle>{{ model.message("unavailable") }}</AlertTitle>
    <AlertDescription class="text-sm leading-6 text-zinc-700">
      {{ model.details?.availability.reason }}
    </AlertDescription>
  </Alert>

  <div class="overflow-hidden rounded-xl border border-border/60">
    <section class="space-y-4 p-5">
      <div
        v-if="model.formItems.length === 0"
        class="rounded-xl bg-muted/20 px-4 py-4 text-sm leading-6 text-muted-foreground"
      >
        {{ model.message("emptyMapping") }}
      </div>
      <div v-else class="rounded-xl bg-muted/10">
        <Table>
          <TableHeader>
            <TableRow class="hover:bg-transparent">
              <TableHead class="px-4 py-3">
                {{ model.message("subdomain") }}
              </TableHead>
              <TableHead class="w-32 px-4 py-3 text-center">
                {{ model.message(toggleColumnLabelKey) }}
              </TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            <TableRow
              v-for="item in model.formItems"
              :key="item.host"
              class="hover:bg-muted/20"
            >
              <TableCell class="px-4 py-4 align-top">
                <div class="min-w-0 space-y-1.5">
                  <div class="flex flex-wrap items-center gap-2">
                    <div class="break-all font-medium">
                      {{ model.formatHostWithAccessEntryPort(item.host) }}
                    </div>
                    <Badge
                      v-if="item.title"
                      variant="secondary"
                      class="max-w-full"
                    >
                      {{ item.title }}
                    </Badge>
                  </div>
                  <div class="break-all text-xs text-muted-foreground">
                    {{ item.target }}
                  </div>
                </div>
              </TableCell>
              <TableCell class="px-4 py-4 text-center">
                <div class="flex justify-center">
                  <Switch
                    :model-value="model.getToggleValue(item)"
                    :aria-label="item.host"
                    :disabled="model.isSaving || !model.isAvailable"
                    @update:model-value="
                      model.updateHostToggle(item.host, $event === true)
                    "
                  />
                </div>
              </TableCell>
            </TableRow>
          </TableBody>
        </Table>
      </div>
    </section>

    <FloatingActionDock
      :active="model.isDirty"
      inline-class="space-y-4 border-t border-border/60 p-5"
    >
      <template #inline>
        <div
          class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between"
        >
          <p class="text-sm leading-6 text-muted-foreground">
            {{ model.saveBlockedReason || model.message("saveHint") }}
          </p>
          <div class="flex flex-wrap items-center justify-end gap-3">
            <Button
              variant="outline"
              :disabled="!model.isDirty || model.isSaving"
              @click="model.resetForm"
            >
              {{ model.message("reset") }}
            </Button>
            <Button
              :disabled="
                !model.isDirty ||
                model.isSaving ||
                Boolean(model.saveBlockedReason)
              "
              @click="model.saveSettings"
            >
              <span
                v-if="model.isSaving"
                class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
              />
              {{
                model.isSaving
                  ? model.message("saving")
                  : model.message("saveAndSync")
              }}
            </Button>
          </div>
        </div>
      </template>
      <template #floating>
        <Button
          variant="outline"
          :disabled="!model.isDirty || model.isSaving"
          @click="model.resetForm"
        >
          {{ model.message("reset") }}
        </Button>
        <Button
          :disabled="
            !model.isDirty ||
            model.isSaving ||
            Boolean(model.saveBlockedReason)
          "
          @click="model.saveSettings"
        >
          <span
            v-if="model.isSaving"
            class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
          />
          {{
            model.isSaving
              ? model.message("saving")
              : model.message("saveAndSync")
          }}
        </Button>
      </template>
    </FloatingActionDock>
  </div>
</template>
