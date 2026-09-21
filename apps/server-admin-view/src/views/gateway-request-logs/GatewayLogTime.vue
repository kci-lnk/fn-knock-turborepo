<script setup lang="ts">
import HumanFriendlyTime from "@admin-shared/components/common/HumanFriendlyTime.vue";
import { formatLogClock } from "./ip-view-model";
defineProps<{ value: string; locale: string; absolute?: boolean }>();
</script>
<template>
  <time v-if="absolute" :datetime="value" :title="value">{{
    formatLogClock(value)
  }}</time>
  <HumanFriendlyTime v-else :value="value" :locale="locale" />
</template>
