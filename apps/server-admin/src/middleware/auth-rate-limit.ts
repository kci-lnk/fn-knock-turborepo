import { randomUUID } from "node:crypto";
import { Elysia } from "elysia";
import { redis } from "../lib/redis";
import { getClientIp } from "../lib/auth-request";

const AUTH_RATE_LIMIT_WINDOW_MS = 1000;
const AUTH_RATE_LIMIT_MAX_REQUESTS = 6;
const AUTH_RATE_LIMIT_KEY_PREFIX = "fn_knock:rate_limit:auth:";

const AUTH_RATE_LIMIT_SCRIPT = `

local time = redis.call('TIME')
local now = tonumber(time[1]) * 1000 + math.floor(tonumber(time[2]) / 1000)

local key = KEYS[1]
local windowMs = tonumber(ARGV[1])
local maxRequests = tonumber(ARGV[2])
local member = ARGV[3]
local minScore = now - windowMs

redis.call('ZREMRANGEBYSCORE', key, 0, minScore)

local current = redis.call('ZCARD', key)
if current >= maxRequests then
  local oldest = redis.call('ZRANGE', key, 0, 0, 'WITHSCORES')
  local retryAfterMs = windowMs
  if oldest[2] then
    retryAfterMs = math.max(1, windowMs - (now - tonumber(oldest[2])))
  end
  redis.call('PEXPIRE', key, windowMs)
  return {0, current, retryAfterMs}
end

redis.call('ZADD', key, now, member)
redis.call('PEXPIRE', key, windowMs)
return {1, current + 1, 0}
`;

const normalizeAuthApiPath = (path: string) =>
  path.startsWith("/auth/api") ? path.replace("/auth/api", "/api/auth") : path;

const buildAuthRateLimitKey = (clientIp: string) =>
  `${AUTH_RATE_LIMIT_KEY_PREFIX}${clientIp}`;

const normalizeClientIp = (clientIp: string) => {
  const trimmed = clientIp.trim().toLowerCase();
  if (!trimmed) return "";

  if (trimmed.startsWith("::ffff:")) {
    return trimmed.slice("::ffff:".length);
  }

  return trimmed;
};

const isPrivateIpv4 = (clientIp: string) => {
  const parts = clientIp.split(".").map((part) => Number(part));
  if (
    parts.length !== 4 ||
    parts.some((part) => !Number.isInteger(part) || part < 0 || part > 255)
  ) {
    return false;
  }

  const [first, second] = parts;
  if (first === 10) return true;
  if (first === 127) return true;
  if (first === 192 && second === 168) return true;
  if (first === 169 && second === 254) return true;
  if (
    first === 172 &&
    typeof second === "number" &&
    second >= 16 &&
    second <= 31
  )
    return true;
  return false;
};

const isPrivateIpv6 = (clientIp: string) => {
  if (!clientIp.includes(':')) return false;
  if (clientIp === "::1" || clientIp === "0:0:0:0:0:0:0:1") return true;
  if (clientIp.startsWith("fe80:")) return true;
  if (clientIp.startsWith("fc") || clientIp.startsWith("fd")) return true;
  return false;
};

const shouldBypassRateLimit = (clientIp: string) => {
  const normalizedIp = normalizeClientIp(clientIp);
  if (!normalizedIp) return false;
  if (normalizedIp === "localhost") return true;
  if (isPrivateIpv4(normalizedIp)) return true;
  if (isPrivateIpv6(normalizedIp)) return true;
  return false;
};

export const authRateLimitMiddleware = new Elysia({ name: "auth-rate-limit" })
  .onBeforeHandle({ as: "global" }, async ({ request, path, set }) => {
    const normalizedPath = normalizeAuthApiPath(path);
    if (!normalizedPath.startsWith("/api/auth")) {
      return;
    }

    const clientIp = getClientIp(request);
    if (shouldBypassRateLimit(clientIp)) {
      return;
    }

    const now = Date.now();
    const requestId = `${request.headers.get("x-nonce") || "anon"}:${now}:${randomUUID()}`;
    const result = await redis.eval(
      AUTH_RATE_LIMIT_SCRIPT,
      1,
      buildAuthRateLimitKey(clientIp),
      String(AUTH_RATE_LIMIT_WINDOW_MS),
      String(AUTH_RATE_LIMIT_MAX_REQUESTS),
      requestId,
    );
    const [allowedRaw, countRaw, retryAfterMsRaw] = Array.isArray(result)
      ? result
      : [1, 0, 0];
    const allowed = Number(allowedRaw) === 1;
    const count = Number(countRaw) || 0;
    const retryAfterMs = Number(retryAfterMsRaw) || 0;

    set.headers["X-RateLimit-Limit"] = String(AUTH_RATE_LIMIT_MAX_REQUESTS);
    set.headers["X-RateLimit-Remaining"] = String(
      Math.max(0, AUTH_RATE_LIMIT_MAX_REQUESTS - count),
    );

    if (allowed) {
      return;
    }

    const retryAfter = Math.max(1, Math.ceil(retryAfterMs / 1000));
    set.status = 429;
    set.headers["Retry-After"] = String(retryAfter);
    return {
      success: false,
      message: "请求过于频繁，请稍后重试",
      retryAfter,
    };
  });
