import type { FrpcInstanceRuntime } from "./types";

export const mergeDetectedFrpcRuntime = (
  runtime: FrpcInstanceRuntime,
  pid: number,
  nowIso: () => string,
): FrpcInstanceRuntime => ({
  ...runtime,
  pid,
  startedAt: runtime.startedAt ?? nowIso(),
  stoppedAt: null,
  lastExitCode: null,
  lastMessage:
    runtime.pid === pid &&
    runtime.stoppedAt === null &&
    runtime.lastExitCode === null &&
    runtime.lastMessage
      ? runtime.lastMessage
      : `frpc process detected pid=${pid}`,
});

export const shouldPersistDetectedFrpcRuntime = (
  current: FrpcInstanceRuntime,
  next: FrpcInstanceRuntime,
): boolean =>
  current.pid !== next.pid ||
  current.startedAt !== next.startedAt ||
  current.stoppedAt !== next.stoppedAt ||
  current.lastExitCode !== next.lastExitCode ||
  current.lastMessage !== next.lastMessage;
