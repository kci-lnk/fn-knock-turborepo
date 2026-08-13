<script setup lang="ts">
import DDNSAddressSourceBaseFields from "./DDNSAddressSourceBaseFields.vue";
import DDNSInterfaceAddressFields from "./DDNSInterfaceAddressFields.vue";
import DDNSStaticAddressFields from "./DDNSStaticAddressFields.vue";
import type { DDNSAddressSourceFieldsProps } from "./ddns-address-source-fields-contract";

const props = defineProps<DDNSAddressSourceFieldsProps>();
</script>

<template>
  <template v-if="props.selectedProvider">
    <DDNSAddressSourceBaseFields :model="props" />
    <DDNSStaticAddressFields :model="props" />
    <DDNSInterfaceAddressFields :model="props" />
  </template>
</template>
