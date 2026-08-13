import type { InternalAxiosRequestConfig } from "axios";
import { createApiClient } from "./createApiClient";

export interface SignedApiClientOptions {
  baseURL: string;
  hmacSecret?: string;
  getHmacSecret?: () => string | undefined | Promise<string | undefined>;
}

function serializeSignedBody(data: unknown): string {
  if (data === undefined || data === null) return "";
  if (typeof data === "string") return data;
  if (data instanceof URLSearchParams) return data.toString();
  return JSON.stringify(data);
}

function signedRequestUri(uri: string) {
  const parsed = new URL(uri, "http://fn-knock.local");
  return `${parsed.pathname}${parsed.search}`;
}

function secureNonce() {
  const bytes = new Uint8Array(16);
  globalThis.crypto.getRandomValues(bytes);
  return Array.from(bytes, (value) => value.toString(16).padStart(2, "0")).join(
    "",
  );
}

const bytesToHex = (bytes: ArrayBuffer) =>
  Array.from(new Uint8Array(bytes), (value) =>
    value.toString(16).padStart(2, "0"),
  ).join("");

const sha256Hex = async (value: string, encoder: TextEncoder) => {
  if (globalThis.crypto?.subtle) {
    return bytesToHex(
      await globalThis.crypto.subtle.digest("SHA-256", encoder.encode(value)),
    );
  }
  const [{ default: sha256 }, { default: hex }] = await Promise.all([
    import("crypto-js/sha256"),
    import("crypto-js/enc-hex"),
  ]);
  return sha256(value).toString(hex);
};

const hmacSha256Hex = async (
  message: string,
  secret: string,
  encoder: TextEncoder,
) => {
  if (globalThis.crypto?.subtle) {
    const key = await globalThis.crypto.subtle.importKey(
      "raw",
      encoder.encode(secret),
      { name: "HMAC", hash: "SHA-256" },
      false,
      ["sign"],
    );
    return bytesToHex(
      await globalThis.crypto.subtle.sign("HMAC", key, encoder.encode(message)),
    );
  }
  const [{ default: hmacSha256 }, { default: hex }] = await Promise.all([
    import("crypto-js/hmac-sha256"),
    import("crypto-js/enc-hex"),
  ]);
  return hmacSha256(message, secret).toString(hex);
};

export async function buildRequestSignature(
  hmacSecret: string,
  method: string,
  requestUri: string,
  body: string,
) {
  const timestamp = Date.now().toString();
  const nonce = secureNonce();
  const encoder = new TextEncoder();
  const bodyDigest = await sha256Hex(body, encoder);
  const message = [
    "fn-knock-v1",
    method.toUpperCase(),
    signedRequestUri(requestUri),
    bodyDigest,
    timestamp,
    nonce,
  ].join("\n");
  const signature = await hmacSha256Hex(message, hmacSecret, encoder);

  return { timestamp, nonce, signature };
}

export function createSignedApiClient(options: SignedApiClientOptions) {
  const apiClient = createApiClient({
    baseURL: options.baseURL,
  });

  let cachedSecret = options.hmacSecret;

  apiClient.interceptors.request.use(
    async (config: InternalAxiosRequestConfig) => {
      if (!cachedSecret && options.getHmacSecret) {
        cachedSecret = await options.getHmacSecret();
      }
      if (!cachedSecret) {
        // Production browser traffic is signed by the trusted Go gateway. An
        // explicit secret remains available for direct local development only.
        return config;
      }
      const body = serializeSignedBody(config.data);
      if (config.data !== undefined && typeof config.data !== "string") {
        config.data = body;
      }
      const { timestamp, nonce, signature } = await buildRequestSignature(
        cachedSecret,
        config.method || "GET",
        apiClient.getUri(config),
        body,
      );

      config.headers["x-timestamp"] = timestamp;
      config.headers["x-nonce"] = nonce;
      config.headers["x-signature"] = signature;

      return config;
    },
  );
  return apiClient;
}
