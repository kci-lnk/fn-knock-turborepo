<script setup lang="ts">
import { Badge } from "@/components/ui/badge";
import {
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import type { NotificationRuleEditorController } from "./notification-rule-editor-contract";

const props = defineProps<{ controller: NotificationRuleEditorController }>();
const {
  dialogDescriptionText,
  dialogModeBadgeLabel,
  dialogSelectionBadgeLabel,
  dialogTargetsBadgeLabel,
  dialogTitleText,
} = props.controller;
</script>

<template>
  <DialogHeader
    class="border-b bg-gradient-to-r from-muted/40 via-background to-background px-4 py-5 sm:px-6"
  >
    <div class="space-y-3">
      <div class="space-y-1.5">
        <DialogTitle>{{ dialogTitleText }}</DialogTitle>
        <DialogDescription>{{ dialogDescriptionText }}</DialogDescription>
      </div>
      <div class="flex flex-wrap items-center gap-2">
        <Badge
          variant="secondary"
          class="rounded-full bg-primary/10 px-3 py-1 text-primary"
        >
          {{ dialogModeBadgeLabel }}
        </Badge>
        <Badge variant="outline" class="rounded-full px-3 py-1">
          {{ dialogSelectionBadgeLabel }}
        </Badge>
        <Badge variant="outline" class="rounded-full px-3 py-1">
          {{ dialogTargetsBadgeLabel }}
        </Badge>
      </div>
    </div>
  </DialogHeader>
</template>
