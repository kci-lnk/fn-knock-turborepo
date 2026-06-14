<script setup lang="ts">
import type { PaginationEllipsisProps } from "reka-ui"
import type { HTMLAttributes } from "vue"
import { reactiveOmit } from "@vueuse/core"
import { MoreHorizontal } from "lucide-vue-next"
import { PaginationEllipsis } from "reka-ui"
import { useI18n } from "vue-i18n"
import { cn } from "@/lib/utils"

const props = defineProps<PaginationEllipsisProps & { class?: HTMLAttributes["class"] }>()

const delegatedProps = reactiveOmit(props, "class")
const { t } = useI18n()
</script>

<template>
  <PaginationEllipsis
    data-slot="pagination-ellipsis"
    v-bind="delegatedProps"
    :class="cn('flex size-9 items-center justify-center', props.class)"
  >
    <slot>
      <MoreHorizontal class="size-4" />
      <span class="sr-only">{{ t("common.morePages") }}</span>
    </slot>
  </PaginationEllipsis>
</template>
