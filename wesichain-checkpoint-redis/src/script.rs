pub const LUA_SAVE: &str = r#"
-- KEYS[1] = {tag}:seq
-- KEYS[2] = {tag}:latest
-- KEYS[3] = {tag}:hist
-- ARGV[1] = serialized checkpoint JSON
-- ARGV[2] = ttl_seconds (0 = no TTL)
local seq = redis.call('INCR', KEYS[1])
local hist_key = KEYS[3] .. ':' .. seq
redis.call('SET', KEYS[2], ARGV[1])
redis.call('SET', hist_key, ARGV[1])
if tonumber(ARGV[2]) > 0 then
  redis.call('EXPIRE', KEYS[1], ARGV[2])
  redis.call('EXPIRE', KEYS[2], ARGV[2])
  redis.call('EXPIRE', hist_key, ARGV[2])
end
return seq
"#;
