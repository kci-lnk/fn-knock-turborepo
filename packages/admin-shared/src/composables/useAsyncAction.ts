import { ref } from 'vue';

export function extractErrorMessage(error: unknown, fallback = '操作失败') {
  if (error && typeof error === 'object') {
    const maybeResponse = error as { response?: { data?: { message?: unknown } } };
    const responseMessage = maybeResponse.response?.data?.message;
    if (typeof responseMessage === 'string' && responseMessage.trim()) {
      return responseMessage;
    }

    const maybeMessage = error as { message?: unknown };
    if (typeof maybeMessage.message === 'string' && maybeMessage.message.trim()) {
      return maybeMessage.message;
    }
  }
  return fallback;
}

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
