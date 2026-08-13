<script setup lang="ts">
import { Loader2 } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import type { SSHSecurityController } from "./ssh-security-contract";

const props = defineProps<{ controller: SSHSecurityController }>();
const {
  clearFirewall,
  isClearFirewallDialogOpen,
  isSyncingFirewall,
  t,
} = props.controller;
</script>

<template>
<Dialog v-model:open="isClearFirewallDialogOpen">
  <DialogContent class="sm:max-w-md">
    <DialogHeader>
      <DialogTitle>
        {{ t("admin.sshSecurity.clearFirewallTitle") }}
      </DialogTitle>
      <DialogDescription>
        {{ t("admin.sshSecurity.clearFirewallDescription") }}
      </DialogDescription>
    </DialogHeader>
    <DialogFooter>
      <Button
        variant="outline"
        :disabled="isSyncingFirewall"
        @click="isClearFirewallDialogOpen = false"
      >
        {{ t("common.cancel") }}
      </Button>
      <Button
        variant="destructive"
        :disabled="isSyncingFirewall"
        @click="clearFirewall"
      >
        <Loader2 v-if="isSyncingFirewall" class="h-4 w-4 animate-spin" />
        {{ t("admin.sshSecurity.clear") }}
      </Button>
    </DialogFooter>
  </DialogContent>
</Dialog>
</template>
