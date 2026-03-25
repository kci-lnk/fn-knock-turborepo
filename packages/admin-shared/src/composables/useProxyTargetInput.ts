import { ref, watch, type Ref } from "vue";
import {
  DEFAULT_PROXY_TARGET_PORT,
  DEFAULT_PROXY_TARGET_PROTOCOL,
  normalizeProxyTargetInput,
  resolveProxyTargetInput,
  type ProxyTargetProtocol,
} from "@admin-shared/utils/proxyTargetInput";

type UseProxyTargetInputOptions = {
  defaultPort?: string;
  defaultProtocol?: ProxyTargetProtocol;
};

export const useProxyTargetInput = (
  model: Ref<string>,
  options: UseProxyTargetInputOptions = {},
) => {
  const defaultProtocol =
    options.defaultProtocol ?? DEFAULT_PROXY_TARGET_PROTOCOL;
  const defaultPort = options.defaultPort ?? DEFAULT_PROXY_TARGET_PORT;

  const protocol = ref<ProxyTargetProtocol>(defaultProtocol);
  const endpoint = ref("");
  let isSyncingInternally = false;

  const syncFromModel = (value: string) => {
    const resolved = resolveProxyTargetInput(defaultProtocol, value);

    isSyncingInternally = true;
    protocol.value = resolved.protocol;
    endpoint.value = resolved.endpoint;
    isSyncingInternally = false;
  };

  const syncToModel = () => {
    const nextTarget = resolveProxyTargetInput(
      protocol.value,
      endpoint.value,
    ).target;

    if (model.value !== nextTarget) {
      model.value = nextTarget;
    }
  };

  const normalize = () => {
    const normalized = normalizeProxyTargetInput(
      protocol.value,
      endpoint.value,
      defaultPort,
    );

    isSyncingInternally = true;
    protocol.value = normalized.protocol;
    endpoint.value = normalized.endpoint;
    isSyncingInternally = false;

    if (model.value !== normalized.target) {
      model.value = normalized.target;
    }

    return normalized.target;
  };

  watch(
    model,
    (nextValue) => {
      syncFromModel(nextValue);
    },
    { immediate: true, flush: "sync" },
  );

  watch(
    [protocol, endpoint],
    () => {
      if (isSyncingInternally) return;
      syncToModel();
    },
    { flush: "sync" },
  );

  return {
    protocol,
    endpoint,
    normalize,
  };
};
