<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import type { components as ApiContractComponents } from "@fn-knock/api-contract";
import BinaryDownloadCard from "@admin-shared/components/system/BinaryDownloadCard.vue";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { usePollingResourceStatus } from "@admin-shared/composables/usePollingResourceStatus";
import { toast } from "@admin-shared/utils/toast";

type ResourceDownloadStatus =
  ApiContractComponents["schemas"]["SystemAssetDownloadProgressData"]["status"];

type ResourceStatusPayload =
  | ApiContractComponents["schemas"]["CloudflaredAssetStatusData"]
  | ApiContractComponents["schemas"]["FrpAssetStatusData"];

type ResourceApiResponse<T = unknown> = Omit<
  ApiContractComponents["schemas"]["ApiSuccessEnvelope"],
  "data"
> & { data?: T };

const props = withDefaults(
  defineProps<{
    allowManagePlatforms?: string[];
    cancelDownload: () => Promise<ResourceApiResponse>;
    deleteResource: () => Promise<ResourceApiResponse>;
    fetchStatus: (
      signal?: AbortSignal,
    ) => Promise<ResourceApiResponse<ResourceStatusPayload>>;
    messageKeyPrefix: string;
    startDownload: () => Promise<ResourceApiResponse>;
  }>(),
  {
    allowManagePlatforms: () => [],
  },
);

const { t } = useI18n();

const supported = ref(false);
const platform = ref("unsupported");
const downloaded = ref(false);
const status = ref<ResourceDownloadStatus>("idle");
const percent = ref(0);
const error = ref("");
const message = (key: string) => t(`${props.messageKeyPrefix}.${key}`);

const allowManage = computed(
  () =>
    props.allowManagePlatforms.length === 0 ||
    props.allowManagePlatforms.includes(platform.value),
);

const { run: runStartDownload } = useAsyncAction({
  onError: (error) => {
    toast.error(extractErrorMessage(error, message("startDownloadFailed")));
  },
});
const { run: runDeleteResource } = useAsyncAction({
  onError: (error) => {
    toast.error(extractErrorMessage(error, message("deleteFailed")));
  },
});
const { isPending: isCancelling, run: runCancelDownload } = useAsyncAction({
  onError: (error) => {
    toast.error(extractErrorMessage(error, message("cancelFailed")));
  },
});

const { isInitializing, refresh: refreshStatus } = usePollingResourceStatus({
  fetcher: props.fetchStatus,
  onData: (res) => {
    if (!res.success || !res.data) return;
    supported.value = res.data.supported;
    platform.value = res.data.platform;
    downloaded.value = res.data.downloaded;
    status.value = res.data.progress?.status || "idle";
    percent.value = res.data.progress?.percent || 0;
    error.value = res.data.progress?.error || "";
  },
  isDownloading: (res) =>
    Boolean(res.success && res.data?.progress?.status === "downloading"),
});

const startResourceDownload = async () => {
  await runStartDownload(async () => {
    error.value = "";
    const res = await props.startDownload();
    if (res.success) {
      toast.success(message("downloadStarted"));
      await refreshStatus();
      return;
    }
    toast.error(res.message || message("startDownloadFailed"));
  });
};

const deleteManagedResource = async () => {
  await runDeleteResource(async () => {
    const res = await props.deleteResource();
    if (res.success) {
      toast.success(message("deleted"));
      await refreshStatus();
      return;
    }
    toast.error(res.message || message("deleteFailed"));
  });
};

const cancelResourceDownload = async () => {
  await runCancelDownload(async () => {
    const res = await props.cancelDownload();
    if (res.success) {
      toast.info(message("cancelRequested"));
      await refreshStatus();
      return;
    }
    toast.error(res.message || message("cancelFailed"));
  });
};
</script>

<template>
  <BinaryDownloadCard
    :title="message('title')"
    :description="message('description')"
    :is-initializing="isInitializing"
    :supported="supported"
    :platform="platform"
    :downloaded="downloaded"
    :status="status"
    :percent="percent"
    :error="error"
    :is-cancelling="isCancelling"
    :allow-manage="allowManage"
    :ready-label="message('readyLabel')"
    :pending-label="message('pendingLabel')"
    :download-button-text="message('downloadButton')"
    :downloading-text="message('downloading')"
    :redownload-confirm-title="message('redownloadConfirmTitle')"
    :redownload-confirm-description="message('redownloadConfirmDescription')"
    :delete-confirm-title="message('deleteConfirmTitle')"
    :delete-confirm-description="message('deleteConfirmDescription')"
    @start="startResourceDownload"
    @cancel="cancelResourceDownload"
    @redownload="startResourceDownload"
    @delete="deleteManagedResource"
  />
</template>
