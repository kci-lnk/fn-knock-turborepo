import { Elysia, t } from "elysia";
import { goBackend } from "../lib/go-backend";
import {
  clearCA,
  existsCA,
  getCAInfo,
  initRootCA,
  issueServerCert,
  readCACert,
} from "../lib/ca-store";
import { listSSLSharedFiles, readSSLSharedFile } from "../lib/fnos-data-share";
import { configManager } from "../lib/redis";
import { syncSSLDeploymentToGateway } from "../lib/ssl-gateway";
import {
  buildSubdomainCertificateCoverage,
  buildSubdomainCertificateInventoryCoverage,
} from "../lib/subdomain-mode";
import { routeDoc, withRouteDoc } from "../lib/openapi";
import { createRequestTranslator, tDefault } from "../lib/i18n";

type RequestTranslator = ReturnType<typeof createRequestTranslator>["t"];

const getSslTranslator = async (request: Request) =>
  createRequestTranslator(request, await configManager.getLocaleConfig());
const sslRouteT = (
  t: RequestTranslator,
  key: string,
  params?: Record<string, string | number | boolean | null | undefined>,
) => t(`server.sslRoutes.${key}`, params);
const sslDefaultT = (
  key: string,
  params?: Record<string, string | number | boolean | null | undefined>,
) => tDefault(`server.sslRoutes.${key}`, params);

function crc32(buf: Uint8Array) {
  let c = ~0 >>> 0;
  for (let i = 0; i < buf.length; i++) {
    c ^= buf[i] ?? 0;
    for (let k = 0; k < 8; k++) {
      const mask = -(c & 1);
      c = (c >>> 1) ^ (0xedb88320 & mask);
    }
  }
  return ~c >>> 0;
}

function dtNow() {
  const d = new Date();
  const dosTime =
    ((d.getHours() & 0x1f) << 11) |
    ((d.getMinutes() & 0x3f) << 5) |
    (Math.floor(d.getSeconds() / 2) & 0x1f);
  const dosDate =
    (((d.getFullYear() - 1980) & 0x7f) << 9) |
    (((d.getMonth() + 1) & 0xf) << 5) |
    (d.getDate() & 0x1f);
  return { dosTime, dosDate };
}

function u16(v: number) {
  const b = new Uint8Array(2);
  const dv = new DataView(b.buffer);
  dv.setUint16(0, v, true);
  return b;
}
function u32(v: number) {
  const b = new Uint8Array(4);
  const dv = new DataView(b.buffer);
  dv.setUint32(0, v, true);
  return b;
}

function createZip(entries: { name: string; data: Uint8Array }[]) {
  const files: Uint8Array[] = [];
  const central: Uint8Array[] = [];
  let offset = 0;
  const { dosTime, dosDate } = dtNow();
  for (const e of entries) {
    const nameBytes = new TextEncoder().encode(e.name);
    const csum = crc32(e.data);
    const lfh = new Uint8Array([
      ...u32(0x04034b50),
      ...u16(20),
      ...u16(0),
      ...u16(0),
      ...u16(dosTime),
      ...u16(dosDate),
      ...u32(csum),
      ...u32(e.data.length),
      ...u32(e.data.length),
      ...u16(nameBytes.length),
      ...u16(0),
      ...nameBytes,
      ...e.data,
    ]);
    files.push(lfh);
    const cdfh = new Uint8Array([
      ...u32(0x02014b50),
      ...u16(20),
      ...u16(20),
      ...u16(0),
      ...u16(0),
      ...u16(dosTime),
      ...u16(dosDate),
      ...u32(csum),
      ...u32(e.data.length),
      ...u32(e.data.length),
      ...u16(nameBytes.length),
      ...u16(0),
      ...u16(0),
      ...u16(0),
      ...u16(0),
      ...u32(0),
      ...u32(offset),
      ...nameBytes,
    ]);
    central.push(cdfh);
    offset += lfh.length;
  }
  const centralDir = central.reduce(
    (a, b) => new Uint8Array([...a, ...b]),
    new Uint8Array(),
  );
  const filesBlob = files.reduce(
    (a, b) => new Uint8Array([...a, ...b]),
    new Uint8Array(),
  );
  const eocd = new Uint8Array([
    ...u32(0x06054b50),
    ...u16(0),
    ...u16(0),
    ...u16(entries.length),
    ...u16(entries.length),
    ...u32(centralDir.length),
    ...u32(filesBlob.length),
    ...u16(0),
  ]);
  return new Uint8Array([...filesBlob, ...centralDir, ...eocd]);
}

