export interface RuntimeRouteAccess {
  canUseTerminal: boolean;
  canUseSshSecurity: boolean;
  sshSecurityEnabled: boolean;
  canUseSmartConnect: boolean;
  canUseFnosCertificateSync: boolean;
}

export type RuntimeRouteRedirect = {
  path: string;
  query?: Record<string, string>;
};

export const resolveRuntimeCapabilityRedirect = (
  path: string,
  access: RuntimeRouteAccess,
): RuntimeRouteRedirect | null => {
  if (path === "/terminal" && !access.canUseTerminal) {
    return { path: "/system" };
  }
  if (
    path === "/ssh-security" &&
    (!access.canUseSshSecurity || !access.sshSecurityEnabled)
  ) {
    return { path: "/system", query: { tab: "features" } };
  }
  if (path === "/system/smart-connect" && !access.canUseSmartConnect) {
    return { path: "/system", query: { tab: "features" } };
  }
  if (
    path === "/system/fnos-certificate-sync" &&
    !access.canUseFnosCertificateSync
  ) {
    return { path: "/system", query: { tab: "fnos" } };
  }
  return null;
};
