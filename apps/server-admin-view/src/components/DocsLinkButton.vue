<script setup lang="ts">
import { ExternalLink } from "lucide-vue-next";
import { Button } from "@/components/ui/button";

withDefaults(
  defineProps<{
    href: string;
    label?: string;
  }>(),
  {
    label: "文档",
  },
);
</script>

<template>
  <Button as-child variant="ghost" class="gap-0" size="sm">
    <a :href="href" target="_blank" rel="noopener noreferrer" @click.stop>
      <ExternalLink class="mr-1.5 h-4 w-2" />
      {{ label }}
    </a>
  </Button>
</template>
