import { Elysia, t } from "elysia";
import { AcmeService } from "./AcmeService";
import { configManager } from "../../lib/redis";

export const acmeService = new AcmeService();

export const acmePlugin = new Elysia()
  .decorate("acme", acmeService)
  
  .onStart(async () => {
    await acmeService.checkInstalled();
    console.log(`[Acme Plugin] 初始状态: ${acmeService.getState().status}`);
  })
  
  .get("/check", async ({ acme }) => {
    await acme.checkInstalled();
    return acme.getState();
  })

  .post("/install", async ({ acme, set }) => {
    const currentState = acme.getState();
    
    if (currentState.status === "installed") {
      set.status = 400;
      return { error: "acme.sh 已经安装过了" };
    }
    if (currentState.status === "installing") {
      set.status = 409;
      return { error: "安装任务正在进行中" };
    }

    const clientSettings = await configManager.ensureAcmeClientSettings(
      await acme.getDefaultCertificateAuthority(),
    );
    void acme.startInstall(undefined, clientSettings.certificateAuthority);
    
    return { 
      message: "安装任务已提交", 
      status: "installing" 
    };
  })

  // 新增：触发证书签发接口 (支持各类 DNS)
  .post("/issue", async ({ acme, set, body }) => {
    try {
      const clientSettings = await configManager.ensureAcmeClientSettings(
        await acme.getDefaultCertificateAuthority(),
      );
      await acme.issueCertificate({
        domains: body.domains,
        method: "dns",
        dnsType: body.dnsType,
        envVars: body.envVars,
        certificateAuthority: clientSettings.certificateAuthority,
      });
      return { message: "证书签发成功" };
    } catch (error: any) {
      set.status = 500;
      return { error: error.message };
    }
  }, {
    body: t.Object({
      domains: t.Array(t.String(), { minItems: 1 }),
      dnsType: t.String({ description: "例如 dns_cf, dns_dp, dns_ali" }),
      envVars: t.Record(t.String(), t.String(), { description: "注入给 API 的环境变量配置" })
    })
  });
