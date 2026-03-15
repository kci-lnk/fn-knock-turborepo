import type { DDNSProviderDefinition, DDNSProviderUpdater } from "../types";
import { alidnsProvider, alidnsUpdate } from "./alidns";
import { baiduProvider, baiduUpdate } from "./baidu";
import { cloudflareProvider, cloudflareUpdate } from "./cloudflare";
import { dnspodProvider, dnspodUpdate } from "./dnspod";
import { duckdnsProvider, duckdnsUpdate } from "./duckdns";
import { dynv6Provider, dynv6Update } from "./dynv6";
import { esaProvider, esaUpdate } from "./esa";
import { godaddyProvider, godaddyUpdate } from "./godaddy";
import { huaweiProvider, huaweiUpdate } from "./huawei";
import { porkbunProvider, porkbunUpdate } from "./porkbun";
import { tencentcloudProvider, tencentcloudUpdate } from "./tencentcloud";

export const providerDefinitions: DDNSProviderDefinition[] = [
  alidnsProvider,
  baiduProvider,
  cloudflareProvider,
  dnspodProvider,
  duckdnsProvider,
  dynv6Provider,
  esaProvider,
  godaddyProvider,
  huaweiProvider,
  porkbunProvider,
  tencentcloudProvider,
];

export const providerUpdaters: Record<string, DDNSProviderUpdater> = {
  alidns: alidnsUpdate,
  baiducloud: baiduUpdate,
  cloudflare: cloudflareUpdate,
  dnspod: dnspodUpdate,
  duckdns: duckdnsUpdate,
  dynv6: dynv6Update,
  esa: esaUpdate,
  godaddy: godaddyUpdate,
  huaweicloud: huaweiUpdate,
  porkbun: porkbunUpdate,
  tencentcloud: tencentcloudUpdate,
};
