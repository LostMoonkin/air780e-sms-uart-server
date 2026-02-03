# Air780E SMS Forwarding Server

[简体中文](README.md) | **English**

A Rust-based SMS reception server for Air780E module that receives SMS via serial port, stores them in SQLite database, and sends iOS push notifications via Bark.

## 📋 Overview

This project consists of two main components:

1. **LuatOS Scripts** (`script/` directory) - Running on Air780E module, listening for SMS and sending via serial port
2. **Rust Server** (`server/` directory) - Running on PC, receiving SMS data, storing and sending notifications

## ✨ Features

### Air780E Side (LuatOS)
- ✅ Automatic SMS reception
- ✅ UUID identifier for each message
- ✅ FSKV-based message queue
- ✅ Exponential backoff retry mechanism (5s base × 3^retry_count, max 5 retries)
- ✅ ACK confirmation mechanism
- ✅ Serial communication (115200 baud rate)
- ✅ Heartbeat detection
- ✅ Device information query

### Server Side (Rust)
- ✅ Automatic port detection (up to 10 retries)
- ✅ Connection state machine with auto-reconnection
- ✅ SQLite database storage
- ✅ Bark push notifications (iOS)
- ✅ Base64 + JSON message parsing
- ✅ Automatic ACK response
- ✅ Comprehensive error handling and logging
- ✅ Graceful shutdown (Ctrl+C)

## 🏗 System Architecture

```
┌─────────────┐                      ┌──────────────┐
│  Air780E    │  Serial (UART/USB)   │   PC Server  │
│  Module     │◄────────────────────►│   (Rust)     │
│             │                      │              │
│ - Recv SMS  │   Message Format:    │ - Parse Msg  │
│ - Queue Mgmt│   UUID:TYPE:BASE64   │ - Store in DB│
│ - Retry Logic│                     │ - Send Notify│
│ - Recv ACK  │◄─── ACK:UUID ────────│ - Send ACK   │
└─────────────┘                      └──────────────┘
                                            │
                                            ▼
                                     ┌──────────────┐
                                     │  SQLite DB   │
                                     │  + Bark Push │
                                     └──────────────┘
```

## 📦 System Requirements

### Air780E Side
- Air780E Development Board
- LuatOS Firmware
- USB Data Cable

### Server Side
- Windows/Linux/macOS
- Rust Toolchain (1.70+)
- SQLite (automatically included)
- **CH341 USB-to-Serial Driver** (Air780E uses CH341 chip)

