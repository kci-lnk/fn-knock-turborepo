import axios, { type InternalAxiosRequestConfig } from 'axios';
import CryptoJS from 'crypto-js';

export interface SignedApiClientOptions {
  baseURL: string;
  hmacSecret?: string;
  getHmacSecret?: () => string | undefined | Promise<string | undefined>;
}

export function buildRequestSignature(hmacSecret: string) {
  const timestamp = Date.now().toString();
  const nonce =
    Math.random().toString(36).substring(2, 15) +
    Math.random().toString(36).substring(2, 15);
  const message = `${timestamp}:${nonce}`;
  const signature = CryptoJS.HmacSHA256(message, hmacSecret).toString(
    CryptoJS.enc.Hex,
  );

  return { timestamp, nonce, signature };
}

export function createSignedApiClient(options: SignedApiClientOptions) {
  const apiClient = axios.create({
    baseURL: options.baseURL,
  });

  let cachedSecret = options.hmacSecret;

  apiClient.interceptors.request.use(async (config: InternalAxiosRequestConfig) => {
    if (!cachedSecret && options.getHmacSecret) {
      cachedSecret = await options.getHmacSecret();
    }
    if (!cachedSecret) {
      throw new Error("Missing HMAC secret for signed API request");
    }
    const { timestamp, nonce, signature } = buildRequestSignature(
      cachedSecret,
    );

    config.headers['x-timestamp'] = timestamp;
    config.headers['x-nonce'] = nonce;
    config.headers['x-signature'] = signature;

    return config;
  });

  return apiClient;
}
