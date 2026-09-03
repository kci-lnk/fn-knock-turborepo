/// <reference types="node" />

import assert from "node:assert/strict";
import { describe, it } from "node:test";
import {
  AxiosHeaders,
  type AxiosInstance,
  type InternalAxiosRequestConfig,
} from "axios";

import {
  INVALID_API_RESPONSE_ERROR_CODE,
  createApiClient,
  isInvalidApiResponseError,
} from "../src/api/createApiClient";

interface StubResponse {
  data: unknown;
  headers?: Record<string, string>;
  status?: number;
}

const stubResponse = (client: AxiosInstance, response: StubResponse) => {
  client.defaults.adapter = async (config: InternalAxiosRequestConfig) => ({
    config,
    data: response.data,
    headers: new AxiosHeaders(response.headers),
    status: response.status ?? 200,
    statusText: "OK",
  });
};

describe("shared API response validation", () => {
  it("accepts envelope and direct-object JSON responses", async () => {
    for (const data of [
      { success: true, data: { items: [] } },
      { challenge: "pow" },
    ]) {
      const client = createApiClient({ baseURL: "https://api.example.com" });
      stubResponse(client, {
        data,
        headers: { "Content-Type": "application/json" },
      });

      assert.equal((await client.get("/resource")).data, data);
    }
  });

  it("rejects successful HTML, empty, and scalar responses as API errors", async () => {
    const cases: StubResponse[] = [
      {
        data: "<!doctype html><html><body>login</body></html>",
        headers: { "Content-Type": "text/html; charset=utf-8" },
      },
      { data: undefined },
      { data: null },
      { data: {} },
      { data: [] },
      { data: "not valid json" },
      { data: 42, headers: { "Content-Type": "application/json" } },
    ];

    for (const response of cases) {
      const client = createApiClient({
        baseURL: "https://api.example.com",
        invalidResponseMessage: "Localized invalid response",
      });
      stubResponse(client, response);

      await assert.rejects(client.get("/resource"), (error: unknown) => {
        assert.equal(isInvalidApiResponseError(error), true);
        assert.equal(
          (error as { code?: string }).code,
          INVALID_API_RESPONSE_ERROR_CODE,
        );
        assert.equal(
          (error as { response?: { data?: { message?: string } } }).response
            ?.data?.message,
          "Localized invalid response",
        );
        assert.equal(
          (error as { message?: string }).message,
          "Localized invalid response",
        );
        return true;
      });
    }
  });

  it("allows intentional empty and binary responses", async () => {
    const emptyClient = createApiClient({
      baseURL: "https://api.example.com",
    });
    stubResponse(emptyClient, { data: "", status: 204 });
    assert.equal((await emptyClient.delete("/resource")).status, 204);

    const binaryClient = createApiClient({
      baseURL: "https://api.example.com",
    });
    const archive = new Blob([new Uint8Array([0x50, 0x4b, 0x03, 0x04])], {
      type: "application/zip",
    });
    stubResponse(binaryClient, {
      data: archive,
      headers: { "Content-Type": "application/zip" },
    });
    assert.equal(
      (await binaryClient.get("/archive", { responseType: "blob" })).data,
      archive,
    );
  });

  it("rejects an HTML document returned to a binary download request", async () => {
    const client = createApiClient({ baseURL: "https://api.example.com" });
    stubResponse(client, {
      data: new Blob(["<html><body>login</body></html>"], {
        type: "application/octet-stream",
      }),
      headers: { "Content-Type": "application/octet-stream" },
    });

    await assert.rejects(
      client.get("/archive", { responseType: "blob" }),
      isInvalidApiResponseError,
    );
  });

  it("rejects a JSON error payload returned to a binary download request", async () => {
    const client = createApiClient({ baseURL: "https://api.example.com" });
    stubResponse(client, {
      data: new Blob(['{"success":false,"message":"login required"}'], {
        type: "application/json",
      }),
      headers: { "Content-Type": "application/problem+json" },
    });

    await assert.rejects(
      client.get("/archive", { responseType: "blob" }),
      isInvalidApiResponseError,
    );
  });

  it("allows an explicitly declared HTML document download", async () => {
    const client = createApiClient({ baseURL: "https://api.example.com" });
    const document = new Blob(["<html><body>bookmarks</body></html>"], {
      type: "text/html",
    });
    stubResponse(client, {
      data: document,
      headers: {
        "Content-Disposition": 'attachment; filename="bookmarks.html"',
        "Content-Type": "text/html",
      },
    });

    assert.equal(
      (
        await client.get("/bookmarks", {
          fnKnockAllowDocumentResponse: true,
          responseType: "blob",
        })
      ).data,
      document,
    );

    const jsonClient = createApiClient({
      baseURL: "https://api.example.com",
    });
    stubResponse(jsonClient, {
      data: "<html><body>login</body></html>",
      headers: { "Content-Type": "text/html" },
    });
    await assert.rejects(
      jsonClient.get("/resource", {
        fnKnockAllowDocumentResponse: true,
      }),
      isInvalidApiResponseError,
    );

    const interceptedClient = createApiClient({
      baseURL: "https://api.example.com",
    });
    stubResponse(interceptedClient, {
      data: document,
      headers: { "Content-Type": "text/html" },
    });
    await assert.rejects(
      interceptedClient.get("/bookmarks", {
        fnKnockAllowDocumentResponse: true,
        responseType: "blob",
      }),
      isInvalidApiResponseError,
    );
  });
});
