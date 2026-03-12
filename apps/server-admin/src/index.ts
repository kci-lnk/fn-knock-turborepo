import { Elysia, t } from "elysia";
import { cors } from "@elysiajs/cors";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { existsSync, readFileSync } from "node:fs";
import { createServer, type IncomingHttpHeaders, type IncomingMessage, type ServerResponse } from "node:http";
import { Readable } from "node:stream";
import { setTimeout as sleep } from "node:timers/promises";
import { adminRoutes } from "./routes/admin";
import { sslRoutes } from "./routes/ssl";
import { authRoutes } from "./routes/auth";
import { systemRoutes } from "./routes/system";
import { backoffRoutes } from "./routes/backoff";
import { scannerRoutes } from "./routes/scanner";
import { hmacMiddleware } from "./middleware/hmac";
import { frpcRoutes, restoreFrpcOnBoot } from "./routes/frpc";
import { goBackend } from "./lib/go-backend";
import { configManager, type ProxyMapping, type SSLConfig, type SSLStatus } from "./lib/redis";
import { whitelistRoutes } from "./routes/whitelist";
import { whitelistManager } from "./lib/whitelist-manager";
import { portScannerPlugin } from "./plugins/scanner";
import { cron } from "@elysiajs/cron";
import { assetsRoutes } from "./routes/assets";
import { acmeRoutes } from "./routes/acme";
import { cloudflaredRoutes, restoreCloudflaredOnBoot } from "./routes/cloudflared";
import { ddnsRoutes } from "./routes/ddns";
import { registerAcmeRenewCron } from "./cron/acme-renew";
import { registerTrafficCleanupCron, registerTrafficCollectCron } from "./cron/traffic";
import { registerDDNSCron } from "./cron/ddns";
import { registerUpdateCron } from "./cron/update";
import { dashboardRoutes } from "./routes/dashboard";
import { dataPath } from './lib/AppDirManager';
import { initCleanScript } from './lib/init-scripts';
import { ipDetectorPlugin } from './plugins/ip-detector'
import { getRequiredEnv } from "./lib/env";
import { updatePlugin } from "./plugins/update";
import { updateRoutes } from "./routes/update";
import { updateManager } from "./lib/update-manager";
import { createStaticFilesPlugin } from "./plugins/static-files";
import { firewallService } from "./lib/firewall-service";

const __dirname = dirname(fileURLToPath(import.meta.url));

const BACKEND_PORT = process.env.BACKEND_PORT || 7998;
const AUTH_PORT = process.env.AUTH_PORT || 7997;

const ADMIN_STATIC_PATH_FROM_ENV = process.env.ADMIN_STATIC_PATH?.trim();
const DEV_STATIC_PATH = join(__dirname, '../../app/ui/www');
const PROD_STATIC_PATH = join(__dirname, '../../../ui/www');

const STATIC_PATH = ADMIN_STATIC_PATH_FROM_ENV
    || (existsSync(DEV_STATIC_PATH) ? DEV_STATIC_PATH : (existsSync(PROD_STATIC_PATH) ? PROD_STATIC_PATH : join(__dirname, '../public')));
if (!existsSync(STATIC_PATH)) {
    console.warn(`Static path ${STATIC_PATH} does not exist. Creating...`);
    import("node:fs").then(fs => fs.mkdirSync(STATIC_PATH, { recursive: true }));
}
console.log(`Serving static files from: ${STATIC_PATH}`);

const RUNTIME_HMAC_SECRET = getRequiredEnv("HMAC_SECRET");
const EXPOSE_RUNTIME_HMAC_SECRET =
    process.env.EXPOSE_RUNTIME_HMAC_SECRET === "1" || process.env.NODE_ENV !== "production";

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

const toRequest = (req: IncomingMessage): Request => {
    const host = req.headers.host || "127.0.0.1";
    const rawUrl = req.url || "/";
    const url = new URL(rawUrl, `http://${host}`);
    const method = req.method || "GET";
    const headers = toHeaders(req.headers);
    const init: RequestInit & { duplex?: "half" } = { method, headers };
    if (method !== "GET" && method !== "HEAD") {
        init.body = Readable.toWeb(req) as unknown as BodyInit;
        init.duplex = "half";
    }
    const request = new Request(url, init) as Request & { fnOriginalUrl?: string };
    request.fnOriginalUrl = rawUrl;
    return request;
};

const writeResponse = async (res: ServerResponse, response: Response): Promise<void> => {
    res.statusCode = response.status;
    response.headers.forEach((value, key) => {
        res.setHeader(key, value);
    });
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
            res.end(JSON.stringify({ success: false, message: "Internal Server Error" }));
        }
    });
    server.listen(port, hostname);
    return server;
};

const serveInjectedIndex = (rootPath: string) => {
    const indexPath = join(rootPath, "index.html");
    if (!existsSync(indexPath)) {
        return new Response("Not Found", { status: 404 });
    }
    const html = readFileSync(indexPath, "utf-8");
    return new Response(injectRuntimeScript(html), {
        headers: {
            "Content-Type": "text/html; charset=utf-8",
            "Cache-Control": "no-cache",
        },
    });
};

const app = new Elysia();

const authApp = new Elysia();

