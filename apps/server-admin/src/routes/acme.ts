import { Elysia, t } from "elysia";
import { acmePlugin } from "../plugins/acme";
import { configManager } from "../lib/redis";
import { randomUUID } from "node:crypto";
import { goBackend } from "../lib/go-backend";
import { DEFAULT_REDIS_LOG_BUFFER_MAX_LEN } from "../lib/redis-log-buffer";

type DnsProvider = {
  dnsType: string;
  label: string;
  group: string;
  envKeys: string[];
};

const dnsProviders: DnsProvider[] = [
  { dnsType: "dns_cf", label: "Cloudflare", group: "常用", envKeys: ["CF_Key", "CF_Email"] },
  { dnsType: "dns_ali", label: "阿里云 DNS", group: "常用", envKeys: ["Ali_Key", "Ali_Secret"] },
  { dnsType: "dns_dp", label: "DNSPod", group: "常用", envKeys: ["DP_Id", "DP_Key"] },
  { dnsType: "dns_tencent", label: "腾讯云 DNSPod (TencentCloud)", group: "常用", envKeys: ["Tencent_SecretId", "Tencent_SecretKey"] },
  { dnsType: "dns_gd", label: "GoDaddy", group: "常用", envKeys: ["GD_Key", "GD_Secret"] },
  { dnsType: "dns_dgon", label: "DigitalOcean", group: "常用", envKeys: ["DO_API_KEY"] },
  { dnsType: "dns_netlify", label: "Netlify", group: "常用", envKeys: ["NETLIFY_TOKEN"] },
  { dnsType: "dns_vercel", label: "Vercel", group: "常用", envKeys: ["VERCEL_TOKEN", "VERCEL_TEAM_ID"] },
  { dnsType: "dns_aws", label: "AWS Route53", group: "常用", envKeys: ["AWS_ACCESS_KEY_ID", "AWS_SECRET_ACCESS_KEY", "AWS_REGION"] },
  { dnsType: "dns_google", label: "Google Cloud DNS", group: "常用", envKeys: ["GCE_PROJECT", "GCE_SERVICE_ACCOUNT_FILE"] },
  { dnsType: "dns_azure", label: "Azure DNS", group: "常用", envKeys: ["AZUREDNS_SUBSCRIPTIONID", "AZUREDNS_TENANTID", "AZUREDNS_APPID", "AZUREDNS_CLIENTSECRET"] },
  { dnsType: "dns_linode_v4", label: "Linode", group: "国际", envKeys: ["LINODE_V4_API_KEY"] },
  { dnsType: "dns_vultr", label: "Vultr", group: "国际", envKeys: ["VULTR_API_KEY"] },
  { dnsType: "dns_ovh", label: "OVH", group: "国际", envKeys: ["OVH_AK", "OVH_AS", "OVH_CK"] },
  { dnsType: "dns_hetzner", label: "Hetzner", group: "国际", envKeys: ["HETZNER_Token"] },
  { dnsType: "dns_namecheap", label: "Namecheap", group: "国际", envKeys: ["NAMECHEAP_API_KEY", "NAMECHEAP_USERNAME", "NAMECHEAP_SOURCEIP"] },
  { dnsType: "dns_porkbun", label: "Porkbun", group: "国际", envKeys: ["PORKBUN_API_KEY", "PORKBUN_SECRET_API_KEY"] },
  { dnsType: "dns_dynv6", label: "dynv6", group: "国际", envKeys: ["DYNV6_TOKEN"] },
  { dnsType: "dns_cloudns", label: "ClouDNS", group: "国际", envKeys: ["CLOUDNS_AUTH_ID", "CLOUDNS_AUTH_PASSWORD"] },
  { dnsType: "dns_gandi_livedns", label: "Gandi LiveDNS", group: "国际", envKeys: ["GANDI_LIVEDNS_KEY"] },
  { dnsType: "dns_nsone", label: "NS1", group: "国际", envKeys: ["NS1_Key"] },
  { dnsType: "dns_dnsimple", label: "DNSimple", group: "国际", envKeys: ["DNSimple_OAUTH_TOKEN", "DNSimple_ACCOUNT_ID"] },
  { dnsType: "dns_he", label: "Hurricane Electric", group: "国际", envKeys: ["HE_Username", "HE_Password"] },
  { dnsType: "dns_transip", label: "TransIP", group: "国际", envKeys: ["TRANSIP_Username", "TRANSIP_Key_File"] },
];

