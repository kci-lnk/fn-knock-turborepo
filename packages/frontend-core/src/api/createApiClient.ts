import axios, {
  AxiosError,
  type AxiosInstance,
  type AxiosResponse,
} from "axios";

import { extractErrorMessage } from "../errors/extractErrorMessage";

declare module "axios" {
  interface AxiosRequestConfig<D = any> {
    /** Allow an explicitly requested Blob download to contain an HTML document. */
    fnKnockAllowDocumentResponse?: boolean;
  }
}

export interface ApiClientOptions {
  baseURL: string;
  invalidResponseMessage?: string | (() => string);
  withCredentials?: boolean;
}

export const INVALID_API_RESPONSE_ERROR_CODE =
  "ERR_FN_KNOCK_INVALID_API_RESPONSE";

const DEFAULT_INVALID_RESPONSE_MESSAGE =
  "The server returned an invalid response. Refresh or reopen this page and try again.";

const documentMediaTypes = new Set(["text/html", "application/xhtml+xml"]);

const isJsonMediaType = (value: string): boolean =>
  value === "application/json" || value.endsWith("+json");

const responseHeaderValue = (
  response: AxiosResponse<unknown>,
  name: string,
): string => {
  const headers = response.headers as {
    get?: (name: string) => unknown;
    [name: string]: unknown;
  };
  const value =
    typeof headers.get === "function"
      ? headers.get(name)
      : headers[name.toLowerCase()];
  return String(value ?? "");
};

const responseMediaType = (response: AxiosResponse<unknown>): string =>
  responseHeaderValue(response, "content-type")
    .split(";", 1)[0]!
    .trim()
    .toLowerCase();

const isAttachmentResponse = (response: AxiosResponse<unknown>): boolean =>
  /^attachment(?:\s*;|\s*$)/iu.test(
    responseHeaderValue(response, "content-disposition").trim(),
  );

const looksLikeHtml = (value: string): boolean =>
  /^\s*(?:<!doctype\s+html\b|<html\b|<head\b|<body\b)/iu.test(value);

const looksLikeHtmlBytes = (value: ArrayBuffer | ArrayBufferView): boolean => {
  const bytes =
    value instanceof ArrayBuffer
      ? new Uint8Array(value, 0, Math.min(value.byteLength, 512))
      : new Uint8Array(
          value.buffer,
          value.byteOffset,
          Math.min(value.byteLength, 512),
        );
  return looksLikeHtml(new TextDecoder().decode(bytes));
};

const isExplicitNonJsonResponse = (
  response: AxiosResponse<unknown>,
): boolean => {
  const responseType = response.config.responseType;
  return responseType != null && responseType !== "json";
};

const isInvalidSuccessfulResponse = async (
  response: AxiosResponse<unknown>,
): Promise<boolean> => {
  const allowDocumentResponse =
    response.config.responseType === "blob" &&
    response.config.fnKnockAllowDocumentResponse === true &&
    isAttachmentResponse(response);
  const mediaType = responseMediaType(response);
  if (!allowDocumentResponse && documentMediaTypes.has(mediaType)) return true;
  if (
    (response.config.responseType === "blob" ||
      response.config.responseType === "arraybuffer") &&
    isJsonMediaType(mediaType)
  ) {
    return true;
  }

  const { data } = response;
  if (!allowDocumentResponse && typeof data === "string" && looksLikeHtml(data))
    return true;
  if (
    !allowDocumentResponse &&
    data instanceof ArrayBuffer &&
    looksLikeHtmlBytes(data)
  )
    return true;
  if (
    !allowDocumentResponse &&
    ArrayBuffer.isView(data) &&
    looksLikeHtmlBytes(data)
  )
    return true;
  if (typeof Blob !== "undefined" && data instanceof Blob) {
    const blobMediaType = data.type.split(";", 1)[0]!.trim().toLowerCase();
    if (!allowDocumentResponse && documentMediaTypes.has(blobMediaType))
      return true;
    if (
      !allowDocumentResponse &&
      looksLikeHtml(await data.slice(0, 512).text())
    )
      return true;
  }

  const method = response.config.method?.toLowerCase();
  if (method === "head" || response.status === 204 || response.status === 205) {
    return false;
  }
  if (isExplicitNonJsonResponse(response)) return false;

  return (
    typeof data !== "object" ||
    data === null ||
    Array.isArray(data) ||
    Object.keys(data).length === 0
  );
};

const resolveInvalidResponseMessage = (
  value: ApiClientOptions["invalidResponseMessage"],
): string => {
  try {
    const message = typeof value === "function" ? value() : value;
    return message?.trim() || DEFAULT_INVALID_RESPONSE_MESSAGE;
  } catch {
    return DEFAULT_INVALID_RESPONSE_MESSAGE;
  }
};

const invalidResponseError = (
  response: AxiosResponse<unknown>,
  message: string,
) =>
  new AxiosError(
    message,
    INVALID_API_RESPONSE_ERROR_CODE,
    response.config,
    response.request,
    {
      ...response,
      // Do not leak an HTML login/index document through generic error
      // extractors. Callers receive a stable API-shaped error instead.
      data: { success: false, message },
    },
  );

export const isInvalidApiResponseError = (error: unknown): boolean =>
  axios.isAxiosError(error) && error.code === INVALID_API_RESPONSE_ERROR_CODE;

export const attachApiResponseValidationInterceptor = (
  apiClient: AxiosInstance,
  invalidResponseMessage?: ApiClientOptions["invalidResponseMessage"],
) => {
  apiClient.interceptors.response.use(async (response) => {
    if (await isInvalidSuccessfulResponse(response)) {
      throw invalidResponseError(
        response,
        resolveInvalidResponseMessage(invalidResponseMessage),
      );
    }
    return response;
  });

  return apiClient;
};

export const attachApiErrorMessageInterceptor = (apiClient: AxiosInstance) => {
  apiClient.interceptors.response.use(
    (response) => response,
    (error) => {
      if (axios.isAxiosError(error)) {
        const message = extractErrorMessage(error, "");
        if (message) {
          error.message = message;
        }
      }

      return Promise.reject(error);
    },
  );

  return apiClient;
};

export function createApiClient(options: ApiClientOptions) {
  const apiClient = axios.create({
    baseURL: options.baseURL,
    withCredentials: options.withCredentials,
  });
  attachApiResponseValidationInterceptor(
    apiClient,
    options.invalidResponseMessage,
  );
  return attachApiErrorMessageInterceptor(apiClient);
}
