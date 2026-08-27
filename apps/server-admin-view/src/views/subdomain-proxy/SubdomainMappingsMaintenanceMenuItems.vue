<script setup lang="ts">
import { ArrowLeftRight, Eraser } from "lucide-vue-next";
import { useI18n } from "vue-i18n";
import { DropdownMenuItem } from "@/components/ui/dropdown-menu";

defineProps<{
  clearing: boolean;
  hasMappings: boolean;
  saving: boolean;
}>();
const emit = defineEmits<{
  cleanup: [];
  optimize: [];
}>();
const { t } = useI18n();
</script>

<template>
  <DropdownMenuItem
    :disabled="!hasMappings || saving || clearing"
    @select="emit('cleanup')"
  >
    <Eraser class="mr-2 h-4 w-4" />
    {{ t("admin.subdomainProxy.cleanupStaleServices") }}
  </DropdownMenuItem>
  <DropdownMenuItem
    :disabled="!hasMappings || saving || clearing"
    @select="emit('optimize')"
  >
    <ArrowLeftRight class="mr-2 h-4 w-4" />
    {{ t("admin.subdomainProxy.targetOptimizationMenu") }}
  </DropdownMenuItem>
</template>
