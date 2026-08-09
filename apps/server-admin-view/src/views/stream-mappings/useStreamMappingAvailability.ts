import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { extractErrorMessage } from "@admin-shared/composables/useAsyncAction";
import { toast } from "@admin-shared/utils/toast";
import {
  formatDailyAvailabilityWindow,
  getAvailabilityWindowValidationError,
  isAvailabilityWindowOpen,
  normalizeDailyAvailability,
} from "../../lib/daily-availability";
import { SystemAPI } from "../../lib/api";
import { useConfigStore } from "../../store/config";
import { useSystemClockStore } from "../../store/systemClock";
import type { DailyAvailability } from "../../types";

export function useStreamMappingAvailability() {
  const configStore = useConfigStore();
  const systemClockStore = useSystemClockStore();
  const { t } = useI18n();
  const isAvailabilityDialogOpen = ref(false);
  const isSavingAvailability = ref(false);
  const availabilityFormEnabled = ref(false);
  const availabilityFormStartTime = ref("09:00");
  const availabilityFormEndTime = ref("18:00");
  const scheduleClock = ref(Date.now());
  const serverClockAnchor = ref<{
    browserTimeMs: number;
    serverTimeMs: number;
  } | null>(null);
  let scheduleClockTimer: number | null = null;

  const protocolMappingAvailability = computed(() =>
    normalizeDailyAvailability(
      configStore.config?.protocol_mapping_feature?.availability,
    ),
  );
  const scheduleWindow = computed(() =>
    formatDailyAvailabilityWindow(protocolMappingAvailability.value),
  );
  const estimatedServerNow = computed(() => {
    const anchor = serverClockAnchor.value;
    if (!anchor) return new Date(scheduleClock.value);
    return new Date(
      anchor.serverTimeMs +
        Math.max(0, scheduleClock.value - anchor.browserTimeMs),
    );
  });
  const scheduleTimeZone = computed(
    () =>
      systemClockStore.status?.systemTimeZone ||
      systemClockStore.status?.expectedTimeZone ||
      null,
  );
  const scheduleState = computed<"open" | "closed" | null>(() => {
    const availability = protocolMappingAvailability.value;
    if (!availability) return null;
    return isAvailabilityWindowOpen(
      availability,
      estimatedServerNow.value,
      scheduleTimeZone.value,
    )
      ? "open"
      : "closed";
  });
  const availabilityValidationMessage = computed(() => {
    if (!availabilityFormEnabled.value) return "";
    const error = getAvailabilityWindowValidationError(
      availabilityFormStartTime.value.trim(),
      availabilityFormEndTime.value.trim(),
    );
    if (error === "invalid_time") {
      return t("admin.streamMappings.availabilityInvalidTime");
    }
    if (error === "same_time") {
      return t("admin.streamMappings.availabilitySameTimeInvalid");
    }
    return "";
  });

  watch(
    () => systemClockStore.status?.systemTimeMs,
    (systemTimeMs) => {
      if (typeof systemTimeMs !== "number" || !Number.isFinite(systemTimeMs)) {
        serverClockAnchor.value = null;
        return;
      }
      serverClockAnchor.value = {
        browserTimeMs: Date.now(),
        serverTimeMs: systemTimeMs,
      };
      scheduleClock.value = Date.now();
    },
    { immediate: true },
  );

  onMounted(() => {
    void systemClockStore.initialize().then(() => {
      if (!systemClockStore.status) {
        void systemClockStore.loadStatus(true);
      }
    });
    scheduleClockTimer = window.setInterval(() => {
      scheduleClock.value = Date.now();
    }, 60_000);
  });

  onUnmounted(() => {
    if (scheduleClockTimer !== null) {
      window.clearInterval(scheduleClockTimer);
    }
  });

  function openAvailabilityDialog() {
    const availability = protocolMappingAvailability.value;
    availabilityFormEnabled.value = availability !== null;
    availabilityFormStartTime.value = availability?.start_time || "09:00";
    availabilityFormEndTime.value = availability?.end_time || "18:00";
    isAvailabilityDialogOpen.value = true;
  }

  function closeAvailabilityDialog() {
    isAvailabilityDialogOpen.value = false;
  }

  function handleAvailabilityDialogOpenChange(open: boolean) {
    if (!open) closeAvailabilityDialog();
  }

  async function saveAvailability() {
    if (isSavingAvailability.value || availabilityValidationMessage.value)
      return;
    const availability: DailyAvailability | null = availabilityFormEnabled.value
      ? {
          enabled: true,
          start_time: availabilityFormStartTime.value.trim(),
          end_time: availabilityFormEndTime.value.trim(),
        }
      : null;

    isSavingAvailability.value = true;
    try {
      const updated = await SystemAPI.updateProtocolMappingFeatureConfig({
        availability,
      });
      if (configStore.config) {
        configStore.config = {
          ...configStore.config,
          protocol_mapping_feature: updated,
        };
      }
      await configStore.loadConfig({ force: true });
      toast.success(
        availability
          ? t("admin.streamMappings.availabilitySaved")
          : t("admin.streamMappings.availabilityCleared"),
      );
      closeAvailabilityDialog();
    } catch (error: any) {
      toast.error(t("admin.streamMappings.availabilitySaveFailed"), {
        description: extractErrorMessage(error, t("common.tryLater")),
      });
    } finally {
      isSavingAvailability.value = false;
    }
  }

  return {
    availabilityFormEnabled,
    availabilityFormEndTime,
    availabilityFormStartTime,
    availabilityValidationMessage,
    closeAvailabilityDialog,
    handleAvailabilityDialogOpenChange,
    isAvailabilityDialogOpen,
    isSavingAvailability,
    openAvailabilityDialog,
    saveAvailability,
    scheduleState,
    scheduleWindow,
  };
}
