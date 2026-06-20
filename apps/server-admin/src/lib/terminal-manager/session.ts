import { join } from "node:path";
import type { TerminalSessionRecord } from "../terminal-shared";
import {
  DEFAULT_SESSION_TITLE_PATTERN,
  DEFAULT_SESSION_TITLE_PREFIX,
  TERMINAL_RELAY_NODE_SCRIPT,
  TMUX_TARGET_PANE_SUFFIX,
  shellQuote,
} from "./common";

export const buildSessionName = (id: string): string =>
  `fnk_${id.replace(/-/g, "").slice(0, 16)}`;

export const buildOutputLogPath = (
  streamDirectory: string,
  id: string,
): string => join(streamDirectory, `${id}.log`);

export const buildInputPipePath = (
  streamDirectory: string,
  id: string,
): string => join(streamDirectory, `${id}.in`);

export const paneTarget = (session: TerminalSessionRecord): string =>
  `${session.backend_session_name}${TMUX_TARGET_PANE_SUFFIX}`;

export const sanitizeTitle = (rawTitle: string | undefined): string =>
  (rawTitle || "").trim();

export const buildDefaultSessionTitle = (
  existingSessions: TerminalSessionRecord[],
): string => {
  const usedIndexes = new Set<number>();
  for (const session of existingSessions) {
    const match = session.title.trim().match(DEFAULT_SESSION_TITLE_PATTERN);
    if (!match) continue;
    const parsed = Number.parseInt(match[1]!, 10);
    if (Number.isFinite(parsed) && parsed > 0) {
      usedIndexes.add(parsed);
    }
  }

  let nextIndex = 1;
  while (usedIndexes.has(nextIndex)) {
    nextIndex += 1;
  }

  return `${DEFAULT_SESSION_TITLE_PREFIX}${nextIndex}`;
};

export const formatIoError = (prefix: string, error: unknown): string => {
  const detail = error instanceof Error ? error.message.trim() : "";
  return detail ? `${prefix}: ${detail}` : prefix;
};

export const buildRelayCommand = (
  outputLogPath: string,
  inputPipePath: string,
): string =>
  [
    shellQuote(process.execPath),
    "-e",
    shellQuote(TERMINAL_RELAY_NODE_SCRIPT),
    shellQuote(outputLogPath),
    shellQuote(inputPipePath),
  ].join(" ");
