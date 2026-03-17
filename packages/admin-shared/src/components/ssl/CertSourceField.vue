<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue';
import { Button } from '@/components/ui/button';
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from '@/components/ui/dialog';
import { Label } from '@/components/ui/label';
import { Sheet, SheetContent, SheetDescription, SheetFooter, SheetHeader, SheetTitle } from '@/components/ui/sheet';
import { Textarea } from '@/components/ui/textarea';
import { ChevronRight, FolderTree, Laptop, Smartphone, Upload } from 'lucide-vue-next';
import DataShareFilePicker from '../common/DataShareFilePicker.vue';

type CertFieldKey = 'cert' | 'sslKey';

interface SharedDataFileEntry {
  name: string;
  relativePath: string;
  extension: string;
  size: number;
  modifiedAt: string;
}

const props = withDefaults(defineProps<{
  id: string;
  label: string;
  value: string;
  placeholder: string;
  accept: string;
  fieldKey: CertFieldKey;
  supportedFileTypes: string[];
  shareName?: string;
  sharedFiles?: SharedDataFileEntry[];
  sharedFilesAvailable?: boolean;
  sharedFilesLoading?: boolean;
  sharedFilesError?: string;
  sharedFileSelecting?: boolean;
}>(), {
  shareName: 'fn-knock',
  sharedFiles: () => [],
  sharedFilesAvailable: false,
  sharedFilesLoading: false,
  sharedFilesError: '',
  sharedFileSelecting: false,
});

const emit = defineEmits<{
  'update:value': [value: string];
  'request-shared-files': [payload: { field: CertFieldKey; force?: boolean }];
  'select-shared-file': [payload: { field: CertFieldKey; relativePath: string }];
}>();

const localFileInput = ref<HTMLInputElement | null>(null);
const sourceChooserOpen = ref(false);
const pickerOpen = ref(false);
const isMobileViewport = ref(false);

let viewportQuery: MediaQueryList | null = null;
let viewportQueryListener: ((event: MediaQueryListEvent) => void) | null = null;
let overlayTimer: ReturnType<typeof window.setTimeout> | null = null;

const uploadLabel = computed(() => (isMobileViewport.value ? '从手机上传' : '从电脑上传'));
const supportedTypesLabel = computed(() => props.supportedFileTypes.join(' / '));

onMounted(() => {
  if (typeof window === 'undefined') {
    return;
  }

  viewportQuery = window.matchMedia('(max-width: 768px)');
  isMobileViewport.value = viewportQuery.matches;

  viewportQueryListener = (event: MediaQueryListEvent) => {
    isMobileViewport.value = event.matches;
  };

  if (typeof viewportQuery.addEventListener === 'function') {
    viewportQuery.addEventListener('change', viewportQueryListener);
    return;
  }

  viewportQuery.addListener(viewportQueryListener);
});

onBeforeUnmount(() => {
  if (overlayTimer) {
    window.clearTimeout(overlayTimer);
  }

  if (!viewportQuery || !viewportQueryListener) {
    return;
  }

  if (typeof viewportQuery.removeEventListener === 'function') {
    viewportQuery.removeEventListener('change', viewportQueryListener);
    return;
  }

  viewportQuery.removeListener(viewportQueryListener);
});

function scheduleOverlayAction(action: () => void) {
  if (typeof window === 'undefined') {
    action();
    return;
  }

  if (overlayTimer) {
    window.clearTimeout(overlayTimer);
  }

  overlayTimer = window.setTimeout(() => {
    overlayTimer = null;
    action();
  }, 140);
}

function openLocalFileDialog() {
  localFileInput.value?.click();
}

function openSourceChooser() {
  sourceChooserOpen.value = true;
}

function chooseLocalUpload() {
  sourceChooserOpen.value = false;
  scheduleOverlayAction(() => {
    openLocalFileDialog();
  });
}

function chooseSharedFile() {
  sourceChooserOpen.value = false;
  scheduleOverlayAction(() => {
    pickerOpen.value = true;
    emit('request-shared-files', { field: props.fieldKey });
  });
}

function refreshSharedFiles() {
  emit('request-shared-files', { field: props.fieldKey, force: true });
}

function handleLocalFileUpload(event: Event) {
  const target = event.target as HTMLInputElement;
  const file = target.files?.[0];
  if (!file) {
    return;
  }

  const reader = new FileReader();
  reader.onload = (loadEvent) => {
    const result = loadEvent.target?.result;
    if (typeof result === 'string') {
      emit('update:value', result);
    }
  };
  reader.readAsText(file);
  target.value = '';
}

function handleSharedFileSelect(file: SharedDataFileEntry) {
  pickerOpen.value = false;
  emit('select-shared-file', {
    field: props.fieldKey,
    relativePath: file.relativePath,
  });
}

function setSourceChooserOpen(value: boolean) {
  sourceChooserOpen.value = value;
}
</script>

