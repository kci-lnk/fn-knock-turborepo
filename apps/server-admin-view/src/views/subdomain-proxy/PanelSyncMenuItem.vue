<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import { PanelsTopLeft } from "lucide-vue-next";
import { DropdownMenuItem } from "@/components/ui/dropdown-menu";

const { t } = useI18n();
const router = useRouter();
</script>

<template>
  <DropdownMenuItem @select="router.push('/subdomains/panel-sync')">
    <PanelsTopLeft class="mr-2 h-4 w-4" />
    {{ t("admin.panelSync.menuEntry") }}
  </DropdownMenuItem>
</template>
