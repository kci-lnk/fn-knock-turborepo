import { useRouter } from "vue-router";

export const useSubdomainNavigation = () => {
  const router = useRouter();
  const openSubdomainPage = (host: string, page: string) =>
    void router.push(`/subdomains/${encodeURIComponent(host)}/${page}`);

  return {
    openAdvancedAuth: (host: string) =>
      openSubdomainPage(host, "advanced-auth"),
    openDeepMonitor: (host: string) => openSubdomainPage(host, "deep-monitor"),
    navigateToGatewayLocations: (host: string) =>
      openSubdomainPage(host, "paths"),
  };
};
