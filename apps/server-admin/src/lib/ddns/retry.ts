export type RetryOptions = {
  maxAttempts: number;
  delayMs: number;
};

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

export const runWithRetry = async <T>(action: () => Promise<T>, options: RetryOptions): Promise<T> => {
  let lastError: unknown = null;
  for (let attempt = 1; attempt <= options.maxAttempts; attempt += 1) {
    try {
      return await action();
    } catch (error) {
      lastError = error;
      if (attempt < options.maxAttempts) {
        await sleep(options.delayMs);
      }
    }
  }
  throw lastError;
};
