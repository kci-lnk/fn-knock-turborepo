import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import http from "node:http";
import https from "node:https";
import net from "node:net";
import os from "node:os";
import path from "node:path";
import { promisify } from "node:util";
import { brotliDecompressSync, gunzipSync, inflateSync } from "node:zlib";
import { chromium } from "playwright";
import { fetchRuntime, startRuntime } from "./runtime-test-harness.mjs";

// Real HTTPS browser cookies and public admin APIs, with an external client
// address carried over trusted PROXY protocol to avoid local-network exemptions.
const cookieName = "x-go-reauth-proxy-session-id";
const username = "single-service";
const password = "single-service-password123";
const temporary = await mkdtemp(path.join(os.tmpdir(), "auth-single-service-"));
let runtime;
let browser;
let ingress;
let upstream;
let agent;
let recordLogin;
const listen = (server) =>
  new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", () => resolve(server.address().port));
  });
const close = async (server) => {
  if (!server) return;
  server.closeAllConnections();
  await new Promise((resolve) => server.close(resolve));
};

try {
  runtime = await startRuntime({
    collectMetrics: false,
    gatewayBinary: process.env.FN_KNOCK_RUNTIME_GATEWAY_BIN,
    serverBinary:
      process.env.FN_KNOCK_RUNTIME_SERVER_BIN ??
      "apps/server-admin-rs/target/runtime-test/server-admin-rs",
  });
  const api = async (pathname, body, method = "POST") => {
    const response = await fetchRuntime(runtime.backendUrl + pathname, {
      method,
      headers: { "content-type": "application/json" },
      body: JSON.stringify(body),
    });
    const result = await response.json();
    assert.equal(
      response.status,
      200,
      `${pathname}: ${JSON.stringify(result)}`,
    );
    return result.data;
  };
  upstream = http.createServer((request, response) => {
    response.writeHead(200, { "content-type": "text/html" });
    response.end(`<html><body>single-service-ok:${request.url}</body></html>`);
  });
  const upstreamPort = await listen(upstream);
  await promisify(execFile)("openssl", [
    "req",
    "-x509",
    "-newkey",
    "rsa:2048",
    "-nodes",
    "-days",
    "1",
    "-subj",
    "/CN=*.abc.com",
    "-keyout",
    path.join(temporary, "key.pem"),
    "-out",
    path.join(temporary, "cert.pem"),
  ]);
  const gatewayPort = Number(new URL(runtime.gatewayProxyUrl).port);
  agent = new http.Agent({ keepAlive: false });
  agent.createConnection = (_options, callback) => {
    const socket = net.connect(gatewayPort, "127.0.0.1");
    socket.once("error", callback);
    socket.once("connect", () => {
      socket.removeListener("error", callback);
      socket.write(
        `PROXY TCP4 203.0.113.79 127.0.0.1 45678 ${gatewayPort}\r\n`,
      );
      callback(null, socket);
    });
  };
  ingress = https.createServer(
    {
      key: await readFile(path.join(temporary, "key.pem")),
      cert: await readFile(path.join(temporary, "cert.pem")),
    },
    (request, response) => {
      const headers = { ...request.headers, "x-forwarded-proto": "https" };
      for (const header of [
        "forwarded",
        "x-forwarded-for",
        "x-real-ip",
        "cf-connecting-ip",
      ])
        delete headers[header];
      const forwarded = http.request(
        {
          hostname: "127.0.0.1",
          port: gatewayPort,
          agent,
          method: request.method,
          path: request.url,
          headers,
        },
        (reply) => {
          if (
            request.method === "POST" &&
            request.url.endsWith("/api/auth/login")
          ) {
            const chunks = [];
            reply.on("data", (chunk) => chunks.push(chunk));
            reply.on("end", () => {
              const decode =
                {
                  br: brotliDecompressSync,
                  gzip: gunzipSync,
                  deflate: inflateSync,
                }[reply.headers["content-encoding"]] ?? ((body) => body);
              recordLogin?.({
                status: reply.statusCode,
                body: decode(Buffer.concat(chunks)).toString("utf8"),
              });
            });
          }
          response.writeHead(reply.statusCode, reply.headers);
          reply.pipe(response);
        },
      );
      forwarded.on("error", (error) => {
        response.writeHead(502);
        response.end(error.message);
      });
      request.pipe(forwarded);
    },
  );
  const publicPort = await listen(ingress);
  const authOrigin = `https://auth.abc.com:${publicPort}`;
  const allowedOrigin = `https://ss.abc.com:${publicPort}`;
  const otherOrigin = `https://blocked.abc.com:${publicPort}`;
  const target = `${allowedOrigin}/dashboard?tab=files`;
  await api("/api/admin/config/run_type", { run_type: 3 });
  await api("/api/admin/config/gateway/proxy-protocol", {
    enabled: true,
    trusted_sources: ["127.0.0.1"],
  });
  const setDomain = (cookie_domain) =>
    api("/api/admin/config/subdomain_mode", {
      root_domain: "abc.com",
      auth_host: "auth.abc.com",
      auth_target: runtime.authUrl,
      cookie_domain,
      public_auth_base_url: authOrigin,
      public_https_port: publicPort,
      auth_cache_ttl_seconds: 0,
      auth_cache_unauthorized_ttl_seconds: 0,
    });
  await setDomain("");
  await api("/api/admin/config/host_mappings", {
    mappings: [
      {
        host: "auth.abc.com",
        target: runtime.authUrl,
        use_auth: false,
        service_role: "auth",
      },
      ...["ss.abc.com", "blocked.abc.com", "bb.def.com"].map((host) => ({
        host,
        target: `http://127.0.0.1:${upstreamPort}`,
        use_auth: true,
        suppress_toolbar: true,
      })),
    ],
  });
  const account = await api("/api/admin/auth/accounts", { username, password });
  await api(
    `/api/admin/auth/accounts/${account.id}/subdomain-access`,
    {
      subdomain_access: { mode: "custom", hosts: ["ss.abc.com"], streams: [] },
    },
    "PATCH",
  );
  await api("/api/admin/auth/mode/switch", { mode: "password" });
  browser = await chromium.launch({
    headless: true,
    args: [
      "--no-proxy-server",
      "--host-resolver-rules=MAP *.abc.com 127.0.0.1, MAP *.def.com 127.0.0.1",
    ],
  });
  const newContext = () =>
    browser.newContext({
      ignoreHTTPSErrors: true,
      // Incognito contexts can inherit the system proxy despite launch flags.
      // Bypass every destination so these local test domains stay local.
      proxy: { server: "http://127.0.0.1:9", bypass: "*" },
    });
  const submitLogin = async (page) => {
    const captcha = page.locator('altcha-widget input[type="checkbox"]');
    const fallback = page.getByRole("button", {
      name: /我不是机器人|I'm not a robot|I am not a robot/i,
    });
    await captcha.or(fallback).first().click();
    await page.locator("#login-username").fill(username);
    await page.locator("#login-password").fill(password);
    // Observe the response at the TLS ingress. Chromium discards response
    // bodies on navigation; observation here leaves browser traffic intact.
    const completed = new Promise((resolve, reject) => {
      const timer = setTimeout(
        () => reject(new Error("Password login response timed out")),
        30_000,
      );
      timer.unref();
      recordLogin = (reply) => {
        clearTimeout(timer);
        resolve(reply);
      };
    });
    await page.locator('form button[type="submit"]').click();
    const reply = await completed;
    recordLogin = undefined;
    assert.equal(reply.status, 200, reply.body);
    const result = JSON.parse(reply.body).data;
    assert.equal(result.grant_type, "browser_session");
    return result;
  };
  const login = async (page) => {
    const result = await submitLogin(page);
    await page.waitForURL(result.redirect_to);
    await page.waitForSelector("body");
    assert.match(await page.locator("body").innerText(), /single-service-ok/);
    return result;
  };
  const denied = async (page) => {
    const response = await page.goto(otherOrigin + "/private");
    assert.equal(response.status(), 403);
    assert.equal(new URL(page.url()).hostname, "blocked.abc.com");
  };
  let context = await newContext();
  let page = await context.newPage();
  await page.goto(target);
  await page.waitForURL(
    (url) => url.hostname === "auth.abc.com" && url.pathname === "/login",
  );
  assert.equal(new URL(page.url()).searchParams.get("redirect_uri"), target);
  assert.equal((await login(page)).redirect_to, target);
  let sessions = (await context.cookies()).filter(
    (cookie) => cookie.name === cookieName,
  );
  assert.equal(sessions.length, 1);
  assert.equal(sessions[0].domain, ".abc.com");
  assert.equal(sessions[0].secure, true);
  assert.equal(sessions[0].httpOnly, true);
  await denied(page);
  assert.equal((await page.goto(target)).status(), 200);
  console.log(
    "PASS service → auth → ss.abc.com, shared Secure cookie, blocked.abc.com 403",
  );
  await context.addCookies([
    {
      name: cookieName,
      value: "stale-service-cookie",
      url: allowedOrigin,
      httpOnly: true,
      secure: true,
    },
  ]);
  assert.equal((await page.goto(target)).status(), 200);
  await denied(page);
  console.log(
    "PASS stale host-only Cookie cannot hide the shared session or widen permissions",
  );
  const loggedOut = page.waitForResponse(
    (reply) => new URL(reply.url()).pathname === "/__auth__/api/auth/logout",
  );
  await page.goto(allowedOrigin + "/__auth__/api/auth/logout");
  assert.equal((await loggedOut).status(), 302);
  assert.equal(
    (await context.cookies()).filter((cookie) => cookie.name === cookieName)
      .length,
    0,
  );
  // Replaying the former cookie must not resurrect the server-side session.
  await context.addCookies(sessions);
  await page.goto(target);
  await page.waitForURL(
    (url) => url.hostname === "auth.abc.com" && url.pathname === "/login",
  );
  console.log(
    "PASS logout with a stale duplicate revokes the live session and rejects its replay",
  );
  await context.close();
  context = await newContext();
  page = await context.newPage();
  await page.goto(authOrigin + "/login");
  assert.equal((await login(page)).redirect_to, allowedOrigin + "/");
  await denied(page);
  console.log(
    "PASS direct auth password login automatically opens the only authorized service",
  );
  await context.close();
  await setDomain("auth.abc.com");
  context = await newContext();
  page = await context.newPage();
  await page.goto(target);
  await page.waitForURL(
    (url) =>
      url.hostname === "ss.abc.com" && url.pathname === "/__auth__/login",
  );
  assert.equal(new URL(page.url()).searchParams.get("redirect_uri"), target);
  assert.equal((await login(page)).redirect_to, target);
  await denied(page);
  console.log(
    "PASS incompatible auth Cookie domain uses service-local login, ss.abc.com 200, blocked.abc.com 403",
  );
  await context.close();
  await setDomain("");
  await api(
    `/api/admin/auth/accounts/${account.id}/subdomain-access`,
    {
      subdomain_access: { mode: "custom", hosts: ["bb.def.com"], streams: [] },
    },
    "PATCH",
  );
  const foreignOrigin = `https://bb.def.com:${publicPort}`;
  const foreignTarget = foreignOrigin + "/dashboard?tab=files";
  context = await newContext();
  page = await context.newPage();
  await page.goto(foreignTarget);
  await page.waitForURL(
    (url) =>
      url.hostname === "bb.def.com" && url.pathname === "/__auth__/login",
  );
  assert.equal(
    new URL(page.url()).searchParams.get("redirect_uri"),
    foreignTarget,
  );
  assert.equal((await login(page)).redirect_to, foreignTarget);
  sessions = (await context.cookies()).filter(
    (cookie) => cookie.name === cookieName,
  );
  assert.equal(sessions.length, 1);
  assert.equal(sessions[0].domain, "bb.def.com");
  assert.equal(sessions[0].secure, true);
  assert.equal((await page.goto(foreignTarget)).status(), 200);
  // Reauthenticate on the root-domain origin to test the account ACL with a
  // real browser cookie there: the same account must still get a scope 403.
  await page.goto(target);
  await page.waitForURL(
    (url) => url.hostname === "auth.abc.com" && url.pathname === "/login",
  );
  const restricted = await submitLogin(page);
  await page.waitForURL(restricted.redirect_to);
  assert.equal((await page.reload()).status(), 403);
  assert.equal(new URL(page.url()).hostname, "ss.abc.com");
  console.log(
    "PASS root abc.com: bb.def.com host-only login reaches service; ss.abc.com is denied by the same account",
  );
  await context.close();
  context = await newContext();
  page = await context.newPage();
  await page.goto(
    authOrigin +
      "/login?" +
      new URLSearchParams({ redirect_uri: foreignTarget }),
  );
  await page.waitForURL(
    (url) =>
      url.hostname === "bb.def.com" && url.pathname === "/__auth__/login",
  );
  assert.equal((await login(page)).redirect_to, foreignTarget);
  assert.equal(
    (await context.cookies()).filter((cookie) => cookie.name === cookieName)[0]
      .domain,
    "bb.def.com",
  );
  console.log(
    "PASS shared auth with cross-domain return URL moves to bb.def.com before credentials are submitted",
  );
  await context.close();
  context = await newContext();
  page = await context.newPage();
  await page.goto(authOrigin + "/login");
  const sharedLogin = await submitLogin(page);
  await page.waitForURL(sharedLogin.redirect_to);
  assert.equal(new URL(page.url()).hostname, "bb.def.com");
  assert.equal(new URL(page.url()).pathname, "/__auth__/login");
  assert.equal((await login(page)).redirect_to, foreignOrigin + "/");
  console.log(
    "PASS direct shared-auth login opens the foreign service's local login instead of remaining on logged-in AUTH",
  );
  await context.close();
} finally {
  await browser?.close();
  agent?.destroy();
  await close(ingress);
  await close(upstream);
  await runtime?.stop();
  await rm(temporary, { recursive: true, force: true });
}
