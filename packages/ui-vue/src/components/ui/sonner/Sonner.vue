<script lang="ts" setup>
import type { CSSProperties, Component } from "vue";
import type { ToasterProps } from "vue-sonner";
import { computed, defineComponent, h, markRaw } from "vue";
import {
  CircleCheckIcon,
  InfoIcon,
  Loader2Icon,
  OctagonXIcon,
  TriangleAlertIcon,
  XIcon,
} from "lucide-vue-next";
import { Toaster as Sonner } from "vue-sonner";
import { cn } from "@/lib/utils";

const props = defineProps<ToasterProps>();

const themedStyle: CSSProperties = {
  "--normal-bg": "var(--popover)",
  "--normal-text": "var(--popover-foreground)",
  "--normal-border": "var(--border)",
  "--border-radius": "var(--radius)",
};

const createIconComponent = (icon: Component, className = "size-4") =>
  markRaw(
    defineComponent({
      name: "ToastIcon",
      setup() {
        return () => h(icon, { class: className });
      },
    }),
  );

const defaultIcons = {
  success: createIconComponent(CircleCheckIcon),
  info: createIconComponent(InfoIcon),
  warning: createIconComponent(TriangleAlertIcon),
  error: createIconComponent(OctagonXIcon),
  loading: createIconComponent(Loader2Icon, "size-4 animate-spin"),
  close: createIconComponent(XIcon),
};

const mergedProps = computed<ToasterProps>(() => ({
  ...props,
  class: cn("toaster group", props.class),
  style: {
    ...themedStyle,
    ...(props.style ?? {}),
  },
  icons: {
    ...defaultIcons,
    ...(props.icons ?? {}),
  },
}));
</script>

<template>
  <Sonner v-bind="mergedProps" />
</template>
