<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import TomlCodeEditor from '../../../components/TomlCodeEditor.vue'
import { toast } from '@admin-shared/utils/toast'
import { extractErrorMessage } from '@admin-shared/composables/useAsyncAction'
import {
  extractVisualFieldsFromToml,
  mergeVisualFieldsIntoToml,
  type FrpcVisualFields,
} from '../../../lib/frpc-config-editor'

const props = defineProps<{
  modelValue: string
  defaults: { local_port: string }
  idPrefix: string
}>()

const emit = defineEmits<{
  'update:modelValue': [value: string]
}>()

const { t } = useI18n()

const rawContent = ref('')
const customToml = ref('')
const editorMode = ref<'visual' | 'custom'>('visual')
const visualSyncError = ref<string | null>(null)

const serverAddr = ref('')
const serverPort = ref('7000')
const serverToken = ref('')
const webUser = ref('admin')
const webPassword = ref('')
const localPort = ref('7999')
const remotePort = ref('7999')

const isCustomMode = computed(() => editorMode.value === 'custom')
const currentModeLabel = computed(() =>
  isCustomMode.value ? t('admin.frpcInstanceEditor.sourceMode') : t('admin.frpcInstanceEditor.formMode'),
)
const currentModeDescription = computed(() =>
  isCustomMode.value
    ? t('admin.frpcInstanceEditor.customModeDescription')
    : t('admin.frpcInstanceEditor.formModeDescription'),
)

function fieldId(name: string) {
  return `${props.idPrefix}-${name}`
}

function getVisualDefaults() {
  return {
    localPort: props.defaults.local_port,
  }
}

function getVisualFields(): FrpcVisualFields {
  return {
    serverAddr: serverAddr.value,
    serverPort: serverPort.value,
    serverToken: serverToken.value,
    webUser: webUser.value,
    webPassword: webPassword.value,
    localPort: localPort.value,
    remotePort: remotePort.value,
  }
}

function applyVisualFields(fields: FrpcVisualFields) {
  serverAddr.value = fields.serverAddr
  serverPort.value = fields.serverPort
  serverToken.value = fields.serverToken
  webUser.value = fields.webUser
  webPassword.value = fields.webPassword
  localPort.value = fields.localPort
  remotePort.value = fields.remotePort
}

function syncVisualFieldsFromRaw(raw: string) {
  applyVisualFields(extractVisualFieldsFromToml(raw, getVisualDefaults()))
  visualSyncError.value = null
}

function buildVisualConfig(baseRaw = customToml.value || rawContent.value): string {
  return mergeVisualFieldsIntoToml(baseRaw, getVisualFields(), getVisualDefaults())
}

function resetFromRaw(raw: string) {
  rawContent.value = raw
  customToml.value = raw
  try {
    syncVisualFieldsFromRaw(raw)
    editorMode.value = 'visual'
  } catch (error) {
    editorMode.value = 'custom'
    visualSyncError.value = extractErrorMessage(error, t('admin.frpcInstanceEditor.unmappableFallback'))
  }
}

function enterCustomMode() {
  try {
    customToml.value = buildVisualConfig(customToml.value || rawContent.value)
    rawContent.value = customToml.value
    editorMode.value = 'custom'
    visualSyncError.value = null
    emit('update:modelValue', customToml.value)
  } catch (error) {
    toast.error(t('admin.frpcInstanceEditor.enterCustomFailed'), {
      description: extractErrorMessage(error, t('admin.frpcInstanceEditor.convertToTomlFailed')),
    })
  }
}

function exitCustomMode() {
  try {
    syncVisualFieldsFromRaw(customToml.value)
    rawContent.value = customToml.value
    editorMode.value = 'visual'
    emit('update:modelValue', customToml.value)
  } catch (error) {
    const message = extractErrorMessage(error, t('admin.frpcInstanceEditor.invalidToml'))
    visualSyncError.value = message
    toast.error(t('admin.frpcInstanceEditor.exitCustomFailed'), {
      description: t('admin.frpcInstanceEditor.fixCustomBeforeSwitch', { message }),
    })
  }
}

function toggleCustomMode() {
  if (isCustomMode.value) {
    exitCustomMode()
    return
  }
  enterCustomMode()
}

