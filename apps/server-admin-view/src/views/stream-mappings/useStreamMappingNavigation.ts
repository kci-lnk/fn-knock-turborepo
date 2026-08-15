import { useRouter } from "vue-router";
import type { StreamMapping } from "../../types";

export const useStreamMappingNavigation = () => {
  const router = useRouter();
  return {
    openBypassPolicy: (mapping: StreamMapping) =>
      void router.push(
        `/streams/${mapping.protocol}/${mapping.listen_port}/bypass-policy`,
      ),
  };
};