async function buildSSLStatusPayload(t?: RequestTranslator) {
  const [status, config, gatewayStatusResp] = await Promise.all([
    configManager.getSSLStatus(),
    configManager.getConfig(),
    goBackend.getSSLStatus(),
  ]);
  const gatewayStatus =
    gatewayStatusResp.success && gatewayStatusResp.data
      ? gatewayStatusResp.data
      : null;
  const effectiveDeploymentMode =
    gatewayStatus?.deployment_mode === "multi_sni"
      ? "multi_sni"
      : status.deploymentMode;

  const certificates = status.certificates.map((certificate) => ({
    ...certificate,
    coverage: buildSubdomainCertificateCoverage({
      config,
      certificateDomains: certificate.certInfo?.dnsNames || [],
    }),
  }));

  return {
    ...status,
    enabled: gatewayStatus?.enabled ?? status.enabled,
    certificates,
    subdomain_coverage: buildSubdomainCertificateCoverage({
      config,
      certificateDomains: status.certInfo?.dnsNames || [],
    }),
    library_coverage: buildSubdomainCertificateInventoryCoverage({
      config,
      certificates: certificates.map((certificate) => ({
        id: certificate.id,
        certificateDomains: certificate.certInfo?.dnsNames || [],
      })),
      activeCertificateId: status.activeCertId,
      deploymentMode: effectiveDeploymentMode,
    }),
    configuredDeploymentMode: status.deploymentMode,
    deploymentMode: effectiveDeploymentMode,
    gateway_status: gatewayStatus
      ? {
          enabled: gatewayStatus.enabled,
          deployment_mode:
            gatewayStatus.deployment_mode === "multi_sni"
              ? "multi_sni"
              : "single_active",
          certificates: gatewayStatus.certificates || [],
          sync_error: undefined,
        }
      : {
          enabled: false,
          deployment_mode: "single_active",
          certificates: [],
          sync_error:
            gatewayStatusResp.message ||
            (t
              ? sslRouteT(t, "gatewayStatusReadFailed")
              : sslDefaultT("gatewayStatusReadFailed")),
        },
  };
}

