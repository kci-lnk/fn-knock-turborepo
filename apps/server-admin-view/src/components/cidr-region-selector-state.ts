import { computed, reactive, ref, watch, type Ref } from "vue";
import type {
  CidrCapabilitiesPayload,
  CidrCityOption,
  CidrOperator,
  CidrProvinceOption,
  GatewayVisibilitySelection,
} from "@/types";
import {
  CIDR_OPERATORS,
  getCidrRegionSelectionKey,
  getCidrRegionSelectionLabel,
} from "@/types/cidr";

export interface CidrRegionSelectorStateOptions {
  disabled: Readonly<Ref<boolean>>;
  formatLoadError: (error: unknown) => string;
  loadCapabilities: () => Promise<CidrCapabilitiesPayload>;
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
  selection: GatewayVisibilitySelection;
}

const selectionSetsEqual = (
  left: GatewayVisibilitySelection[],
  right: GatewayVisibilitySelection[],
) => {
  const leftKeys = left.map(getCidrRegionSelectionKey);
  const rightKeys = new Set(right.map(getCidrRegionSelectionKey));
  return (
    leftKeys.length === rightKeys.size &&
    leftKeys.every((key) => rightKeys.has(key))
  );
};

const geographyKey = (selection: {
  province: string;
  query_city?: string | null;
}) => `${selection.province}::${selection.query_city ?? ""}`;

const operatorEquals = (
  left?: CidrOperator | null,
  right?: CidrOperator | null,
) => (left ?? null) === (right ?? null);