<template>
  <div class="min-w-0 rounded-[10px] border border-border/60 bg-background/80 px-4 py-4 sm:px-5 sm:py-5">
    <div class="flex items-start justify-between gap-3">
      <div class="min-w-0 grid gap-1">
        <Label :for="id" class="text-sm font-medium">{{ label }}</Label>
      </div>
    </div>

    <input
      ref="localFileInput"
      type="file"
      :accept="accept"
      class="hidden"
      @change="handleLocalFileUpload"
    />

    <div class="mt-4 flex flex-wrap items-center gap-3">
      <Button
        type="button"
        variant="outline"
        class="h-10 rounded-full border-border/70 bg-background/80 px-4 text-sm shadow-none"
        :disabled="sharedFileSelecting"
        @click="openSourceChooser"
      >
        <Upload class="mr-2 h-4 w-4" />
        上传文件
      </Button>
    </div>

    <Textarea
      :id="id"
      :model-value="value"
      @update:model-value="(next) => emit('update:value', String(next))"
      :placeholder="placeholder"
      wrap="soft"
      style="field-sizing: fixed;"
      class="mt-4 min-h-32 w-full min-w-0 max-w-full resize-y rounded-[10px] border-border/60 bg-muted/15 px-4 py-3 font-mono text-sm shadow-none"
    />

    <component
      :is="isMobileViewport ? Sheet : Dialog"
      :open="sourceChooserOpen"
      @update:open="setSourceChooserOpen"
    >
      <component
        :is="isMobileViewport ? SheetContent : DialogContent"
        v-bind="isMobileViewport ? { side: 'bottom' } : {}"
        :class="isMobileViewport
          ? 'flex flex-col gap-0 rounded-t-[10px] border-x-0 border-b-0 bg-background/98 px-0 pb-0'
          : 'border-border/60 bg-background/98 sm:max-w-[420px]'"
      >
        <component
          :is="isMobileViewport ? SheetHeader : DialogHeader"
          class="px-6 pb-0 pt-6"
        >
          <component :is="isMobileViewport ? SheetTitle : DialogTitle">选择文件来源</component>
          <component :is="isMobileViewport ? SheetDescription : DialogDescription">
            先选择导入方式，再读取 {{ label }} 文件。
          </component>
        </component>

        <div class="grid gap-3 px-6 py-5">
          <button
            type="button"
            class="flex w-full items-center justify-between rounded-[10px] border border-border/60 bg-background px-4 py-4 text-left transition-colors hover:bg-muted/15"
            @click="chooseLocalUpload"
          >
            <div class="flex min-w-0 items-start gap-3">
              <component
                :is="isMobileViewport ? Smartphone : Laptop"
                class="mt-0.5 h-4 w-4 shrink-0 text-muted-foreground"
              />
              <div class="grid gap-1">
                <span class="text-sm font-medium">{{ uploadLabel }}</span>
                <span class="text-xs leading-5 text-muted-foreground">
                  从设备中选择 {{ supportedTypesLabel }} 文件并自动读取
                </span>
              </div>
            </div>
            <ChevronRight class="h-4 w-4 shrink-0 text-muted-foreground" />
          </button>

          <button
            type="button"
            class="flex w-full items-center justify-between rounded-[10px] border border-border/60 bg-background px-4 py-4 text-left transition-colors hover:bg-muted/15"
            @click="chooseSharedFile"
          >
            <div class="flex min-w-0 items-start gap-3">
              <FolderTree class="mt-0.5 h-4 w-4 shrink-0 text-muted-foreground" />
              <div class="grid gap-1">
                <span class="text-sm font-medium">从飞牛中选择</span>
                <span class="text-xs leading-5 text-muted-foreground">
                  从 {{ shareName }} 根目录及三层以内读取已有文件
                </span>
              </div>
            </div>
            <ChevronRight class="h-4 w-4 shrink-0 text-muted-foreground" />
          </button>
        </div>

        <component
          :is="isMobileViewport ? SheetFooter : DialogFooter"
          class="border-t border-border/50 bg-background/95 px-6 py-4"
        >
          <Button type="button" variant="outline" @click="sourceChooserOpen = false">取消</Button>
        </component>
      </component>
    </component>

    <DataShareFilePicker
      v-model:open="pickerOpen"
      :title="`从飞牛中选择${label}`"
      :description="`请先将证书文件移动到 应用数据-> fn-knock目录下`"
      :share-name="shareName"
      :files="sharedFiles"
      :supported-file-types="supportedFileTypes"
      :available="sharedFilesAvailable"
      :loading="sharedFilesLoading"
      :selecting="sharedFileSelecting"
      :error-message="sharedFilesError"
      confirm-text="读取此文件"
      @refresh="refreshSharedFiles"
      @select="handleSharedFileSelect"
    />
  </div>
</template>
