import { Elysia } from "elysia";
import { cors } from "@elysiajs/cors";
import { openapi } from "@elysiajs/openapi";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { existsSync, readFileSync } from "node:fs";
import {
  createServer,
  type IncomingHttpHeaders,
  type IncomingMessage,
  type ServerResponse,
} from "node:http";
import { Readable } from "node:stream";
import { adminRoutes } from "./routes/admin";
import { sslRoutes } from "./routes/ssl";
import { authRoutes } from "./routes/auth";
import { systemRoutes } from "./routes/system";
import { backoffRoutes } from "./routes/backoff";
import { scannerRoutes } from "./routes/scanner";
import { hmacMiddleware } from "./middleware/hmac";
import { frpcRoutes, restoreFrpcOnBoot } from "./routes/frpc";
import {
  DEFAULT_FNOS_PORT_ICON_HIJACK_CONFIG,
  configManager,
} from "./lib/redis";
import { whitelistRoutes } from "./routes/whitelist";
import { whitelistManager } from "./lib/whitelist-manager";
import { portScannerPlugin } from "./plugins/scanner";
import { cron } from "@elysiajs/cron";
import { assetsRoutes } from "./routes/assets";
import { acmeRoutes } from "./routes/acme";
import {
  cloudflaredRoutes,
  restoreCloudflaredOnBoot,
} from "./routes/cloudflared";
import { ddnsRoutes } from "./routes/ddns";
import { gatewayLogsRoutes } from "./routes/gateway-logs";
import { wafRoutes } from "./routes/waf";
import { registerAcmeRenewCron } from "./cron/acme-renew";
import {
  registerTrafficCleanupCron,
  registerTrafficCollectCron,
} from "./cron/traffic";
import { registerDDNSCron } from "./cron/ddns";
import { registerSystemMonitorCron } from "./cron/system-monitor";
import { registerUpdateCron } from "./cron/update";
import { dashboardRoutes } from "./routes/dashboard";
import { initCleanScript } from "./lib/init-scripts";
import { ipDetectorPlugin } from "./plugins/ip-detector";
import { getRequiredEnv } from "./lib/env";
import { updatePlugin } from "./plugins/update";
import { updateRoutes } from "./routes/update";
import { updateManager } from "./lib/update-manager";
import { createStaticFilesPlugin } from "./plugins/static-files";
import { firewallService } from "./lib/firewall-service";
import {
  scheduleSyncReverseProxyTrustedIPs,
  syncReverseProxyTrustedIPsNow,
} from "./lib/reverse-proxy-trusted-ips";
import { syncGatewayLoggingToGateway } from "./lib/gateway-logging";
import {
  startWAFSystemRulesAutoUpdate,
  syncWAFToGatewayOnBoot,
} from "./lib/waf/service";
import { wafCollector } from "./lib/waf/collector";
import { syncSSLDeploymentToGateway } from "./lib/ssl-gateway";
import { syncSmartConnectOnBoot } from "./lib/smart-connect";
import { terminalRoutes } from "./routes/terminal";
import { terminalManager } from "./lib/terminal-manager";
import { sshSecurityRoutes } from "./routes/ssh-security";
import { sshSecurityService } from "./lib/ssh-security/service";
import { applyNoStoreHeaders } from "./lib/auth-access";
import { cidrRoutes } from "./routes/cidr";
import { ipLocationRoutes } from "./routes/ip-location";
import { internalSystemEventRoutes } from "./routes/internal-system-events";
import { cleanupLegacyAuthLogStorage } from "./lib/cleanup-legacy-auth-logs";
import { eventRoutes } from "./routes/events";
import { notificationRoutes } from "./routes/notifications";
import { oidcAdminRoutes } from "./routes/auth-oidc-admin";
import { systemNotificationRuntime } from "./lib/system-notifications/runtime";
import { authMobilitySessionManager } from "./lib/auth-mobility-session";
import { systemClockManager } from "./lib/system-clock-manager";
import { adminOpenApiTags, hideFromDocs } from "./lib/openapi";
import { APP_LOCAL_VERSION } from "./lib/app-version";
import {
  getCapabilityUnavailableMessage,
  getRuntimeCapabilities,
  getRuntimeProfile,
} from "./lib/runtime-profile";
import {
  DOCKER_ADMIN_DISCOVER_IP_HEADER_NAME,
  DOCKER_ADMIN_PROXY_HEADER_NAME,
  dockerAdminPanelManager,
  getDockerAdminTrustedProxyCidrs,
  isDockerAdminPublicPath,
  getDockerAdminProxySecret,
  isDockerAdminProtectedPath,
  isDockerAdminProxyRequest,
  resolveDockerAdminDiscoverIpFromIncomingMessage,
  resolveDockerAdminIncomingRequestContext,
} from "./lib/docker-admin-panel";
import { autoHttpsRedirectManager } from "./lib/auto-https-redirect";
import { goBackend } from "./lib/go-backend";
import { rebuildCommonAuthLocationsRuntimeState } from "./lib/common-auth-locations";
import {
  type LocaleCode,
  normalizeLocaleConfig,
  translate,
} from "../../../packages/i18n/src";
import { createRequestTranslator } from "./lib/i18n";