const normalizeProvinceSelections = (values: GatewayVisibilitySelection[]) => {
  const seen = new Set<string>();
  const deduped = values.filter((selection) => {
    const key = getCidrRegionSelectionKey(selection);
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
  const provinceWideOperators = new Set(
    deduped
      .filter(
        (selection) => selection.is_province_wide || !selection.query_city,
      )
      .map((selection) => selection.operator ?? ""),
  );
  const allOperatorGeographies = new Set(
    deduped.filter((selection) => !selection.operator).map(geographyKey),
  );
  return deduped.filter((selection) => {
    if (
      selection.query_city &&
      provinceWideOperators.has(selection.operator ?? "")
    ) {
      return false;
    }
    return (
      !selection.operator ||
      !allOperatorGeographies.has(geographyKey(selection))
    );
  });
};

const regionLabel = (selection: GatewayVisibilitySelection) => {
  const operator = selection.operator;
  const label = getCidrRegionSelectionLabel(selection);
  if (!operator) return label;
  const suffix = ` · ${operator}`;
  return label.endsWith(suffix) ? label.slice(0, -suffix.length) : label;
};

export const createCidrRegionSelectorState = ({
  disabled,
  formatLoadError,
  loadCapabilities: fetchCapabilities,
  loadCities,
  loadProvinces: fetchProvinces,
  onLoadError,
  selections,
}: CidrRegionSelectorStateOptions) => {
  const provinces = ref<CidrProvinceOption[]>([]);
  const cityOptions = ref<CidrCityOption[]>([]);
  const originalProvinceSelections = ref<GatewayVisibilitySelection[]>([]);
  const capabilities = ref<CidrCapabilitiesPayload | null>(null);
  const capabilitiesLoading = ref(false);
  const capabilityLoadError = ref("");
  const provincesLoading = ref(false);
  const cityOptionsLoading = ref(false);
  const cityOptionsReady = ref(false);
  const provincesLoadError = ref("");
  const isDialogOpen = ref(false);
  const draft = reactive<{
    province: string;
    operator: CidrOperator | null;
    selections: GatewayVisibilitySelection[];
  }>({
    province: "",
    operator: null,
    selections: [],
  });
  let cityRequestToken = 0;

  const operatorFilteringSupported = computed(
    () => capabilities.value?.operatorFiltering.supported === true,
  );
  const operators = computed(
    () =>
      capabilities.value?.operatorFiltering.operators ?? [...CIDR_OPERATORS],
  );

  const selectionFromOption = (
    option: CidrCityOption,
    operator: CidrOperator | null,
  ): GatewayVisibilitySelection => ({
    province: draft.province,
    city: option.isProvinceWide ? null : option.label,
    label: operator ? `${option.label} · ${operator}` : option.label,
    value: option.value,
    query_city: option.queryCity,
    operator,
    is_province_wide: option.isProvinceWide,
    is_municipality: option.isMunicipality,
  });

  const cityChoices = computed<CidrCityChoice[]>(() => {
    const choices = cityOptions.value.map((option) => {
      const selection = selectionFromOption(option, draft.operator);
      return {
        key: getCidrRegionSelectionKey(selection),
        label: option.label,
        isProvinceWide: option.isProvinceWide,
        unavailable: false,
        selection,
      };
    });
    const availableKeys = new Set(choices.map((choice) => choice.key));

    for (const selection of draft.selections) {
      if (!operatorEquals(selection.operator, draft.operator)) continue;
      const key = getCidrRegionSelectionKey(selection);
      if (availableKeys.has(key)) continue;
      availableKeys.add(key);
      choices.push({
        key,
        label: regionLabel(selection),
        isProvinceWide: selection.is_province_wide || !selection.query_city,
        unavailable: true,
        selection: { ...selection },
      });
    }

    return choices;
  });
  const activeSelectionKeys = computed(() =>
    draft.selections
      .filter((selection) => operatorEquals(selection.operator, draft.operator))
      .map(getCidrRegionSelectionKey),
  );
  const selectedCityCount = computed(() => activeSelectionKeys.value.length);
  const hasDraftChanges = computed(
    () =>
      cityOptionsReady.value &&
      !selectionSetsEqual(draft.selections, originalProvinceSelections.value),
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

  const loadCapabilities = async () => {
    if (capabilitiesLoading.value) return;
    capabilitiesLoading.value = true;
    capabilityLoadError.value = "";
    try {
      capabilities.value = await fetchCapabilities();
    } catch (error) {
      capabilities.value = null;
      capabilityLoadError.value = formatLoadError(error);
    } finally {
      capabilitiesLoading.value = false;
    }
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
    draft.selections = [];
    cityOptionsReady.value = false;
  };

  const clearDraft = () => {
    cityRequestToken += 1;
    cityOptionsLoading.value = false;
    draft.province = "";
    draft.operator = null;
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
        .map((selection) => ({
          ...selection,
          operator: selection.operator ?? null,
        }));
      draft.selections = normalizeProvinceSelections(
        originalProvinceSelections.value.map((selection) => ({ ...selection })),
      );
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
    draft.operator = null;
    void loadCityOptions(province);
  };

  const selectOperator = (operator: CidrOperator | null) => {
    if (operator && !operatorFilteringSupported.value) return;
    draft.operator = operator;
  };

  const toggleCity = (key: string, checked: boolean) => {
    if (disabled.value || !cityOptionsReady.value) return;
    const choice = cityChoices.value.find((item) => item.key === key);
    if (!choice) return;

    draft.selections = draft.selections.filter(
      (selection) => getCidrRegionSelectionKey(selection) !== key,
    );
    if (!checked) return;

    const next = choice.selection;
    draft.selections = draft.selections.filter((selection) => {
      if (
        operatorEquals(selection.operator, next.operator) &&
        (next.is_province_wide || !next.query_city
          ? Boolean(selection.query_city)
          : selection.is_province_wide || !selection.query_city)
      ) {
        return false;
      }
      if (geographyKey(selection) !== geographyKey(next)) return true;
      return next.operator ? Boolean(selection.operator) : !selection.operator;
    });
    draft.selections.push({ ...next });
  };

  const saveProvinceSelections = () => {
    if (!canSaveSelections.value) return;
    const replacements = normalizeProvinceSelections(
      draft.selections.map((selection) => ({ ...selection })),
    );
    const firstProvinceIndex = selections.value.findIndex(
      (selection) => selection.province === draft.province,
    );
    const insertionIndex =
      firstProvinceIndex >= 0 ? firstProvinceIndex : selections.value.length;
    const nextSelections = selections.value.filter(
      (selection) => selection.province !== draft.province,
    );
    nextSelections.splice(insertionIndex, 0, ...replacements);
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

  const dispose = () => stopDisabledWatch();

  return {
    activeSelectionKeys,
    canSaveSelections,
    capabilities,
    capabilitiesLoading,
    capabilityLoadError,
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
    loadCapabilities,
    loadCityOptions,
    loadProvinces,
    openDialog,
    operatorFilteringSupported,
    operators,
    originalProvinceSelections,
    provinces,
    provincesLoadError,
    provincesLoading,
    removeRegion,
    saveProvinceSelections,
    selectOperator,
    selectProvince,
    selectedCityCount,
    toggleCity,
  };
};
