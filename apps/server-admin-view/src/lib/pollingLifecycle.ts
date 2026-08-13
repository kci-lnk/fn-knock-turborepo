type PollingLifecycleOptions = {
  initialize: () => Promise<void>;
  start: () => void;
};

export const createPollingLifecycle = ({
  initialize,
  start,
}: PollingLifecycleOptions) => {
  let initializationPromise: Promise<void> | null = null;
  let lifecycleGeneration = 0;

  const activate = async () => {
    const activationGeneration = lifecycleGeneration;
    initializationPromise ??= initialize();
    await initializationPromise;
    if (activationGeneration !== lifecycleGeneration) return;

    start();
  };

  const deactivate = () => {
    lifecycleGeneration += 1;
  };

  return { activate, deactivate };
};
