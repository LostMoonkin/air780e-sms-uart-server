PROJECT = "Air780e_SMS_UART_Sender"
VERSION = "1.0.0"

log.setLevel("DEBUG")
log.info("main", PROJECT, VERSION)

sys = require("sys")
config = require("config")
util = require("util")
sms_handler = require("sms_handler")
uart_handler = require("uart_handler")

if wdt then
    --添加硬狗防止程序卡死，在支持的设备上启用这个功能
    wdt.init(9000)                     --初始化watchdog设置为9s
    sys.timerLoopStart(wdt.feed, 3000) --3s喂一次狗
end

-- SIM 自动恢复, 周期性获取小区信息, 网络遇到严重故障时尝试自动恢复等功能
mobile.setAuto(10000, 30000, 8, true, 60000)

-- 串口初始化
local uart_setup_res = uart.setup(uart.VUART_0, 115200, 8, 1, uart.NONE)
if uart_setup_res ~= 0 then
    log.error("uart init", "UART init failed" .. uart_setup_res .. ", reboot after 10s.")
    sys.wait(5000)
    rtos.reboot()
end

-- fskv 初始化
if fskv.init() then
    local used, total, kv_count = fskv.status()
    log.info("fskv", "fskv init, status: used=" .. used .. "(bytes), total=" .. total .. "(bytes), kv_count=" .. kv_count)
end

-- 模块初始化
sys.taskInit(function()
    -- 等待网络环境准备就绪
    sys.waitUntil("IP_READY", 1000 * 60 * 5)
    local imei = mobile.imei()
    local number = mobile.number()
    local status = mobile.status()
    log.info("main", "device startup: ",
        string.format("Device startup, imei=%s, number=%s, status=%d", imei, number, status))

    -- init SMS handler
    sms_handler.init()
    -- init UART handler
    uart_handler.init()
    log.info("main", "SMS handler initialized")
    util.uart_send("", "SYSTEM_INIT", { imei = imei, number = number, status = status })
    -- sys.wait(60000)
    -- -- EC618配置小区重选信号差值门限，不能大于15dbm，必须在飞行模式下才能用
    -- mobile.flymode(0, true)
    -- mobile.config(mobile.CONF_RESELTOWEAKNCELL, 10)
    -- mobile.config(mobile.CONF_STATICCONFIG, 1) -- 开启网络静态优化
    -- mobile.flymode(0, false)
end)

-- Heart beat timer
if config.ENABLE_HEART_BEAT then
    sys.timerLoopStart(function()
        local imei = mobile.imei()
        local number = mobile.number()
        local status = mobile.status()
        log.info("main", "heart beat: ", string.format("imei=%s, number=%s, status=%d", imei, number, status))
        util.uart_send("", "HEART_BEAT", { imei = imei, number = number, status = status })
    end, config.HEART_BEAT_INTERVAL)
end

-- 定时开关飞行模式
if type(config.FLYMODE_INTERVAL) == "number" and config.FLYMODE_INTERVAL >= 1000 * 60 then
    sys.timerLoopStart(function()
        sys.taskInit(function()
            log.info("main", "change flymode.")
            mobile.reset()
            sys.wait(1000)
            mobile.flymode(0, true)
            mobile.flymode(0, false)
        end)
    end, config.FLYMODE_INTERVAL)
end

-- 用户代码已结束---------------------------------------------
sys.run()
-- sys.run()之后后面不要加任何语句!!!!!
