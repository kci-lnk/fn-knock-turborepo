<script setup lang="ts">
import { computed } from "vue";
import { ChevronDown, Loader2, RefreshCw, Trash2 } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import type { SSHSecurityController } from "./ssh-security-contract";

const props = withDefaults(
  defineProps<{
    compact?: boolean;
    controller: SSHSecurityController;
  }>(),
  { compact: false },
);
const {
  details,
  isSaving,
  isSyncingFirewall,
  openClearFirewallDialog,
  syncFirewall,
  t,
} = props.controller;
const disabled = computed(
  () =>
    isSaving.value ||
    isSyncingFirewall.value ||
    !details.value ||
    !details.value.summary.available,
);
</script>

<template>
  <DropdownMenu>
    <DropdownMenuTrigger as-child>
      <Button
        variant="outline"
        :class="compact ? 'w-24 gap-2' : 'gap-2'"
        :disabled="disabled"
      >
        <Loader2 v-if="isSyncingFirewall" class="h-4 w-4 animate-spin" />
        <span>{{ t("admin.sshSecurity.actions") }}</span>
        <ChevronDown class="h-4 w-4 text-muted-foreground" />
      </Button>
    </DropdownMenuTrigger>
    <DropdownMenuContent align="end" class="w-56">
      <DropdownMenuItem :disabled="disabled" @select="syncFirewall">
        <RefreshCw class="h-4 w-4" />
        {{ t("admin.sshSecurity.syncFirewall") }}
      </DropdownMenuItem>
      <DropdownMenuItem
        class="text-destructive focus:text-destructive"
        :disabled="disabled"
        @select="openClearFirewallDialog"
      >
        <Trash2 class="h-4 w-4" />
        {{ t("admin.sshSecurity.clearSshFirewall") }}
      </DropdownMenuItem>
    </DropdownMenuContent>
  </DropdownMenu>
</template>
