import { onMounted, onUnmounted, ref } from "vue";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { toast } from "@admin-shared/utils/toast";
import {
  WhitelistAPI,
  type WhiteListRecord,
  type WhitelistRegionGroupRecord,
} from "@/lib/api/whitelist";
import { createVisibilityPoller } from "@/composables/useVisibilityPolling";

type Translate = (key: string) => string;

export function useWhitelistRecords(translate: Translate) {
  const records = ref<WhiteListRecord[]>([]);
  const regionGroups = ref<WhitelistRegionGroupRecord[]>([]);
  const isInitializing = ref(true);

  const { isPending: loading, run: runFetchRecords } = useAsyncAction({
    onError: (error) => {
      toast.error(translate("admin.ipWhitelist.networkLoadTitle"), {
        description: extractErrorMessage(
          error,
          translate("admin.ipWhitelist.loadFailed"),
        ),
      });
    },
  });

  async function fetchRecords() {
    await runFetchRecords(
      async () => {
        const [recordsResponse, regionsResponse] = await Promise.all([
          WhitelistAPI.getRecords(),
          WhitelistAPI.getRegions(),
        ]);

        if (recordsResponse.success) {
          records.value = recordsResponse.data;
        } else {
          toast.error(translate("admin.ipWhitelist.getFailed"), {
            description: recordsResponse.message ?? undefined,
          });
        }

        if (regionsResponse.success && regionsResponse.data) {
          regionGroups.value = regionsResponse.data;
        } else {
          toast.error(translate("admin.ipWhitelist.regionGroupsLoadFailed"), {
            description: regionsResponse.message ?? undefined,
          });
        }
      },
      {
        onFinally: () => {
          isInitializing.value = false;
        },
      },
    );
  }

  const recordsPoller = createVisibilityPoller({
    intervalMs: 30_000,
    task: fetchRecords,
  });

  onMounted(() => {
    recordsPoller.start();
  });

  onUnmounted(recordsPoller.stop);

  return {
    fetchRecords,
    isInitializing,
    loading,
    records,
    regionGroups,
  };
}