const __dirname = dirname(fileURLToPath(import.meta.url));
const runtimeProfile = getRuntimeProfile();
const runtimeCapabilities = getRuntimeCapabilities(runtimeProfile);

const BACKEND_PORT = process.env.BACKEND_PORT || 7998;
const AUTH_PORT = process.env.AUTH_PORT || 7997;
const ADMIN_VIEW_PORT = runtimeProfile.is_docker
  ? process.env.ADMIN_VIEW_PORT?.trim() || "7991"
  : "";
const BACKEND_HOST =
  process.env.BACKEND_HOST?.trim() ||
  (runtimeProfile.is_docker ? "127.0.0.1" : "127.0.0.1");
const AUTH_HOST = process.env.AUTH_HOST?.trim() || "127.0.0.1";
const ADMIN_VIEW_HOST =
  process.env.ADMIN_VIEW_HOST?.trim() ||
  (runtimeProfile.is_docker ? "0.0.0.0" : BACKEND_HOST);

const ADMIN_STATIC_PATH_FROM_ENV = process.env.ADMIN_STATIC_PATH?.trim();
const DEV_STATIC_PATH = join(__dirname, "../../app/ui/www");
const PROD_STATIC_PATH = join(__dirname, "../../../ui/www");

const STATIC_PATH =
  ADMIN_STATIC_PATH_FROM_ENV ||
  (existsSync(DEV_STATIC_PATH)
    ? DEV_STATIC_PATH
    : existsSync(PROD_STATIC_PATH)
      ? PROD_STATIC_PATH
      : join(__dirname, "../public"));
if (!existsSync(STATIC_PATH)) {
  console.warn(`Static path ${STATIC_PATH} does not exist. Creating...`);
  import("node:fs").then((fs) =>
    fs.mkdirSync(STATIC_PATH, { recursive: true }),
  );
}
console.log(`Serving static files from: ${STATIC_PATH}`);
console.log(
  `[runtime] deployment_target=${runtimeProfile.deployment_target} docker=${runtimeProfile.is_docker} linux=${runtimeProfile.is_linux} root=${runtimeProfile.is_root_process}`,
);
if (runtimeProfile.is_docker) {
  const trustedProxyCidrs = getDockerAdminTrustedProxyCidrs();
  if (trustedProxyCidrs.length > 0) {
    console.log(
      `[docker-admin] trusted reverse proxies enabled via DOCKER_ADMIN_TRUSTED_PROXY_CIDRS=${trustedProxyCidrs.join(",")}`,
    );
  }
}

const RUNTIME_HMAC_SECRET = getRequiredEnv("HMAC_SECRET");
const EXPOSE_RUNTIME_HMAC_SECRET =
  process.env.EXPOSE_RUNTIME_HMAC_SECRET === "1" ||
  process.env.NODE_ENV !== "production";

const injectRuntimeScript = (html: string) => {
  const runtimeScript = `<script>window.__FN_KNOCK_HMAC_SECRET__=${JSON.stringify(RUNTIME_HMAC_SECRET)};</script>`;
  if (html.includes("</head>")) {
    return html.replace("</head>", `${runtimeScript}</head>`);
  }
  return `${runtimeScript}${html}`;
};

const toPort = (value: string | number): number => {
  const port = Number(value);
  if (!Number.isInteger(port) || port <= 0 || port > 65535) {
    throw new Error(`Invalid port: ${value}`);
  }
  return port;
};

const toOptionalPort = (value: string): number | null => {
  if (!value) return null;
  return toPort(value);
};

