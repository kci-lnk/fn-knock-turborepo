import axios, { type AxiosInstance } from "axios";

import { extractErrorMessage } from "../errors/extractErrorMessage";

export interface ApiClientOptions {
  baseURL: string;
  withCredentials?: boolean;
}

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
  return attachApiErrorMessageInterceptor(
    axios.create({
      baseURL: options.baseURL,
      withCredentials: options.withCredentials,
    }),
  );
}
