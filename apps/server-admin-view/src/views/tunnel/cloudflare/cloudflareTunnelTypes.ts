import type { CloudflaredProtocol } from "@/lib/api/tunnel";

export type CloudflareTranslate = (
  key: string,
  named?: Record<string, unknown>,
) => string;

export type CloudflaredProtocolOption = {
  value: CloudflaredProtocol;
  label: string;
  description: string;
};