const toHeaders = (rawHeaders: IncomingHttpHeaders): Headers => {
  const headers = new Headers();
  for (const [key, value] of Object.entries(rawHeaders)) {
    if (value === undefined) continue;
    if (Array.isArray(value)) {
      for (const item of value) headers.append(key, item);
    } else {
      headers.set(key, value);
    }
  }
  return headers;
};

const toRequest = (
  req: IncomingMessage,
  options?: {
    url?: URL;
    headerOverrides?: Record<string, string | null | undefined>;
  },
): Request => {
  const host = req.headers.host || "127.0.0.1";
  const rawUrl = req.url || "/";
  const url = options?.url || new URL(rawUrl, `http://${host}`);
  const method = req.method || "GET";
  const headers = toHeaders(req.headers);
  for (const [key, value] of Object.entries(options?.headerOverrides || {})) {
    if (value == null || value === "") {
      headers.delete(key);
    } else {
      headers.set(key, value);
    }
  }
  const init: RequestInit & { duplex?: "half" } = { method, headers };
  if (method !== "GET" && method !== "HEAD") {
    init.body = Readable.toWeb(req) as any;
    init.duplex = "half";
  }
  const request = new Request(url, init) as Request & {
    fnOriginalUrl?: string;
  };
  request.fnOriginalUrl = rawUrl;
  return request;
};

const writeResponse = async (
  res: ServerResponse,
  response: Response,
): Promise<void> => {
  res.statusCode = response.status;
  const getSetCookie = (
    response.headers as Headers & {
      getSetCookie?: () => string[];
    }
  ).getSetCookie;
  const setCookies =
    typeof getSetCookie === "function"
      ? getSetCookie.call(response.headers)
      : [];

  response.headers.forEach((value, key) => {
    if (key.toLowerCase() === "set-cookie" && setCookies.length > 0) {
      return;
    }
    res.setHeader(key, value);
  });
  if (setCookies.length > 0) {
    res.setHeader("set-cookie", setCookies);
  }
  if (!response.body) {
    res.end();
    return;
  }
  const reader = response.body.getReader();
  while (true) {
    const { done, value } = await reader.read();
    if (done) break;
    if (value) {
      res.write(Buffer.from(value));
    }
  }
  res.end();
};

const startNodeServer = (service: Elysia, port: number, hostname: string) => {
  const server = createServer(async (req, res) => {
    try {
      const request = toRequest(req);
      const response = await service.fetch(request);
      await writeResponse(res, response);
    } catch (error) {
      console.error("Failed to handle request:", error);
      res.statusCode = 500;
      res.setHeader("content-type", "application/json; charset=utf-8");
      res.end(
        JSON.stringify({ success: false, message: "Internal Server Error" }),
      );
    }
  });
  server.listen(port, hostname);
  return server;
};

const inferRequestProtocol = (req: IncomingMessage): "http" | "https" => {
  const forwardedProto = String(req.headers["x-forwarded-proto"] || "")
    .split(",")[0]
    ?.trim()
    ?.toLowerCase();
  if (forwardedProto === "https") return "https";
  if (forwardedProto === "http") return "http";
  return "http";
};

const escapeHtml = (value: string) =>
  value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#39;");

