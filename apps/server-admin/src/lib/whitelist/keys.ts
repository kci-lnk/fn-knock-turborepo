import { createHash } from "node:crypto";
import { normalizeIp } from "../ip-normalize";

const PREFIX = "fn_knock:whitelist";

export const KEYS = {
  RECORDS: `${PREFIX}:records`,
  RECORD_ORDER: `${PREFIX}:record_order`,
  EXPIRY: `${PREFIX}:expiry`,
  IPS: `${PREFIX}:ips`,
  CIDR_RECORDS: `${PREFIX}:cidr_records`,
  DELETED: `${PREFIX}:deleted`,
};

export const getIPRecordsKey = (ip: string): string => {
  const normalizedIp = normalizeIp(ip) || String(ip || "").trim();
  return `${PREFIX}:ip_records:${normalizedIp}`;
};

export const getAutoOwnerRecordKey = (ownerKey: string): string => {
  const digest = createHash("sha256")
    .update(String(ownerKey || "").trim())
    .digest("hex");
  return `${PREFIX}:auto_owner:${digest}`;
};
