<template>
  <div class="grid gap-4">
    <div class="grid gap-2">
      <Label for="cert-input">SSL 证书 (.crt / .pem)</Label>
      <div class="flex gap-2 mb-1">
        <Input 
          type="file" 
          accept=".crt,.pem" 
          @change="handleFileUpload($event, 'cert')" 
          class="flex-1" 
        />
      </div>
      <Textarea
        id="cert-input"
        :model-value="cert"
        @update:model-value="(v) => $emit('update:cert', String(v))"
        placeholder="-----BEGIN CERTIFICATE-----\n..."
        class="font-mono text-sm h-28"
      />
    </div>

    <div class="grid gap-2">
      <Label for="key-input">私钥 (.key / .pem)</Label>
      <div class="flex gap-2 mb-1">
        <Input 
          type="file" 
          accept=".key,.pem" 
          @change="handleFileUpload($event, 'sslKey')" 
          class="flex-1" 
        />
      </div>
      <Textarea
        id="key-input"
        :model-value="sslKey"
        @update:model-value="(v) => $emit('update:sslKey', String(v))"
        placeholder="-----BEGIN PRIVATE KEY-----\n..."
        class="font-mono text-sm h-28"
      />
    </div>
  </div>
</template>

<script setup lang="ts">
import { Label } from '@/components/ui/label';
import { Input } from '@/components/ui/input';
import { Textarea } from '@/components/ui/textarea';

// 定义 Props
defineProps<{
  cert: string;
  sslKey: string;
}>();

// 使用 Vue 3.3+ 更简洁且对 TS 更友好的 emit 定义方式
const emit = defineEmits<{
  'update:cert': [value: string];
  'update:sslKey': [value: string];
}>();

function handleFileUpload(event: Event, type: 'cert' | 'sslKey') {
  const target = event.target as HTMLInputElement;
  const file = target.files?.[0];
  if (!file) return;

  const reader = new FileReader();
  reader.onload = (e) => {
    const result = e.target?.result;
    if (typeof result === 'string') {
      if (type === 'cert') {
        emit('update:cert', result);
      } else {
        emit('update:sslKey', result);
      }
    }
  };
  reader.readAsText(file);
}
</script>