const normalizeDnsType = (value: string | undefined | null) => {
  if (!value) return null;
  const v = value.trim();
  if (!v) return null;
  if (v === "aliyun") return "dns_ali";
  if (v === "cloudflare") return "dns_cf";
  if (v === "dnspod") return "dns_dp";
  if (/^dns_[a-z0-9_]+$/i.test(v)) return v;
  return null;
};

const normalizeDomains = (domains: string[]) => {
  const out: string[] = [];
  const seen = new Set<string>();
  for (const raw of domains || []) {
    const v = String(raw ?? "").trim().toLowerCase();
    if (!v) continue;
    if (!isValidDomain(v)) continue;
    if (seen.has(v)) continue;
    seen.add(v);
    out.push(v);
  }
  return out;
};

const isValidDomain = (value: string) => {
  if (!value) return false;
  if (value.length > 253) return false;
  const v = value.trim();
  if (!v) return false;
  if (v.includes("..")) return false;
  if (v.startsWith(".") || v.endsWith(".")) return false;
  if (v.includes("/") || v.includes(" ") || v.includes("\t")) return false;
  return /^(\*\.)?([a-z0-9-]+\.)+[a-z0-9-]+$/i.test(v);
};

const normalizeCredentials = (value: unknown) => {
  const out: Record<string, string> = {};
  if (!value || typeof value !== "object") return out;
  for (const [k, v] of Object.entries(value as Record<string, unknown>)) {
    const kk = String(k ?? "").trim();
    const vv = String(v ?? "").trim();
    if (!kk || !vv) continue;
    out[kk] = vv;
  }
  return out;
};

const validateAndNormalizeAcmeRequest = (input: {
  domains: string[];
  dnsType?: string;
  provider?: string;
  credentials?: Record<string, string>;
}) => {
  const domains = normalizeDomains(input.domains);
  if (domains.length === 0) throw new Error("域名列表不能为空或格式无效");

  const dnsType = normalizeDnsType(input.dnsType ?? input.provider);
  if (!dnsType) throw new Error("dnsType required");
  const provider = dnsProviders.find(p => p.dnsType === dnsType) || null;
  if (!provider) throw new Error("不支持的 DNS 服务商");

  const credentials = normalizeCredentials(input.credentials);
  const missing = provider.envKeys.filter(k => !credentials[k]);
  if (missing.length > 0) throw new Error(`缺少 DNS API 凭据: ${missing.join(", ")}`);

  return { domains, dnsType, provider, credentials };
};

function crc32(buf: Uint8Array) {
  let c = ~0 >>> 0;
  for (let i = 0; i < buf.length; i++) {
    c ^= buf[i] ?? 0;
    for (let k = 0; k < 8; k++) {
      const mask = -(c & 1);
      c = (c >>> 1) ^ (0xEDB88320 & mask);
    }
  }
  return (~c) >>> 0;
}