> **⚠️ Important Notice**: Before first use, ensure CH341 driver is installed.
> - **Windows**: [CH341SER.EXE](http://www.wch.cn/downloads/CH341SER_EXE.html) - Download from WCH official website
> - **Linux**: Usually included in kernel, check with `lsmod | grep ch341`
> - **macOS**: [CH341SER_MAC.ZIP](http://www.wch.cn/downloads/CH341SER_MAC_ZIP.html)

## 🔧 Configuration

### 1. Server Configuration

Edit `server/config.toml`:

```toml
[serial]
port_name = "auto"          # Auto-detect port, or specify "COM3" (Windows) or "/dev/ttyUSB0" (Linux)
baud_rate = 115200          # Baud rate
timeout_ms = 1000           # Timeout in milliseconds
max_retry_count = 30        # Maximum retry count for port validation
retry_delay_ms = 10000      # Retry delay in milliseconds

[database]
path = "sms.db"             # SQLite database path

[notification]
bark_server_url = "https://api.day.app"
bark_device_key = "YOUR_BARK_DEVICE_KEY"  # Replace with your actual Bark key
enabled = true              # Enable notifications
```

### 2. Air780E Configuration

Edit `script/config.lua`:

```lua
return {
    MESSAGE_PROCESS_INTERVAL = 500,
    FLYMODE_INTERVAL = 1000 * 60 * 60 * 24,  -- 24 hours
    HEART_BEAT_INTERVAL = 1000 * 60 * 1,     -- 1 minute
    ENABLE_HEART_BEAT = true,
    SMS_FORWARD_ENABLED = true,

    -- SMS retry configuration
    SMS_RETRY_INTERVAL_BASE = 5000,          -- 5s base retry interval
    SMS_RETRY_BACKOFF_MULTIPLIER = 3,        -- 3x exponential backoff
    SMS_MAX_RETRY_COUNT = 5,                 -- Max 5 retries
    SMS_QUEUE_CHECK_INTERVAL = 5000,         -- Check queue every 5s
    SMS_MAX_QUEUE_SIZE = 100,                -- Max 100 pending messages
}
```

## 🚀 Usage

### Start Server

```bash
cd server
cargo build --release
cargo run --release
```

### Deploy LuatOS Scripts

**Method 1: Using Pre-built Firmware (Recommended)**

The `soc_release/` folder contains pre-built Air780E firmware that can be directly flashed using OpenLuat's LuaTools:

1. Download and install [LuaTools](https://luatos.com/luatools/download/last)
2. Connect Air780E using USB data cable
3. Open LuaTools, select the firmware file from `soc_release/` folder
4. Click "Download Core and Scripts" to flash
5. Wait for flashing to complete, restart module

**Method 2: Manual Script Upload**

1. Connect Air780E using LuaTools
2. Upload all `.lua` files from `script/` directory
3. Restart module

### Testing

1. Start server, you should see output like:
   ```
   === Air780E UART Server Starting ===
   Configuration loaded successfully
   Database initialized: sms.db
   Bark notifications enabled
   Auto-detecting port (attempt 1/10)
   Successfully detected port: COM3
   Port COM3 validated successfully
   Serial port opened successfully
   Sending GET_DEVICE_INFO command to device...
   Message handling loop started, waiting for data...
   ```

2. Send an SMS to the Air780E's SIM card number

3. Check server logs to confirm message received:
   ```
   Received 123 bytes: 'uuid:SMS_RECEIVED:base64data...'
   Successfully parsed message with ID: abc-123-def
   SMS received from +86xxx: Test message content
   ```

4. Check iOS device for Bark notification

5. Query database to confirm storage:
   ```bash
   sqlite3 sms.db "SELECT * FROM sms_messages;"
   ```

## 📡 Communication Protocol

### Message Format

All messages follow: `{uuid}:{message_type}:{base64_encoded_json}\r\n`

### Message Types

#### 1. Device Info (DEVICE_INFO)
```
{uuid}:DEVICE_INFO:{base64_json}
```
JSON content:
```json
{
    "imei": "Device IMEI",
    "number": "Phone number",
    "status": "Network status",
    "rssi": "Signal strength",
    "iccid": "SIM card number",
    "timestamp": "Timestamp"
}
```

#### 2. SMS Message (SMS_RECEIVED)
```
{uuid}:SMS_RECEIVED:{base64_json}
```
JSON content:
```json
{
    "id": "Message UUID",
    "sender": "Sender number",
    "content": "SMS content",
    "received_at": "Received timestamp",
    "metas": "Metadata"
}
```

#### 3. System Init (SYSTEM_INIT)
```
{uuid}:SYSTEM_INIT:{base64_json}
```

#### 4. Heartbeat (HEART_BEAT)
```
{uuid}:HEART_BEAT:{base64_json}
```

#### 5. Acknowledgment (ACK)
Server sends:
```
ACK:{uuid}\r\n
```

#### 6. Command (CMD)
Server queries device info:
```
CMD:GET_DEVICE_INFO\r\n
```

## 📊 Database Schema

### sms_messages Table

| Field | Type | Description |
|-------|------|-------------|
| id | TEXT PRIMARY KEY | Message UUID |
| sender | TEXT | Sender number |
| content | TEXT | SMS content |
| received_at | INTEGER | Device receive timestamp |
| metas | TEXT | JSON format metadata |
| acknowledged | INTEGER | Acknowledged flag (0/1) |
| ack_sent_at | INTEGER | ACK sent timestamp |
| created_at | INTEGER | Server receive timestamp |

## 🔍 Troubleshooting

### 1. Port Detection Failed

**Issue**: `Failed to auto-detect port after 10 attempts`

**Solutions**:
- Check USB connection
- Confirm CH341 driver is installed
- Manually specify port: Set `port_name = "COM3"` (Windows) or `"/dev/ttyUSB0"` (Linux) in `config.toml`
- Check if other programs are using the port (like LuaTools, serial debugger, etc.)

### 2. Port Validation Failed

**Issue**: `Port validation failed`

**Solutions**:
- Confirm LuatOS scripts are running correctly on Air780E
- Check if serial baud rate matches (115200)
- Increase retry interval: `retry_delay_ms = 10000`

### 3. No Messages Received

**Issue**: `No data received in last 30 seconds`

**Solutions**:
- Check Air780E logs to confirm SMS was received
- Confirm `SMS_FORWARD_ENABLED = true` in `config.lua`
- Test by sending `CMD:GET_DEVICE_INFO` command to check if communication is normal
- Check USB cable quality

### 4. Bark Notification Failed

**Issue**: `Failed to send notification`

**Solutions**:
- Verify Bark key is correct
- Check network connection
- Manually test Bark API:
  ```bash
  curl "https://api.day.app/YOUR_KEY/Test_Title/Test_Content"
  ```
- Temporarily disable notifications to continue testing: `enabled = false`

### 5. Database Error

**Issue**: `Failed to insert SMS into database`

**Solutions**:
- Check disk space
- Confirm database file permissions
- Delete and rebuild database: `rm sms.db` then restart server

## 📝 Development Guide

### Project Structure

```
air780e-sms-uart-server/
├── script/                    # LuatOS scripts (Air780E side)
│   ├── main.lua              # Main entry
│   ├── config.lua            # Configuration
│   ├── sms_handler.lua       # SMS handling
│   ├── uart_handler.lua      # UART handling
│   └── util.lua              # Utility functions
├── server/                    # Rust server
│   ├── src/
│   │   ├── main.rs           # Main program
│   │   ├── config.rs         # Configuration management
│   │   ├── database.rs       # Database operations
│   │   ├── notification.rs   # Notification service
│   │   ├── serial_port.rs    # Serial communication and message parsing
│   │   └── connection.rs     # Connection state machine
│   ├── Cargo.toml            # Dependencies configuration
│   └── config.toml           # Runtime configuration
└── README.md                  # This document
```

### Build

```bash
cd server
cargo build --release
```

Generated executable: `server/target/release/air780e-uart-server.exe` (Windows)

### Run Tests

```bash
cargo test
```

### Debug Mode

Enable verbose logging:
```bash
RUST_LOG=debug cargo run
```

### Adding New Message Types

1. Add new enum value to `MessageType` in `server/src/serial_port.rs`
2. Add parsing logic in `parse_message()`
3. Add handling logic in `process_message()` in `server/src/connection.rs`
4. Send new format message from LuatOS side accordingly

## 🚀 Continuous Integration/Deployment (CI/CD)

This project is configured with GitHub Actions for automated builds:

### Automated Build Platforms

On every push or tag creation, binaries are automatically built for:

| Platform | Architecture | Artifact Name |
|----------|--------------|---------------|
| Windows | x64 | `air780e-uart-server-windows-x64.exe` |
| Linux | x64 | `air780e-uart-server-linux-x64` |
| Linux | ARM64 | `air780e-uart-server-linux-arm64` |
| macOS | Intel (x64) | `air780e-uart-server-macos-x64` |
| macOS | Apple Silicon (ARM64) | `air780e-uart-server-macos-arm64` |

### Automated Testing

Every commit runs:
- ✅ Unit tests (`cargo test`)
- ✅ Format checking (`cargo fmt`)
- ✅ Code quality checks (`cargo clippy`)

### Release Process

1. Create a version tag:
   ```bash
   git tag -a v1.0.0 -m "Release version 1.0.0"
   git push origin v1.0.0
   ```

2. GitHub Actions will automatically:
   - Build binaries for all platforms
   - Create a GitHub Release
   - Upload all platform artifacts

3. Users can download pre-built binaries for their platform from the [Releases page](../../releases)

### Workflow Files

- `.github/workflows/build.yml` - Multi-platform build and release
- `.github/workflows/test.yml` - Automated testing and code checks

## 🤝 Contributing

Issues and Pull Requests are welcome!

## 📄 License

This project is licensed under the MIT License. See LICENSE file for details.

## 🙏 Acknowledgments

- [LuatOS](https://github.com/openLuat/LuatOS) - Air780E development framework
- [tokio](https://tokio.rs/) - Rust async runtime
- [tokio-serial](https://github.com/berkowski/tokio-serial) - Async serial library
- [Bark](https://github.com/Finb/Bark) - iOS push notification service

## 📞 Contact

For questions or suggestions, please submit an Issue.

---

**Note**: Before first use, make sure to modify the Bark device key in `server/config.toml`!
