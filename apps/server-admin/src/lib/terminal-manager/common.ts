import { constants as fsConstants } from "node:fs";
import { homedir } from "node:os";
import { tDefault } from "../i18n";
import type { TerminalTmuxDetectionSource } from "../terminal-shared";

export const DEFAULT_CWD = homedir();

export const terminalT = (
  key: string,
  params?: Record<string, string | number | boolean | null | undefined>,
): string => tDefault(`server.terminal.${key}`, params);

export const TMUX_TARGET_PANE_SUFFIX = ":0.0";
export const TERMINAL_STREAM_DIR_NAME = "terminal-streams";
export const TERMINAL_STREAM_CHUNK_MAX_BYTES = 256 * 1024;
export const TERMINAL_SNAPSHOT_SCROLLBACK_ROWS = 200;
export const INPUT_SESSION_TOUCH_THROTTLE_MS = 5_000;
export const INPUT_PIPE_OPEN_FLAGS =
  fsConstants.O_WRONLY | (fsConstants.O_NONBLOCK ?? 0);
export const DEFAULT_SESSION_TITLE_PREFIX = terminalT(
  "defaultSessionTitlePrefix",
);
export const TMUX_ABSOLUTE_FALLBACK_PATH = "/usr/bin/tmux";
export const DEBIAN_APT_GET_PATH = "/usr/bin/apt-get";
export const ZSH_SHELL_CANDIDATES = ["zsh", "/bin/zsh", "/usr/bin/zsh"];
export const FALLBACK_SHELL_CANDIDATES = [
  "bash",
  "/bin/bash",
  "/usr/bin/bash",
  "sh",
  "/bin/sh",
  "/usr/bin/sh",
];
export const TERMINAL_RELAY_NODE_SCRIPT = [
  "const fs=require('node:fs');",
  "const [logPath,inputPath]=process.argv.slice(-2);",
  "const log=fs.createWriteStream(logPath,{flags:'a'});",
  "const inputFd=fs.openSync(inputPath,'r+');",
  "const input=fs.createReadStream(null,{fd:inputFd,autoClose:true});",
  "log.on('error',()=>process.exit(1));",
  "input.on('error',()=>process.exit(1));",
  "process.stdout.on('error',()=>process.exit(0));",
  "process.stdin.pipe(log);",
  "input.pipe(process.stdout);",
].join("");

export type ExecResult = {
  code: number;
  stdout: string;
  stderr: string;
};

export type CreateSessionInput = {
  title?: string;
  shell?: string;
  cwd?: string;
  cols?: number;
  rows?: number;
};

export type TmuxExecutableInfo = {
  path: string;
  detectionSource: TerminalTmuxDetectionSource;
  version: string;
};

export const parseTmuxNumber = (value: string, fallback: number): number => {
  const parsed = Number.parseInt(value, 10);
  return Number.isFinite(parsed) ? parsed : fallback;
};

export const parseOutputCursor = (
  value: number | string | null | undefined,
  fallback = 0,
): number => {
  const parsed = Number.parseInt(String(value ?? ""), 10);
  if (!Number.isFinite(parsed) || parsed < 0) {
    return fallback;
  }
  return parsed;
};

export const shellQuote = (value: string): string =>
  `'${value.replace(/'/g, `'\"'\"'`)}'`;

const escapeRegExp = (value: string): string =>
  value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

export const dedupeStrings = (values: string[]): string[] =>
  Array.from(new Set(values.map((value) => value.trim()).filter(Boolean)));

export const DEFAULT_SESSION_TITLE_PATTERN = new RegExp(
  `^${escapeRegExp(DEFAULT_SESSION_TITLE_PREFIX)}(\\d+)$`,
);