const buildDockerAdminDeniedResponse = (
  clientIp: string,
  prefersJson: boolean,
  locale: LocaleCode,
) => {
  const t = (
    key: string,
    params?: Record<string, string | number | boolean | null | undefined>,
  ) => translate(locale, key, params);

  if (prefersJson) {
    return new Response(
      JSON.stringify({
        success: false,
        message: t("server.dockerAdminDenied"),
      }),
      {
        status: 403,
        headers: {
          "content-type": "application/json; charset=utf-8",
          "content-language": locale,
          "cache-control": "no-store",
          "x-content-type-options": "nosniff",
          "x-frame-options": "DENY",
          "referrer-policy": "no-referrer",
        },
      },
    );
  }

  const body = `<!doctype html>
<html lang="${escapeHtml(locale)}">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>${escapeHtml(t("server.dockerAdminDeniedTitle"))}</title>
    <style>
      :root {
        color-scheme: light;
        font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
      }
      body {
        margin: 0;
        min-height: 100vh;
        display: flex;
        align-items: center;
        justify-content: center;
        background:
          radial-gradient(circle at top left, rgba(59, 130, 246, 0.16), transparent 28%),
          radial-gradient(circle at bottom right, rgba(244, 63, 94, 0.12), transparent 30%),
          #f5f7fb;
        color: #111827;
      }
      .card {
        width: min(92vw, 420px);
        border: 1px solid rgba(15, 23, 42, 0.08);
        border-radius: 20px;
        background: rgba(255, 255, 255, 0.94);
        box-shadow: 0 22px 60px rgba(15, 23, 42, 0.12);
        padding: 28px 24px;
      }
      .badge {
        display: inline-flex;
        align-items: center;
        justify-content: center;
        width: 48px;
        height: 48px;
        border-radius: 999px;
        background: rgba(239, 68, 68, 0.12);
        color: #dc2626;
        font-size: 22px;
        font-weight: 700;
      }
      h1 {
        margin: 18px 0 10px;
        font-size: 24px;
      }
      p {
        margin: 0;
        line-height: 1.7;
        color: #475569;
      }
      .meta {
        margin-top: 18px;
        padding: 12px 14px;
        border-radius: 14px;
        background: #f8fafc;
        color: #334155;
        font-size: 14px;
      }
    </style>
  </head>
  <body>
    <section class="card">
      <div class="badge">!</div>
      <h1>${escapeHtml(t("server.dockerAdminDeniedTitle"))}</h1>
      <p>${escapeHtml(t("server.dockerAdminDeniedDescription"))}</p>
      <div class="meta">${escapeHtml(t("server.dockerAdminCurrentIp", { ip: clientIp || "unknown" }))}</div>
    </section>
  </body>
</html>`;

  return new Response(body, {
    status: 403,
    headers: {
      "content-type": "text/html; charset=utf-8",
      "content-language": locale,
      "cache-control": "no-store",
      "content-security-policy":
        "default-src 'none'; style-src 'unsafe-inline'; img-src 'none'; form-action 'none'; frame-ancestors 'none'; base-uri 'none'",
      "x-content-type-options": "nosniff",
      "x-frame-options": "DENY",
      "referrer-policy": "no-referrer",
    },
  });
};

const startDockerAdminViewServer = (
  service: Elysia,
  port: number,
  hostname: string,
) => {
  const server = createServer(async (req, res) => {
    try {
      const accessContext = resolveDockerAdminIncomingRequestContext(req);
      const clientIp = accessContext.clientIp;
      const discoverIp = resolveDockerAdminDiscoverIpFromIncomingMessage(
        req,
        accessContext,
      );
      const rawUrl = req.url || "/";
      const accepts = String(req.headers.accept || "").toLowerCase();
      const prefersJson =
        rawUrl.startsWith("/api/") || accepts.includes("application/json");

      if (!accessContext.trustedIngress) {
        const locale = (await configManager.getLocaleConfig()).default_locale;
        await writeResponse(
          res,
          buildDockerAdminDeniedResponse(
            accessContext.socketIp || clientIp,
            prefersJson,
            locale,
          ),
        );
        return;
      }

      const protocol = inferRequestProtocol(req);
      const host = req.headers.host?.trim() || `127.0.0.1:${port}`;
      const request = toRequest(req, {
        url: new URL(rawUrl, `${protocol}://${host}`),
        headerOverrides: {
          [DOCKER_ADMIN_PROXY_HEADER_NAME]: getDockerAdminProxySecret(),
          [DOCKER_ADMIN_DISCOVER_IP_HEADER_NAME]: discoverIp || null,
          "x-forwarded-for": clientIp,
          "x-real-ip": clientIp,
        },
      });
      const response = await service.fetch(request);
      await writeResponse(res, response);
    } catch (error) {
      console.error("Failed to handle docker admin view request:", error);
      res.statusCode = 500;
      res.setHeader("content-type", "application/json; charset=utf-8");
      res.end(
        JSON.stringify({ success: false, message: "Internal Server Error" }),
      );
    }
  });
  server.listen(port, hostname);
  return server;
};

const serveIndexHtml = (rootPath: string, injectRuntimeSecret = false) => {
  const indexPath = join(rootPath, "index.html");
  if (!existsSync(indexPath)) {
    return new Response("Not Found", { status: 404 });
  }
  const html = readFileSync(indexPath, "utf-8");
  const body = injectRuntimeSecret ? injectRuntimeScript(html) : html;
  return new Response(body, {
    headers: {
      "Content-Type": "text/html; charset=utf-8",
      "Cache-Control": "no-cache",
    },
  });
};

const buildRuntimeHmacSecretResponse = (set: {
  status?: number | string;
  headers: Record<string, string | number | boolean | undefined>;
}) => {
  applyNoStoreHeaders(set.headers);
  if (!EXPOSE_RUNTIME_HMAC_SECRET) {
    set.status = 404;
    return { success: false, message: "Not found" };
  }
  return { success: true, data: { hmacSecret: RUNTIME_HMAC_SECRET } };
};

