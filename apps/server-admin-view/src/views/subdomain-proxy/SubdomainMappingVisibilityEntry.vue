<script setup lang="ts">
import {
  AlertTriangle,
  ChevronRight,
  RefreshCw,
  ShieldCheck,
} from "lucide-vue-next";
import { useI18n } from "vue-i18n";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import type { SubdomainMappingDialogProps } from "./subdomain-mapping-dialog-contract";

defineProps<{ dialog: SubdomainMappingDialogProps }>();
const { t } = useI18n();
</script>

<template>
  <Alert
    v-if="
      !dialog.isMappingAuthService &&
      dialog.visibilityEditor.globalVisibilityLoadError
    "
    variant="destructive"
    class="items-start"
  >
    <AlertTriangle class="h-4 w-4" />
    <AlertTitle>
      {{ t("admin.subdomainProxy.visibilityLoadFailed") }}
    </AlertTitle>
    <AlertDescription class="space-y-3">
      <p class="break-words">
        {{ dialog.visibilityEditor.globalVisibilityLoadError }}
      </p>
      <Button
        type="button"
        variant="outline"
        size="sm"
        :disabled="dialog.visibilityEditor.isGlobalVisibilityLoading"
        @click="dialog.visibilityEditor.loadGlobalVisibility"
      >
        <RefreshCw
          class="mr-2 h-3.5 w-3.5"
          :class="{
            'animate-spin': dialog.visibilityEditor.isGlobalVisibilityLoading,
          }"
        />
        {{ t("admin.subdomainProxy.retry") }}
      </Button>
    </AlertDescription>
  </Alert>

  <Button
    v-if="dialog.visibilityEditor.visibilityAvailable"
    type="button"
    variant="outline"
    class="h-auto w-full justify-between gap-3 px-4 py-3 text-left"
    @click="dialog.visibilityEditor.openVisibilityView"
  >
    <span class="flex min-w-0 flex-1 items-start gap-3">
      <ShieldCheck class="mt-0.5 h-4 w-4 text-muted-foreground" />
      <span class="min-w-0 flex-1 space-y-1">
        <span class="block text-sm font-medium">
          {{ t("admin.subdomainProxy.visibilityTitle") }}
        </span>
        <span
          class="block whitespace-normal break-words text-xs font-normal leading-5"
          :class="
            dialog.visibilityEditor.visibilityValidationMessage
              ? 'text-destructive'
              : 'text-muted-foreground'
          "
        >
          {{
            dialog.visibilityEditor.visibilityValidationMessage ||
            dialog.visibilityEditor.visibilitySummary
          }}
        </span>
      </span>
    </span>
    <ChevronRight class="h-4 w-4 shrink-0 text-muted-foreground" />
  </Button>
</template>
