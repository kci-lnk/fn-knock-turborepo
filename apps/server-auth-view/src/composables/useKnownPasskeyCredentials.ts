export interface CredentialDigestStorage {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
  removeItem(key: string): void;
}

type CredentialIdHasher = (credentialId: string) => Promise<string | null>;

interface UseKnownPasskeyCredentialsOptions {
  storage?: CredentialDigestStorage | null;
  storageKey?: string;
  hashCredentialId?: CredentialIdHasher;
}

export const KNOWN_PASSKEY_CREDENTIAL_DIGESTS_STORAGE_KEY =
  "server-auth-view:known-passkey-credential-digests";

export const normalizePasskeyCredentialIds = (value: unknown): string[] => {
  if (!Array.isArray(value)) {
    return [];
  }

  return [
    ...new Set(
      value
        .filter((item): item is string => typeof item === "string")
        .map((item) => item.trim())
        .filter(Boolean),
    ),
  ];
};

const getBrowserStorage = (): CredentialDigestStorage | null => {
  if (typeof window === "undefined") {
    return null;
  }

  try {
    return window.localStorage;
  } catch {
    return null;
  }
};

export const hashPasskeyCredentialId: CredentialIdHasher = async (
  credentialId,
) => {
  if (
    typeof window === "undefined" ||
    !window.isSecureContext ||
    typeof window.crypto === "undefined" ||
    !window.crypto.subtle
  ) {
    return null;
  }

  const normalizedCredentialId = credentialId.trim();
  if (!normalizedCredentialId) {
    return null;
  }

  const bytes = new TextEncoder().encode(normalizedCredentialId);
  const digest = await window.crypto.subtle.digest("SHA-256", bytes);
  return Array.from(new Uint8Array(digest), (value) =>
    value.toString(16).padStart(2, "0"),
  ).join("");
};

export const useKnownPasskeyCredentials = (
  options: UseKnownPasskeyCredentialsOptions = {},
) => {
  const storage =
    options.storage === undefined ? getBrowserStorage() : options.storage;
  const storageKey =
    options.storageKey ?? KNOWN_PASSKEY_CREDENTIAL_DIGESTS_STORAGE_KEY;
  const hashCredentialId = options.hashCredentialId ?? hashPasskeyCredentialId;

  const readKnownPasskeyCredentialDigests = () => {
    if (!storage) {
      return [] as string[];
    }

    try {
      const raw = storage.getItem(storageKey);
      return raw
        ? normalizePasskeyCredentialIds(JSON.parse(raw))
        : ([] as string[]);
    } catch {
      return [] as string[];
    }
  };

  const persistKnownPasskeyCredentialDigests = (digests: string[]) => {
    if (!storage) {
      return false;
    }

    try {
      const normalizedDigests = normalizePasskeyCredentialIds(digests);
      if (normalizedDigests.length === 0) {
        storage.removeItem(storageKey);
      } else {
        storage.setItem(storageKey, JSON.stringify(normalizedDigests));
      }
      return true;
    } catch {
      return false;
    }
  };

  const rememberKnownPasskeyCredentialId = async (credentialId: unknown) => {
    if (typeof credentialId !== "string") {
      return false;
    }

    const digest = await hashCredentialId(credentialId);
    if (!digest) {
      return false;
    }

    const knownDigests = readKnownPasskeyCredentialDigests();
    if (knownDigests.includes(digest)) {
      return true;
    }

    return persistKnownPasskeyCredentialDigests([...knownDigests, digest]);
  };

  const hasKnownPasskeyCredential = async (credentialIds: unknown) => {
    const knownDigests = new Set(readKnownPasskeyCredentialDigests());
    if (knownDigests.size === 0) {
      return false;
    }

    for (const credentialId of normalizePasskeyCredentialIds(credentialIds)) {
      const digest = await hashCredentialId(credentialId);
      if (digest && knownDigests.has(digest)) {
        return true;
      }
    }

    return false;
  };

  return {
    hasKnownPasskeyCredential,
    persistKnownPasskeyCredentialDigests,
    readKnownPasskeyCredentialDigests,
    rememberKnownPasskeyCredentialId,
  };
};
