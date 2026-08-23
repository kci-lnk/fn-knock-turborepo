import { createApiClient } from "@frontend-core/api/createApiClient";
import { isDockerAdminAuthRequiredResponse } from "../docker-admin-auth-response";
import { createGatewayAuthRecovery } from "../gateway-auth-recovery";
import { isSynologyCgiApiPath } from "./synology-cgi";

export const resolveAppRelativePath = (relativePath: string) => {
  if (typeof window === "undefined") return relativePath;
  const basePath = window.location.pathname.endsWith("/")
    ? window.location.pathname
    : `${window.location.pathname}/`;
  return new URL(relativePath, `${window.location.origin}${basePath}`).pathname;
};

export const adminApiBasePath = resolveAppRelativePath("./api/admin");

export const apiClient = createApiClient({
  baseURL: adminApiBasePath,
  withCredentials: true,
});

const isSynologyCgiApi = isSynologyCgiApiPath(adminApiBasePath);
const cgiMethodOverrides = new Set(["put", "patch", "delete"]);
const gatewayAuthRecovery =
  typeof window === "undefined"
    ? null
    : createGatewayAuthRecovery({
        fetchImpl: window.fetch.bind(window),
        location: window.location,
        navigationTarget: window,
      });

apiClient.interceptors.request.use((config) => {
  const method = config.method?.toLowerCase();
  if (isSynologyCgiApi && method && cgiMethodOverrides.has(method)) {
    config.headers.set("X-HTTP-Method-Override", method.toUpperCase());
    config.method = "post";
  }
  return config;
});

apiClient.interceptors.response.use(
  (response) => response,
  async (error) => {
    if (
      typeof window !== "undefined" &&
      isDockerAdminAuthRequiredResponse(error)
    ) {
      window.dispatchEvent(
        new CustomEvent("fn-knock:docker-admin-auth-required"),
      );
      return Promise.reject(error);
    }

    if (gatewayAuthRecovery && (await gatewayAuthRecovery.recover(error))) {
      // Keep callers pending while the document navigates so feature-level
      // error handlers do not render a transient Network Error notification.
      return new Promise<never>(() => undefined);
    }

    return Promise.reject(error);
  },
);