const app = new Elysia();

const authApp = new Elysia();

app.onBeforeHandle(async ({ request, set }) => {
  if (!runtimeProfile.is_docker) {
    return;
  }

  const requestPath = new URL(request.url).pathname;
  if (!isDockerAdminProtectedPath(requestPath)) {
    return;
  }

  applyNoStoreHeaders(set.headers);

  if (!isDockerAdminProxyRequest(request)) {
    const { t } = createRequestTranslator(
      request,
      await configManager.getLocaleConfig(),
    );
    set.status = 403;
    return {
      success: false,
      message: t("server.dockerAdminProxyRequired"),
    };
  }

  if (isDockerAdminPublicPath(requestPath)) {
    return;
  }

  const session =
    await dockerAdminPanelManager.resolveSessionFromRequest(request);
  if (!session) {
    const { t } = createRequestTranslator(
      request,
      await configManager.getLocaleConfig(),
    );
    set.status = 401;
    return {
      success: false,
      message: t("server.dockerAdminLoginRequired"),
    };
  }
});

const AUTH_STATIC_PATH_FROM_ENV = process.env.AUTH_STATIC_PATH?.trim();
const AUTH_DEV_STATIC_PATH = join(__dirname, "../../server-auth-view/dist");
const AUTH_PROD_STATIC_PATH = join(__dirname, "../../../server-auth-view/dist");
const AUTH_PUBLIC_PREFIX = "/auth";
const AUTH_LOCAL_PREFIX = "/__auth__";
const AUTH_STATIC_PATH =
  AUTH_STATIC_PATH_FROM_ENV ||
  (existsSync(AUTH_DEV_STATIC_PATH)
    ? AUTH_DEV_STATIC_PATH
    : existsSync(AUTH_PROD_STATIC_PATH)
      ? AUTH_PROD_STATIC_PATH
      : join(__dirname, "../public-auth"));

if (!existsSync(AUTH_STATIC_PATH)) {
  console.warn(
    `Auth Static path ${AUTH_STATIC_PATH} does not exist. Creating...`,
  );
  import("node:fs").then((fs) =>
    fs.mkdirSync(AUTH_STATIC_PATH, { recursive: true }),
  );
}
console.log(`Serving auth static files from: ${AUTH_STATIC_PATH}`);
const normalizeAuthPath = (path: string) => {
  if (
    path === AUTH_PUBLIC_PREFIX ||
    path === `${AUTH_PUBLIC_PREFIX}/` ||
    path === AUTH_LOCAL_PREFIX ||
    path === `${AUTH_LOCAL_PREFIX}/`
  ) {
    return "/";
  }
  if (path.startsWith(`${AUTH_PUBLIC_PREFIX}/`)) {
    return path.slice(AUTH_PUBLIC_PREFIX.length);
  }
  if (path.startsWith(`${AUTH_LOCAL_PREFIX}/`)) {
    return path.slice(AUTH_LOCAL_PREFIX.length);
  }
  return path;
};

const AUTH_VIEW_ROUTES = new Set([
  "/",
  "/index.html",
  "/login",
  "/login/",
  "/oidc/bind",
  "/oidc/bind/",
]);

const isKnownAuthViewPath = (path: string) =>
  AUTH_VIEW_ROUTES.has(normalizeAuthPath(path));

const serveAuthNotFoundHtml = () =>
  new Response(
    `<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>404</title><style>body{margin:0;min-height:100vh;display:grid;place-items:center;font-family:ui-sans-serif,system-ui,sans-serif;color:#111;background:#fff}.wrap{text-align:center}h1{margin:0;font-size:3rem;line-height:1;font-weight:600}p{margin:.75rem 0 0;color:#666;font-size:.875rem}</style></head><body><main class="wrap"><h1>404</h1><p>Page not found</p></main></body></html>`,
    {
      status: 404,
      headers: {
        "Content-Type": "text/html; charset=utf-8",
        "Cache-Control": "no-cache",
      },
    },
  );

