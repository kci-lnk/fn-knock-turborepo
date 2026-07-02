import {
  computed,
  reactive,
  ref,
  watch,
  type ComputedRef,
  type Ref,
} from "vue";
import { extractErrorMessage } from "@admin-shared/composables/useAsyncAction";
import { toast } from "@admin-shared/utils/toast";
import type {
  CidrCityOption,
  CidrProvinceOption,
  GatewayVisibilitySelection,
} from "@/types";

type TranslateParams = Record<string, string | number>;
type RegionSelectorTextKey =
  | "loading"
  | "selectProvinceFirst"
  | "selectCityOrProvince"
  | "selectCity"
  | "regionsLoadFailed"
  | "regionsLoadDescription";

export const useCidrRegionSelector = ({
  selections,
  isEnabled,
  loadCities,
  provinces,
  regionInputsDisabled,
  translate,
}: {
  selections: Ref<GatewayVisibilitySelection[]>;
  isEnabled: Readonly<Ref<boolean>>;
  loadCities: (province: string) => Promise<{
    defaultValue?: string | null;
    options: CidrCityOption[];
  }>;
  provinces: Ref<CidrProvinceOption[]>;
  regionInputsDisabled: ComputedRef<boolean>;
  translate: (key: RegionSelectorTextKey, params?: TranslateParams) => string;
}) => {
  const cityOptions = ref<CidrCityOption[]>([]);
  const cityOptionsLoading = ref(false);
  const isRegionDialogOpen = ref(false);
  const regionDraft = reactive({
    province: "",
    cityValue: "",
  });
  let cityRequestToken = 0;

  const selectionKey = (selection: {
    province: string;
    query_city?: string | null;
  }) => `${selection.province}::${selection.query_city ?? ""}`;

  const selectedCityOption = computed(
    () =>
      cityOptions.value.find(
        (option) => option.value === regionDraft.cityValue,
      ) ?? null,
  );
  const citySelectKey = computed(() => regionDraft.province || "empty");
  const citySelectPlaceholder = computed(() => {
    if (cityOptionsLoading.value) return translate("loading");
    if (!regionDraft.province) {
      return translate("selectProvinceFirst");
    }
    return cityOptions.value.some((option) => option.isProvinceWide)
      ? translate("selectCityOrProvince")
      : translate("selectCity");
  });
  const pendingRegionExists = computed(() => {
    const city = selectedCityOption.value;
    if (!regionDraft.province || !city) return false;
    return selections.value.some(
      (item) =>
        selectionKey(item) ===
        selectionKey({
          province: regionDraft.province,
          query_city: city.queryCity,
        }),
    );
  });
  const canAddRegion = computed(
    () =>
      isEnabled.value &&
      Boolean(regionDraft.province) &&
      Boolean(selectedCityOption.value) &&
      !pendingRegionExists.value &&
      !cityOptionsLoading.value,
  );

  const clearRegionDraft = () => {
    cityRequestToken += 1;
    cityOptionsLoading.value = false;
    regionDraft.province = "";
    regionDraft.cityValue = "";
    cityOptions.value = [];
  };

  const prepareRegionDraft = () => {
    const preferredProvince =
      selections.value[0]?.province || provinces.value[0]?.value || "";

    clearRegionDraft();

    if (preferredProvince) {
      regionDraft.province = preferredProvince;
    }
  };

  const openRegionDialog = () => {
    if (regionInputsDisabled.value || provinces.value.length === 0) {
      return;
    }

    isRegionDialogOpen.value = true;
    prepareRegionDraft();
  };

  const handleRegionDialogOpenChange = (nextOpen: boolean) => {
    isRegionDialogOpen.value = nextOpen;

    if (!nextOpen) {
      clearRegionDraft();
    }
  };

  const loadCityOptions = async (province: string) => {
    if (!province) {
      cityOptions.value = [];
      regionDraft.cityValue = "";
      return;
    }

    const token = ++cityRequestToken;
    cityOptionsLoading.value = true;
    cityOptions.value = [];
    regionDraft.cityValue = "";
    try {
      const payload = await loadCities(province);
      if (token !== cityRequestToken) return;
      cityOptions.value = payload.options;
      const hasCurrentValue = payload.options.some(
        (option) => option.value === regionDraft.cityValue,
      );
      regionDraft.cityValue = hasCurrentValue
        ? regionDraft.cityValue
        : (payload.defaultValue ?? payload.options[0]?.value ?? "");
    } catch (error) {
      if (token !== cityRequestToken) return;
      cityOptions.value = [];
      regionDraft.cityValue = "";
      toast.error(translate("regionsLoadFailed"), {
        description: extractErrorMessage(
          error,
          translate("regionsLoadDescription"),
        ),
      });
    } finally {
      if (token === cityRequestToken) {
        cityOptionsLoading.value = false;
      }
    }
  };

  watch(
    () => regionDraft.province,
    (province, previousProvince) => {
      if (province !== previousProvince) {
        void loadCityOptions(province);
      }
    },
  );

  watch(isEnabled, (enabled) => {
    if (!enabled) {
      handleRegionDialogOpenChange(false);
    }
  });

  const addRegion = () => {
    const option = selectedCityOption.value;
    if (!option || !canAddRegion.value) return;
    selections.value.push({
      province: regionDraft.province,
      city: option.isProvinceWide ? null : option.label,
      label: option.label,
      value: option.value,
      query_city: option.queryCity,
      is_province_wide: option.isProvinceWide,
      is_municipality: option.isMunicipality,
    });
    handleRegionDialogOpenChange(false);
  };

  const removeRegion = (selection: GatewayVisibilitySelection) => {
    if (regionInputsDisabled.value) return;
    selections.value = selections.value.filter(
      (item) => selectionKey(item) !== selectionKey(selection),
    );
  };

  return {
    addRegion,
    canAddRegion,
    cityOptions,
    cityOptionsLoading,
    citySelectKey,
    citySelectPlaceholder,
    clearRegionDraft,
    handleRegionDialogOpenChange,
    isRegionDialogOpen,
    openRegionDialog,
    regionDraft,
    removeRegion,
    selectionKey,
  };
};
