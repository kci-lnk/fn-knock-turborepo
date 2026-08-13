import type { CaptchaSubmission } from "@frontend-core/captcha/types";

const SUPPORTED_POW_ALGORITHMS = ["SHA-256", "SHA-384", "SHA-512"] as const;

type PowAlgorithm = (typeof SUPPORTED_POW_ALGORITHMS)[number];
export type CaptchaErrorCode =
  "powUnsupportedAlgorithm" | "powInvalidChallenge" | "powSolveFailed";

export class CaptchaError extends Error {
  readonly code: CaptchaErrorCode;

  constructor(code: CaptchaErrorCode) {
    super(code);
    this.name = "CaptchaError";
    this.code = code;
  }
}

export type PowChallenge = {
  algorithm: PowAlgorithm;
  challenge: string;
  maxnumber: number;
  salt: string;
  signature: string;
};

export const normalizePowChallenge = (payload: unknown): PowChallenge => {
  const raw = payload as Partial<PowChallenge> | null;
  const algorithmRaw = String(raw?.algorithm || "SHA-256").toUpperCase();
  if (!SUPPORTED_POW_ALGORITHMS.includes(algorithmRaw as PowAlgorithm)) {
    throw new CaptchaError("powUnsupportedAlgorithm");
  }

  const challenge = String(raw?.challenge || "").toLowerCase();
  const salt = String(raw?.salt || "");
  const signature = String(raw?.signature || "");
  const maxnumber = Number(raw?.maxnumber);
  if (
    !challenge ||
    !salt ||
    !signature ||
    !Number.isFinite(maxnumber) ||
    maxnumber < 0
  ) {
    throw new CaptchaError("powInvalidChallenge");
  }

  return {
    algorithm: algorithmRaw as PowAlgorithm,
    challenge,
    maxnumber: Math.floor(maxnumber),
    salt,
    signature,
  };
};

export const solvePowChallenge = async (
  challenge: PowChallenge,
  signal?: AbortSignal,
): Promise<number> => {
  if (signal?.aborted) throw new DOMException("Aborted", "AbortError");
  const worker = new Worker(new URL("./pow.worker.ts", import.meta.url), {
    type: "module",
    name: "fn-knock-pow",
  });
  return new Promise<number>((resolve, reject) => {
    let settled = false;
    const finish = (result: { number?: number; error?: unknown }) => {
      if (settled) return;
      settled = true;
      signal?.removeEventListener("abort", handleAbort);
      worker.terminate();
      if (Number.isInteger(result.number)) {
        resolve(result.number as number);
      } else {
        reject(result.error ?? new CaptchaError("powSolveFailed"));
      }
    };
    const handleAbort = () =>
      finish({ error: new DOMException("Aborted", "AbortError") });
    worker.onmessage = (
      event: MessageEvent<{ number?: number; error?: string }>,
    ) => {
      finish({
        number: event.data.number,
        error: event.data.error
          ? new CaptchaError("powSolveFailed")
          : undefined,
      });
    };
    worker.onerror = () => {
      finish({ error: new CaptchaError("powSolveFailed") });
    };
    worker.onmessageerror = () => {
      finish({ error: new CaptchaError("powSolveFailed") });
    };
    signal?.addEventListener("abort", handleAbort, { once: true });
    try {
      worker.postMessage(challenge);
    } catch (error) {
      finish({ error });
    }
  });
};

export const buildPowSubmission = (
  challenge: PowChallenge,
  number: number,
): CaptchaSubmission => ({
  provider: "pow",
  proof: btoa(
    JSON.stringify({
      algorithm: challenge.algorithm,
      challenge: challenge.challenge,
      number,
      salt: challenge.salt,
      signature: challenge.signature,
    }),
  ),
});