function dtNow() {
  const d = new Date();
  const dosTime =
    ((d.getHours() & 0x1f) << 11) |
    ((d.getMinutes() & 0x3f) << 5) |
    ((Math.floor(d.getSeconds() / 2)) & 0x1f);
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
  const centralDir = central.reduce((a, b) => new Uint8Array([...a, ...b]), new Uint8Array());
  const filesBlob = files.reduce((a, b) => new Uint8Array([...a, ...b]), new Uint8Array());
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

type AcmeJobNonNull = NonNullable<Awaited<ReturnType<typeof configManager.getAcmeJob>>>;

type AcmeLogAnalysis = {
  reason: "dns_credentials_invalid" | "dns_credentials_invalid_email" | "dns_api_rate_limited" | "acme_frequency_limited" | "unknown";
  provider?: string;
  message: string;
  evidence?: string[];
};

const pickEvidence = (logs: string[], match: (line: string) => boolean, max: number = 3) => {
  const hits: string[] = [];
  for (let i = logs.length - 1; i >= 0; i--) {
    const line = logs[i];
    if (!line) continue;
    if (!match(line)) continue;
    hits.push(line);
    if (hits.length >= max) break;
  }
  return hits.length ? hits.reverse() : undefined;
};

const analyzeAcmeLogs = (job: AcmeJobNonNull, logs: string[]): AcmeLogAnalysis | null => {
  if (!logs.length) return null;
  const provider = job.provider || undefined;

  const has = (re: RegExp) => logs.some(line => re.test(line));

  const isCloudflare = provider === "dns_cf" || has(/\bCloudflare\b/i) || has(/\bX-Auth-Key\b/i);
  if (isCloudflare) {
    const invalidKey = has(/Invalid format for X-Auth-Key header/i) || has(/"code"\s*:\s*6103/i);
    if (invalidKey) {
      return {
        reason: "dns_credentials_invalid",
        provider: "dns_cf",
        message: "Cloudflare API 密钥不正确（X-Auth-Key 格式无效）",
        evidence: pickEvidence(logs, line => /X-Auth-Key/i.test(line) || /"code"\s*:\s*6103/i.test(line)),
      };
    }

    const invalidEmail = has(/Invalid format for X-Auth-Email header/i);
    if (invalidEmail) {
      return {
        reason: "dns_credentials_invalid_email",
        provider: "dns_cf",
        message: "Cloudflare 邮箱不正确（X-Auth-Email 格式无效）",
        evidence: pickEvidence(logs, line => /X-Auth-Email/i.test(line)),
      };
    }

    const invalidHeaders = has(/Invalid request headers/i) || has(/"code"\s*:\s*6003/i);
    if (invalidHeaders) {
      return {
        reason: "dns_credentials_invalid",
        provider: "dns_cf",
        message: "Cloudflare API 请求头无效，通常是 API 密钥/邮箱不正确导致",
        evidence: pickEvidence(logs, line => /Invalid request headers/i.test(line) || /"code"\s*:\s*6003/i.test(line)),
      };
    }
  }

  const retryAfterLine = [...logs].reverse().find(line => /retryafter\s*=\s*\d+/i.test(line));
  if (retryAfterLine && /will not retry|too large/i.test(retryAfterLine)) {
    const m = retryAfterLine.match(/retryafter\s*=\s*(\d+)/i);
    const seconds = m ? Number(m[1]) : NaN;
    const isTooLarge = Number.isFinite(seconds) && seconds > 600;
    if (isTooLarge) {
      return {
        reason: "acme_frequency_limited",
        provider,
        message: `申请频率受限（Retry-After=${seconds} 秒，超过 600 秒将不再重试），请等待后再试`,
        evidence: pickEvidence(logs, line => /retryafter\s*=\s*\d+/i.test(line) || /will not retry|too large/i.test(line)),
      };
    }
  }

  const rateLimited = has(/rate limit|too many requests|429/i);
  if (rateLimited) {
    return {
      reason: "dns_api_rate_limited",
      provider,
      message: "DNS API 触发限流（429/Rate limit），稍后重试",
      evidence: pickEvidence(logs, line => /rate limit|too many requests|429/i.test(line)),
    };
  }

  const failure = has(/failed|invalid/i);
  if (failure) {
    return {
      reason: "unknown",
      provider,
      message: "日志中检测到错误，但未能自动归因",
      evidence: pickEvidence(logs, line => /failed|invalid/i.test(line)),
    };
  }

  return null;
};

export const acmeRoutes = new Elysia({ prefix: "/api/admin/acme" })
  .use(acmePlugin)
  .get("/status", async ({ acme }) => {
    await acme.checkInstalled();
    const state = acme.getState();
    const settings = await configManager.getAcmeSettings();
    const primaryDomain = settings?.domains?.[0] || null;
    let acmeCert: { primaryDomain: string; info: any } | null = null;
    if (primaryDomain) {
      const loaded = await configManager.saveAcmeCertFromFS(primaryDomain);
      if (loaded) {
        const info = await configManager.getAcmeCertInfo(primaryDomain);
        acmeCert = { primaryDomain, info };
      }
    }
    return { success: true, data: { ...state, acmeCert } };
  })
  .get("/config", async () => {
    const cfg = await configManager.getAcmeSettings();
    return { success: true, data: cfg };
  })
  .get("/dns-providers", () => {
    return { success: true, data: dnsProviders };
  })
  .delete("/", async ({ acme, set }) => {
    try {
      const st = acme.getState();
      if (st.status === "installing") {
        set.status = 409;
        return { success: false, message: "acme.sh 安装中，无法删除" };
      }
      await acme.uninstall();
      await acme.checkInstalled();
      return { success: true, data: acme.getState() };
    } catch (e: any) {
      set.status = 500;
      return { success: false, message: e?.message || String(e) };
    }
  })
  .post("/init", ({ acme }) => {
    acme.startInstall();
    return {
      success: true,
      data: {
        executablePath: acme.getState().executablePath,
      }
    };
  })
  .post("/config", async ({ body, set }) => {
    try {
      const normalized = validateAndNormalizeAcmeRequest(body);
      const saved = await configManager.saveAcmeSettings({
        domains: normalized.domains,
        dnsType: normalized.dnsType,
        credentials: normalized.credentials,
      });
      return { success: true, data: saved };
    } catch (e: any) {
      set.status = 400;
      return { success: false, message: e?.message || String(e) };
    }
  }, {
    body: t.Object({
      domains: t.Array(t.String(), { minItems: 1 }),
      dnsType: t.String(),
      credentials: t.Optional(t.Record(t.String(), t.String()))
    })
  })
  .post("/request", async ({ acme, body, set }) => {
    try {
      const method = body.method ?? "dns";
      if (method !== "dns") {
        set.status = 400;
        return { success: false, message: "仅支持 DNS-01 验证方式" };
      }
      const normalized = validateAndNormalizeAcmeRequest({
        domains: body.domains,
        dnsType: body.dnsType,
        provider: body.provider,
        credentials: body.credentials,
      });
      const jobId = randomUUID();
      const primaryDomain = normalized.domains[0]!;
      await configManager.saveAcmeSettings({
        domains: normalized.domains,
        dnsType: normalized.dnsType,
        credentials: normalized.credentials,
      });
      await configManager.createAcmeJob({
        id: jobId,
        domains: normalized.domains,
        method: "dns",
        provider: normalized.dnsType,
        createdAt: new Date().toISOString(),
        status: "queued",
        progress: 0,
        message: ""
      });
      await configManager.clearAcmeLogs(jobId);
      void (async () => {
        await configManager.updateAcmeJob(jobId, { status: "running", progress: 5, message: "running" });
        try {
          await acme.issueCertificate({
            domains: normalized.domains,
            method: "dns",
            dnsType: normalized.dnsType,
            envVars: normalized.credentials,
            onLog: async (line: string) => {
              await configManager.appendAcmeLog(jobId, line);
            }
          });
          await configManager.updateAcmeJob(jobId, { progress: 80, message: "saving" });
          const saved = await configManager.saveAcmeCertFromFS(primaryDomain, { forceInstall: true });
          if (!saved) {
            await configManager.appendAcmeLog(jobId, "证书签发成功，但读取证书文件失败（请稍后重试或检查 acme.sh 目录）");
          }
          await configManager.updateAcmeJob(jobId, { status: "succeeded", progress: 100, message: saved ? "succeeded" : "signed" });
        } catch (e: any) {
          const msg = e?.message || String(e);
          await configManager.appendAcmeLog(jobId, `证书签发失败: ${msg}`);
          await configManager.updateAcmeJob(jobId, { status: "failed", progress: 100, message: msg });
        }
      })();
      return { success: true, data: { jobId } };
    } catch (e: any) {
      set.status = 400;
      return { success: false, message: e?.message || String(e) };
    }
  }, {
    body: t.Object({
      domains: t.Array(t.String(), { minItems: 1 }),
      method: t.Optional(t.Union([t.Literal("dns"), t.Literal("http"), t.Literal("https")])),
      provider: t.Optional(t.String()),
      dnsType: t.Optional(t.String()),
      credentials: t.Optional(t.Record(t.String(), t.String()))
    })
  })
  .get("/jobs/:id/poll", async ({ params, query, set }) => {
    const job = await configManager.getAcmeJob(params.id);
    if (!job) {
      set.status = 404;
      return { success: false, message: "not found" };
    }
    const limit = Math.max(1, Math.min(DEFAULT_REDIS_LOG_BUFFER_MAX_LEN, Number(query.limit ?? 500)));
    const order = query.order === "asc" ? "asc" : "desc";
    const logs = await configManager.getAcmeLogs(params.id, limit, order);
    const analysis = analyzeAcmeLogs(job, logs);
    return { success: true, data: { job, logs, analysis } };
  }, {
    query: t.Object({
      limit: t.Optional(t.Numeric()),
      order: t.Optional(t.Union([t.Literal("asc"), t.Literal("desc")]))
    })
  })
  .get("/jobs/:id", async ({ params, set }) => {
    const job = await configManager.getAcmeJob(params.id);
    if (!job) {
      set.status = 404;
      return { success: false, message: "not found" };
    }
    return { success: true, data: job };
  })
  .get("/jobs/:id/logs", async ({ params }) => {
    const logs = await configManager.getAcmeLogs(params.id, 500, "desc");
    return { success: true, data: logs };
  })
  .get("/certs/:domain", async ({ params, set }) => {
    const cert = await configManager.getAcmeCert(params.domain);
    if (!cert) {
      set.status = 404;
      return { success: false, message: "not found" };
    }
    const info = await configManager.getAcmeCertInfo(params.domain);
    return { success: true, data: { domain: params.domain, info } };
  })
  .delete("/certs/:domain", async ({ params }) => {
    const domain = params.domain;
    const pair = await configManager.getAcmeCert(domain);
    await configManager.deleteAcmeCert(domain);

    const { join } = await import("node:path");
    const { rm } = await import("node:fs/promises");
    await rm(join(process.cwd(), "data", "ssl", domain), { recursive: true, force: true });

    if (pair) {
      const config = await configManager.getConfig();
      if (config.ssl?.cert === pair.cert && config.ssl?.key === pair.key) {
        await configManager.clearSSL();
        await goBackend.clearSSL();
      }
    }

    return { success: true };
  })
  .get("/certs/:domain/download", async ({ params, set }) => {
    const cert = await configManager.getAcmeCert(params.domain);
    if (!cert) {
      set.status = 404;
      return { success: false, message: "not found" };
    }
    const entries = [
      { name: `${params.domain}.cert.pem`, data: new TextEncoder().encode(cert.cert) },
      { name: `${params.domain}.key.pem`, data: new TextEncoder().encode(cert.key) }
    ];
    const zipData = createZip(entries);
    return new Response(zipData, {
      headers: {
        "content-type": "application/zip",
        "content-disposition": `attachment; filename="${params.domain}.zip"`
      }
    });
  })
  .post("/certs/:domain/deploy", async ({ params }) => {
    const pair = await configManager.getAcmeCert(params.domain);
    if (!pair) {
      return { success: false, message: "证书不存在" };
    }
    const validation = configManager.validateSSLCert(pair.cert, pair.key);
    if (!validation.valid) {
      return { success: false, message: validation.error || "证书或私钥无效" };
    }
    await configManager.updateSSLConfig({ cert: pair.cert, key: pair.key });
    const resp = await goBackend.setSSL(pair.cert, pair.key);
    return { success: resp.success, message: resp.message || "成功" };
  });
