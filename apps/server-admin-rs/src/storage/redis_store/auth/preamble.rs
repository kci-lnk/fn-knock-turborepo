use super::*;

pub(super) const UPDATE_JSON_CAS_SCRIPT: &str = r#"
-- fn-knock:eval:update-json-cas:v1
local current = redis.call("GET", KEYS[1])
if not current then return -1 end
if current ~= ARGV[1] then return 0 end
local ttl = redis.call("PTTL", KEYS[1])
if ttl == -2 or ttl == 0 then return -1 end
if ttl > 0 then
  redis.call("SET", KEYS[1], ARGV[2], "PX", ttl)
else
  redis.call("SET", KEYS[1], ARGV[2])
end
return 1
"#;

pub(super) const COLLECT_AUTH_MOBILITY_SESSION_WHITELIST_SCRIPT: &str = r#"
-- fn-knock:eval:collect-mobility-session-whitelist:v1
local session_id = ARGV[1]
local whitelist_ids = {}
local seen_whitelist = {}

local function add_whitelist(id)
  if type(id) == "string" and id ~= "" and not seen_whitelist[id] then
    seen_whitelist[id] = true
    table.insert(whitelist_ids, id)
  end
end

local function decode_json(raw)
  if not raw then return nil end
  local ok, decoded = pcall(cjson.decode, raw)
  if not ok or type(decoded) ~= "table" then return nil end
  return decoded
end

local binding_keys = redis.call("SMEMBERS", KEYS[1])
table.insert(binding_keys, KEYS[3])
local seen_binding = {}
for _, binding_key in ipairs(binding_keys) do
  if not seen_binding[binding_key] then
    seen_binding[binding_key] = true
    local decoded = decode_json(redis.call("GET", binding_key))
    if decoded and (binding_key == KEYS[3] or decoded["ownerSessionId"] == session_id) then
      add_whitelist(decoded["whitelistRecordId"])
    end
  end
end
for _, raw in ipairs(redis.call("HVALS", KEYS[2])) do
  local decoded = decode_json(raw)
  if decoded then add_whitelist(decoded["whitelistRecordId"]) end
end
local pending = redis.call("HKEYS", KEYS[4])
for _, id in ipairs(pending) do add_whitelist(id) end
table.sort(whitelist_ids)
return whitelist_ids
"#;

pub(super) async fn compare_and_set_json(
    conn: &mut ConnectionManager,
    key: &str,
    expected_raw: &str,
    next_raw: &str,
) -> crate::storage::StorageResult<i64> {
    redis::cmd("EVAL")
        .arg(UPDATE_JSON_CAS_SCRIPT)
        .arg(1)
        .arg(key)
        .arg(expected_raw)
        .arg(next_raw)
        .query_async(conn)
        .await
}
