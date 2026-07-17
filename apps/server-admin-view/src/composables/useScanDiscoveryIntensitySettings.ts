import {
  computed,
  onBeforeUnmount,
  ref,
  toValue,
  type MaybeRefOrGetter,
} from "vue";
import { useI18n } from "vue-i18n";
import { toast } from "@admin-shared/utils/toast";
import {
  ScanAPI,
  type ScanDiscoverySettings,
  type ScanIntensityLevel,
} from "@/lib/api";

type UseScanDiscoveryIntensitySettingsOptions = {
  disabled: MaybeRefOrGetter<boolean>;
  onSaved?: (settings: ScanDiscoverySettings) => void;
};

const levelOrder: ScanIntensityLevel[] = ["low", "medium", "high", "extreme"];
const concurrencyByLevel: Record<ScanIntensityLevel, number> = {
  low: 32,
  medium: 115,
  high: 256,
  extreme: 512,
};
const levelPositions = [16, 50, 70, 100] as const;

function levelIndexFromPosition(position: number) {
  if (position < 33) return 0;
  if (position < 66) return 1;
  if (position < 100) return 2;
  return 3;
}

export function useScanDiscoveryIntensitySettings({
  disabled,
  onSaved,
}: UseScanDiscoveryIntensitySettingsOptions) {
  const { t } = useI18n();
  const loading = ref(false);
  const saving = ref(false);
  const automatic = ref(true);
  const manualIndex = ref(1);
  const recommendedIndex = ref(1);
  const sliderPosition = ref(50);
  const safeConcurrency = ref<number | null>(null);

  const displayedIndex = computed(() =>
    levelIndexFromPosition(sliderPosition.value),
  );
  const displayedOption = computed(() => {
    const value = levelOrder[displayedIndex.value] ?? "medium";
    return {
      value,
      concurrency: concurrencyByLevel[value],
      label: t(`admin.scanIntensity.levels.${value}`),
    };
  });
  const currentEffectiveConcurrency = computed(() =>
    Math.min(
      displayedOption.value.concurrency,
      safeConcurrency.value ?? displayedOption.value.concurrency,
    ),
  );
  const currentConcurrencyText = computed(() =>
    t("admin.scanIntensity.effectiveConcurrency", {
      count: currentEffectiveConcurrency.value,
    }),
  );

  let pendingSaveTimer: number | null = null;
  let saveRevision = 0;

  async function loadSettings() {
    loading.value = true;
    try {
      const payload = await ScanAPI.getDiscoverSettings();
      automatic.value = payload.intensityMode === "auto";
      manualIndex.value = Math.max(
        0,
        levelOrder.indexOf(payload.configuredLevel),
      );
      recommendedIndex.value = Math.max(
        0,
        levelOrder.indexOf(payload.recommendedLevel),
      );
      safeConcurrency.value = payload.capability.safeConcurrency;
      sliderPosition.value =
        levelPositions[
          automatic.value ? recommendedIndex.value : manualIndex.value
        ] ?? 50;
    } catch (error) {
      toast.error(t("admin.scanIntensity.loadFailed"), {
        description: error instanceof Error ? error.message : undefined,
      });
    } finally {
      loading.value = false;
    }
  }

  async function persistSettings(mode: "auto" | "manual") {
    const revision = ++saveRevision;
    saving.value = true;
    try {
      const payload = await ScanAPI.saveDiscoverSettings({
        intensity_mode: mode,
        intensity_level: levelOrder[manualIndex.value] ?? "medium",
      });
      if (revision !== saveRevision) return;
      automatic.value = payload.intensityMode === "auto";
      manualIndex.value = Math.max(
        0,
        levelOrder.indexOf(payload.configuredLevel),
      );
      recommendedIndex.value = Math.max(
        0,
        levelOrder.indexOf(payload.recommendedLevel),
      );
      safeConcurrency.value = payload.capability.safeConcurrency;
      if (automatic.value) {
        sliderPosition.value = levelPositions[recommendedIndex.value] ?? 50;
      }
      onSaved?.(payload);
    } catch (error) {
      if (revision !== saveRevision) return;
      toast.error(t("admin.scanIntensity.saveFailed"), {
        description: error instanceof Error ? error.message : undefined,
      });
    } finally {
      if (revision === saveRevision) saving.value = false;
    }
  }

  function handleSliderInput(event: Event) {
    const value = Number((event.target as HTMLInputElement).value);
    sliderPosition.value = Number.isFinite(value)
      ? Math.min(100, Math.max(0, Math.round(value)))
      : 50;
    manualIndex.value = levelIndexFromPosition(sliderPosition.value);
    automatic.value = false;
    scheduleManualSave();
  }

  function scheduleManualSave() {
    clearPendingSave();
    pendingSaveTimer = window.setTimeout(() => {
      pendingSaveTimer = null;
      void persistSettings("manual");
    }, 220);
  }

  function flushManualSave() {
    if (pendingSaveTimer == null) return;
    clearPendingSave();
    void persistSettings("manual");
  }

  function clearPendingSave() {
    if (pendingSaveTimer == null) return;
    window.clearTimeout(pendingSaveTimer);
    pendingSaveTimer = null;
  }

  function restoreAutomaticMode() {
    if (automatic.value || loading.value || saving.value || toValue(disabled)) {
      return;
    }
    clearPendingSave();
    automatic.value = true;
    sliderPosition.value = levelPositions[recommendedIndex.value] ?? 50;
    void persistSettings("auto");
  }

  onBeforeUnmount(clearPendingSave);

  return {
    loading,
    saving,
    automatic,
    sliderPosition,
    safeConcurrency,
    displayedIndex,
    displayedOption,
    currentEffectiveConcurrency,
    currentConcurrencyText,
    loadSettings,
    handleSliderInput,
    flushManualSave,
    clearPendingSave,
    restoreAutomaticMode,
  };
}
