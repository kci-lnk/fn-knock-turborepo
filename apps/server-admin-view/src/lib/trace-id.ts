export const TRACE_ID_PATTERN =
  /^(?:trc|waf)_[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;

export const normalizeTraceId = (value: unknown) => String(value ?? "").trim();

export const isTraceId = (value: unknown) =>
  TRACE_ID_PATTERN.test(normalizeTraceId(value));
