import { Rule } from "../../lib/go-backend";

export interface ScanOptions {
  timeout?: number;
  maxConcurrent?: number;
  skipPorts?: number[];
  portRanges?: { start: number; end: number }[];
}

export interface ScanResult {
  host: string; // [新增] 必须传递 host，方便拼接 URL
  port: number;
  open: boolean;
  httpStatus?: number;
  headers?: Record<string, string>;
  body?: string;
  error?: string;
  serviceIdentity?: string;
}

export interface AnalyzerRule {
  name: string;
  match: (result: ScanResult) => boolean | Promise<boolean>; 
  label: string;
  rule: Rule;
  isDefault: boolean;
}