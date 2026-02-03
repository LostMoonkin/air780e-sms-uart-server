return {
    MESSAGE_PROCESS_INTERVAL = 500,
    MESSAGE_PROCESS_MAX_RETRY_COUNT = 10, -- Deprecated, kept for compatibility
    FLYMODE_INTERVAL = 1000 * 60 * 60 * 24,
    HEART_BEAT_INTERVAL = 1000 * 60 * 1,
    ENABLE_HEART_BEAT = true,
    SMS_FORWARD_ENABLED = true,

    -- SMS retry configuration
    SMS_RETRY_INTERVAL_BASE = 5000,   -- 5 seconds initial retry
    SMS_RETRY_BACKOFF_MULTIPLIER = 3, -- 3x exponential backoff
    SMS_MAX_RETRY_COUNT = 5,          -- Max 5 retries (~10 min total)
    SMS_QUEUE_CHECK_INTERVAL = 5000,  -- Check queue every 5 seconds
    SMS_MAX_QUEUE_SIZE = 100,         -- Max 100 pending messages
}
