/// <reference types="node" />

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import {
  createGatewayAuthRecovery,
  isAxiosNetworkErrorWithoutResponse,
  type GatewayAuthRecoveryLocation,
} from "../src/lib/gateway-auth-recovery";

const currentUrl = "https://admin.example.com/settings?tab=update#/details";

const networkError = () => ({
  isAxiosError: true,
  code: "ERR_NETWORK",
  message: "Network Error",
});

const jsonResponse = (payload: unknown, status = 200) =>
  new Response(JSON.stringify(payload), {
    status,
    headers: { "Content-Type": "application/json; charset=utf-8" },
  });

const unauthenticatedPayload = (redirectTo?: string) => ({
  success: true,
  data: {
    auth: { authenticated: false },
    ...(redirectTo === undefined ? {} : { redirect_to: redirectTo }),
  },
});

const createLocation = () => {
  const replacements: string[] = [];
  const location: GatewayAuthRecoveryLocation = {
    href: currentUrl,
    origin: "https://admin.example.com",
    replace(url) {
      replacements.push(url);
    },
  };
  return { location, replacements };
};

const createNavigationTarget = () => {
  const listeners = new Set<EventListenerOrEventListenerObject>();
  const target = {
    addEventListener(
      type: string,
      listener: EventListenerOrEventListenerObject | null,
    ) {
      if (type === "pagehide" && listener) listeners.add(listener);
    },
    removeEventListener(
      type: string,
      listener: EventListenerOrEventListenerObject | null,
    ) {
      if (type === "pagehide" && listener) listeners.delete(listener);
    },
  };
  const dispatchPageHide = () => {
    const event = new Event("pagehide");
    for (const listener of listeners) {
      if (typeof listener === "function") listener(event);
      else listener.handleEvent(event);
    }
  };
  return { target, dispatchPageHide };
};

