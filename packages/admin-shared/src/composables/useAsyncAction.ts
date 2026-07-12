import { ref } from "vue";

export { extractErrorMessage } from "@frontend-core/errors/extractErrorMessage";

interface UseAsyncActionOptions {
  onError?: (error: unknown) => void;
  rethrow?: boolean;
}

interface AsyncActionHooks<T> {
  onSuccess?: (result: T) => void | Promise<void>;
  onError?: (error: unknown) => void;
  onFinally?: () => void;
}

export function useAsyncAction(options?: UseAsyncActionOptions) {
  const isPending = ref(false);

  const run = async <T>(
    action: () => Promise<T>,
    hooks?: AsyncActionHooks<T>,
  ): Promise<T | undefined> => {
    if (isPending.value) return;
    isPending.value = true;
    try {
      const result = await action();
      await hooks?.onSuccess?.(result);
      return result;
    } catch (error) {
      hooks?.onError?.(error);
      options?.onError?.(error);
      if (options?.rethrow) {
        throw error;
      }
    } finally {
      isPending.value = false;
      hooks?.onFinally?.();
    }
  };

  return {
    isPending,
    run,
  };
}
