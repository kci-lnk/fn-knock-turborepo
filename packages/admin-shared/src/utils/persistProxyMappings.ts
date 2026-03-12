export type PersistProxyMappingsOptions = {
  defaultRoutePath?: string | null;
  resetPage?: boolean;
  resetSearch?: boolean;
  onAfterPersist?: () => void | Promise<void>;
};

type PersistProxyMappingsDeps<T> = {
  saveMappings: (newList: T[]) => Promise<void>;
  saveDefaultRoute: (path: string) => Promise<void>;
  resetPage?: () => void;
  resetSearch?: () => void;
};

export const persistProxyMappings = async <T>(
  newList: T[],
  deps: PersistProxyMappingsDeps<T>,
  options: PersistProxyMappingsOptions = {},
) => {
  await deps.saveMappings(newList);

  if (options.defaultRoutePath) {
    await deps.saveDefaultRoute(options.defaultRoutePath);
  }
  if (options.resetPage) {
    deps.resetPage?.();
  }
  if (options.resetSearch) {
    deps.resetSearch?.();
  }
  if (options.onAfterPersist) {
    await options.onAfterPersist();
  }
};
