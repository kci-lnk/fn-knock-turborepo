export const DEFAULT_LOG_WINDOW_SIZE = 1000;

export function mergePollingLogWindow<T>(
  current: T[],
  incoming: T[],
  options?: { reset?: boolean; max?: number },
): T[] {
  const max = options?.max ?? DEFAULT_LOG_WINDOW_SIZE;
  if (options?.reset) {
    return incoming.slice(-max);
  }
  if (!incoming.length) {
    return current.slice(-max);
  }
  return [...current, ...incoming].slice(-max);
}