const AUTH_STATIC_PATH_FROM_ENV = process.env.AUTH_STATIC_PATH?.trim();
const AUTH_DEV_STATIC_PATH = join(__dirname, '../../server-auth-view/dist');
const AUTH_PROD_STATIC_PATH = join(__dirname, '../../../server-auth-view/dist');
const AUTH_PUBLIC_PREFIX = "/auth";
const AUTH_STATIC_PATH = AUTH_STATIC_PATH_FROM_ENV
    || (existsSync(AUTH_DEV_STATIC_PATH)
    ? AUTH_DEV_STATIC_PATH
    : (existsSync(AUTH_PROD_STATIC_PATH) ? AUTH_PROD_STATIC_PATH : join(__dirname, '../public-auth')));

if (!existsSync(AUTH_STATIC_PATH)) {
    console.warn(`Auth Static path ${AUTH_STATIC_PATH} does not exist. Creating...`);
    import("node:fs").then(fs => fs.mkdirSync(AUTH_STATIC_PATH, { recursive: true }));
}
console.log(`Serving auth static files from: ${AUTH_STATIC_PATH}`);
const normalizeAuthPath = (path: string) => {
    if (path === AUTH_PUBLIC_PREFIX || path === `${AUTH_PUBLIC_PREFIX}/`) return "/";
    if (path.startsWith(`${AUTH_PUBLIC_PREFIX}/`)) return path.slice(AUTH_PUBLIC_PREFIX.length);
    return path;
};

authApp.use(cors());
authApp.use(hmacMiddleware);
authApp.use(authRoutes);
authApp.use(new Elysia({ prefix: AUTH_PUBLIC_PREFIX }).use(authRoutes));

const isRoot = process.getuid && process.getuid() === 0;

if (!isRoot) {
    console.warn("Warning: Not running as root. Iptables operations will likely fail.");
}

app.use(ipDetectorPlugin);
app.use(updatePlugin);

app.use(cors());
app.use(hmacMiddleware);

app.use(portScannerPlugin);
app.use(assetsRoutes);
app.use(adminRoutes);
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
app.use(updateRoutes);

app.get("/__fn-knock/runtime-hmac-secret", ({ set }) => {
    if (!EXPOSE_RUNTIME_HMAC_SECRET) {
        set.status = 404;
        return { success: false, message: "Not found" };
    }
    return { success: true, data: { hmacSecret: RUNTIME_HMAC_SECRET } };
});

app.use(
    cron({
        name: 'whitelist-expiry-check',
        pattern: '* * * * *',
        run() {
            whitelistManager.processExpiredRecords();
        }
    })
);

registerAcmeRenewCron(app);
registerTrafficCollectCron(app);
registerTrafficCleanupCron(app);
registerDDNSCron(app);
registerUpdateCron(app);
void updateManager.prepareOnBoot();
void updateManager.checkNow("startup");

app.get("/", () => serveInjectedIndex(STATIC_PATH));
app.get("/index.html", () => serveInjectedIndex(STATIC_PATH));

app.use(createStaticFilesPlugin({
    root: STATIC_PATH,
}));

// SPA Fallback for admin app
app.get("*", ({ path }) => {
    if (path.startsWith("/api")) return;
    return serveInjectedIndex(STATIC_PATH);
});


// Route for auth app frontend
authApp.get("/", () => serveInjectedIndex(AUTH_STATIC_PATH));
authApp.get("/index.html", () => serveInjectedIndex(AUTH_STATIC_PATH));
authApp.get("/auth", () => serveInjectedIndex(AUTH_STATIC_PATH));
authApp.get("/auth/", () => serveInjectedIndex(AUTH_STATIC_PATH));
authApp.get("/auth/index.html", () => serveInjectedIndex(AUTH_STATIC_PATH));
authApp.get("/__fn-knock/runtime-hmac-secret", ({ set }) => {
    if (!EXPOSE_RUNTIME_HMAC_SECRET) {
        set.status = 404;
        return { success: false, message: "Not found" };
    }
    return { success: true, data: { hmacSecret: RUNTIME_HMAC_SECRET } };
});
authApp.get("/auth/__fn-knock/runtime-hmac-secret", ({ set }) => {
    if (!EXPOSE_RUNTIME_HMAC_SECRET) {
        set.status = 404;
        return { success: false, message: "Not found" };
    }
    return { success: true, data: { hmacSecret: RUNTIME_HMAC_SECRET } };
});
authApp.use(createStaticFilesPlugin({
    root: AUTH_STATIC_PATH,
    mountPrefixes: ["/", "/auth"],
}));

// SPA Fallback for auth routes
authApp.get("*", ({ path }) => {
    const normalizedPath = normalizeAuthPath(path);
    if (normalizedPath.startsWith("/api")) return;
    return serveInjectedIndex(AUTH_STATIC_PATH);
});

const config = await configManager.getConfig()
if (config.run_type === 0) {
    await firewallService.applyRunTypeConfig(1);
    await sleep(500);
    await firewallService.applyRunTypeConfig(0);
} else {
    await firewallService.applyRunTypeConfig(config.run_type);
}
// 自动同步SSL
configManager.getSSLStatus().then(async data => {
    if (data.enabled) {
        goBackend.setSSL(config.ssl.cert, config.ssl.key);
    } else {
        goBackend.clearSSL();
    }
});
await restoreFrpcOnBoot();
await restoreCloudflaredOnBoot();

const backendPort = toPort(BACKEND_PORT);
const authPort = toPort(AUTH_PORT);

console.log(`Elysia Admin Backend is running at ${backendPort}`);

startNodeServer(authApp, authPort, "127.0.0.1");
console.log(`Elysia Auth Backend is running at ${authPort}`);

initCleanScript();

startNodeServer(app, backendPort, "127.0.0.1");
