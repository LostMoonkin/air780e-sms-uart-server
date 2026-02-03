local random = math.random
local util = {}
function util.uuid()
    local template = 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'
    return string.gsub(template, '[xy]', function(c)
        local v = (c == 'x') and random(0, 0xf) or random(8, 0xb)
        return string.format('%x', v)
    end)
end

function util.uart_send(id, dataType, body)
    if id == nil or id == "" then
        id = util.uuid()
    end
    if dataType == nil or dataType == "" then
        dataType = "UNKNOWN"
    end
    if type(body) ~= "table" then
        body = {
            data = body
        }
    end
    local rawData = id .. ":" .. dataType .. ":" .. string.toBase64(json.encode(body)) .. "\r\n"
    return uart.write(uart.VUART_0, rawData)
end

return util
