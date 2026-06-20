import { createApiClient } from "@frontend-core/api/createApiClient";

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

apiClient.interceptors.response.use(
  (response) => response,
  (error) => {
    if (typeof window !== "undefined" && error?.response?.status === 401) {
      window.dispatchEvent(
        new CustomEvent("fn-knock:docker-admin-auth-required"),
      );
    }
    return Promise.reject(error);
  },
);