function getContent(): string {
  const content = isCustomMode.value ? customToml.value : buildVisualConfig()
  rawContent.value = content
  customToml.value = content
  emit('update:modelValue', content)
  return content
}

watch(
  () => props.modelValue,
  (value) => {
    if (value === rawContent.value) return
    resetFromRaw(value)
  },
  { immediate: true },
)

watch(customToml, (value) => {
  if (isCustomMode.value) {
    rawContent.value = value
    emit('update:modelValue', value)
  }
})

defineExpose({
  getContent,
  resetFromRaw,
})
</script>

<template>
  <div class="overflow-hidden rounded-lg border divide-y divide-border">
    <div class="bg-linear-to-r from-muted/40 via-muted/15 to-transparent px-4 py-4 sm:px-5">
      <div class="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
        <div class="space-y-1">
          <div class="text-sm font-medium tracking-tight">{{ t('admin.frpcInstanceEditor.editMethod') }}</div>
          <p class="max-w-2xl text-xs leading-relaxed text-muted-foreground">
            {{ currentModeDescription }}
          </p>
        </div>
        <div class="flex items-center gap-2">
          <span
            class="inline-flex w-fit items-center rounded-full border px-2.5 py-1 text-[11px] font-medium"
            :class="isCustomMode ? 'border-primary/20 bg-primary/5 text-primary' : 'border-border bg-background/80 text-muted-foreground'"
          >
            {{ currentModeLabel }}
          </span>
          <Button
            variant="outline"
            size="sm"
            :class="isCustomMode ? 'border-primary bg-primary/5 text-primary hover:bg-primary/10' : ''"
            @click="toggleCustomMode"
          >
            {{ t('admin.frpcInstanceEditor.custom') }}
          </Button>
        </div>
      </div>
    </div>

    <div v-if="isCustomMode" class="space-y-4 p-4 sm:p-5">
      <div
        v-if="visualSyncError"
        class="rounded-lg border border-destructive/20 bg-destructive/5 px-4 py-3 text-sm leading-relaxed text-destructive"
      >
        {{ t('admin.frpcInstanceEditor.cannotSwitchToForm', { message: visualSyncError }) }}
      </div>
      <div class="rounded-lg border border-dashed border-border/80 bg-muted/20 px-4 py-3 text-xs leading-relaxed text-muted-foreground">
        {{ t('admin.frpcInstanceEditor.customHint') }}
      </div>
      <TomlCodeEditor v-model="customToml" />
    </div>

    <div v-else class="divide-y divide-border">
      <div class="p-4 sm:p-5 grid gap-2 sm:grid-cols-[180px_1fr] md:grid-cols-[220px_1fr] items-start transition-colors hover:bg-muted/10">
        <div class="space-y-1 mt-1.5">
          <Label :for="fieldId('server-addr')" class="text-sm font-medium flex items-center gap-1">
            {{ t('admin.frpcInstanceEditor.serverAddr') }}
            <span class="text-destructive">*</span>
          </Label>
          <p class="text-xs text-muted-foreground leading-relaxed hidden sm:block pr-4">
            {{ t('admin.frpcInstanceEditor.serverAddrHint') }}
          </p>
        </div>
        <div class="w-full max-w-md space-y-2">
          <Input
            :id="fieldId('server-addr')"
            v-model.trim="serverAddr"
            placeholder="example.com"
            autocomplete="off"
            autocapitalize="off"
            autocorrect="off"
            :spellcheck="false"
            data-form-type="other"
            data-1p-ignore="true"
            data-lpignore="true"
            data-bwignore="true"
          />
          <p class="text-[11px] text-muted-foreground sm:hidden mt-1.5">
            {{ t('admin.frpcInstanceEditor.serverAddrHint') }}
          </p>
        </div>
      </div>

      <div class="p-4 sm:p-5 grid gap-2 sm:grid-cols-[180px_1fr] md:grid-cols-[220px_1fr] items-start transition-colors hover:bg-muted/10">
        <div class="space-y-1 mt-1.5">
          <Label :for="fieldId('server-port')" class="text-sm font-medium flex items-center gap-1">
            {{ t('admin.frpcInstanceEditor.serverPort') }}
            <span class="text-destructive">*</span>
          </Label>
        </div>
        <div class="w-full max-w-md">
          <Input
            :id="fieldId('server-port')"
            v-model="serverPort"
            type="number"
            autocomplete="off"
            autocapitalize="off"
            autocorrect="off"
            :spellcheck="false"
            data-form-type="other"
            data-1p-ignore="true"
            data-lpignore="true"
            data-bwignore="true"
          />
        </div>
      </div>

      <div class="p-4 sm:p-5 grid gap-2 sm:grid-cols-[180px_1fr] md:grid-cols-[220px_1fr] items-start transition-colors hover:bg-muted/10">
        <div class="space-y-1 mt-1.5">
          <Label :for="fieldId('server-token')" class="text-sm font-medium">Token</Label>
          <p class="text-xs text-muted-foreground leading-relaxed hidden sm:block pr-4">
            {{ t('admin.frpcInstanceEditor.tokenHint') }}
          </p>
        </div>
        <div class="w-full max-w-md space-y-2">
          <Input
            :id="fieldId('server-token')"
            v-model.trim="serverToken"
            :placeholder="t('admin.frpcInstanceEditor.optional')"
            autocomplete="off"
            autocapitalize="off"
            autocorrect="off"
            :spellcheck="false"
            data-form-type="other"
            data-1p-ignore="true"
            data-lpignore="true"
            data-bwignore="true"
          />
          <p class="text-[11px] text-muted-foreground sm:hidden mt-1.5">
            {{ t('admin.frpcInstanceEditor.tokenHint') }}
          </p>
        </div>
      </div>

      <div class="p-4 sm:p-5 grid gap-2 sm:grid-cols-[180px_1fr] md:grid-cols-[220px_1fr] items-start transition-colors hover:bg-muted/10">
        <div class="space-y-1 mt-1.5">
          <Label :for="fieldId('local-port')" class="text-sm font-medium flex items-center gap-1">
            {{ t('admin.frpcInstanceEditor.localPort') }}
            <span class="text-destructive">*</span>
          </Label>
          <p class="text-xs text-muted-foreground leading-relaxed hidden sm:block pr-4">
            {{ t('admin.frpcInstanceEditor.localPortHint', { port: defaults.local_port }) }}
          </p>
        </div>
        <div class="w-full max-w-md space-y-2">
          <Input
            :id="fieldId('local-port')"
            v-model="localPort"
            type="number"
            :placeholder="defaults.local_port"
            autocomplete="off"
            autocapitalize="off"
            autocorrect="off"
            :spellcheck="false"
            data-form-type="other"
            data-1p-ignore="true"
            data-lpignore="true"
            data-bwignore="true"
          />
          <p class="text-[11px] text-muted-foreground sm:hidden mt-1.5">
            {{ t('admin.frpcInstanceEditor.defaultPort', { port: defaults.local_port }) }}
          </p>
        </div>
      </div>

      <div class="p-4 sm:p-5 grid gap-2 sm:grid-cols-[180px_1fr] md:grid-cols-[220px_1fr] items-start transition-colors hover:bg-muted/10">
        <div class="space-y-1 mt-1.5">
          <Label :for="fieldId('remote-port')" class="text-sm font-medium flex items-center gap-1">
            {{ t('admin.frpcInstanceEditor.remotePort') }}
            <span class="text-destructive">*</span>
          </Label>
          <p class="text-xs text-muted-foreground leading-relaxed hidden sm:block pr-4">
            {{ t('admin.frpcInstanceEditor.remotePortHint') }}
          </p>
        </div>
        <div class="w-full max-w-md space-y-2">
          <Input
            :id="fieldId('remote-port')"
            v-model="remotePort"
            type="number"
            autocomplete="off"
            autocapitalize="off"
            autocorrect="off"
            :spellcheck="false"
            data-form-type="other"
            data-1p-ignore="true"
            data-lpignore="true"
            data-bwignore="true"
          />
          <p class="text-[11px] text-muted-foreground sm:hidden mt-1.5">
            {{ t('admin.frpcInstanceEditor.remotePortHint') }}
          </p>
        </div>
      </div>
    </div>
  </div>
</template>
