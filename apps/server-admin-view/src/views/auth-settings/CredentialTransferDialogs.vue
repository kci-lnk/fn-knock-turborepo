<script setup lang="ts">
import { computed } from "vue";
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
import { Download, Upload } from "lucide-vue-next";

const props = defineProps<{
  credentialCount: number;
  credentialTransferOpen: boolean;
  exportOpen: boolean;
  importOpen: boolean;
  isCredentialTransferBusy: boolean;
  isExportingCredentials: boolean;
  isImportingCredentials: boolean;
  pendingCredentialImportFilename: string;
}>();

const emit = defineEmits<{
  "update:credentialTransferOpen": [value: boolean];
  "update:exportOpen": [value: boolean];
  "update:importOpen": [value: boolean];
  confirmExport: [];
  confirmImport: [];
  exportFromTransfer: [];
  importFromTransfer: [];
  resetImport: [];
}>();

const { t } = useI18n();

const transferDialogOpen = computed({
  get: () => props.credentialTransferOpen,
  set: (value) => emit("update:credentialTransferOpen", value),
});
const exportDialogOpen = computed({
  get: () => props.exportOpen,
  set: (value) => emit("update:exportOpen", value),
});
const importDialogOpen = computed({
  get: () => props.importOpen,
  set: (value) => {
    emit("update:importOpen", value);
    if (!value) emit("resetImport");
  },
});

const closeImportDialog = () => {
  importDialogOpen.value = false;
};
</script>

<template>
  <Dialog v-model:open="transferDialogOpen">
    <DialogContent class="max-h-[88vh] overflow-y-auto sm:max-w-[520px]">
      <DialogHeader>
        <DialogTitle>
          {{ t("admin.authSettings.credentialTransfer") }}
        </DialogTitle>
        <DialogDescription>
          {{ t("admin.authSettings.credentialTransferDescription") }}
        </DialogDescription>
      </DialogHeader>
      <div class="grid gap-3 sm:grid-cols-2">
        <Button
          variant="outline"
          class="h-auto justify-start gap-3 px-4 py-3 text-left"
          :disabled="credentialCount === 0 || isCredentialTransferBusy"
          @click="emit('exportFromTransfer')"
        >
          <Download class="h-4 w-4 shrink-0" />
          <span class="min-w-0 whitespace-normal font-medium">
            {{ t("admin.authSettings.exportCredentials") }}
          </span>
        </Button>
        <Button
          variant="outline"
          class="h-auto justify-start gap-3 px-4 py-3 text-left"
          :disabled="isCredentialTransferBusy"
          @click="emit('importFromTransfer')"
        >
          <Upload class="h-4 w-4 shrink-0" />
          <span class="min-w-0 whitespace-normal font-medium">
            {{ t("admin.authSettings.importCredentials") }}
          </span>
        </Button>
      </div>
      <DialogFooter>
        <Button
          variant="outline"
          :disabled="isCredentialTransferBusy"
          @click="transferDialogOpen = false"
        >
          {{ t("admin.authSettings.cancel") }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>

  <Dialog v-model:open="exportDialogOpen">
    <DialogContent class="max-h-[88vh] overflow-y-auto sm:max-w-[520px]">
      <DialogHeader>
        <DialogTitle>
          {{ t("admin.authSettings.exportCredentialsTitle") }}
        </DialogTitle>
        <DialogDescription>
          {{ t("admin.authSettings.exportCredentialsDescription") }}
        </DialogDescription>
      </DialogHeader>
      <DialogFooter class="gap-2">
        <Button
          variant="outline"
          :disabled="isExportingCredentials"
          @click="exportDialogOpen = false"
        >
          {{ t("admin.authSettings.cancel") }}
        </Button>
        <Button
          :disabled="isExportingCredentials"
          @click="emit('confirmExport')"
        >
          <span
            v-if="isExportingCredentials"
            class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
          ></span>
          {{ t("admin.authSettings.confirmExportCredentials") }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>

  <Dialog v-model:open="importDialogOpen">
    <DialogContent class="max-h-[88vh] overflow-y-auto sm:max-w-[520px]">
      <DialogHeader>
        <DialogTitle>
          {{ t("admin.authSettings.importCredentialsTitle") }}
        </DialogTitle>
        <DialogDescription>
          {{ t("admin.authSettings.importCredentialsDescription") }}
        </DialogDescription>
      </DialogHeader>
      <div class="rounded-md border bg-muted/20 px-3 py-2 text-sm">
        <p class="break-all font-medium">
          {{ pendingCredentialImportFilename }}
        </p>
      </div>
      <DialogFooter class="gap-2">
        <Button
          variant="outline"
          :disabled="isImportingCredentials"
          @click="closeImportDialog"
        >
          {{ t("admin.authSettings.cancel") }}
        </Button>
        <Button
          :disabled="isImportingCredentials"
          @click="emit('confirmImport')"
        >
          <span
            v-if="isImportingCredentials"
            class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
          ></span>
          {{ t("admin.authSettings.confirmImportCredentials") }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
