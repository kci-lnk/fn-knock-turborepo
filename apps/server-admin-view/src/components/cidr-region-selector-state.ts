import { computed, reactive, ref, watch, type Ref } from "vue";
import type {
  CidrCityOption,
  CidrProvinceOption,
  GatewayVisibilitySelection,
} from "@/types";
import { getCidrRegionSelectionKey } from "@/types/cidr";

export interface CidrRegionSelectorStateOptions {
  disabled: Readonly<Ref<boolean>>;
  formatLoadError: (error: unknown) => string;
  loadCities: (province: string) => Promise<{
    defaultValue?: string | null;
    options: CidrCityOption[];
  }>;
  loadProvinces: () => Promise<{ options: CidrProvinceOption[] }>;
  onLoadError: (description: string) => void;
  selections: Ref<GatewayVisibilitySelection[]>;
}

export interface CidrCityChoice {
  key: string;
  label: string;
  isProvinceWide: boolean;
  unavailable: boolean;
}

const selectionSetsEqual = (left: string[], right: string[]) => {
  if (left.length !== right.length) return false;
  const rightSet = new Set(right);
  return left.every((value) => rightSet.has(value));
};

export const createCidrRegionSelectorState = ({
  disabled,
  formatLoadError,
  loadCities,
  loadProvinces: fetchProvinces,
  onLoadError,
  selections,
}: CidrRegionSelectorStateOptions) => {
  const provinces = ref<CidrProvinceOption[]>([]);
  const cityOptions = ref<CidrCityOption[]>([]);
  const originalProvinceSelections = ref<GatewayVisibilitySelection[]>([]);
  const provincesLoading = ref(false);
  const cityOptionsLoading = ref(false);
  const cityOptionsReady = ref(false);
  const provincesLoadError = ref("");
  const isDialogOpen = ref(false);
  const draft = reactive({
    province: "",
    cityValues: [] as string[],
  });
  let cityRequestToken = 0;

  const keyForOption = (option: CidrCityOption) =>
    getCidrRegionSelectionKey({
      province: draft.province,
      query_city: option.queryCity,
    });

  const cityChoices = computed<CidrCityChoice[]>(() => {
    const choices = cityOptions.value.map((option) => ({
      key: keyForOption(option),
      label: option.label,
      isProvinceWide: option.isProvinceWide,
      unavailable: false,
    }));
    const availableKeys = new Set(choices.map((choice) => choice.key));

    for (const selection of originalProvinceSelections.value) {
      const key = getCidrRegionSelectionKey(selection);
      if (availableKeys.has(key)) continue;
      availableKeys.add(key);
      choices.push({
        key,
        label: selection.label,
        isProvinceWide: selection.is_province_wide || !selection.query_city,
        unavailable: true,
      });
    }

    return choices;
  });
  const selectedCityCount = computed(() => draft.cityValues.length);
  const originalCityValues = computed(() => [
    ...new Set(
      originalProvinceSelections.value.map((selection) =>
        getCidrRegionSelectionKey(selection),
      ),
    ),
  ]);
  const hasDraftChanges = computed(
    () =>
      cityOptionsReady.value &&
      !selectionSetsEqual(draft.cityValues, originalCityValues.value),
  );
  const canSaveSelections = computed(
    () =>
      !disabled.value &&
      Boolean(draft.province) &&
      cityOptionsReady.value &&
      !cityOptionsLoading.value &&
      hasDraftChanges.value,
  );

  const reportLoadError = (error: unknown) => {
    const description = formatLoadError(error);
    onLoadError(description);
    return description;
  };

  const loadProvinces = async () => {
    if (provincesLoading.value || provinces.value.length > 0) return;
    provincesLoading.value = true;
    provincesLoadError.value = "";
    try {
      provinces.value = (await fetchProvinces()).options;
    } catch (error) {
      provincesLoadError.value = reportLoadError(error);
    } finally {
      provincesLoading.value = false;
    }
  };

  const clearCityDraft = () => {
    cityOptions.value = [];
    originalProvinceSelections.value = [];
    draft.cityValues = [];
    cityOptionsReady.value = false;
  };

  const clearDraft = () => {
    cityRequestToken += 1;
    cityOptionsLoading.value = false;
    draft.province = "";
    clearCityDraft();
  };

  const handleDialogOpenChange = (nextOpen: boolean) => {
    isDialogOpen.value = nextOpen;
    if (!nextOpen) clearDraft();
  };

  const openDialog = () => {
    if (disabled.value || provinces.value.length === 0) return;
    const preferredProvince =
      selections.value[0]?.province || provinces.value[0]?.value || "";
    clearDraft();
    isDialogOpen.value = true;
    selectProvince(preferredProvince);
  };

  const loadCityOptions = async (province: string) => {
    if (!province) {
      clearCityDraft();
      return;
    }

    const token = ++cityRequestToken;
    cityOptionsLoading.value = true;
    clearCityDraft();
    try {
      const payload = await loadCities(province);
      if (token !== cityRequestToken) return;
      cityOptions.value = payload.options;
      originalProvinceSelections.value = selections.value
        .filter((selection) => selection.province === province)
        .map((selection) => ({ ...selection }));
      const loadedCityValues = originalCityValues.value;
      const selectedProvinceWideChoice = cityChoices.value.find(
        (choice) =>
          choice.isProvinceWide && loadedCityValues.includes(choice.key),
      );
      draft.cityValues = selectedProvinceWideChoice
        ? [selectedProvinceWideChoice.key]
        : [...loadedCityValues];
      cityOptionsReady.value = true;
    } catch (error) {
      if (token !== cityRequestToken) return;
      clearCityDraft();
      reportLoadError(error);
    } finally {
      if (token === cityRequestToken) cityOptionsLoading.value = false;
    }
  };

  const selectProvince = (province: string) => {
    draft.province = province;
    void loadCityOptions(province);
  };

  const toggleCity = (key: string, checked: boolean) => {
    if (disabled.value || !cityOptionsReady.value) return;
    const choice = cityChoices.value.find((item) => item.key === key);
    if (!choice) return;

    if (!checked) {
      draft.cityValues = draft.cityValues.filter((value) => value !== key);
      return;
    }

    if (choice.isProvinceWide) {
      draft.cityValues = [key];
      return;
    }

    const provinceWideKeys = new Set(
      cityChoices.value
        .filter((item) => item.isProvinceWide)
        .map((item) => item.key),
    );
    draft.cityValues = [
      ...draft.cityValues.filter((value) => !provinceWideKeys.has(value)),
      ...(draft.cityValues.includes(key) ? [] : [key]),
    ];
  };

  const selectionFromChoice = (
    choice: CidrCityChoice,
  ): GatewayVisibilitySelection | null => {
    const option = cityOptions.value.find(
      (item) => keyForOption(item) === choice.key,
    );
    if (option) {
      return {
        province: draft.province,
        city: option.isProvinceWide ? null : option.label,
        label: option.label,
        value: option.value,
        query_city: option.queryCity,
        is_province_wide: option.isProvinceWide,
        is_municipality: option.isMunicipality,
      };
    }
    const existing = originalProvinceSelections.value.find(
      (item) => getCidrRegionSelectionKey(item) === choice.key,
    );
    return existing ? { ...existing } : null;
  };

  const saveProvinceSelections = () => {
    if (!canSaveSelections.value) return;
    const selectedKeys = new Set(draft.cityValues);
    const replacements = cityChoices.value
      .filter((choice) => selectedKeys.has(choice.key))
      .map(selectionFromChoice)
      .filter((selection): selection is GatewayVisibilitySelection =>
        Boolean(selection),
      );

    const firstProvinceIndex = selections.value.findIndex(
      (selection) => selection.province === draft.province,
    );
    const insertionIndex =
      firstProvinceIndex >= 0 ? firstProvinceIndex : selections.value.length;
    const nextSelections: GatewayVisibilitySelection[] = [];
    const seen = new Set<string>();

    selections.value.forEach((selection, index) => {
      if (index === insertionIndex) {
        for (const replacement of replacements) {
          const key = getCidrRegionSelectionKey(replacement);
          if (seen.has(key)) continue;
          seen.add(key);
          nextSelections.push(replacement);
        }
      }
      if (selection.province === draft.province) return;
      const key = getCidrRegionSelectionKey(selection);
      if (seen.has(key)) return;
      seen.add(key);
      nextSelections.push(selection);
    });

    if (insertionIndex === selections.value.length) {
      for (const replacement of replacements) {
        const key = getCidrRegionSelectionKey(replacement);
        if (seen.has(key)) continue;
        seen.add(key);
        nextSelections.push(replacement);
      }
    }

    selections.value = nextSelections;
    handleDialogOpenChange(false);
  };

  const removeRegion = (selection: GatewayVisibilitySelection) => {
    if (disabled.value) return;
    const key = getCidrRegionSelectionKey(selection);
    selections.value = selections.value.filter(
      (item) => getCidrRegionSelectionKey(item) !== key,
    );
  };

  const stopDisabledWatch = watch(disabled, (isDisabled) => {
    if (isDisabled) handleDialogOpenChange(false);
  });

  const dispose = () => {
    stopDisabledWatch();
  };

  return {
    canSaveSelections,
    cityChoices,
    cityOptions,
    cityOptionsLoading,
    cityOptionsReady,
    clearDraft,
    dispose,
    draft,
    handleDialogOpenChange,
    hasDraftChanges,
    isDialogOpen,
    loadCityOptions,
    loadProvinces,
    openDialog,
    originalProvinceSelections,
    provinces,
    provincesLoadError,
    provincesLoading,
    removeRegion,
    saveProvinceSelections,
    selectProvince,
    selectedCityCount,
    toggleCity,
  };
};
