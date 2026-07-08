<template>
  <Card :class="['auth-glass-card w-full max-w-sm', cardClass]">
    <slot name="header">
      <CardHeader v-if="showDefaultHeader">
        <CardTitle v-if="title" :class="titleClass">
          {{ title }}
        </CardTitle>
        <CardDescription v-if="description" :class="descriptionClass">
          {{ description }}
        </CardDescription>
        <slot name="header-extra" />
      </CardHeader>
    </slot>

    <CardContent :class="contentClass">
      <slot />
    </CardContent>
  </Card>
</template>

<script setup lang="ts">
import { computed, useSlots } from "vue";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";

const props = withDefaults(
  defineProps<{
    cardClass?: string;
    contentClass?: string;
    description?: string;
    descriptionClass?: string;
    title?: string;
    titleClass?: string;
  }>(),
  {
    cardClass: "",
    contentClass: "",
    description: "",
    descriptionClass: "text-center",
    title: "",
    titleClass: "text-2xl text-center",
  },
);

const slots = useSlots();
const showDefaultHeader = computed(
  () =>
    Boolean(props.title || props.description) || Boolean(slots["header-extra"]),
);
</script>
