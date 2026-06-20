import { basename } from "node:path";
import {
  FALLBACK_SHELL_CANDIDATES,
  ZSH_SHELL_CANDIDATES,
  dedupeStrings,
  shellQuote,
  terminalT,
} from "./common";

type ShellAvailabilityProbe = (command: string) => Promise<boolean>;

export const isZshShell = (shell: string): boolean =>
  basename(shell).toLowerCase() === "zsh";

export const buildAutoShellCandidates = (): string[] => {
  const envShell = (process.env.SHELL || "").trim();
  return dedupeStrings([
    // Prefer zsh so Oh My Zsh works in the web terminal without extra setup.
    ...(envShell && isZshShell(envShell) ? [envShell] : []),
    ...ZSH_SHELL_CANDIDATES,
    envShell,
    ...FALLBACK_SHELL_CANDIDATES,
  ]);
};

const pickAvailableShell = async (
  candidates: string[],
  canStartShell: ShellAvailabilityProbe,
): Promise<string | null> => {
  for (const candidate of dedupeStrings(candidates)) {
    if (await canStartShell(candidate)) {
      return candidate;
    }
  }
  return null;
};

export const resolveTerminalShell = async (
  shell: string | undefined,
  canStartShell: ShellAvailabilityProbe,
): Promise<string> => {
  const requestedShell = (shell || "").trim();
  if (requestedShell) {
    const resolvedRequestedShell = await pickAvailableShell(
      [requestedShell],
      canStartShell,
    );
    if (!resolvedRequestedShell) {
      throw new Error(
        terminalT("requestedShellUnavailable", { shell: requestedShell }),
      );
    }
    return resolvedRequestedShell;
  }

  const autoDetectedShell = await pickAvailableShell(
    buildAutoShellCandidates(),
    canStartShell,
  );
  if (autoDetectedShell) {
    return autoDetectedShell;
  }

  throw new Error(terminalT("noShellDetected"));
};

export const buildSessionShellCommand = (shell: string): string => {
  if (isZshShell(shell)) {
    return `exec ${shellQuote(shell)} -il`;
  }
  return `exec ${shellQuote(shell)}`;
};
