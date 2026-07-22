export const DOCKER_ADMIN_AUTH_RESPONSE_HEADER =
  "x-fn-knock-admin-auth";

const readHeader = (headers: unknown, name: string): string => {
  if (!headers || typeof headers !== "object") return "";

  const getter = (headers as { get?: unknown }).get;
  if (typeof getter === "function") {
    const value = getter.call(headers, name);
    return typeof value === "string" ? value.trim() : "";
  }

  for (const [key, value] of Object.entries(headers)) {
    if (key.toLowerCase() !== name) continue;
    if (Array.isArray(value)) return String(value[0] ?? "").trim();
    return typeof value === "string" ? value.trim() : "";
  }
  return "";
};

export const isDockerAdminAuthRequiredResponse = (error: unknown): boolean => {
  if (!error || typeof error !== "object") return false;
  const response = (error as { response?: unknown }).response;
  if (!response || typeof response !== "object") return false;

  const typedResponse = response as { status?: unknown; headers?: unknown };
  return (
    typedResponse.status === 401 &&
    readHeader(typedResponse.headers, DOCKER_ADMIN_AUTH_RESPONSE_HEADER) ===
      "required"
  );
};