describe("gateway authentication recovery", () => {
  it("recognizes only Axios network errors without an HTTP response", () => {
    assert.equal(isAxiosNetworkErrorWithoutResponse(networkError()), true);
    assert.equal(
      isAxiosNetworkErrorWithoutResponse({
        isAxiosError: true,
        message: "Network Error",
      }),
      true,
    );
    assert.equal(
      isAxiosNetworkErrorWithoutResponse({
        isAxiosError: true,
        code: "ERR_NETWORK",
        message: "Network Error",
        response: { status: 401 },
      }),
      false,
    );
    assert.equal(
      isAxiosNetworkErrorWithoutResponse({
        isAxiosError: true,
        code: "ERR_BAD_RESPONSE",
        message: "Request failed with status code 500",
      }),
      false,
    );
    assert.equal(isAxiosNetworkErrorWithoutResponse(new Error("offline")), false);
  });

  it("uses the shared authentication redirect returned by bootstrap", async () => {
    const { location, replacements } = createLocation();
    const sharedLoginUrl =
      "https://auth.example.com/login?redirect_uri=https%3A%2F%2Fadmin.example.com%2Fsettings";
    let requestCount = 0;
    const fetchImpl: typeof fetch = async (input, init) => {
      requestCount += 1;
      const requestUrl = new URL(String(input));
      assert.equal(requestUrl.pathname, "/__auth__/api/auth/bootstrap");
      assert.equal(requestUrl.searchParams.get("redirect_uri"), currentUrl);
      assert.equal(requestUrl.searchParams.get("_ts"), "1234");
      assert.equal(init?.method, "GET");
      assert.equal(init?.credentials, "include");
      assert.equal(init?.cache, "no-store");
      const headers = new Headers(init?.headers);
      assert.equal(headers.get("Accept"), "application/json");
      assert.equal(headers.get("Cache-Control"), "no-cache");
      return jsonResponse(unauthenticatedPayload(sharedLoginUrl));
    };
    const recovery = createGatewayAuthRecovery({
      fetchImpl,
      location,
      now: () => 1234,
    });

    assert.equal(await recovery.recover(networkError()), true);
    assert.equal(requestCount, 1);
    assert.deepEqual(replacements, [sharedLoginUrl]);
  });

  it("falls back to the gateway login route and preserves the full page URL", async () => {
    const { location, replacements } = createLocation();
    const recovery = createGatewayAuthRecovery({
      fetchImpl: async () => jsonResponse(unauthenticatedPayload()),
      location,
    });

    assert.equal(await recovery.recover(networkError()), true);
    assert.equal(replacements.length, 1);
    const redirect = new URL(replacements[0]!);
    assert.equal(redirect.origin, location.origin);
    assert.equal(redirect.pathname, "/__auth__/login");
    assert.equal(redirect.searchParams.get("redirect_uri"), currentUrl);
  });

  it("rejects unsafe shared login URLs and uses the same-origin fallback", async () => {
    for (const redirectTo of [
      "javascript:alert(1)",
      "https://user:secret@auth.example.com/login",
    ]) {
      const { location, replacements } = createLocation();
      const recovery = createGatewayAuthRecovery({
        fetchImpl: async () =>
          jsonResponse(unauthenticatedPayload(redirectTo)),
        location,
      });

      assert.equal(await recovery.recover(networkError()), true);
      assert.equal(replacements.length, 1);
      const redirect = new URL(replacements[0]!);
      assert.equal(redirect.origin, location.origin);
      assert.equal(redirect.pathname, "/__auth__/login");
      assert.equal(redirect.searchParams.get("redirect_uri"), currentUrl);
    }
  });

  it("does not redirect without strict unauthenticated JSON evidence", async () => {
    const cases: Array<{ name: string; response: () => Promise<Response> }> = [
      {
        name: "authenticated session",
        response: async () =>
          jsonResponse({
            success: true,
            data: { auth: { authenticated: true } },
          }),
      },
      {
        name: "failed envelope",
        response: async () =>
          jsonResponse({
            success: false,
            data: { auth: { authenticated: false } },
          }),
      },
      {
        name: "malformed payload",
        response: async () => jsonResponse({ success: true, data: {} }),
      },
      {
        name: "non-JSON response",
        response: async () =>
          new Response("<html>login</html>", {
            headers: { "Content-Type": "text/html" },
          }),
      },
      {
        name: "deceptive non-JSON media type",
        response: async () =>
          new Response(JSON.stringify(unauthenticatedPayload()), {
            headers: {
              "Content-Type": "text/html; note=application/json",
            },
          }),
      },
      {
        name: "failed HTTP response",
        response: async () => jsonResponse(unauthenticatedPayload(), 503),
      },
      {
        name: "fetch failure",
        response: async () => {
          throw new TypeError("Failed to fetch");
        },
      },
    ];

    for (const testCase of cases) {
      const { location, replacements } = createLocation();
      const recovery = createGatewayAuthRecovery({
        fetchImpl: testCase.response,
        location,
      });

      assert.equal(
        await recovery.recover(networkError()),
        false,
        testCase.name,
      );
      assert.deepEqual(replacements, [], testCase.name);
    }
  });

  it("times out an unavailable authentication bootstrap", async () => {
    const { location, replacements } = createLocation();
    const fetchImpl: typeof fetch = async (_input, init) =>
      new Promise<Response>((_resolve, reject) => {
        init?.signal?.addEventListener(
          "abort",
          () => reject(new DOMException("Aborted", "AbortError")),
          { once: true },
        );
      });
    const recovery = createGatewayAuthRecovery({
      fetchImpl,
      location,
      timeoutMs: 5,
    });

    assert.equal(await recovery.recover(networkError()), false);
    assert.deepEqual(replacements, []);
  });

  it("restores error handling when a beforeunload prompt cancels navigation", async () => {
    const { location: baseLocation, replacements } = createLocation();
    const { target } = createNavigationTarget();
    let requestCount = 0;
    let confirmReplacement: (() => void) | undefined;
    const replacementStarted = new Promise<void>((resolve) => {
      confirmReplacement = resolve;
    });
    const recovery = createGatewayAuthRecovery({
      fetchImpl: async () => {
        requestCount += 1;
        return jsonResponse(unauthenticatedPayload());
      },
      location: {
        ...baseLocation,
        replace(url) {
          replacements.push(url);
          confirmReplacement?.();
        },
      },
      navigationTarget: target,
      navigationTimeoutMs: 5,
    });

    const first = recovery.recover(networkError());
    await replacementStarted;
    const concurrent = recovery.recover(networkError());
    assert.deepEqual(await Promise.all([first, concurrent]), [false, false]);
    assert.equal(requestCount, 1);
    assert.equal(replacements.length, 1);

    assert.equal(await recovery.recover(networkError()), false);
    assert.equal(requestCount, 2);
    assert.equal(replacements.length, 2);
  });

  it("suppresses the request error only after pagehide confirms navigation", async () => {
    const { location, replacements } = createLocation();
    const { target, dispatchPageHide } = createNavigationTarget();
    const recovery = createGatewayAuthRecovery({
      fetchImpl: async () => jsonResponse(unauthenticatedPayload()),
      location: {
        ...location,
        replace(url) {
          replacements.push(url);
          dispatchPageHide();
        },
      },
      navigationTarget: target,
      navigationTimeoutMs: 5,
    });

    assert.equal(await recovery.recover(networkError()), true);
    assert.equal(replacements.length, 1);
  });

  it("shares one probe and one redirect across concurrent failures", async () => {
    const { location, replacements } = createLocation();
    let requestCount = 0;
    let resolveResponse: ((response: Response) => void) | undefined;
    const responsePromise = new Promise<Response>((resolve) => {
      resolveResponse = resolve;
    });
    const recovery = createGatewayAuthRecovery({
      fetchImpl: async () => {
        requestCount += 1;
        return responsePromise;
      },
      location,
    });

    const first = recovery.recover(networkError());
    const second = recovery.recover(networkError());
    assert.equal(requestCount, 1);
    resolveResponse?.(jsonResponse(unauthenticatedPayload()));

    assert.deepEqual(await Promise.all([first, second]), [true, true]);
    assert.equal(requestCount, 1);
    assert.equal(replacements.length, 1);
    assert.equal(await recovery.recover(networkError()), true);
    assert.equal(requestCount, 1);
    assert.equal(replacements.length, 1);
  });

  it("does not probe ordinary HTTP errors, including marked 401 responses", async () => {
    const { location, replacements } = createLocation();
    let requestCount = 0;
    const recovery = createGatewayAuthRecovery({
      fetchImpl: async () => {
        requestCount += 1;
        return jsonResponse(unauthenticatedPayload());
      },
      location,
    });
    const marked401 = {
      isAxiosError: true,
      code: "ERR_BAD_REQUEST",
      message: "Request failed with status code 401",
      response: {
        status: 401,
        headers: { "x-fn-knock-admin-auth": "required" },
      },
    };

    assert.equal(await recovery.recover(marked401), false);
    assert.equal(requestCount, 0);
    assert.deepEqual(replacements, []);
  });
});
