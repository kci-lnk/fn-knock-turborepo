<script setup lang="ts">
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
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { Ban, Loader2 } from "lucide-vue-next";
import type { GeneralBlacklistPageController } from "./useGeneralBlacklistPage";

const props = defineProps<{ controller: GeneralBlacklistPageController }>();
const { t } = useI18n();
const {
  addComment,
  addDialogOpen,
  addIpsText,
  addManualBlacklist,
  isAdding,
  parsedAddIps,
} = props.controller;
</script>

<template>
  <Dialog v-model:open="addDialogOpen">
    <DialogContent class="sm:max-w-[560px]">
      <DialogHeader>
        <DialogTitle>
          {{ t("admin.sessions.generalBlacklist.addDialogTitle") }}
        </DialogTitle>
        <DialogDescription>
          {{ t("admin.sessions.generalBlacklist.addDialogDescription") }}
        </DialogDescription>
      </DialogHeader>

      <div class="grid gap-4 py-2">
        <div class="grid gap-2">
          <Label for="general-blacklist-ips">
            {{ t("admin.sessions.generalBlacklist.ipInputLabel") }}
          </Label>
          <Textarea
            id="general-blacklist-ips"
            v-model="addIpsText"
            :placeholder="t('admin.sessions.generalBlacklist.ipInputPlaceholder')"
            class="min-h-[160px] font-mono text-sm"
          />
          <p class="text-xs text-muted-foreground">
            {{
              t("admin.sessions.generalBlacklist.parsedCount", {
                count: parsedAddIps.length,
              })
            }}
          </p>
        </div>

        <div class="grid gap-2">
          <Label for="general-blacklist-comment">
            {{ t("admin.sessions.generalBlacklist.comment") }}
          </Label>
          <Textarea
            id="general-blacklist-comment"
            v-model="addComment"
            :placeholder="t('admin.sessions.generalBlacklist.commentPlaceholder')"
            class="min-h-[72px]"
          />
        </div>
      </div>

      <DialogFooter>
        <Button
          variant="outline"
          :disabled="isAdding"
          @click="addDialogOpen = false"
        >
          {{ t("common.cancel") }}
        </Button>
        <Button
          :disabled="parsedAddIps.length === 0 || isAdding"
          @click="addManualBlacklist"
        >
          <Loader2 v-if="isAdding" class="h-4 w-4 animate-spin" />
          <Ban v-else class="h-4 w-4" />
          {{ t("admin.sessions.generalBlacklist.addConfirm") }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
