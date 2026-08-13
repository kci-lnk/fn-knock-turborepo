<script setup lang="ts">
import DDNSTargetAddressBaseFields from "./DDNSTargetAddressBaseFields.vue";
import DDNSTargetInterfaceAddressFields from "./DDNSTargetInterfaceAddressFields.vue";
import DDNSTargetStaticAddressFields from "./DDNSTargetStaticAddressFields.vue";
import type { DDNSTargetAddressFieldsProps } from "./ddns-target-dialog-contract";

const props = defineProps<DDNSTargetAddressFieldsProps>();
const emit = defineEmits<{ "update:networkInterface": [value: string] }>();
</script>

<template>
  <DDNSTargetStaticAddressFields :model="props" />
  <DDNSTargetAddressBaseFields
    :model="props"
    @update:network-interface="emit('update:networkInterface', $event)"
  />
  <DDNSTargetInterfaceAddressFields :model="props" />
</template>
