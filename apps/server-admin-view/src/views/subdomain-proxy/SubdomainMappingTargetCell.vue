<script setup lang="ts">
import { File, Folder, Network } from "lucide-vue-next";
import { useI18n } from "vue-i18n";
import { Badge } from "@/components/ui/badge";
import { TableCell } from "@/components/ui/table";
import type { HostMapping } from "@/types";
import {
  getHostMappingTargetText,
  normalizeHostMappingTargetType,
} from "./model";

defineProps<{
  mapping: HostMapping;
  unavailable: boolean;
}>();
const { t } = useI18n();
</script>

<template>
  <TableCell :class="{ 'text-muted-foreground': unavailable }">
    <div class="flex min-w-0 items-center gap-2">
      <Network
        v-if="normalizeHostMappingTargetType(mapping.target_type) === 'proxy'"
        class="h-3.5 w-3.5 shrink-0 text-muted-foreground"
      />
      <File
        v-else-if="mapping.target_type === 'file'"
        class="h-3.5 w-3.5 shrink-0 text-muted-foreground"
      />
      <Folder v-else class="h-3.5 w-3.5 shrink-0 text-muted-foreground" />
      <span class="min-w-0 break-all font-mono text-xs">
        {{ getHostMappingTargetText(mapping) }}
      </span>
      <Badge
        v-if="normalizeHostMappingTargetType(mapping.target_type) !== 'proxy'"
        variant="secondary"
        class="shrink-0 text-[10px]"
      >
        {{
          t(
            `admin.subdomainProxy.staticServe.targetTypes.${normalizeHostMappingTargetType(mapping.target_type)}`,
          )
        }}
      </Badge>
    </div>
  </TableCell>
</template>
