import { Elysia, t } from "elysia";
import { AcmeService } from "./AcmeService";
import { configManager } from "../../lib/redis";
import { hideFromDocs } from "../../lib/openapi";
import { createRequestTranslator } from "../../lib/i18n";

export const acmeService = new AcmeService();

export const acmePlugin = new Elysia()
  .decorate("acme", acmeService)

  .onStart(async () => {
    await acmeService.checkInstalled();
    console.log(
      `[Acme Plugin] initial status: ${acmeService.getState().status}`,
    );
  })

  .get(
    "/check",
    async ({ acme }) => {
      await acme.checkInstalled();
      return acme.getState();
    },
    hideFromDocs,
  )

  .post(
    "/install",
    async ({ acme, request, set }) => {
      const { t } = createRequestTranslator(
        request,
        await configManager.getLocaleConfig(),
      );
      const currentState = acme.getState();

      if (currentState.status === "installed") {
        set.status = 400;
        return { error: t("server.acme.alreadyInstalled") };
      }
      if (currentState.status === "installing") {
        set.status = 409;
        return { error: t("server.acme.installInProgress") };
      }

      const clientSettings = await configManager.ensureAcmeClientSettings(
        await acme.getDefaultCertificateAuthority(),
      );
      void acme.startInstall(undefined, clientSettings.certificateAuthority);

      return {
        message: t("server.acme.installSubmitted"),
        status: "installing",
      };
    },
    hideFromDocs,
  )

  // Trigger certificate issuance for DNS providers.
  .post(
    "/issue",
    async ({ acme, request, set, body }) => {
      const { t } = createRequestTranslator(
        request,
        await configManager.getLocaleConfig(),
      );
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
        return { message: t("server.acme.issueSucceeded") };
      } catch (error: any) {
        set.status = 500;
        return { error: error.message };
      }
    },
    {
      body: t.Object({
        domains: t.Array(t.String(), { minItems: 1 }),
        dnsType: t.String({
          description: "For example: dns_cf, dns_dp, dns_ali",
        }),
        envVars: t.Record(t.String(), t.String(), {
          description: "Environment variable configuration injected into API",
        }),
      }),
      ...hideFromDocs,
    },
  );
