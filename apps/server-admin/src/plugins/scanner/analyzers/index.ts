import { ScanResult, AnalyzerRule } from "../types";
import { mongoExpressRule } from "./rules/mongo-express";
import { redisInsightRule } from "./rules/redis-insight";
import { go2rtcRule } from "./rules/go2trc";
import { fnosRule } from "./rules/fnos";
import { luckyRule } from "./rules/lucky";
import { alistRule, openListRule, xiaoyaRule } from "./rules/alist";
import { homeAssistantRule } from "./rules/homeassistant";
import { webdavRule } from "./rules/webdav";
import { xunleiRule } from "./rules/xunlei";
import { miniDLNARule } from "./rules/minidlna";
import { sunPanelRule } from "./rules/sun-panel";
import { nowenRule } from "./rules/nowen";
import { fnysRule } from "./rules/fnys";
import { dpanelRule } from "./rules/dpanel";
import { lotteryRule } from "./rules/lottery";
import { kuakeRule } from "./rules/kuake";
import { jellyfinRule } from "./rules/jellyfin";
import { mefrpRule } from "./rules/mefrp";
import { moontvRule } from "./rules/moontv";
import { fnosappsRule } from "./rules/fnosapps";
import { embyRule } from "./rules/emby";
import { dlymusicRule } from "./rules/dlymusic";
import { onePanelRule } from "./rules/1panel";

const rules: AnalyzerRule[] = [
  mongoExpressRule,
  redisInsightRule,
  go2rtcRule,
  fnosRule,
  luckyRule,
  xiaoyaRule,
  alistRule,
  openListRule,
  homeAssistantRule,
  sunPanelRule,
  webdavRule,
  xunleiRule,
  miniDLNARule,
  nowenRule,
  fnysRule,
  dpanelRule,
  lotteryRule,
  kuakeRule,
  jellyfinRule,
  mefrpRule,
  moontvRule,
  fnosappsRule,
  embyRule,
  dlymusicRule,
  onePanelRule,
];

function extractTitle(body?: string): string {
  if (!body) return "";
  const match = body.match(/<title[^>]*>(.*?)<\/title>/i);
  return match && match[1] ? match[1].trim() : "";
}

export async function analyzeService(
  result: ScanResult,
): Promise<AnalyzerRule> {
  for (const rule of rules) {
    try {
      const isMatch = await rule.match(result);
      if (isMatch) {
        return rule;
      }
    } catch (error) {
      continue;
    }
  }

  const title = extractTitle(result.body);

  return {
    name: title,
    label: title,
    rule: {
      path: "",
      rewrite_html: true,
      use_auth: true,
      use_root_mode: false,
      strip_path: true,
      target: "",
    },
    isDefault: false,
    match: () => true,
  };
}
