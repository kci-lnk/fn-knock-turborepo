export const toUnixSeconds = (iso?: string): number | null => {
  if (!iso) return null;
  const ms = Date.parse(iso);
  if (!Number.isFinite(ms)) return null;
  return Math.floor(ms / 1000);
};

export const nowSeconds = () => Math.floor(Date.now() / 1000);

export const remainingSeconds = (expireAt: number | null): number | null => {
  if (expireAt === null) return null;
  const remaining = expireAt - nowSeconds();
  if (remaining <= 0) return null;
  return remaining;
};

export const resolveProxySessionTTL = (expireAt: number | null) =>
  remainingSeconds(expireAt);

export const resolveFnosTTL = (expireAt: number | null) =>
  remainingSeconds(expireAt);

export const resolveFnosSessionTTL = (expiresAt?: string) =>
  resolveFnosTTL(toUnixSeconds(expiresAt));
