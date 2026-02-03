local util = require("util")
local config = require("config")

local sms_handler = {}

-- ======================== Queue Index Management ========================

local function add_to_index(sms_id)
    local index_str = fskv.get("sms_queue_index") or ""

    -- Check queue size
    local count = 0
    for _ in string.gmatch(index_str, "[^,]+") do
        count = count + 1
    end

    if count >= config.SMS_MAX_QUEUE_SIZE then
        log.error("sms_handler", "Queue full, cannot add more SMS")
        return false
    end

    if index_str == "" then
        index_str = sms_id
    else
        index_str = index_str .. "," .. sms_id
    end

    fskv.set("sms_queue_index", index_str)
    return true
end

local function remove_from_index(sms_id)
    local index_str = fskv.get("sms_queue_index")
    if not index_str then
        return
    end

    -- Remove from comma-separated list
    local new_index = {}
    for id in string.gmatch(index_str, "[^,]+") do
        if id ~= sms_id then
            table.insert(new_index, id)
        end
    end

    local new_index_str = table.concat(new_index, ",")
    if new_index_str == "" then
        fskv.del("sms_queue_index")
    else
        fskv.set("sms_queue_index", new_index_str)
    end
end

-- ======================== SMS Storage ========================

local function store_sms(sms_id, sms_data)
    local queue_item = {
        id = sms_id,
        sender = sms_data.sender,
        content = sms_data.content,
        received_at = sms_data.received_at,
        metas = sms_data.metas,
        retry_count = 0,
        next_retry_time = os.time() + (config.SMS_RETRY_INTERVAL_BASE / 1000),
        created_at = os.time()
    }

    -- Store to FSKV
    local success = fskv.set("sms_queue:" .. sms_id, json.encode(queue_item))
    if not success then
        log.error("sms_handler", "Failed to store SMS to FSKV")
        return false
    end

    -- Update index
    if not add_to_index(sms_id) then
        -- Rollback if index update failed
        fskv.del("sms_queue:" .. sms_id)
        return false
    end

    log.info("sms_handler", "SMS stored to queue: " .. sms_id)
    return true
end

-- ======================== ACK Handler ========================

function sms_handler.handle_ack(sms_id)
    log.info("sms_handler", "Received ACK for: " .. sms_id)

    -- Delete from FSKV
    fskv.del("sms_queue:" .. sms_id)

    -- Remove from index
    remove_from_index(sms_id)

    log.info("sms_handler", "SMS removed from queue: " .. sms_id)
end

-- ======================== Retry Logic ========================

local function calculate_retry_delay(retry_count)
    local base = config.SMS_RETRY_INTERVAL_BASE / 1000 -- Convert to seconds
    local multiplier = config.SMS_RETRY_BACKOFF_MULTIPLIER
    return base * (multiplier ^ retry_count)
end

local function retry_pending_sms()
    local index_str = fskv.get("sms_queue_index")
    if not index_str or index_str == "" then
        return
    end

    local current_time = os.time()

    -- Iterate through all queued SMS
    for id in string.gmatch(index_str, "[^,]+") do
        local key = "sms_queue:" .. id
        local data_str = fskv.get(key)

        if data_str then
            local queue_item = json.decode(data_str)

            -- Check if it's time to retry
            if current_time >= queue_item.next_retry_time then
                if queue_item.retry_count >= config.SMS_MAX_RETRY_COUNT then
                    -- Max retries exceeded, remove
                    log.error("sms_handler", "Max retries exceeded for: " .. id)
                    fskv.del(key)
                    remove_from_index(id)
                else
                    -- Retry sending
                    log.info("sms_handler", "Retrying SMS: " .. id .. " attempt: " .. (queue_item.retry_count + 1))

                    local payload = {
                        id = queue_item.id,
                        sender = queue_item.sender,
                        content = queue_item.content,
                        received_at = queue_item.received_at,
                        metas = queue_item.metas
                    }

                    -- Send with the same UUID as message ID
                    util.uart_send(id, "SMS_RECEIVED", payload)

                    -- Update retry metadata
                    queue_item.retry_count = queue_item.retry_count + 1
                    local next_delay = calculate_retry_delay(queue_item.retry_count)
                    queue_item.next_retry_time = current_time + next_delay

                    fskv.set(key, json.encode(queue_item))
                end
            end
        end
    end
end

-- ======================== SMS Callback ========================

local function on_sms_received(num, txt, metas)
    if not config.SMS_FORWARD_ENABLED then
        log.info("sms_handler", "SMS forwarding disabled, ignoring message")
        return
    end

    log.info("sms_handler", "SMS received from " .. (num or "unknown") .. " content: " .. (txt or ""))

    -- Generate UUID for this SMS
    local sms_id = util.uuid()

    -- Prepare payload
    local payload = {
        id = sms_id,
        sender = num or "",
        content = txt or "",
        received_at = os.time(),
        metas = metas
    }

    -- Store to FSKV first
    if not store_sms(sms_id, payload) then
        log.error("sms_handler", "Failed to store SMS, sending without persistence (best effort)")
    end

    -- Send once using the UUID as message ID
    local bytes = util.uart_send(sms_id, "SMS_RECEIVED", payload)
    if bytes and bytes > 0 then
        log.info("sms_handler", "SMS sent, bytes: " .. bytes .. " id: " .. sms_id)
    else
        log.warn("sms_handler", "Initial SMS send failed, will retry: " .. sms_id)
    end
end

-- ======================== Initialization ========================

function sms_handler.init()
    sms.debug(true)
    sms.setNewSmsCb(on_sms_received)
    log.info("sms_handler", "SMS callback registered")

    -- Start retry timer
    sys.timerLoopStart(retry_pending_sms, config.SMS_QUEUE_CHECK_INTERVAL)
    log.info("sms_handler", "SMS retry timer started (interval: " .. config.SMS_QUEUE_CHECK_INTERVAL .. "ms)")
end

return sms_handler
