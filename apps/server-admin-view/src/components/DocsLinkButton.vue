<script setup lang="ts">
import { ExternalLink } from "lucide-vue-next";
import { Button, type ButtonVariants } from "@/components/ui/button";

withDefaults(
  defineProps<{
    href: string;
    label?: string;
    size?: ButtonVariants["size"];
  }>(),
  {
    label: "文档",
    size: "sm",
  },
);
</script>

<template>
  <Button as-child variant="ghost" class="gap-0 leading-none" :size="size">
    <a :href="href" target="_blank" rel="noopener noreferrer" @click.stop>
      <ExternalLink class="mr-1.5 h-4 w-4" />
      {{ label }}
    </a>
  </Button>
</template>
