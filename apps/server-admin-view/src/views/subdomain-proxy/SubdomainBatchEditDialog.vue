<script setup lang="ts">
import { nextTick, reactive, ref } from "vue";
import { useI18n } from "vue-i18n";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import SubdomainActionConfirmDialog from "./SubdomainActionConfirmDialog.vue";
import { isProxyHostMapping } from "./model";
import type { SubdomainBatchEditController } from "./useSubdomainBatchEdit";

const props = defineProps<{ controller: SubdomainBatchEditController }>();
const editor = reactive(props.controller);
const { t } = useI18n();
const fields = ref<HTMLElement | null>(null);
const save = async () => {
  const saving = editor.save();
  await nextTick();
  fields.value
    ?.querySelector<HTMLInputElement>('[aria-invalid="true"]')
    ?.focus();
  await saving;
};
</script>

<template>
  <Dialog
    :open="editor.open"
    @update:open="(open) => !open && editor.requestClose()"
  >
    <DialogContent
      class="flex max-h-[90dvh] flex-col overflow-hidden sm:max-w-5xl"
      :show-close-button="!editor.saving"
    >
      <DialogHeader class="shrink-0 pr-6 text-left">
        <DialogTitle>{{
          t("admin.subdomainProxy.batchEdit.title", {
            count: editor.rows.length,
          })
        }}</DialogTitle>
        <DialogDescription>{{
          t("admin.subdomainProxy.batchEdit.description")
        }}</DialogDescription>
      </DialogHeader>
      <div
        ref="fields"
        class="min-h-0 flex-1 space-y-4 overflow-y-auto overscroll-contain p-1"
      >
        <fieldset
          v-for="(row, index) in editor.rows"
          :key="row.originalHost"
          :disabled="editor.saving || editor.mappingsSaved"
          class="min-w-0 rounded-md border p-3"
        >
          <legend class="max-w-full break-all px-1 text-sm font-medium">
            {{ row.originalHost }}
          </legend>
          <div class="grid gap-4 md:grid-cols-3">
            <div class="min-w-0 space-y-2">
              <Label :for="`batch-title-${index}`">{{
                t("admin.subdomainProxy.displayTitle")
              }}</Label>
              <Input
                :id="`batch-title-${index}`"
                v-model="row.title"
                :placeholder="
                  row.original.title ||
                  t('admin.subdomainProxy.batchEdit.autoTitle')
                "
              />
              <p class="break-words text-xs text-muted-foreground">
                {{ t("admin.subdomainProxy.batchEdit.titleHint") }}
                <span v-if="row.original.title" class="block">
                  {{
                    t("admin.subdomainProxy.fetchedTitle", {
                      title: row.original.title,
                    })
                  }}
                </span>
              </p>
            </div>
            <div class="min-w-0 space-y-2">
              <Label :for="`batch-host-${index}`">{{
                t("admin.subdomainProxy.fullHost")
              }}</Label>
              <Input
                :id="`batch-host-${index}`"
                v-model="row.host"
                autocapitalize="none"
                :spellcheck="false"
                :aria-invalid="editor.attempted && !!editor.errors[index]?.host"
                :aria-describedby="
                  editor.attempted && editor.errors[index]?.host
                    ? `batch-host-error-${index}`
                    : undefined
                "
              />
              <p
                v-if="editor.attempted && editor.errors[index]?.host"
                :id="`batch-host-error-${index}`"
                class="text-sm text-destructive"
              >
                {{ editor.errors[index]?.host }}
              </p>
            </div>
            <div class="min-w-0 space-y-2">
              <Label :for="`batch-target-${index}`">{{
                t(
                  isProxyHostMapping(row.original)
                    ? "admin.subdomainProxy.batchEdit.target"
                    : "admin.subdomainProxy.batchEdit.path",
                )
              }}</Label>
              <Input
                :id="`batch-target-${index}`"
                v-model="row.target"
                autocapitalize="none"
                :spellcheck="false"
                :aria-invalid="
                  editor.attempted && !!editor.errors[index]?.target
                "
                :aria-describedby="
                  editor.attempted && editor.errors[index]?.target
                    ? `batch-target-error-${index}`
                    : undefined
                "
              />
              <p
                v-if="editor.attempted && editor.errors[index]?.target"
                :id="`batch-target-error-${index}`"
                class="text-sm text-destructive"
              >
                {{ editor.errors[index]?.target }}
              </p>
            </div>
          </div>
        </fieldset>
      </div>
      <p
        v-if="editor.error"
        role="alert"
        class="max-h-28 shrink-0 overflow-y-auto break-words text-sm text-destructive"
      >
        {{ editor.error }}
      </p>
      <DialogFooter class="shrink-0 border-t pt-4">
        <Button
          variant="outline"
          :disabled="editor.saving"
          @click="editor.requestClose()"
          >{{ t("admin.subdomainProxy.cancel") }}</Button
        >
        <Button
          :disabled="editor.saving || (!editor.dirty && !editor.mappingsSaved)"
          @click="save"
        >
          {{
            t(
              editor.saving
                ? "admin.subdomainProxy.batchEdit.saving"
                : editor.mappingsSaved
                  ? "admin.subdomainProxy.batchEdit.retry"
                  : "admin.subdomainProxy.batchEdit.save",
            )
          }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
  <SubdomainActionConfirmDialog
    :open="editor.discardOpen"
    :loading="editor.saving"
    :title="t('admin.subdomainProxy.batchEdit.discardTitle')"
    :description="
      t(
        editor.mappingsSaved
          ? 'admin.subdomainProxy.batchEdit.partialDiscard'
          : 'admin.subdomainProxy.batchEdit.discardDescription',
      )
    "
    :cancel-label="t('admin.subdomainProxy.batchEdit.continueEditing')"
    :confirm-label="t('admin.subdomainProxy.batchEdit.discard')"
    confirm-variant="destructive"
    @cancel="editor.discardOpen = false"
    @update:open="editor.discardOpen = $event"
    @confirm="editor.discard()"
  />
</template>