export const sslRoutes = new Elysia({
  prefix: "/api/admin/ssl",
  tags: ["SSL"],
})
  .get(
    "/status",
    async ({ request }) => {
      const { t } = await getSslTranslator(request);
      return {
        success: true,
        data: await buildSSLStatusPayload(t),
      };
    },
    routeDoc("获取 SSL 状态"),
  )
  .get(
    "/shared-files",
    async () => {
      const files = await listSSLSharedFiles();
      return { success: true, data: files };
    },
    routeDoc("获取共享目录中的 SSL 文件"),
  )
  .get(
    "/shared-files/content",
    async ({ request, query, set }) => {
      const { t: rt } = await getSslTranslator(request);
      try {
        const data = await readSSLSharedFile(query.path);
        return { success: true, data };
      } catch (error: any) {
        const message = error?.message ?? sslRouteT(rt, "readSharedFileFailed");
        if (
          error?.code === "ENOENT" ||
          message === rt("server.fnosDataShare.shareMissing")
        ) {
          set.status = 404;
        } else if (error?.code === "EACCES") {
          set.status = 403;
        } else {
          set.status = 400;
        }
        return { success: false, message };
      }
    },
    withRouteDoc("读取共享目录中的 SSL 文件内容", {
      query: t.Object({
        path: t.String(),
      }),
    }),
  )
  .get(
    "/ca/status",
    async () => {
      const initialized = await existsCA();
      if (!initialized) return { success: true, data: { initialized: false } };
      const info = await getCAInfo();
      return { success: true, data: { initialized: true, info } };
    },
    routeDoc("获取本地 CA 状态"),
  )
  .post(
    "/ca/init",
    async () => {
      const info = await initRootCA({
        commonName: "KCI-LNK Root Certificate Authority",
        organization: "KCI-LNK Corporation",
        organizationalUnit: "Information Security Department",
        country: "TW",
        state: "Taiwan",
        locality: "Taipei",
        validityYears: 20,
        keySize: 2048,
      });
      return { success: true, data: info };
    },
    routeDoc("初始化本地 CA"),
  )
  .delete(
    "/ca",
    async () => {
      await clearCA();
      return { success: true };
    },
    routeDoc("删除本地 CA"),
  )
  .get(
    "/ca/cert.pem",
    async ({ set }) => {
      const pem = await readCACert();
      set.headers["content-type"] = "application/x-pem-file; charset=utf-8";
      set.headers["content-disposition"] =
        'attachment; filename="KCI-LNK-Root-CA.pem"';
      return pem;
    },
    routeDoc("下载本地 CA 证书"),
  )
  .get(
    "/ca/server-cert.zip",
    async ({ request, set }) => {
      const { t: rt } = await getSslTranslator(request);
      try {
        const hosts = await configManager.getCAHosts();
        if (!hosts.length) {
          set.status = 400;
          return { success: false, message: sslRouteT(rt, "emptyDomains") };
        }
        const { certPem, keyPem } = await issueServerCert(hosts, 20);
        const validation = configManager.validateSSLCert(certPem, keyPem);
        if (!validation.valid) {
          set.status = 400;
          return {
            success: false,
            message: validation.error || sslRouteT(rt, "certOrKeyInvalid"),
          };
        }
        const entries = [
          { name: `server-cert.pem`, data: new TextEncoder().encode(certPem) },
          { name: `server-key.pem`, data: new TextEncoder().encode(keyPem) },
        ];
        const zipData = createZip(entries);
        return new Response(zipData, {
          headers: {
            "content-type": "application/zip",
            "content-disposition": `attachment; filename="server-cert.zip"`,
          },
        });
      } catch (e: any) {
        set.status = 500;
        return { success: false, message: e?.message ?? String(e) };
      }
    },
    routeDoc("下载本地 CA 签发的服务端证书压缩包"),
  )
  .get(
    "/ca/hosts",
    async () => {
      const hosts = await configManager.getCAHosts();
      return { success: true, data: hosts };
    },
    routeDoc("获取本地 CA Host 列表"),
  )
  .post(
    "/ca/hosts",
    async ({ request, body, set }) => {
      const { t: rt } = await getSslTranslator(request);
      const value = body.value?.trim?.();
      if (!value) {
        set.status = 400;
        return { success: false, message: sslRouteT(rt, "hostRequired") };
      }
      const hosts = await configManager.addCAHost(value);
      return { success: true, data: hosts };
    },
    withRouteDoc("新增本地 CA Host", {
      body: t.Object({
        value: t.String(),
      }),
    }),
  )
  .delete(
    "/ca/hosts",
    async ({ body }) => {
      if (body?.all) {
        await configManager.clearCAHosts();
        return { success: true };
      }
      const value = body?.value;
      if (!value) return { success: true };
      const hosts = await configManager.removeCAHost(value);
      return { success: true, data: hosts };
    },
    withRouteDoc("删除本地 CA Host", {
      body: t.Optional(
        t.Object({
          value: t.Optional(t.String()),
          all: t.Optional(t.Boolean()),
        }),
      ),
    }),
  )
  .post(
    "/ca/issue",
    async ({ request, set }) => {
      const { t: rt } = await getSslTranslator(request);
      try {
        const hosts = await configManager.getCAHosts();
        if (!hosts.length) {
          set.status = 400;
          return { success: false, message: sslRouteT(rt, "emptyDomains") };
        }
        const { certPem, keyPem } = await issueServerCert(hosts, 20);
        const validation = configManager.validateSSLCert(certPem, keyPem);
        if (!validation.valid) {
          set.status = 400;
          return {
            success: false,
            message: validation.error || sslRouteT(rt, "certOrKeyInvalid"),
          };
        }
        await configManager.saveSSLCertificate({
          label: hosts[0] || sslRouteT(rt, "localCaCertificateLabel"),
          source: "ca",
          cert: certPem,
          key: keyPem,
          activate: true,
          matchBy: {
            cert: certPem,
            key: keyPem,
          },
        });
        console.info(
          `[SSL] Issued and stored server certificate for ${hosts.length} host(s).`,
        );
        await syncSSLDeploymentToGateway();
        return { success: true, message: sslRouteT(rt, "success") };
      } catch (e: any) {
        set.status = 500;
        return { success: false, message: e?.message ?? String(e) };
      }
    },
    routeDoc("签发并激活本地 CA 证书"),
  )
  .get(
    "/cert.pem",
    async ({ request, set }) => {
      const { t: rt } = await getSslTranslator(request);
      const config = await configManager.getConfig();
      if (!config.ssl?.cert) {
        set.status = 404;
        return { success: false, message: sslRouteT(rt, "certNotInstalled") };
      }
      set.headers["content-type"] = "application/x-pem-file; charset=utf-8";
      set.headers["content-disposition"] =
        'attachment; filename=\"server-cert.pem\"';
      return config.ssl.cert;
    },
    routeDoc("下载当前激活证书 PEM"),
  )
  .get(
    "/cert.zip",
    async ({ request, set }) => {
      const { t: rt } = await getSslTranslator(request);
      const config = await configManager.getConfig();
      if (!config.ssl?.cert || !config.ssl?.key) {
        set.status = 404;
        return { success: false, message: sslRouteT(rt, "certNotInstalled") };
      }
      const entries = [
        {
          name: `server-cert.pem`,
          data: new TextEncoder().encode(config.ssl.cert),
        },
        {
          name: `server-key.pem`,
          data: new TextEncoder().encode(config.ssl.key),
        },
      ];
      const zipData = createZip(entries);
      return new Response(zipData, {
        headers: {
          "content-type": "application/zip",
          "content-disposition": `attachment; filename="server-cert.zip"`,
        },
      });
    },
    routeDoc("下载当前激活证书与私钥压缩包"),
  )
  .post(
    "/certificates",
    async ({ body, set }) => {
      const { cert, key } = body;
      const validation = configManager.validateSSLCert(cert, key);
      if (!validation.valid) {
        set.status = 400;
        return { success: false, message: validation.error };
      }

      const saved = await configManager.saveSSLCertificate({
        id: body.id,
        label: body.label,
        source: body.source,
        primary_domain: body.primary_domain,
        source_ref_id: body.source_ref_id,
        cert,
        key,
        activate: body.activate !== false,
        matchBy: {
          cert,
          key,
        },
      });

      const currentConfig = await configManager.getConfig();
      if (
        body.activate !== false ||
        currentConfig.ssl.deployment_mode === "multi_sni"
      ) {
        await syncSSLDeploymentToGateway(currentConfig);
      }

      return { success: true, data: { id: saved.id } };
    },
    withRouteDoc("保存证书到证书库", {
      body: t.Object({
        id: t.Optional(t.String()),
        label: t.Optional(t.String()),
        source: t.Optional(
          t.Union([t.Literal("manual"), t.Literal("acme"), t.Literal("ca")]),
        ),
        primary_domain: t.Optional(t.String()),
        source_ref_id: t.Optional(t.String()),
        cert: t.String(),
        key: t.String(),
        activate: t.Optional(t.Boolean()),
      }),
    }),
  )
  .post(
    "/",
    async ({ request, body, set }) => {
      const { t: rt } = await getSslTranslator(request);
      const { cert, key } = body.ssl;
      const validation = configManager.validateSSLCert(cert, key);
      if (!validation.valid) {
        set.status = 400;
        return { success: false, message: validation.error };
      }

      await configManager.saveSSLCertificate({
        label: sslRouteT(rt, "manualCertificateLabel"),
        source: "manual",
        cert,
        key,
        activate: true,
        matchBy: {
          cert,
          key,
        },
      });
      await syncSSLDeploymentToGateway();
      return { success: true };
    },
    withRouteDoc("上传并激活单张 SSL 证书", {
      body: t.Object({
        ssl: t.Object({
          cert: t.String(),
          key: t.String(),
        }),
      }),
    }),
  )
  .post(
    "/activate",
    async ({ request, body, set }) => {
      const { t: rt } = await getSslTranslator(request);
      const active = await configManager.activateSSLCertificate(body.id);
      if (!active) {
        set.status = 404;
        return { success: false, message: sslRouteT(rt, "certNotFound") };
      }

      await syncSSLDeploymentToGateway();
      return { success: true };
    },
    withRouteDoc("激活指定证书", {
      body: t.Object({
        id: t.String(),
      }),
    }),
  )
  .post(
    "/deployment-mode",
    async ({ body, request, set }) => {
      const { t: rt } = await getSslTranslator(request);
      const previousConfig = await configManager.getConfig();
      const nextConfig = structuredClone(previousConfig);
      nextConfig.ssl = {
        ...nextConfig.ssl,
        deployment_mode:
          body.deployment_mode === "multi_sni" ? "multi_sni" : "single_active",
      };

      if (
        nextConfig.ssl.deployment_mode === "multi_sni" &&
        !nextConfig.ssl.active_cert_id &&
        nextConfig.ssl.certificates?.length
      ) {
        const fallback = nextConfig.ssl.certificates[0];
        if (fallback) {
          nextConfig.ssl.active_cert_id = fallback.id;
          nextConfig.ssl.cert = fallback.cert;
          nextConfig.ssl.key = fallback.key;
        }
      }

      await configManager.saveConfig(nextConfig);
      try {
        await syncSSLDeploymentToGateway(nextConfig);
      } catch (error) {
        await configManager.saveConfig(previousConfig);
        throw error;
      }

      return {
        success: true,
        data: await buildSSLStatusPayload(rt),
      };
    },
    withRouteDoc("切换 SSL 部署模式", {
      body: t.Object({
        deployment_mode: t.Union([
          t.Literal("single_active"),
          t.Literal("multi_sni"),
        ]),
      }),
    }),
  )
  .delete(
    "/certificates/:id",
    async ({ request, params, set }) => {
      const { t: rt } = await getSslTranslator(request);
      const result = await configManager.deleteSSLCertificate(params.id);
      if (!result.removed) {
        set.status = 404;
        return { success: false, message: sslRouteT(rt, "certNotFound") };
      }

      const currentConfig = await configManager.getConfig();
      if (
        result.removedActive ||
        currentConfig.ssl.deployment_mode === "multi_sni"
      ) {
        await syncSSLDeploymentToGateway(currentConfig);
      }

      return { success: true };
    },
    routeDoc("删除指定证书"),
  )
  .delete(
    "/certificates",
    async () => {
      await configManager.clearSSLCertificateLibrary();
      await syncSSLDeploymentToGateway();
      return { success: true };
    },
    routeDoc("清空证书库"),
  )
  .delete(
    "/",
    async () => {
      await configManager.clearSSL();
      await syncSSLDeploymentToGateway();
      return { success: true };
    },
    routeDoc("清空当前 SSL 配置"),
  );
