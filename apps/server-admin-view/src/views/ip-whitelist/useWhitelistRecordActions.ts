import { ref, type ComputedRef, type Ref } from "vue";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { toast } from "@admin-shared/utils/toast";
import { WhitelistAPI, type WhiteListRecord } from "../../lib/api";

type Translate = (key: string, params?: Record<string, unknown>) => string;

interface UseWhitelistRecordActionsOptions {
  currentPage: Ref<number>;
  fetchRecords: () => Promise<unknown>;
  paginatedRecords: ComputedRef<WhiteListRecord[]>;
  records: Ref<WhiteListRecord[]>;
  translate: Translate;
}

export function useWhitelistRecordActions({
  currentPage,
  fetchRecords,
  paginatedRecords,
  records,
  translate,
}: UseWhitelistRecordActionsOptions) {
  const removingId = ref<string | null>(null);
  const removingRegionGroupId = ref<string | null>(null);
  const refreshingId = ref<string | null>(null);

  const { run: runRemoveRecord } = useAsyncAction({
    onError: (error) => {
      toast.error(translate("admin.ipWhitelist.networkDeleteTitle"), {
        description: extractErrorMessage(
          error,
          translate("admin.ipWhitelist.deleteFailed"),
        ),
      });
    },
  });
  const { run: runRemoveRegionGroup } = useAsyncAction({
    onError: (error) => {
      toast.error(translate("admin.ipWhitelist.networkRegionDeleteTitle"), {
        description: extractErrorMessage(
          error,
          translate("admin.ipWhitelist.regionGroupDeleteFailed"),
        ),
      });
    },
  });
  const { run: runRefreshRecord } = useAsyncAction({
    onError: (error) => {
      toast.error(translate("admin.ipWhitelist.networkRefreshTitle"), {
        description: extractErrorMessage(
          error,
          translate("admin.ipWhitelist.refreshFailed"),
        ),
      });
    },
  });
  const { run: runSaveComment } = useAsyncAction({ rethrow: true });

  function replaceRecord(nextRecord: WhiteListRecord) {
    const index = records.value.findIndex(
      (record) => record.id === nextRecord.id,
    );
    if (index < 0) return;
    records.value.splice(index, 1, nextRecord);
  }

  async function removeRegionGroup(id: string) {
    removingRegionGroupId.value = id;
    await runRemoveRegionGroup(
      async () => {
        const response = await WhitelistAPI.deleteRegion(id);
        if (response.success) {
          toast.success(translate("admin.ipWhitelist.regionGroupDeleteSuccess"));
          await fetchRecords();
        } else {
          toast.error(translate("admin.ipWhitelist.regionGroupDeleteFailed"), {
            description: response.message ?? undefined,
          });
        }
      },
      {
        onFinally: () => {
          removingRegionGroupId.value = null;
        },
      },
    );
  }

  async function removeRecord(id: string) {
    removingId.value = id;
    await runRemoveRecord(
      async () => {
        const response = await WhitelistAPI.deleteRecord(id);
        if (response.success) {
          toast.success(translate("admin.ipWhitelist.deleteSuccess"));
          await fetchRecords();
          if (paginatedRecords.value.length === 1 && currentPage.value > 1) {
            currentPage.value--;
          }
        } else {
          toast.error(translate("admin.ipWhitelist.deleteFailed"), {
            description: response.message ?? undefined,
          });
        }
      },
      {
        onFinally: () => {
          removingId.value = null;
        },
      },
    );
  }

  async function refreshRecord(id: string) {
    refreshingId.value = id;
    await runRefreshRecord(
      async () => {
        const response = await WhitelistAPI.refreshRecord(id);
        const result = response.data;
        const nextRecord = result?.record;
        if (nextRecord) {
          replaceRecord(nextRecord);
        }

        if (
          !response.success ||
          !result ||
          !nextRecord ||
          nextRecord.resolveStatus === "error"
        ) {
          toast.error(translate("admin.ipWhitelist.refreshFailed"), {
            description:
              response.message ||
              nextRecord?.resolveMessage ||
              translate("admin.ipWhitelist.refreshFallbackError"),
          });
          return;
        }

        toast.success(translate("admin.ipWhitelist.refreshSuccessTitle"), {
          description: result.changed
            ? translate("admin.ipWhitelist.refreshChanged")
            : translate("admin.ipWhitelist.refreshUnchanged"),
        });
      },
      {
        onFinally: () => {
          refreshingId.value = null;
        },
      },
    );
  }

  async function saveComment(id: string, newComment: string) {
    const record = records.value.find((item) => item.id === id);
    if (record && (record.comment || "") === newComment) return;

    await runSaveComment(() => WhitelistAPI.updateComment(id, newComment), {
      onSuccess: (response) => {
        if (!response.success) {
          throw new Error(
            response.message ||
              translate("admin.ipWhitelist.commentUpdateFailed"),
          );
        }
        if (record) record.comment = newComment;
        toast.success(translate("admin.ipWhitelist.commentUpdated"));
      },
      onError: (error) => {
        throw new Error(
          extractErrorMessage(
            error,
            translate("admin.ipWhitelist.commentUpdateFailed"),
          ),
        );
      },
    });
  }

  return {
    refreshRecord,
    refreshingId,
    removeRecord,
    removeRegionGroup,
    removingId,
    removingRegionGroupId,
    saveComment,
  };
}
