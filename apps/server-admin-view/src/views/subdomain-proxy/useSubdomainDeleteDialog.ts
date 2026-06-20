import { computed, ref, type ComputedRef } from "vue";
import {
  buildDeleteDialogCopy,
  type DeleteDialogState,
  type TranslationParams,
  type TranslationSpec,
} from "./model";

export const useSubdomainDeleteDialog = ({
  mappingsCount,
  translate,
}: {
  mappingsCount: ComputedRef<number>;
  translate: (key: string, params?: TranslationParams) => string;
}) => {
  const deleteDialogState = ref<DeleteDialogState | null>(null);

  const translateSpec = (spec: TranslationSpec | null | undefined) => {
    if (!spec?.key) return "";
    return translate(spec.key, spec.params);
  };

  const deleteDialogCopy = computed(() => {
    const target = deleteDialogState.value;
    return target ? buildDeleteDialogCopy(target, mappingsCount.value) : null;
  });

  const closeDeleteDialog = () => {
    deleteDialogState.value = null;
  };

  const handleDeleteDialogOpenChange = (nextOpen: boolean) => {
    if (!nextOpen) {
      closeDeleteDialog();
    }
  };

  const openClearAllConfigDialogState = () => {
    deleteDialogState.value = {
      kind: "clear_all",
      step: 1,
    };
  };

  const openDeleteMappingDialog = (host: string) => {
    deleteDialogState.value = {
      kind: "mapping",
      host,
    };
  };

  const advanceClearAllConfirmation = () => {
    const target = deleteDialogState.value;
    if (target?.kind !== "clear_all" || target.step !== 1) return false;

    deleteDialogState.value = {
      kind: "clear_all",
      step: 2,
    };
    return true;
  };

  const deleteDialogConfirmLabel = computed(
    () =>
      translateSpec(deleteDialogCopy.value?.confirmLabel) ||
      translate("admin.subdomainProxy.confirm"),
  );
  const deleteDialogDescription = computed(() =>
    translateSpec(deleteDialogCopy.value?.description),
  );
  const deleteDialogTitle = computed(() =>
    translateSpec(deleteDialogCopy.value?.title),
  );

  return {
    advanceClearAllConfirmation,
    closeDeleteDialog,
    deleteDialogConfirmLabel,
    deleteDialogDescription,
    deleteDialogState,
    deleteDialogTitle,
    handleDeleteDialogOpenChange,
    isDeleteDialogOpen: computed(() => deleteDialogState.value !== null),
    openClearAllConfigDialogState,
    openDeleteMappingDialog,
  };
};
