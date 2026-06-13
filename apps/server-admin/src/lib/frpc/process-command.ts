import path from "node:path";

const splitCommandLine = (command: string): string[] => {
  const args: string[] = [];
  let current = "";
  let quote: "'" | '"' | null = null;
  let escaped = false;
  let inArg = false;

  for (const char of command.trim()) {
    if (escaped) {
      current += char;
      escaped = false;
      inArg = true;
      continue;
    }

    if (char === "\\" && quote !== "'") {
      escaped = true;
      inArg = true;
      continue;
    }

    if (quote) {
      if (char === quote) {
        quote = null;
      } else {
        current += char;
      }
      inArg = true;
      continue;
    }

    if (char === "'" || char === '"') {
      quote = char;
      inArg = true;
      continue;
    }

    if (/\s/.test(char)) {
      if (inArg) {
        args.push(current);
        current = "";
        inArg = false;
      }
      continue;
    }

    current += char;
    inArg = true;
  }

  if (escaped) {
    current += "\\";
  }
  if (inArg) {
    args.push(current);
  }

  return args;
};

export const parseProcessCommandLine = (raw: Buffer | string): string[] => {
  const command = Buffer.isBuffer(raw) ? raw.toString("utf-8") : raw;
  if (!command.trim()) return [];
  if (command.includes("\0")) {
    return command
      .split("\0")
      .map((arg) => arg.trim())
      .filter(Boolean);
  }
  return splitCommandLine(command);
};

const normalizeComparablePath = (value: string): string =>
  path.resolve(value).replace(/\\/g, "/");

const isFrpcExecutable = (value: string | undefined): boolean => {
  if (!value) return false;
  const executable = path.basename(value).toLowerCase();
  return executable === "frpc" || executable === "frpc.exe";
};

const matchesConfigPath = (
  candidate: string | undefined,
  configPath: string,
): boolean => {
  if (!candidate) return false;
  return (
    normalizeComparablePath(candidate) === normalizeComparablePath(configPath)
  );
};

export const isFrpcProcessArgsForConfig = (
  args: readonly string[],
  configPath: string,
): boolean => {
  if (!isFrpcExecutable(args[0])) return false;

  for (let index = 1; index < args.length; index += 1) {
    const arg = args[index];
    if (!arg) continue;

    if (arg === "-c" || arg === "--config" || arg === "--config-file") {
      return matchesConfigPath(args[index + 1], configPath);
    }

    if (arg.startsWith("--config=")) {
      return matchesConfigPath(arg.slice("--config=".length), configPath);
    }

    if (arg.startsWith("--config-file=")) {
      return matchesConfigPath(arg.slice("--config-file=".length), configPath);
    }
  }

  return false;
};
