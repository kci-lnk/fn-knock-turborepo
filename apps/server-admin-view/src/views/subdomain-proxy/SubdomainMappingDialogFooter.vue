<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { Loader2 } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { DialogFooter } from "@/components/ui/dialog";
import type { SubdomainMappingDialogProps } from "./subdomain-mapping-dialog-contract";

const { dialog } = defineProps<{ dialog: SubdomainMappingDialogProps }>();
const emit = defineEmits<{ close: []; save: [] }>();
const { t } = useI18n();
</script>

<template>
  <DialogFooter
    :class="[
      'grid shrink-0 grid-cols-2 border-t bg-background px-6 py-4 sm:flex sm:justify-end',
      !dialog.isMappingDialogSoftKeyboardVisible &&
        'max-sm:pb-[calc(env(safe-area-inset-bottom)+1rem)]',
    ]"
  >
    <template
      v-if="dialog.visibilityEditor.mappingDialogView === 'path-browser'"
    >
      <Button
        class="w-full sm:w-auto"
        variant="outline"
        @click="dialog.pathBrowserEditor.cancel"
      >
        {{ t("admin.subdomainProxy.staticServe.browser.cancel") }}
      </Button>
      <Button
        class="w-full sm:w-auto"
        :disabled="!dialog.pathBrowserEditor.canConfirm"
        @click="dialog.pathBrowserEditor.confirmSelection"
      >
        <Loader2
          v-if="dialog.pathBrowserEditor.isConfirming"
          class="mr-2 h-4 w-4 animate-spin"
        />
        {{
          t(
            dialog.pathBrowserEditor.targetType === "directory"
              ? "admin.subdomainProxy.staticServe.browser.useCurrentFolder"
              : "admin.subdomainProxy.staticServe.browser.useSelectedFile",
          )
        }}
      </Button>
    </template>
    <template v-else>
      <Button
        class="w-full sm:w-auto"
        variant="outline"
        @click="emit('close')"
      >
        {{ t("admin.subdomainProxy.cancel") }}
      </Button>
      <Button
        class="w-full sm:w-auto"
        :disabled="
          !dialog.isMappingValid ||
          dialog.isSavingMappings ||
          dialog.isGatewayAdvancedLoading ||
          dialog.iconEditor.isIconBusy
        "
        @click="emit('save')"
      >
        {{ t("admin.subdomainProxy.saveMapping") }}
      </Button>
    </template>
  </DialogFooter>
</template>
