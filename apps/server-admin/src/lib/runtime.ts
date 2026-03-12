import { constants } from "node:fs";
import { access } from "node:fs/promises";
import type { ChildProcess } from "node:child_process";

export const sleep = (ms: number): Promise<void> =>
  new Promise((resolve) => setTimeout(resolve, ms));

export const waitForProcessExit = (proc: ChildProcess): Promise<number> =>
  new Promise((resolve, reject) => {
    proc.once("error", reject);
    proc.once("close", (code) => resolve(code ?? -1));
  });

export const collectStreamOutput = async (
  stream: NodeJS.ReadableStream | null | undefined,
): Promise<string> => {
  if (!stream) return "";

  const chunks: Buffer[] = [];
  for await (const chunk of stream) {
    if (typeof chunk === "string") {
      chunks.push(Buffer.from(chunk));
    } else {
      chunks.push(Buffer.from(chunk));
    }
  }
  return Buffer.concat(chunks).toString("utf-8");
};

export const fileExists = async (path: string): Promise<boolean> => {
  try {
    await access(path, constants.F_OK);
    return true;
  } catch {
    return false;
  }
};
