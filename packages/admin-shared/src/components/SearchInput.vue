<script setup lang="ts">

import { Search, X } from 'lucide-vue-next'
import { InputGroup, InputGroupAddon, InputGroupInput, InputGroupButton } from '@/components/ui/input-group'

export interface Props {
  placeholder?: string
  class?: any
}

const props = withDefaults(defineProps<Props>(), {
  placeholder: 'Search...',
  class: ''
})

const modelValue = defineModel<string>({ default: '' })

const emit = defineEmits<{
  search: [value: string]
  clear: []
}>()

const handleSearch = () => {
  emit('search', modelValue.value)
}

const handleClear = () => {
  modelValue.value = ''
  emit('clear')
  emit('search', '')
}
</script>

<template>
  <InputGroup :class="props.class">
    <InputGroupInput 
      v-model="modelValue" 
      :placeholder="placeholder" 
      @keyup.enter="handleSearch"
      class="border-r-0 focus-visible:ring-0 shadow-none border-transparent"
    />
    <InputGroupAddon align="inline-end" class="py-0 pl-0 pr-2 has-[>button]:mr-0">
      <InputGroupButton 
        v-if="modelValue" 
        size="icon-sm" 
        variant="ghost" 
        @click="handleClear"
        title="Clear"
      >
        <X class="h-4 w-4 text-muted-foreground hover:text-foreground transition-colors" />
      </InputGroupButton>
      <InputGroupButton 
        v-else 
        size="icon-sm" 
        variant="ghost" 
        @click="handleSearch"
        title="Search"
      >
        <Search class="h-4 w-4 text-muted-foreground" />
      </InputGroupButton>
    </InputGroupAddon>
  </InputGroup>
</template>
