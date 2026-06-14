import { createServer, type IncomingMessage, type Server } from "node:http";
import { tDefault } from "./i18n";

export interface AutoHttpsConfig {
  enabled: boolean;
}

export type AutoHttpsRuntimeStatus = "disabled" | "active" | "error";

export interface AutoHttpsRuntimeState {
  enabled: boolean;
  active: boolean;
  status: AutoHttpsRuntimeStatus;
  listen_host: string;
  listen_port: number;
  redirect_scheme: "https";
  last_error: string | null;
  last_error_at: string | null;
  updated_at: string;
}

const AUTO_HTTPS_LISTEN_PORT = 80;
const AUTO_HTTPS_CONFIGURED_LISTEN_HOST =
  process.env.FN_KNOCK_AUTO_HTTPS_HOST?.trim() || "";
const AUTO_HTTPS_DUAL_STACK_LISTEN_HOST = "::";
const AUTO_HTTPS_FALLBACK_IPV4_LISTEN_HOST = "0.0.0.0";
const autoHttpsT = (
  key: string,
  params?: Record<string, string | number | boolean | null | undefined>,
) => tDefault(`server.autoHttps.${key}`, params);

interface AutoHttpsListenTarget {
  host: string;
  ipv6Only?: boolean;
}

const nowIso = () => new Date().toISOString();

const isIpv6AnyAddress = (host: string) =>
  host === "::" || host === "0:0:0:0:0:0:0:0";

export const resolveAutoHttpsListenTargets = (
  configuredHost = AUTO_HTTPS_CONFIGURED_LISTEN_HOST,
): AutoHttpsListenTarget[] => {
  const host = configuredHost.trim();
  if (host) {
    return [
      isIpv6AnyAddress(host)
        ? { host, ipv6Only: false }
        : { host },
    ];
  }

  return [
    {
      host: AUTO_HTTPS_DUAL_STACK_LISTEN_HOST,
      ipv6Only: false,
    },
    {
      host: AUTO_HTTPS_FALLBACK_IPV4_LISTEN_HOST,
    },
  ];
};

const stripDefaultHttpPort = (host: string) => {
  if (/^\[[^\]]+\]:80$/i.test(host)) {
    return host.slice(0, -3);
  }
  if (!host.startsWith("[") && /:80$/i.test(host)) {
    return host.slice(0, -3);
  }
  return host;
};

const normalizeRequestHost = (req: IncomingMessage) => {
  const rawHost = Array.isArray(req.headers.host)
    ? req.headers.host[0]
    : req.headers.host;
  const host = String(rawHost || "localhost")
    .replace(/[\r\n]/g, "")
    .trim();
  return stripDefaultHttpPort(host || "localhost");
};

const resolveRedirectPath = (req: IncomingMessage, host: string) => {
  try {
    const url = new URL(req.url || "/", `http://${host}`);
    return `${url.pathname}${url.search}`;
  } catch {
    return "/";
  }
};

const normalizeListenError = (error: unknown) => {
  const err = error as NodeJS.ErrnoException;
  if (err?.code === "EACCES") {
    return autoHttpsT("listenEacces");
  }
  if (err?.code === "EADDRINUSE") {
    return autoHttpsT("listenEaddrinuse");
  }
  if (err?.message) {
    return autoHttpsT("listenFailedWithMessage", { message: err.message });
  }
  return autoHttpsT("listenFailed");
};

const closeServer = (server: Server) =>
  new Promise<void>((resolve, reject) => {
    server.close((error) => {
      if (error) {
        reject(error);
        return;
      }
      resolve();
    });
  });

const shouldFallbackToIpv4 = (
  configuredHost: string,
  target: AutoHttpsListenTarget,
  error: unknown,
) => {
  if (configuredHost.trim()) {
    return false;
  }
  if (target.host !== AUTO_HTTPS_DUAL_STACK_LISTEN_HOST) {
    return false;
  }

  const err = error as NodeJS.ErrnoException;
  return err?.code === "EAFNOSUPPORT" || err?.code === "EADDRNOTAVAIL";
};

const listenServer = (port: number, target: AutoHttpsListenTarget) => {
  const server = createRedirectServer();

  return new Promise<Server>((resolve, reject) => {
    const onError = (error: Error) => {
      server.off("listening", onListening);
      reject(error);
    };
    const onListening = () => {
      server.off("error", onError);
      resolve(server);
    };

    server.once("error", onError);
    server.once("listening", onListening);
    server.listen({
      port,
      host: target.host,
      ...(typeof target.ipv6Only === "boolean"
        ? { ipv6Only: target.ipv6Only }
        : {}),
    });
  });
};

