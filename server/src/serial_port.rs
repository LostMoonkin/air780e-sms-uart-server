use regex::Regex;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::task::JoinSet;
use tokio_serial::SerialPortBuilderExt;

const TIMEOUT_MS: u64 = 1000;
// The handshake command
const INIT_CMD: &[u8] = b"CMD:GET_DEVICE_INFO\r\n";
const DEFAULT_BAUD_RATE: u32 = 115_200;
// Auto-detection retry settings
const AUTO_DETECT_MAX_RETRIES: u32 = 10;
const AUTO_DETECT_RETRY_DELAY_MS: u64 = 10000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmsPayload {
    pub id: String,
    pub sender: String,
    pub content: String,
    pub received_at: i64,
    pub metas: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfoPayload {
    pub imei: String,
    pub number: String,
    pub status: i32,
    pub rssi: i32,
    pub iccid: String,
    pub timestamp: i64,
}

#[derive(Debug, Clone)]
pub enum MessageType {
    DeviceInfo(DeviceInfoPayload),
    SmsReceived(SmsPayload),
    SystemInit(serde_json::Value),
    HeartBeat(serde_json::Value),
    Unknown(String),
}

#[derive(Debug, Clone)]
pub struct ParsedMessage {
    pub id: String,
    pub message_type: MessageType,
}

pub fn parse_message(line: &str) -> Option<ParsedMessage> {
    // Parse format: {uuid}:{type}:{base64}\r\n
    let re = Regex::new(r"^(.+?):(.+?):(.+?)[\r\n]*$").ok()?;
    let captures = re.captures(line)?;

    let id = captures.get(1)?.as_str().to_string();
    let msg_type = captures.get(2)?.as_str();
    let base64_data = captures.get(3)?.as_str();

    // Decode base64
    use base64::{engine::general_purpose, Engine as _};
    let decoded = general_purpose::STANDARD.decode(base64_data).ok()?;
    let json_str = String::from_utf8(decoded).ok()?;

    log::debug!(
        "Parsed message - ID: {}, Type: {}, JSON: {}",
        id,
        msg_type,
        json_str
    );

    let message_type = match msg_type {
        "DEVICE_INFO" => {
            let payload: DeviceInfoPayload = serde_json::from_str(&json_str).ok()?;
            MessageType::DeviceInfo(payload)
        }
        "SMS_RECEIVED" => {
            let payload: SmsPayload = serde_json::from_str(&json_str).ok()?;
            MessageType::SmsReceived(payload)
        }
        "SYSTEM_INIT" => {
            let payload: serde_json::Value = serde_json::from_str(&json_str).ok()?;
            MessageType::SystemInit(payload)
        }
        "HEART_BEAT" => {
            let payload: serde_json::Value = serde_json::from_str(&json_str).ok()?;
            MessageType::HeartBeat(payload)
        }
        _ => MessageType::Unknown(msg_type.to_string()),
    };

    Some(ParsedMessage { id, message_type })
}

pub async fn send_ack<W: AsyncWriteExt + Unpin>(writer: &mut W, uuid: &str) -> std::io::Result<()> {
    let ack_msg = format!("ACK:{}\r\n", uuid);
    writer.write_all(ack_msg.as_bytes()).await?;
    writer.flush().await?;
    log::info!("Sent ACK for message: {}", uuid);
    Ok(())
}

pub async fn auto_detect_port(baud_rate: u32) -> Option<String> {
    for attempt in 1..=AUTO_DETECT_MAX_RETRIES {
        log::info!(
            "Auto-detecting port (attempt {}/{})",
            attempt,
            AUTO_DETECT_MAX_RETRIES
        );

        let ports = tokio_serial::available_ports();
        if ports.is_err() {
            log::error!("Failed to list available ports, err={}", ports.unwrap_err());
            if attempt < AUTO_DETECT_MAX_RETRIES {
                log::info!("Retrying in {}ms...", AUTO_DETECT_RETRY_DELAY_MS);
                tokio::time::sleep(Duration::from_millis(AUTO_DETECT_RETRY_DELAY_MS)).await;
                continue;
            }
            return None;
        }

        let ports = ports.unwrap();
        log::info!("Scanning {} ports...", ports.len());
        let mut check_tasks = JoinSet::new();
        for port in ports {
            let port_name = port.port_name.clone();
            check_tasks.spawn(async move { check_port(&port_name, baud_rate).await });
        }

        let mut results = Vec::new();
        while let Some(res) = check_tasks.join_next().await {
            if let Ok(port) = res {
                if port.is_some() {
                    results.push(port.unwrap());
                }
            }
        }

        log::info!("--------------------------------------------------");
        log::info!(
            "Scan Complete. Found {} valid device(s), name=[{}]",
            results.len(),
            results.join(", ")
        );

        if !results.is_empty() {
            log::info!("Successfully detected port: {}", results[0]);
            return Some(results[0].clone());
        }

        if attempt < AUTO_DETECT_MAX_RETRIES {
            log::warn!(
                "No valid device found, retrying in {}ms...",
                AUTO_DETECT_RETRY_DELAY_MS
            );
            tokio::time::sleep(Duration::from_millis(AUTO_DETECT_RETRY_DELAY_MS)).await;
        } else {
            log::error!(
                "Failed to auto-detect port after {} attempts",
                AUTO_DETECT_MAX_RETRIES
            );
        }
    }

    None
}

pub async fn check_port(port_name: &str, baud_rate: u32) -> Option<String> {
    // Compile regex to match: {id}:DEVICE_INFO:{base64}\r\n
    // Explanation:
    // ^          Start of line
    // (.+)       Group 1: The ID (any character except :)
    // :DEVICE_INFO: Literal string
    // ([a-zA-Z0-9+/=]+) Group 2: Base64 characters
    // \s*$       End of line (allowing for \r\n)
    let re = Regex::new(r"^(.+):DEVICE_INFO:([a-zA-Z0-9+/=]+)\s*$").ok()?;

    // Attempt to open the port
    let mut port = tokio_serial::new(port_name, baud_rate)
        .timeout(Duration::from_millis(TIMEOUT_MS))
        .open_native_async()
        .ok()?;
    let write_result =
        tokio::time::timeout(Duration::from_millis(TIMEOUT_MS), port.write_all(INIT_CMD)).await;
    if write_result.is_err() || write_result.unwrap().is_err() {
        return None;
    }

    let mut reader = BufReader::new(port);
    let mut response = String::new();
    let read_result = tokio::time::timeout(
        Duration::from_millis(TIMEOUT_MS),
        reader.read_line(&mut response),
    )
    .await;
    match read_result {
        Ok(Ok(bytes_read)) if bytes_read > 0 => {
            // Check against Regex
            if let Some(_captures) = re.captures(&response) {
                return Some(port_name.to_string());
            }
        }
        _ => {
            // Timeout, Empty read, or Error
            return None;
        }
    }
    None
}