const redirectLegacyOidcBindRoute = (request: Request) => {
  const requestUrl = new URL(request.url);
  const basePrefix = requestUrl.pathname.startsWith(`${AUTH_LOCAL_PREFIX}/`)
    ? AUTH_LOCAL_PREFIX
    : requestUrl.pathname.startsWith(`${AUTH_PUBLIC_PREFIX}/`)
      ? AUTH_PUBLIC_PREFIX
      : "";
  const location = `${basePrefix}/api/auth/oidc/bind${requestUrl.search}`;
  const headers = new Headers({ Location: location });
  applyNoStoreHeaders(headers);
  return new Response("", { status: 302, headers });
};

authApp.use(cors());
authApp.use(hmacMiddleware);
authApp.use(authRoutes);
authApp.use(new Elysia({ prefix: AUTH_PUBLIC_PREFIX }).use(authRoutes));
authApp.use(new Elysia({ prefix: AUTH_LOCAL_PREFIX }).use(authRoutes));

const isRoot = process.getuid && process.getuid() === 0;

if (!isRoot) {
  console.warn(
    "Warning: Not running as root. Iptables operations will likely fail.",
  );
}

app.use(ipDetectorPlugin);
app.use(updatePlugin);

app.use(cors());
app.use(
  openapi({
    path: "/docs",
    specPath: "/docs/json",
    provider: "swagger-ui",
    documentation: {
      info: {
        title: "fn-knock server-admin API",
        version: APP_LOCAL_VERSION,
        description: "server-admin 7998 端口提供的管理端接口文档。",
      },
      servers: [
        {
          url: "/",
          description: "server-admin (port 7998)",
        },
      ],
      tags: [...adminOpenApiTags],
    },
    swagger: {
      version: "5.32.2",
      docExpansion: "list",
      deepLinking: true,
      persistAuthorization: true,
      displayRequestDuration: true,
    },
  }),
);

app.use(internalSystemEventRoutes);
app.use(portScannerPlugin);
app.use(assetsRoutes);
app.use(adminRoutes);
app.use(eventRoutes);
app.use(notificationRoutes);
app.use(oidcAdminRoutes);
app.use(sslRoutes);
app.use(acmeRoutes);
app.use(systemRoutes);
app.use(dashboardRoutes);
app.use(whitelistRoutes);
app.use(backoffRoutes);
app.use(frpcRoutes);
app.use(cloudflaredRoutes);
app.use(scannerRoutes);
app.use(ddnsRoutes);
app.use(gatewayLogsRoutes);
app.use(wafRoutes);
app.use(ipLocationRoutes);
app.use(updateRoutes);
app.use(terminalRoutes);
app.use(sshSecurityRoutes);
app.use(cidrRoutes);

app.use(
  cron({
    name: "whitelist-expiry-check",
    pattern: "* * * * *",
    run() {
      void Promise.all([
        whitelistManager.processExpiredRecords(),
        whitelistManager.processDueCnameRecords(),
      ])
        .then(([expiredChanged, cnameChanged]) => {
          if (!expiredChanged && !cnameChanged) return;
          scheduleSyncReverseProxyTrustedIPs({
            reason: expiredChanged
              ? "whitelist-expiry"
              : "whitelist-cname-refresh",
            delayMs: 50,
          });
        })
        .catch((error) => {
          console.error(
            "[reverse-proxy-trusted-ips] failed to process whitelist maintenance:",
            error,
          );
        });
    },
  }),
);

app.use(
  cron({
    name: "session-active-ip-maintenance",
    pattern: "* * * * *",
    run() {
      void authMobilitySessionManager.maintainSessionActiveIps().catch(
        (error) => {
          console.error(
            "[auth-mobility] failed to maintain session active IPs:",
            error,
          );
        },
      );
    },
  }),
);

app.use(
  cron({
    name: "reverse-proxy-trusted-ips-reconcile",
    pattern: "*/2 * * * *",
    run() {
      void syncReverseProxyTrustedIPsNow().catch((error) => {
        console.error(
          "[reverse-proxy-trusted-ips] periodic reconcile failed:",
          error,
        );
      });
    },
  }),
);

app.use(
  cron({
    name: "common-auth-locations-reconcile",
    pattern: "*/5 * * * *",
    run() {
      void rebuildCommonAuthLocationsRuntimeState().catch((error) => {
        console.error(
          "[common-auth-locations] periodic reconcile failed:",
          error,
        );
      });
    },
  }),
);

app.use(
  cron({
    name: "terminal-session-cleanup",
    pattern: "* * * * *",
    run() {
      void terminalManager.cleanupExpiredSessions().catch((error) => {
        console.error(
          "[terminal] failed to cleanup sessions on schedule:",
          error,
        );
      });
    },
  }),
);

