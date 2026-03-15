import { APP_GITHUB_URL, APP_LOCAL_VERSION } from "../../app-version";
import type {
  DDNSProviderContext,
  DDNSProviderDefinition,
  DDNSUpdateResult,
} from "../types";
import { getTimeoutMs, parseTextResponse } from "./helpers";

const NOIP_ENDPOINT = "https://dynupdate.no-ip.com/nic/update";
const NOIP_SUCCESS_STATUSES = new Set(["good", "nochg"]);

const NOIP_STATUS_MESSAGES: Record<string, string> = {
  nohost: "指定的主机名不存在或不属于当前 DDNS Key",
  badauth: "用户名或密码错误",
  badagent: "客户端被 NO-IP 禁用，请检查 User-Agent 或客户端状态",
  "!donator": "当前账号不支持请求中的增强功能",
  abuse: "该 DDNS Key 因滥用被 NO-IP 封禁",
  "911": "NO-IP 服务端发生临时故障，官方建议至少 30 分钟后再重试",
};

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
      description: "填写完整主机名，支持逗号分隔多个 hostname",
    },
    {
      key: "username",
      label: "用户名",
      type: "text",
      placeholder: "DDNS Key Username",
      required: true,
      description: "建议使用 NO-IP 控制台生成的 DDNS Key 用户名",
    },
    {
      key: "password",
      label: "密码",
      type: "password",
      placeholder: "DDNS Key Password",
      required: true,
      description: "建议使用与 DDNS Key 配套的密码，而不是主账号密码",
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
          NOIP_STATUS_MESSAGES[code] || rawDetail || `返回未知状态: ${code}`;
        return rawDetail && NOIP_STATUS_MESSAGES[code]
          ? `${code} (${reason}; ${rawDetail})`
          : `${code} (${reason})`;
      })
      .join("; ");

    return {
      success: false,
      message: `NO-IP 更新失败: ${detail}`,
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
      ? `NO-IP 更新成功${detailSuffix}`
      : `NO-IP IP 未变化${detailSuffix}`,
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
    return { success: false, message: "NO-IP 配置不完整" };
  }

  if (!ipv4 && !ipv6) {
    return {
      success: false,
      message: "NO-IP 更新失败: 没有可用的 IPv4 或 IPv6 地址",
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
        message: `NO-IP 更新失败 [${response.status}]: ${text || "请求失败"}`,
      };
    }

    const lines = text
      .split(/\r?\n/)
      .map((line) => line.trim())
      .filter(Boolean);

    if (lines.length === 0) {
      return { success: false, message: "NO-IP 更新失败: 返回了空响应" };
    }

    const statuses = lines.map((line) => {
      const [code = "", ...rest] = line.split(/\s+/);
      return { code, detail: rest.join(" ").trim() };
    });

    return buildNoipMessage(statuses, ipv4, ipv6);
  } catch (error) {
    const err = error instanceof Error ? error : new Error(String(error));
    throw new Error(`NO-IP 请求异常: ${err.message}`);
  }
}
