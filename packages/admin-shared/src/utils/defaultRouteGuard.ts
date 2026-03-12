export const needsClearDefaultRouteConfirm = (
  targetPort: number | null,
  defaultSystemPort: number,
) => {
  return targetPort === defaultSystemPort;
};

export const needsSetDefaultRouteConfirm = (
  currentDefaultPort: number | null,
  currentDefaultPath: string | null | undefined,
  nextPath: string,
  defaultSystemPort: number,
) => {
  const hasDefaultSystemPort = currentDefaultPort === defaultSystemPort;
  const isSwitchingFromCurrentDefault = nextPath !== currentDefaultPath;
  return hasDefaultSystemPort && isSwitchingFromCurrentDefault;
};