registerAcmeRenewCron(app);
registerTrafficCollectCron(app);
registerTrafficCleanupCron(app);
registerDDNSCron(app);
registerSystemMonitorCron(app);
registerUpdateCron(app);
void updateManager.prepareOnBoot();
void updateManager.checkNow("startup");
systemClockManager.prepareOnBoot();

app.get("/", () => serveIndexHtml(STATIC_PATH), hideFromDocs);
app.get("/index.html", () => serveIndexHtml(STATIC_PATH), hideFromDocs);

app.use(
  createStaticFilesPlugin({
    root: STATIC_PATH,
  }),
);

app.get(
  "*",
  ({ path }) => {
    if (path.startsWith("/api")) return;
    return serveIndexHtml(STATIC_PATH);
  },
  hideFromDocs,
);

authApp.get("/", () => serveIndexHtml(AUTH_STATIC_PATH, true));
authApp.get("/index.html", () => serveIndexHtml(AUTH_STATIC_PATH, true));
authApp.get("/auth", () => serveIndexHtml(AUTH_STATIC_PATH, true));
authApp.get("/auth/", () => serveIndexHtml(AUTH_STATIC_PATH, true));
authApp.get("/auth/index.html", () => serveIndexHtml(AUTH_STATIC_PATH, true));
authApp.get("/__auth__", () => serveIndexHtml(AUTH_STATIC_PATH, true));
authApp.get("/__auth__/", () => serveIndexHtml(AUTH_STATIC_PATH, true));
authApp.get("/__auth__/index.html", () =>
  serveIndexHtml(AUTH_STATIC_PATH, true),
);
authApp.get("/__fn-knock/runtime-hmac-secret", ({ set }) => {
  return buildRuntimeHmacSecretResponse(set);
});
authApp.get("/auth/__fn-knock/runtime-hmac-secret", ({ set }) => {
  return buildRuntimeHmacSecretResponse(set);
});
authApp.get("/__auth__/__fn-knock/runtime-hmac-secret", ({ set }) => {
  return buildRuntimeHmacSecretResponse(set);
});
authApp.get("/oidc/bind", ({ request }) =>
  redirectLegacyOidcBindRoute(request),
);
authApp.get("/oidc/bind/", ({ request }) =>
  redirectLegacyOidcBindRoute(request),
);
authApp.get("/auth/oidc/bind", ({ request }) =>
  redirectLegacyOidcBindRoute(request),
);
authApp.get("/auth/oidc/bind/", ({ request }) =>
  redirectLegacyOidcBindRoute(request),
);
authApp.get("/__auth__/oidc/bind", ({ request }) =>
  redirectLegacyOidcBindRoute(request),
);
authApp.get("/__auth__/oidc/bind/", ({ request }) =>
  redirectLegacyOidcBindRoute(request),
);
authApp.use(
  createStaticFilesPlugin({
    root: AUTH_STATIC_PATH,
    mountPrefixes: ["/", "/auth", "/__auth__"],
  }),
);

authApp.get("*", ({ path }) => {
  const normalizedPath = normalizeAuthPath(path);
  if (normalizedPath.startsWith("/api")) return;
  if (isKnownAuthViewPath(path)) return serveIndexHtml(AUTH_STATIC_PATH, true);
  return serveAuthNotFoundHtml();
});

let { applied: reverseProxyThrottlePatchApplied, config } =
  await configManager.applyLegacyReverseProxyThrottlePatchIfNeeded();
if (reverseProxyThrottlePatchApplied) {
  console.log(
    "[gateway-throttle] applied legacy reverse proxy throttle patch (20/50/30 -> 100/200/30)",
  );
}
const { applied: eventSystemResourceAlertsPatchApplied, config: nextConfig } =
  await configManager.applyLegacyEventSystemResourceAlertsPatchIfNeeded();
