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
  const provincesLoading = ref(false);
  const cityOptionsLoading = ref(false);
  const provincesLoadError = ref("");
  const isDialogOpen = ref(false);
  const draft = reactive({
    province: "",
    cityValue: "",
  });
  let cityRequestToken = 0;

  const selectedCityOption = computed(
    () =>
      cityOptions.value.find((option) => option.value === draft.cityValue) ??
      null,
  );
  const citySelectKey = computed(() => draft.province || "empty");
  const pendingRegionExists = computed(() => {
    const city = selectedCityOption.value;
    if (!draft.province || !city) return false;
    const pendingKey = getCidrRegionSelectionKey({
      province: draft.province,
      query_city: city.queryCity,
    });
    return selections.value.some(
      (item) => getCidrRegionSelectionKey(item) === pendingKey,
    );
  });
  const canAddRegion = computed(
    () =>
      !disabled.value &&
      Boolean(draft.province) &&
      Boolean(selectedCityOption.value) &&
      !pendingRegionExists.value &&
      !cityOptionsLoading.value,
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

  const clearDraft = () => {
    cityRequestToken += 1;
    cityOptionsLoading.value = false;
    draft.province = "";
    draft.cityValue = "";
    cityOptions.value = [];
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
      cityOptions.value = [];
      draft.cityValue = "";
      return;
    }

    const token = ++cityRequestToken;
    cityOptionsLoading.value = true;
    cityOptions.value = [];
    draft.cityValue = "";
    try {
      const payload = await loadCities(province);
      if (token !== cityRequestToken) return;
      cityOptions.value = payload.options;
      draft.cityValue = payload.defaultValue ?? payload.options[0]?.value ?? "";
    } catch (error) {
      if (token !== cityRequestToken) return;
      cityOptions.value = [];
      draft.cityValue = "";
      reportLoadError(error);
    } finally {
      if (token === cityRequestToken) cityOptionsLoading.value = false;
    }
  };

  const selectProvince = (province: string) => {
    draft.province = province;
    void loadCityOptions(province);
  };

  const addRegion = () => {
    const option = selectedCityOption.value;
    if (!option || !canAddRegion.value) return;
    selections.value = [
      ...selections.value,
      {
        province: draft.province,
        city: option.isProvinceWide ? null : option.label,
        label: option.label,
        value: option.value,
        query_city: option.queryCity,
        is_province_wide: option.isProvinceWide,
        is_municipality: option.isMunicipality,
      },
    ];
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
    addRegion,
    canAddRegion,
    cityOptions,
    cityOptionsLoading,
    citySelectKey,
    clearDraft,
    dispose,
    draft,
    handleDialogOpenChange,
    isDialogOpen,
    loadCityOptions,
    loadProvinces,
    openDialog,
    pendingRegionExists,
    provinces,
    provincesLoadError,
    provincesLoading,
    removeRegion,
    selectProvince,
    selectedCityOption,
  };
};
