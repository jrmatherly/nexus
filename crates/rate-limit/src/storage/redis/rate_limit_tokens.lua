local current_key = KEYS[1]
local previous_key = KEYS[2]
local tokens = tonumber(ARGV[1])
local limit = tonumber(ARGV[2])
local current_window = tonumber(ARGV[3])
local expire_time = tonumber(ARGV[4])
local bucket_percentage = tonumber(ARGV[5])

-- Get counts from both windows
local current_count = tonumber(redis.call('GET', current_key) or '0')
local previous_count = tonumber(redis.call('GET', previous_key) or '0')

-- Calculate weighted count
local weighted_count = previous_count * (1.0 - bucket_percentage) + current_count

-- Check if adding tokens would exceed limit
if weighted_count + tokens > limit then
    return { 0, current_count, previous_count } -- Not allowed
end

-- Increment current window by token amount
current_count = redis.call('INCRBY', current_key, tokens)
redis.call('EXPIRE', current_key, expire_time)

return { 1, current_count, previous_count } -- Allowed
