import { authMobilityKeys } from "./auth-mobility-keys";
import type { AuthMobilityBindingStore } from "./auth-mobility-binding-store";

export type SessionAppAttachment = {
  subjectHash: string;
  currentIp: string;
  createdAt: string;
  lastSeenAt: string;
  expiresAt: string | null;
};

export type SessionFnosAttachment = SessionAppAttachment;
export type SessionTrimMediaAttachment = SessionAppAttachment;

export const listSessionAttachments = async (args: {
  bindingStore: AuthMobilityBindingStore;
  sessionId: string;
  subjectType: "fnos-token" | "trim-media-token";
}): Promise<SessionAppAttachment[]> => {
  const subjectKeys = await args.bindingStore.listSessionBindingKeys(
    args.sessionId,
  );
  const attachmentKeys = subjectKeys.filter((key) =>
    authMobilityKeys.isBindingForSubject(key, args.subjectType),
  );
  if (attachmentKeys.length === 0) {
    return [];
  }

  const resolved = await Promise.all(
    attachmentKeys.map(async (storageKey) => {
      const binding = await args.bindingStore.getByStorageKey(storageKey);
      return { storageKey, binding };
    }),
  );

  const staleKeys = resolved
    .filter(
      ({ binding }) =>
        !binding ||
        binding.subjectType !== args.subjectType ||
        binding.ownerSessionId !== args.sessionId,
    )
    .map(({ storageKey }) => storageKey);

  if (staleKeys.length > 0) {
    await args.bindingStore.removeSessionBindings(args.sessionId, staleKeys);
  }

  return resolved
    .flatMap(({ binding }) => {
      if (
        !binding ||
        binding.subjectType !== args.subjectType ||
        binding.ownerSessionId !== args.sessionId
      ) {
        return [];
      }

      return [
        {
          subjectHash: binding.subjectHash,
          currentIp: binding.currentIp,
          createdAt: binding.createdAt,
          lastSeenAt: binding.lastSeenAt,
          expiresAt: binding.expireAt
            ? new Date(binding.expireAt * 1000).toISOString()
            : null,
        } satisfies SessionAppAttachment,
      ];
    })
    .sort((a, b) => {
      return (Date.parse(b.lastSeenAt) || 0) - (Date.parse(a.lastSeenAt) || 0);
    });
};
