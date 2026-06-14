<script setup lang="ts">
import { useI18n } from 'vue-i18n';
import { Button } from '@/components/ui/button';
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover';
import { Progress } from '@/components/ui/progress';
import { Skeleton } from '@/components/ui/skeleton';
import ResourceStatusCard from '@admin-shared/components/system/ResourceStatusCard.vue';

const { t } = useI18n();

const props = withDefaults(
  defineProps<{
    title: string;
    description: string;
    isInitializing: boolean;
    supported: boolean;
    platform: string;
    downloaded: boolean;
    status: 'idle' | 'downloading' | 'completed' | 'error';
    percent: number;
    error?: string;
    isCancelling?: boolean;
    allowManage?: boolean;
    readyLabel?: string;
    pendingLabel?: string;
    downloadButtonText?: string;
    downloadingText?: string;
    redownloadConfirmTitle?: string;
    redownloadConfirmDescription?: string;
    deleteConfirmTitle?: string;
    deleteConfirmDescription?: string;
  }>(),
  {
    error: '',
    isCancelling: false,
    allowManage: true,
  },
);

const emit = defineEmits<{
  start: [];
  cancel: [];
  redownload: [];
  delete: [];
}>();
</script>

<template>
  <ResourceStatusCard
    :title="props.title"
    :description="props.description"
    :is-initializing="props.isInitializing"
  >
    <template #initial>
      <div class="grid grid-cols-1 md:grid-cols-3 gap-4">
        <div class="border p-4 rounded-lg">
          <Skeleton class="h-4 w-20 mb-2" />
          <Skeleton class="h-5 w-28" />
          <Skeleton class="h-3 w-16 mt-2" />
        </div>
        <div class="border p-4 rounded-lg md:col-span-2">
          <div class="flex justify-between items-center">
            <Skeleton class="h-4 w-24" />
            <Skeleton class="h-5 w-16" />
          </div>
          <div class="mt-4">
            <Skeleton class="h-3 w-full" />
          </div>
        </div>
      </div>
    </template>

    <div class="grid grid-cols-1 md:grid-cols-3 gap-4">
      <div class="border p-4 rounded-lg">
        <div class="text-sm text-muted-foreground mb-2">
          {{ t('shared.binaryDownload.currentPlatform') }}
        </div>
        <div class="font-medium">{{ props.platform }}</div>
        <div class="mt-2 text-xs" :class="props.supported ? 'text-green-600' : 'text-red-500'">
          {{
            props.supported
              ? t('shared.binaryDownload.supported')
              : t('shared.binaryDownload.unsupported')
          }}
        </div>
      </div>
      <div class="border p-4 rounded-lg md:col-span-2">
        <div class="flex justify-between items-center">
          <div class="text-sm text-muted-foreground">
            {{ t('shared.binaryDownload.resourceStatus') }}
          </div>
          <div
            :class="[
              'px-2 py-0.5 rounded text-xs font-medium',
              props.downloaded
                ? 'bg-green-100 text-green-700 border border-green-200'
                : 'bg-yellow-100 text-yellow-700 border border-yellow-200',
            ]"
          >
            {{
              props.downloaded
                ? props.readyLabel ?? t('shared.binaryDownload.readyLabel')
                : props.pendingLabel ?? t('shared.binaryDownload.pendingLabel')
            }}
          </div>
        </div>
        <div v-if="props.status === 'downloading'" class="mt-4">
          <div class="flex justify-between text-sm mb-2 text-muted-foreground font-medium">
            <span>{{ t('shared.binaryDownload.downloadProgress') }}</span>
            <span>{{ props.percent }}%</span>
          </div>
          <Progress :model-value="props.percent" />
        </div>
        <div
          v-if="props.error"
          class="text-sm bg-destructive/10 text-destructive p-3 rounded-md border border-destructive/20 mt-3"
        >
          {{ t('shared.binaryDownload.errorPrefix') }}{{ props.error }}
        </div>
      </div>
    </div>

    <template #footer>
      <template v-if="props.status !== 'downloading'">
        <Button
          v-if="!props.downloaded && props.allowManage"
          @click="emit('start')"
          :disabled="!props.supported"
        >
          {{ props.downloadButtonText ?? t('shared.binaryDownload.downloadButton') }}
        </Button>
        <div v-else-if="props.allowManage" class="flex gap-3">
          <Popover v-slot="{ close }">
            <PopoverTrigger as-child>
              <Button variant="outline">
                {{ t('shared.binaryDownload.redownload') }}
              </Button>
            </PopoverTrigger>
            <PopoverContent class="w-72 text-left">
              <div class="grid gap-3">
                <p class="text-sm font-medium">
                  {{
                    props.redownloadConfirmTitle ??
                    t('shared.binaryDownload.redownloadConfirmTitle')
                  }}
                </p>
                <p class="text-xs text-muted-foreground">
                  {{
                    props.redownloadConfirmDescription ??
                    t('shared.binaryDownload.redownloadConfirmDescription')
                  }}
                </p>
                <div class="flex justify-end gap-2">
                  <Button variant="outline" size="sm" @click="close">
                    {{ t('common.cancel') }}
                  </Button>
                  <Button
                    size="sm"
                    @click="
                      async () => {
                        emit('redownload');
                        close();
                      }
                    "
                  >
                    {{ t('shared.binaryDownload.confirmRedownload') }}
                  </Button>
                </div>
              </div>
            </PopoverContent>
          </Popover>
          <Popover v-slot="{ close }">
            <PopoverTrigger as-child>
              <Button variant="destructive">
                {{ t('shared.binaryDownload.delete') }}
              </Button>
            </PopoverTrigger>
            <PopoverContent class="w-72 text-left">
              <div class="grid gap-3">
                <p class="text-sm font-medium">
                  {{
                    props.deleteConfirmTitle ??
                    t('shared.binaryDownload.deleteConfirmTitle')
                  }}
                </p>
                <p class="text-xs text-muted-foreground">
                  {{
                    props.deleteConfirmDescription ??
                    t('shared.binaryDownload.deleteConfirmDescription')
                  }}
                </p>
                <div class="flex justify-end gap-2">
                  <Button variant="outline" size="sm" @click="close">
                    {{ t('common.cancel') }}
                  </Button>
                  <Button
                    variant="destructive"
                    size="sm"
                    @click="
                      async () => {
                        emit('delete');
                        close();
                      }
                    "
                  >
                    {{ t('shared.binaryDownload.confirmDelete') }}
                  </Button>
                </div>
              </div>
            </PopoverContent>
          </Popover>
        </div>
      </template>
      <template v-else>
        <div class="text-sm text-muted-foreground animate-pulse flex items-center h-10 mr-auto">
          {{ props.downloadingText ?? t('shared.binaryDownload.downloading') }}
        </div>
        <Button variant="destructive" @click="emit('cancel')" :disabled="props.isCancelling">
          {{ t('shared.binaryDownload.cancelTask') }}
        </Button>
      </template>
    </template>
  </ResourceStatusCard>
</template>
