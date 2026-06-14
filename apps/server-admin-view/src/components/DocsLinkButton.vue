<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { ExternalLink } from "lucide-vue-next";
import { Button, type ButtonVariants } from "@/components/ui/button";

const props = withDefaults(
  defineProps<{
    href: string;
    label?: string;
    size?: ButtonVariants["size"];
  }>(),
  {
    size: "sm",
  },
);

const { t } = useI18n();
const displayLabel = computed(() => props.label ?? t("common.docs"));
</script>

<template>
  <Button as-child variant="ghost" class="gap-0 leading-none" :size="size">
    <a :href="href" target="_blank" rel="noopener noreferrer" @click.stop>
      <ExternalLink class="mr-1.5 h-4 w-4" />
      {{ displayLabel }}
    </a>
  </Button>
</template>
