<script setup lang="ts">
import { ArrowLeftRight } from "lucide-vue-next";
import { Button } from "@/components/ui/button";

defineProps<{
  actionLabel: string;
  description: string;
  fields: string[];
  loading?: boolean;
  sourceLabel: string;
}>();

const emit = defineEmits<{
  apply: [];
}>();
</script>

<template>
  <div class="rounded-lg border border-border/80 bg-background/70 px-3.5 py-3">
    <div
      class="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between"
    >
      <div class="min-w-0 grid gap-1.5">
        <div class="flex items-center gap-2 text-[11px] text-muted-foreground">
          <ArrowLeftRight class="h-3.5 w-3.5" />
          <span>可复用凭据</span>
        </div>
        <p class="text-sm leading-6 text-foreground/90">{{ description }}</p>
        <p class="text-xs text-muted-foreground">
          来源：{{ sourceLabel }}。仅填充当前为空的字段。
        </p>
        <div class="flex flex-wrap gap-1.5">
          <span
            v-for="field in fields"
            :key="field"
            class="rounded-md border bg-muted/35 px-2 py-0.5 font-mono text-[11px] text-muted-foreground"
          >
            {{ field }}
          </span>
        </div>
      </div>

      <Button
        type="button"
        variant="outline"
        size="sm"
        class="shrink-0"
        :disabled="loading"
        @click="emit('apply')"
      >
        {{ loading ? "处理中..." : actionLabel }}
      </Button>
    </div>
  </div>
</template>
