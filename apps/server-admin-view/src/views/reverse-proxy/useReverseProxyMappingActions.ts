import { ref, type ComputedRef, type Ref } from "vue";
import type { ProxyMapping } from "@/types";
import { useAsyncAction } from "@admin-shared/composables/useAsyncAction";
import { toast } from "@admin-shared/utils/toast";
import { buildProxyMapping } from "@admin-shared/utils/proxyMapping";
import { isWebSocketProxyTargetUrl } from "@admin-shared/utils/proxyTargetInput";
import { persistProxyMappings } from "@admin-shared/utils/persistProxyMappings";
import {
  createReverseProxyMessages,
  showReverseProxyActionError,
} from "@admin-shared/utils/reverseProxyFeedback";
import { validateSingleMappingDuplicates } from "@admin-shared/utils/validateProxyMappingDuplicates";

type ReverseProxyMessages = ReturnType<typeof createReverseProxyMessages>;

export const useReverseProxyMappingActions = ({
  allMappings,
  closeMappingDialog,
  currentPage,
  editingOriginalMapping,
  form,
  isDefaultRoute,
  isEditing,
  isValid,
  messages,
  paginatedMappings,
  saveDefaultRoute,
  saveProxyMappings,
  searchQuery,
}: {
  allMappings: ComputedRef<ProxyMapping[]>;
  closeMappingDialog: (reset?: boolean) => void;
  currentPage: Ref<number>;
  editingOriginalMapping: Ref<ProxyMapping | null>;
  form: ProxyMapping;
  isDefaultRoute: (path: string) => boolean;
  isEditing: Ref<boolean>;
  isValid: Ref<boolean>;
  messages: ReverseProxyMessages;
  paginatedMappings: ComputedRef<ProxyMapping[]>;
  saveDefaultRoute: (path: string) => Promise<void>;
  saveProxyMappings: (mappings: ProxyMapping[]) => Promise<void>;
  searchQuery: Ref<string>;
}) => {
  const removingPath = ref<string | null>(null);
  const { run: runRemoveMapping } = useAsyncAction({
    onError: (error) => {
      showReverseProxyActionError(
        messages.deleteFailed,
        error,
        messages.unknownError,
      );
    },
  });
  const { isPending: isSaving, run: runSaveAction } = useAsyncAction({
    onError: (error) => {
      showReverseProxyActionError(
        messages.saveFailed,
        error,
        messages.unknownError,
      );
    },
  });

  const removeMapping = async (mapping: ProxyMapping) => {
    removingPath.value = mapping.path;
    await runRemoveMapping(
      async () => {
        const nextMappings = allMappings.value.filter(
          (item) => item !== mapping,
        );
        await saveProxyMappings(nextMappings);

        if (isDefaultRoute(mapping.path)) {
          await saveDefaultRoute("/__select__");
        }

        if (paginatedMappings.value.length === 1 && currentPage.value > 1) {
          currentPage.value--;
        }

        toast.success(messages.deleteSuccess);
      },
      {
        onFinally: () => {
          removingPath.value = null;
        },
      },
    );
  };

  const saveMapping = async () => {
    if (!isValid.value) return;
    const isWebSocketTarget = isWebSocketProxyTargetUrl(form.target);
    const normalizedMapping = buildProxyMapping({
      ...form,
      rewrite_html: isWebSocketTarget ? false : form.rewrite_html,
      use_root_mode: isWebSocketTarget ? false : form.use_root_mode,
    });
    const { path: trimmedPath, target: trimmedTarget } = normalizedMapping;
    const ignorePath = isEditing.value
      ? (editingOriginalMapping.value?.path.trim() ?? null)
      : null;
    const ignoreTarget = isEditing.value
      ? (editingOriginalMapping.value?.target.trim() ?? null)
      : null;
    const { duplicatePath, duplicateTarget } = validateSingleMappingDuplicates(
      allMappings.value,
      { path: trimmedPath, target: trimmedTarget },
      { ignorePath, ignoreTarget },
    );

    if (duplicatePath) {
      toast.error(messages.duplicatePath(trimmedPath));
      return;
    }
    if (duplicateTarget) {
      toast.error(messages.duplicateTarget(trimmedTarget));
      return;
    }

    const isCreate = !isEditing.value;
    await runSaveAction(async () => {
      const nextMappings = [...allMappings.value];
      if (isEditing.value && editingOriginalMapping.value) {
        const index = nextMappings.indexOf(editingOriginalMapping.value);
        if (index !== -1) {
          nextMappings[index] = normalizedMapping;
        }
      } else {
        nextMappings.push(normalizedMapping);
      }

      await persistProxyMappings(
        nextMappings,
        {
          saveMappings: saveProxyMappings,
          saveDefaultRoute,
          resetPage: () => {
            currentPage.value = 1;
          },
          resetSearch: () => {
            searchQuery.value = "";
          },
        },
        {
          resetPage: isCreate,
          resetSearch: isCreate,
          onAfterPersist: () => {
            closeMappingDialog(true);
          },
        },
      );

      toast.success(isCreate ? messages.createSuccess : messages.updateSuccess);
    });
  };

  return {
    isSaving,
    removeMapping,
    removingPath,
    runSaveAction,
    saveMapping,
  };
};
