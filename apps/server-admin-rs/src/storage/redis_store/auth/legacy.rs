pub(in crate::storage::redis_store) const LOGIN_BACKOFF_PREFIX: &str = "fn_knock:login_backoff:";
pub(in crate::storage::redis_store) const LOGIN_BACKOFF_TTL_SECONDS: i64 = 3600;
pub(in crate::storage::redis_store) const TOTP_SUBDOMAIN_ACCESS_SELECT_PAGE: &str =
    "__builtin_select__";
pub(in crate::storage::redis_store) const TOTP_SUBDOMAIN_ACCESS_SELECT_PAGE_PATH: &str =
    "/__select__";
pub(in crate::storage::redis_store) const TOTP_SUBDOMAIN_ACCESS_WOL_PAGE: &str = "__builtin_wol__";
pub(in crate::storage::redis_store) const TOTP_SUBDOMAIN_ACCESS_WOL_PAGE_PATH: &str = "/__wol__";
pub(in crate::storage::redis_store) const LOGIN_BACKOFF_REGISTER_FAILURE_SCRIPT: &str = r#"
-- fn-knock:eval:login-backoff:v1
local key = KEYS[1]
local ip = ARGV[1]
local now = tonumber(ARGV[2])
local ttlSeconds = tonumber(ARGV[3])
local baseDelay = tonumber(ARGV[4])
local maxDelay = tonumber(ARGV[5])
local jitterFactor = tonumber(ARGV[6])

local attempts = 0
local raw = redis.call('GET', key)
if raw then
  local ok, decoded = pcall(cjson.decode, raw)
  if ok and type(decoded) == 'table' and tonumber(decoded.attempts) then
    attempts = tonumber(decoded.attempts)
  end
end

attempts = attempts + 1

local expDelay = math.pow(2, attempts - 1) * baseDelay
local seed = ip .. ':' .. tostring(attempts) .. ':' .. tostring(now)
local hash = 0
for i = 1, #seed do
  hash = (hash * 33 + string.byte(seed, i)) % 1000003
end
local ratio = (hash % 10000) / 10000
local jitter = ((ratio * 2) - 1) * (expDelay * jitterFactor)
local backoffMs = math.floor(expDelay + jitter)
if backoffMs < 0 then
  backoffMs = 0
end
if backoffMs > maxDelay then
  backoffMs = maxDelay
end

local blockedUntil = now + backoffMs
local nextState = cjson.encode({
  ip = ip,
  attempts = attempts,
  lastAttempt = now,
  blockedUntil = blockedUntil,
})

redis.call('SET', key, nextState, 'EX', ttlSeconds)
return {attempts, math.ceil(backoffMs / 1000), blockedUntil}
"#;

pub(in crate::storage::redis_store) fn login_backoff_key(ip: &str) -> String {
    format!("{LOGIN_BACKOFF_PREFIX}{ip}")
}
