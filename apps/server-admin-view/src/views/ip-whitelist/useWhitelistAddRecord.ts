import { computed, ref, type Ref } from "vue";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { isValidCIDR } from "@admin-shared/utils/cidr";
import { toast } from "@admin-shared/utils/toast";
import { WhitelistAPI, type GatewayVisibilitySelection } from "../../lib/api";

type Translate = (
  key: string,
  params?: Record<string, string | number>,
) => string;

interface UseWhitelistAddRecordOptions {
  currentPage: Ref<number>;
  fetchRecords: () => Promise<unknown>;
  searchQuery: Ref<string>;
  translate: Translate;
}

export function useWhitelistAddRecord({
  currentPage,
  fetchRecords,
  searchQuery,
  translate,
}: UseWhitelistAddRecordOptions) {
  const showAddDialog = ref(false);
  const durationSetting = ref("permanent");
  const customHours = ref(24);
  const newRecord = ref({
    ip: "",
    targetType: "ip" as "ip" | "cidr" | "cname",
    checkIntervalMinutes: 5,
    comment: "",
  });
  const cidrInputMode = ref<"manual" | "region">("manual");
  const whitelistRegionSelections = ref<GatewayVisibilitySelection[]>([]);

  const { isPending: isSaving, run: runAddRecord } = useAsyncAction({
    onError: (error) => {
      toast.error(translate("admin.ipWhitelist.networkAddTitle"), {
        description: extractErrorMessage(
          error,
          translate("admin.ipWhitelist.addFailed"),
        ),
      });
    },
  });
  const newRecordPlaceholder = computed(() =>
    newRecord.value.targetType === "cidr"
      ? translate("admin.ipWhitelist.placeholderCidr")
      : newRecord.value.targetType === "cname"
        ? translate("admin.ipWhitelist.placeholderCname")
        : translate("admin.ipWhitelist.placeholderIp"),
  );
  const isRegionCidrMode = computed(
    () =>
      newRecord.value.targetType === "cidr" && cidrInputMode.value === "region",
  );
  const regionInputsDisabled = computed(
    () => isSaving.value || !isRegionCidrMode.value,
  );
  const canSaveNewRecord = computed(() =>
    isRegionCidrMode.value
      ? whitelistRegionSelections.value.length > 0
      : Boolean(newRecord.value.ip.trim()),
  );

  function getNewRecordExpireAt() {
    if (durationSetting.value === "permanent") return null;

    const now = Math.floor(Date.now() / 1000);
    const durationHours =
      durationSetting.value === "1h"
        ? 1
        : durationSetting.value === "24h"
          ? 24
          : durationSetting.value === "7d"
            ? 24 * 7
            : customHours.value || 1;
    return now + durationHours * 3600;
  }

  function resetAddForm() {
    newRecord.value = {
      ip: "",
      targetType: "ip",
      checkIntervalMinutes: 5,
      comment: "",
    };
    cidrInputMode.value = "manual";
    whitelistRegionSelections.value = [];
    durationSetting.value = "permanent";
    customHours.value = 24;
  }

  async function completeAdd() {
    showAddDialog.value = false;
    resetAddForm();
    currentPage.value = 1;
    searchQuery.value = "";
    await fetchRecords();
  }

  async function addRecord() {
    if (isRegionCidrMode.value) {
      if (whitelistRegionSelections.value.length === 0) {
        toast.error(translate("admin.ipWhitelist.regionRequiredTitle"), {
          description: translate("admin.ipWhitelist.regionRequiredDescription"),
        });
        return;
      }

      const expireAt = getNewRecordExpireAt();
      const comment = newRecord.value.comment.trim() || undefined;
      await runAddRecord(async () => {
        const response = await WhitelistAPI.addRegions({
          regions: whitelistRegionSelections.value.map((item) => ({
            province: item.province,
            query_city: item.query_city,
          })),
          expireAt,
          ...(comment ? { comment } : {}),
        });

        if (!response.success || !response.data) {
          toast.error(translate("admin.ipWhitelist.addFailed"), {
            description: response.message,
          });
          return;
        }

        toast.success(translate("admin.ipWhitelist.addRegionsSuccess"), {
          description: translate("admin.ipWhitelist.addRegionsResult", {
            regions: response.data.group.regions.length,
            total: response.data.total,
          }),
        });
        await completeAdd();
      });
      return;
    }

    const ip = newRecord.value.ip.trim();
    if (!ip) return;
    if (newRecord.value.targetType === "cidr" && !isValidCIDR(ip)) {
      toast.error(translate("admin.ipWhitelist.invalidCidrTitle"), {
        description: translate("admin.ipWhitelist.invalidCidrDescription"),
      });
      return;
    }
    if (
      newRecord.value.targetType === "cname" &&
      (!Number.isFinite(newRecord.value.checkIntervalMinutes) ||
        newRecord.value.checkIntervalMinutes < 1)
    ) {
      toast.error(translate("admin.ipWhitelist.invalidIntervalTitle"), {
        description: translate("admin.ipWhitelist.invalidIntervalDescription"),
      });
      return;
    }

    await runAddRecord(async () => {
      const response = await WhitelistAPI.addRecord({
        ip,
        targetType: newRecord.value.targetType,
        expireAt: getNewRecordExpireAt(),
        source: "manual",
        comment: newRecord.value.comment.trim() || undefined,
        checkIntervalMinutes:
          newRecord.value.targetType === "cname"
            ? Math.floor(newRecord.value.checkIntervalMinutes || 5)
            : undefined,
      });
      if (!response.success) {
        toast.error(translate("admin.ipWhitelist.addFailed"), {
          description: response.message,
        });
        return;
      }

      toast.success(translate("admin.ipWhitelist.addSuccess"));
      await completeAdd();
    });
  }

  return {
    addRecord,
    canSaveNewRecord,
    cidrInputMode,
    customHours,
    durationSetting,
    isRegionCidrMode,
    isSaving,
    newRecord,
    newRecordPlaceholder,
    regionInputsDisabled,
    showAddDialog,
    whitelistRegionSelections,
  };
}
