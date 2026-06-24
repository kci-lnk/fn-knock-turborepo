import { AnalyzerRule } from "../../types";

const LUCI_LOGIN_REQUIRED_HEADER = "x-luci-login-required";
const LUCI_TITLE_PATTERN = /(?:^|[^a-z0-9])luci(?:$|[^a-z0-9])/i;

const extractTitle = (body?: string): string => {
  if (!body) return "";

  const match = body.match(/<title[^>]*>([\s\S]*?)<\/title>/i);
  return match?.[1]?.replace(/\s+/g, " ").trim().toLowerCase() ?? "";
};

const hasLuciLoginRequiredHeader = (
  headers: Record<string, string> | undefined,
): boolean =>
  headers?.[LUCI_LOGIN_REQUIRED_HEADER]?.trim().toLowerCase() === "yes";

const hasLuciEntrypoint = (body?: string): boolean => {
  if (!body) return false;

  const normalized = body.toLowerCase();
  return (
    normalized.includes("cgi-bin/luci") &&
    (normalized.includes("luci - lua configuration interface") ||
      normalized.includes('http-equiv="refresh"') ||
      normalized.includes("http-equiv='refresh'") ||
      normalized.includes("http-equiv=refresh"))
  );
};

const hasLuciLoginPage = (body?: string): boolean => {
  if (!body) return false;

  const title = extractTitle(body);
  const normalized = body.toLowerCase();
  return (
    LUCI_TITLE_PATTERN.test(title) &&
    (normalized.includes("/luci-static/") ||
      normalized.includes("application-name") ||
      normalized.includes("apple-mobile-web-app-title"))
  );
};

export const openWrtRule: AnalyzerRule = {
  name: "openwrt",
  label: "OpenWrt LuCI",
  rule: {
    path: "/openwrt",
    rewrite_html: false,
    use_auth: true,
    use_root_mode: true,
    strip_path: true,
    target: "",
  },
  isDefault: false,
  match: (result) => {
    return (
      hasLuciLoginRequiredHeader(result.headers) ||
      hasLuciEntrypoint(result.body) ||
      hasLuciLoginPage(result.body)
    );
  },
};
