import { APP_GITHUB_URL, APP_LOCAL_VERSION } from "../../app-version";
import type {
  DDNSProviderContext,
  DDNSProviderDefinition,
  DDNSUpdateResult,
} from "../types";
import { ddnsProviderT, getTimeoutMs, parseTextResponse } from "./helpers";

const NOIP_ENDPOINT = "https://dynupdate.no-ip.com/nic/update";
const NOIP_SUCCESS_STATUSES = new Set(["good", "nochg"]);
const NOIP_STATUS_CODES = new Set([
  "nohost",
  "badauth",
  "badagent",
  "!donator",
  "abuse",
  "911",
]);
const noipT = (
  key: string,
  params?: Record<string, string | number | boolean | null | undefined>,
) => ddnsProviderT("noip", key, params);

export const noipProvider: DDNSProviderDefinition = {
  name: "noip",
  label: "NO-IP",
  fields: [
    {
      key: "hostname",
      label: "Hostname",
      type: "text",
      placeholder: "home.ddns.net",
      required: true,
      description: noipT("fields.hostname.description"),
    },
    {
      key: "username",
      label: noipT("fields.username.label"),
      type: "text",
      placeholder: "DDNS Key Username",
      required: true,
      description: noipT("fields.username.description"),
    },
    {
      key: "password",
      label: noipT("fields.password.label"),
      type: "password",
      placeholder: "DDNS Key Password",
      required: true,
      description: noipT("fields.password.description"),
    },
  ],
};

function buildNoipMessage(
  statuses: Array<{ code: string; detail: string }>,
  ipv4: string | null,
  ipv6: string | null,
): DDNSUpdateResult {
  const failures = statuses.filter(
    (item) => !NOIP_SUCCESS_STATUSES.has(item.code),
  );
  if (failures.length > 0) {
    const detail = failures
      .map(({ code, detail: rawDetail }) => {
        const reason =
          (NOIP_STATUS_CODES.has(code)
            ? noipT(`statusMessages.${code}`)
            : "") ||
          rawDetail ||
          noipT("unknownStatus", { code });
        return rawDetail && NOIP_STATUS_CODES.has(code)
          ? `${code} (${reason}; ${rawDetail})`
          : `${code} (${reason})`;
      })
      .join("; ");

    return {
      success: false,
      message: noipT("updateFailed", { detail }),
      ipv4Updated: false,
      ipv6Updated: false,
    };
  }

  const changed = statuses.some((item) => item.code === "good");
  const details = statuses.map((item) => item.detail).filter(Boolean);
  const detailSuffix = details.length > 0 ? ` (${details.join("; ")})` : "";

  return {
    success: true,
    message: changed
      ? noipT("updateSuccess", { detail: detailSuffix })
      : noipT("ipUnchanged", { detail: detailSuffix }),
    ipv4Updated: changed && !!ipv4,
    ipv6Updated: changed && !!ipv6,
  };
}

export async function noipUpdate(
  { config, http }: DDNSProviderContext,
  ipv4: string | null,
  ipv6: string | null,
): Promise<DDNSUpdateResult> {
  const hostname = config.hostname?.trim();
  const username = config.username?.trim();
  const password = config.password?.trim();

  if (!hostname || !username || !password) {
    return { success: false, message: noipT("configIncomplete") };
  }

  if (!ipv4 && !ipv6) {
    return {
      success: false,
      message: noipT("noIpAvailable"),
    };
  }

  const params = new URLSearchParams({ hostname });
  if (ipv4 && ipv6) {
    params.set("myip", `${ipv4},${ipv6}`);
  } else if (ipv4) {
    params.set("myip", ipv4);
  } else if (ipv6) {
    params.set("myipv6", ipv6);
  }

  const timeoutMs = getTimeoutMs();
  const authorization = Buffer.from(`${username}:${password}`).toString(
    "base64",
  );
  const userAgent = `fn-knock/${APP_LOCAL_VERSION} (${APP_GITHUB_URL})`;

  try {
    const response = await http.fetch(`${NOIP_ENDPOINT}?${params.toString()}`, {
      headers: {
        Accept: "text/plain",
        Authorization: `Basic ${authorization}`,
        "User-Agent": userAgent,
      },
      signal: AbortSignal.timeout(timeoutMs),
    });

    const text = await parseTextResponse(response);

    if (!response.ok) {
      return {
        success: false,
        message: noipT("updateFailedWithStatus", {
          status: response.status,
          detail: text || noipT("requestFailed"),
        }),
      };
    }

    const lines = text
      .split(/\r?\n/)
      .map((line) => line.trim())
      .filter(Boolean);

    if (lines.length === 0) {
      return { success: false, message: noipT("emptyResponse") };
    }

    const statuses = lines.map((line) => {
      const [code = "", ...rest] = line.split(/\s+/);
      return { code, detail: rest.join(" ").trim() };
    });

    return buildNoipMessage(statuses, ipv4, ipv6);
  } catch (error) {
    const err = error instanceof Error ? error : new Error(String(error));
    throw new Error(noipT("requestError", { detail: err.message }));
  }
}