config = nextConfig;
if (eventSystemResourceAlertsPatchApplied) {
  console.log(
    "[system-monitor] enabled legacy default CPU/RAM resource alerts on upgrade",
  );
}
await cleanupLegacyAuthLogStorage().catch((error) => {
  console.error(
    "[auth-log] failed to cleanup legacy auth logs on boot:",
    error,
  );
});
const runtimeConstraintResult = await configManager.applyRuntimeConstraints();
config = runtimeConstraintResult.config;
if (runtimeConstraintResult.updated) {
  console.log(
    `[runtime] applied runtime config corrections: ${runtimeConstraintResult.corrected.join(", ")}`,
  );
}
if (runtimeCapabilities.smart_connect_available) {
  await syncSmartConnectOnBoot().catch((error) => {
    console.error(
      "[smart-connect] failed to sync dnsmasq state on boot:",
      error,
    );
  });
} else {
  console.log(
    `[smart-connect] skipped boot sync: ${getCapabilityUnavailableMessage("smart_connect_available")}`,
  );
}
if (runtimeProfile.is_docker) {
  await autoHttpsRedirectManager.applyConfig({ enabled: false });
  console.log("[auto-https] skipped boot sync: Docker version is unsupported");
} else {
  await autoHttpsRedirectManager
    .applyConfig(config.auto_https ?? { enabled: false })
    .then(async (runtime) => {
      if (runtime.status === "active") {
        console.log(
          `[auto-https] redirect server listening on ${runtime.listen_host}:${runtime.listen_port}`,
        );
      } else if (runtime.status === "error") {
        console.error(`[auto-https] ${runtime.last_error}`);
        await configManager.updateAutoHttpsConfig({ enabled: false });
      }
    })
    .catch((error) => {
      console.error("[auto-https] failed to apply boot config:", error);
    });
}
await firewallService.applyRunTypeConfig(config.run_type);
await sshSecurityService.syncFromConfigOnBoot().catch((error) => {
  console.error("[ssh-security] failed to sync on boot:", error);
});
syncGatewayLoggingToGateway(config.gateway_logging).catch((error) => {
  console.error(
    "[gateway-logging] failed to sync logging config on boot:",
    error,
  );
});
goBackend
  .setLocaleConfig(normalizeLocaleConfig(config.locale))
  .then((response) => {
    if (!response.success && response.code !== 404) {
      console.error(
        "[i18n] failed to sync locale config on boot:",
        response.message || "unknown gateway error",
      );
    }
  })
  .catch((error) => {
    console.error("[i18n] failed to sync locale config on boot:", error);
  });
goBackend
  .setFnosPortIconHijackConfig(
    config.fnos_port_icon_hijack ?? DEFAULT_FNOS_PORT_ICON_HIJACK_CONFIG,
  )
  .then((response) => {
    if (!response.success) {
      console.error(
        "[fnos-port-icon-hijack] failed to sync config on boot:",
        response.message || "unknown gateway error",
      );
    }
  })
  .catch((error) => {
    console.error(
      "[fnos-port-icon-hijack] failed to sync config on boot:",
      error,
    );
  });
syncWAFToGatewayOnBoot()
  .then(() => {
    startWAFSystemRulesAutoUpdate();
    wafCollector.start();
  })
  .catch((error) => {
    console.error("[waf] failed to sync WAF config on boot:", error);
    startWAFSystemRulesAutoUpdate();
    wafCollector.start();
  });
void rebuildCommonAuthLocationsRuntimeState().catch((error) => {
  console.error("[common-auth-locations] failed to rebuild on boot:", error);
});
syncSSLDeploymentToGateway(config).catch((error) => {
  console.error("[SSL] failed to sync gateway deployment on boot:", error);
});
await restoreFrpcOnBoot();
await restoreCloudflaredOnBoot();
await terminalManager.cleanupExpiredSessions().catch((error) => {
  console.error("[terminal] failed to cleanup sessions on boot:", error);
});
systemNotificationRuntime.start();

const backendPort = toPort(BACKEND_PORT);
const authPort = toPort(AUTH_PORT);
const adminViewPort = runtimeProfile.is_docker
  ? toOptionalPort(ADMIN_VIEW_PORT)
  : null;

console.log(
  `Elysia Admin Backend is running at ${BACKEND_HOST}:${backendPort}`,
);
if (runtimeProfile.is_docker && adminViewPort !== null) {
  console.log(
    `Elysia Docker Admin View is running at ${ADMIN_VIEW_HOST}:${adminViewPort}`,
  );
  startDockerAdminViewServer(app, adminViewPort, ADMIN_VIEW_HOST);
}

startNodeServer(authApp, authPort, AUTH_HOST);
console.log(`Elysia Auth Backend is running at ${AUTH_HOST}:${authPort}`);

if (runtimeCapabilities.host_firewall_available) {
  initCleanScript();
} else {
  console.log(
    `[runtime] skipped clean.sh generation: ${getCapabilityUnavailableMessage("host_firewall_available")}`,
  );
}

startNodeServer(app, backendPort, BACKEND_HOST);
