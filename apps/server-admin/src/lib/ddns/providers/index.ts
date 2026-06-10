import type { DDNSProviderDefinition, DDNSProviderUpdater } from "../types";
import { alidnsProvider, alidnsUpdate } from "./alidns";
import { baiduProvider, baiduUpdate } from "./baidu";
import { cloudflareProvider, cloudflareUpdate } from "./cloudflare";
import { dnspodProvider, dnspodUpdate } from "./dnspod";
import { duckdnsProvider, duckdnsUpdate } from "./duckdns";
import { dynuProvider, dynuUpdate } from "./dynu";
import { dynv6Provider, dynv6Update } from "./dynv6";
import { edgeoneCnameProvider, edgeoneCnameUpdate } from "./edgeone-cname";
import { edgeoneProvider, edgeoneUpdate } from "./edgeone";
import { esaProvider, esaUpdate } from "./esa";
import { godaddyProvider, godaddyUpdate } from "./godaddy";
import { huaweiProvider, huaweiUpdate } from "./huawei";
import { noipProvider, noipUpdate } from "./noip";
import { porkbunProvider, porkbunUpdate } from "./porkbun";
import { tencentcloudProvider, tencentcloudUpdate } from "./tencentcloud";

export const providerDefinitions: DDNSProviderDefinition[] = [
  alidnsProvider,
  baiduProvider,
  cloudflareProvider,
  dnspodProvider,
  duckdnsProvider,
  dynuProvider,
  dynv6Provider,
  edgeoneCnameProvider,
  edgeoneProvider,
  esaProvider,
  godaddyProvider,
  huaweiProvider,
  noipProvider,
  porkbunProvider,
  tencentcloudProvider,
];

export const providerUpdaters: Record<string, DDNSProviderUpdater> = {
  alidns: alidnsUpdate,
  baiducloud: baiduUpdate,
  cloudflare: cloudflareUpdate,
  dnspod: dnspodUpdate,
  duckdns: duckdnsUpdate,
  dynu: dynuUpdate,
  dynv6: dynv6Update,
  edgeone_cname: edgeoneCnameUpdate,
  edgeone: edgeoneUpdate,
  esa: esaUpdate,
  godaddy: godaddyUpdate,
  huaweicloud: huaweiUpdate,
  noip: noipUpdate,
  porkbun: porkbunUpdate,
  tencentcloud: tencentcloudUpdate,
};
