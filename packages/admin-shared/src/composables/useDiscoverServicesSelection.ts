import { computed, ref } from 'vue';

type UseDiscoverServicesSelectionOptions<TService> = {
  getPath: (service: TService) => string | null | undefined;
};

export const useDiscoverServicesSelection = <
  TService,
  TData extends { services: TService[] },
>(
  options: UseDiscoverServicesSelectionOptions<TService>,
) => {
  const open = ref(false);
  const discoveredData = ref<TData | null>(null);
  const selectedServices = ref<TService[]>([]);

  const isAllSelected = computed(() => {
    if (!discoveredData.value || discoveredData.value.services.length === 0) return false;
    return selectedServices.value.length === discoveredData.value.services.length;
  });

  const isSelectionValid = computed(() => {
    if (selectedServices.value.length === 0) return false;
    return selectedServices.value.every((service) => {
      const path = options.getPath(service as TService);
      return Boolean(path && path.trim() !== '');
    });
  });

  const setAllSelected = (checked: boolean) => {
    if (checked && discoveredData.value) {
      selectedServices.value = [...discoveredData.value.services];
      return;
    }
    selectedServices.value = [];
  };

  const resetSelection = () => {
    selectedServices.value = [];
  };

  const setDiscoveredData = (data: TData | null) => {
    discoveredData.value = data;
  };

  const openDialog = () => {
    open.value = true;
  };

  const closeDialog = (reset = false) => {
    open.value = false;
    if (reset) resetSelection();
  };

  return {
    open,
    discoveredData,
    selectedServices,
    isAllSelected,
    isSelectionValid,
    setAllSelected,
    resetSelection,
    setDiscoveredData,
    openDialog,
    closeDialog,
  };
};
