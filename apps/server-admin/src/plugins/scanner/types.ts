import { Rule } from "../../lib/go-backend";

export interface ScanPortProgress {
  host: string;
  port: number;
  scannedPorts: number;
  totalPorts: number;
}

export interface ScanOptions {
  timeout?: number;
  maxConcurrent?: number;
  hostConcurrency?: number;
  skipPorts?: number[];
  portRanges?: { start: number; end: number }[];
  signal?: AbortSignal;
  onPortScanned?: (progress: ScanPortProgress) => void;
  onResult?: (result: ScanResult) => void | Promise<void>;
  onService?: (service: AnalyzedScanService) => void | Promise<void>;
}

export interface ScanResult {
  host: string; // [新增] 必须传递 host，方便拼接 URL
  port: number;
  open: boolean;
  httpStatus?: number;
  headers?: Record<string, string>;
  requiresBasicAuth?: boolean;
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

export interface AnalyzedScanService {
  host: string;
  port: number;
  httpStatus?: number;
  requiresBasicAuth?: boolean;
  detail: AnalyzerRule;
  serviceKey: string;
}
