const readNonEmptyString = (value: unknown): string | null => {
  if (typeof value !== "string") return null;
  const normalized = value.trim();
  return normalized || null;
};

export function extractErrorMessage(
  error: unknown,
  fallback = "Operation failed",
): string {
  if (!error || typeof error !== "object") return fallback;

  const responseData = (error as { response?: { data?: unknown } }).response
    ?.data;
  const responseMessage =
    readNonEmptyString(responseData) ??
    (responseData && typeof responseData === "object"
      ? readNonEmptyString((responseData as { message?: unknown }).message)
      : null);

  return (
    responseMessage ??
    readNonEmptyString((error as { message?: unknown }).message) ??
    fallback
  );
}
