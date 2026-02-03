use regex::Regex;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::task::JoinSet;
use tokio_serial::SerialPortBuilderExt;

const TIMEOUT_MS: u64 = 1000;
// The handshake command
const INIT_CMD: &[u8] = b"CMD:GET_DEVICE_INFO\r\n";
const DEFAULT_BAUD_RATE: u32 = 115_200;

pub async fn auto_detect_port() -> Option<String> {
    let ports = tokio_serial::available_ports();
    if ports.is_err() {
        println!("Failed to list available ports, err={}", ports.unwrap_err());
        return None;
    }
    let ports = ports.unwrap();
    println!("Scanning {} ports...", ports.len());
    let mut check_tasks = JoinSet::new();
    for port in ports {
        let port_name = port.port_name.clone();
        check_tasks.spawn(async move { check_port(&port_name, DEFAULT_BAUD_RATE).await });
    }
    let mut results = Vec::new();
    while let Some(res) = check_tasks.join_next().await {
        if let Ok(port) = res {
            if port.is_some() {
                results.push(port.unwrap());
            }
        }
    }
    println!("--------------------------------------------------");
    println!(
        "Scan Complete. Found {} valid device(s), name=[{}]",
        results.len(),
        results.join(", ")
    );
    if results.len() > 0 {
        return Some(results[0].clone());
    }
    None
}

async fn check_port(port_name: &str, baud_rate: u32) -> Option<String> {
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
            if let Some(captures) = re.captures(&response) {
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