const resolveServerListenHost = (server: Server, fallbackHost: string) => {
  const address = server.address();
  if (!address || typeof address === "string") {
    return fallbackHost;
  }
  return address.address || fallbackHost;
};

const createRedirectServer = () => {
  const server = createServer((req, res) => {
    const host = normalizeRequestHost(req);
    const location = `https://${host}${resolveRedirectPath(req, host)}`;

    res.statusCode = 308;
    res.setHeader("Location", location);
    res.setHeader("Cache-Control", "no-store");
    res.setHeader("Connection", "close");
    res.end();
  });

  server.on("clientError", (_error, socket) => {
    socket.end("HTTP/1.1 400 Bad Request\r\n\r\n");
  });

  return server;
};

export class AutoHttpsRedirectManager {
  private readonly listenPort: number;
  private readonly configuredListenHost: string;
  private server: Server | null = null;
  private state: AutoHttpsRuntimeState;
  private operation: Promise<AutoHttpsRuntimeState>;

  constructor(options?: { listenPort?: number; listenHost?: string }) {
    this.listenPort = options?.listenPort ?? AUTO_HTTPS_LISTEN_PORT;
    this.configuredListenHost =
      options?.listenHost?.trim() ?? AUTO_HTTPS_CONFIGURED_LISTEN_HOST;
    this.state = this.buildState(false, false);
    this.operation = Promise.resolve(this.state);
  }

  getRuntimeState(): AutoHttpsRuntimeState {
    return this.state;
  }

  async applyConfig(config: AutoHttpsConfig): Promise<AutoHttpsRuntimeState> {
    this.operation = this.operation.then(
      () => this.applyConfigNow(config),
      () => this.applyConfigNow(config),
    );
    return this.operation;
  }

  private buildState(
    enabled: boolean,
    active: boolean,
    error?: string | null,
    listenHost = this.getDefaultListenHost(),
  ): AutoHttpsRuntimeState {
    return {
      enabled,
      active,
      status: !enabled ? "disabled" : active ? "active" : "error",
      listen_host: listenHost,
      listen_port: this.listenPort,
      redirect_scheme: "https",
      last_error: error || null,
      last_error_at: error ? nowIso() : null,
      updated_at: nowIso(),
    };
  }

  private buildErrorState(
    error: string,
    listenHost = this.getDefaultListenHost(),
  ): AutoHttpsRuntimeState {
    return {
      enabled: false,
      active: false,
      status: "error",
      listen_host: listenHost,
      listen_port: this.listenPort,
      redirect_scheme: "https",
      last_error: error,
      last_error_at: nowIso(),
      updated_at: nowIso(),
    };
  }

  private getListenTargets() {
    return resolveAutoHttpsListenTargets(this.configuredListenHost);
  }

  private getDefaultListenHost() {
    return (
      this.getListenTargets()[0]?.host || AUTO_HTTPS_FALLBACK_IPV4_LISTEN_HOST
    );
  }

  private async applyConfigNow(
    config: AutoHttpsConfig,
  ): Promise<AutoHttpsRuntimeState> {
    if (!config.enabled) {
      await this.stopServer();
      this.state = this.buildState(false, false);
      return this.state;
    }

    if (this.server?.listening) {
      this.state = this.buildState(
        true,
        true,
        null,
        resolveServerListenHost(this.server, this.getDefaultListenHost()),
      );
      return this.state;
    }

    await this.stopServer();

    let lastError: unknown = null;
    for (const target of this.getListenTargets()) {
      try {
        const server = await listenServer(this.listenPort, target);
        this.server = server;
        this.state = this.buildState(
          true,
          true,
          null,
          resolveServerListenHost(server, target.host),
        );
        return this.state;
      } catch (error) {
        lastError = error;
        if (
          shouldFallbackToIpv4(this.configuredListenHost, target, error)
        ) {
          continue;
        }

        this.server = null;
        this.state = this.buildErrorState(
          normalizeListenError(error),
          target.host,
        );
        return this.state;
      }
    }

    this.server = null;
    this.state = this.buildErrorState(
      normalizeListenError(lastError),
      this.getDefaultListenHost(),
    );
    return this.state;
  }

  private async stopServer() {
    const server = this.server;
    this.server = null;
    if (!server?.listening) {
      return;
    }

    await closeServer(server).catch((error) => {
      console.error("[auto-https] failed to stop redirect server:", error);
    });
  }
}

export const autoHttpsRedirectManager = new AutoHttpsRedirectManager();
