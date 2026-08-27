import { computed, type ComputedRef } from "vue";
import {
  buildHostTargetPlaceholder,
  buildHostTargetSuggestions,
} from "./host-target-candidates";
import { useHostTargetCandidateCatalog } from "./useHostTargetCandidateCatalog";

export {
  buildHostTargetPlaceholder,
  buildHostTargetSuggestions,
} from "./host-target-candidates";

type Translate = (key: string) => string;

export const useHostTargetCandidates = ({
  isDockerDeployment,
  open,
  translate,
}: {
  isDockerDeployment: ComputedRef<boolean>;
  open: ComputedRef<boolean>;
  translate: Translate;
}) => {
  const { candidates, isLoading } = useHostTargetCandidateCatalog({
    isDockerDeployment,
    open,
  });

  const targetSuggestions = computed(() =>
    buildHostTargetSuggestions(candidates.value, isDockerDeployment.value),
  );
  const targetPlaceholder = computed(() =>
    buildHostTargetPlaceholder(
      candidates.value,
      isDockerDeployment.value,
      translate("admin.subdomainProxy.dockerTargetPlaceholder"),
    ),
  );
  const targetCandidateHint = computed(() => {
    if (!isDockerDeployment.value) return "";
    if (isLoading.value) {
      return translate("admin.subdomainProxy.dockerTargetCandidatesLoading");
    }
    return targetSuggestions.value.length > 0
      ? translate("admin.subdomainProxy.dockerTargetCandidatesHint")
      : translate("admin.subdomainProxy.dockerTargetCandidatesEmpty");
  });

  return {
    targetCandidateHint,
    targetPlaceholder,
    targetSuggestions,
  };
};
