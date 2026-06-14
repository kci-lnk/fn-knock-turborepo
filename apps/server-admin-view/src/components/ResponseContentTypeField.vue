<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
} from '@/components/ui/select'

const CUSTOM_CONTENT_TYPE = '__custom__'

const contentTypeOptions = [
  {
    value: 'text/plain; charset=utf-8',
    label: 'Plain Text',
  },
  {
    value: 'application/json; charset=utf-8',
    label: 'JSON',
  },
  {
    value: 'text/html; charset=utf-8',
    label: 'HTML',
  },
  {
    value: 'text/css; charset=utf-8',
    label: 'CSS',
  },
  {
    value: 'application/javascript; charset=utf-8',
    label: 'JavaScript',
  },
  {
    value: 'application/xml; charset=utf-8',
    label: 'XML',
  },
  {
    value: 'image/svg+xml; charset=utf-8',
    label: 'SVG',
  },
  {
    value: 'application/octet-stream',
    label: 'Binary',
  },
] as const

const props = withDefaults(
  defineProps<{
    modelValue: string
    inputId?: string
    selectId?: string
  }>(),
  {
    inputId: 'response-content-type',
    selectId: 'response-content-type-preset',
  },
)

const emit = defineEmits<{
  (event: 'update:modelValue', value: string): void
}>()

const { t } = useI18n()
const customMode = ref(false)

function findPreset(value: string) {
  const current = value.trim()
  return contentTypeOptions.find((option) => option.value === current) ?? null
}

const contentType = computed({
  get: () => props.modelValue,
  set: (value) => {
    const nextValue = String(value ?? '')
    customMode.value = !findPreset(nextValue)
    emit('update:modelValue', nextValue)
  },
})

const selectedPreset = computed({
  get: () => {
    if (customMode.value) return CUSTOM_CONTENT_TYPE
    const matched = findPreset(props.modelValue)
    return matched?.value ?? CUSTOM_CONTENT_TYPE
  },
  set: (value) => {
    if (value === CUSTOM_CONTENT_TYPE) {
      customMode.value = true
      return
    }
    customMode.value = false
    emit('update:modelValue', value)
  },
})

const selectedPresetLabel = computed(() => {
  if (selectedPreset.value === CUSTOM_CONTENT_TYPE) {
    return t('admin.components.responseContentType.custom')
  }
  return (
    findPreset(selectedPreset.value)?.label ??
    t('admin.components.responseContentType.custom')
  )
})
</script>

<template>
  <div class="grid gap-3 md:grid-cols-[13rem_minmax(0,1fr)]">
    <div class="space-y-2">
      <Label :for="selectId">{{
        t('admin.components.responseContentType.commonTypes')
      }}</Label>
      <Select v-model="selectedPreset">
        <SelectTrigger :id="selectId" class="w-full">
          <span data-slot="select-value" class="truncate">
            {{ selectedPresetLabel }}
          </span>
        </SelectTrigger>
        <SelectContent>
          <SelectItem
            v-for="option in contentTypeOptions"
            :key="option.value"
            :value="option.value"
          >
            <span class="flex min-w-0 flex-col items-start gap-0.5">
              <span class="text-sm">{{ option.label }}</span>
              <span class="max-w-[18rem] truncate text-xs text-muted-foreground">
                {{ option.value }}
              </span>
            </span>
          </SelectItem>
          <SelectItem :value="CUSTOM_CONTENT_TYPE">{{
            t('admin.components.responseContentType.custom')
          }}</SelectItem>
        </SelectContent>
      </Select>
    </div>

    <div class="space-y-2">
      <Label :for="inputId">Content-Type</Label>
      <Input
        :id="inputId"
        v-model="contentType"
        placeholder="text/plain; charset=utf-8"
        autocomplete="off"
        autocapitalize="off"
        autocorrect="off"
        spellcheck="false"
      />
    </div>
  </div>
</template>
