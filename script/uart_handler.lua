local sms_handler = require("sms_handler")
local util = require("util")
local uart_handler = {}

-- ======================== Command Handlers ========================

local function handle_get_device_info()
    local imei = mobile.imei()
    local number = mobile.number()
    local status = mobile.status()
    local rssi = mobile.rssi()
    local iccid = mobile.iccid()

    local device_info = {
        imei = imei or "",
        number = number or "",
        status = status or 0,
        rssi = rssi or 0,
        iccid = iccid or "",
        timestamp = os.time()
    }

    util.uart_send("", "DEVICE_INFO", device_info)
    log.info("uart_handler", "Device info sent: IMEI=" .. (imei or "N/A"))
end

-- ======================== Message Handler ========================

function uart_handler.handle_uart_message(message)
    -- Remove any trailing newlines/carriage returns
    message = message:gsub("[\r\n]", "")

    -- Parse ACK messages: ACK:<sms_id>
    if message:match("^ACK:") then
        local sms_id = message:match("^ACK:(.+)")
        if sms_id then
            sms_handler.handle_ack(sms_id)
        else
            log.warn("uart_handler", "Malformed ACK message: " .. message)
        end
        -- Parse command messages: CMD:<command>
    elseif message:match("^CMD:") then
        local command = message:match("^CMD:(.+)")
        if command == "GET_DEVICE_INFO" then
            log.info("uart_handler", "Received command: GET_DEVICE_INFO")
            handle_get_device_info()
        else
            log.warn("uart_handler", "Unknown command: " .. (command or "N/A"))
        end
    else
        log.warn("uart_handler", "Unknown message format: " .. message)
    end
end

function uart_handler.init()
    -- Register UART receive handler for ACK messages
    uart.on(uart.VUART_0, "receive", function(id, len)
        local data = uart.read(id, len)
        if data and data ~= "" then
            uart_handler.handle_uart_message(data)
        end
    end)
    log.info("uart_handler", "UART receive handler registered")
end

return uart_handler
