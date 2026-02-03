use crate::config::SerialConfig;
use crate::database::{Database, SmsMessage};
use crate::notification::Notifier;
use crate::serial_port::{self, MessageType, ParsedMessage};
use anyhow::{Context, Result};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio_serial::{SerialPortBuilderExt, SerialStream};

#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionState {
    Initializing,
    Validating,
    Connected,
    Reconnecting { attempts: u32 },
    Failed,
}

pub struct SerialConnection {
    config: SerialConfig,
    state: ConnectionState,
    db: Database,
    notifier: Arc<dyn Notifier>,
}

impl SerialConnection {
    pub fn new(config: SerialConfig, db: Database, notifier: Arc<dyn Notifier>) -> Self {
        SerialConnection {
            config,
            state: ConnectionState::Initializing,
            db,
            notifier,
        }
    }

    pub async fn establish(&mut self) -> Result<String> {
        log::info!("Establishing serial connection...");
        self.state = ConnectionState::Initializing;

        // Determine port name
        let port_name = if self.config.port_name.to_lowercase() == "auto" {
            log::info!("Auto-detecting serial port...");
            match serial_port::auto_detect_port(self.config.baud_rate).await {
                Some(port) => {
                    log::info!("Auto-detected port: {}", port);

                    // Add delay after auto-detection to ensure port is fully released
                    // Auto-detection validates the port, so we need time before re-validating
                    log::debug!("Waiting for port to be released after auto-detection...");
                    tokio::time::sleep(Duration::from_millis(1000)).await;

                    port
                }
                None => {
                    anyhow::bail!("Failed to auto-detect serial port. No valid device found.");
                }
            }
        } else {
            log::info!("Using configured port: {}", self.config.port_name);
            self.config.port_name.clone()
        };

        // Validate port with retry
        for attempt in 1..=self.config.max_retry_count {
            log::info!(
                "Validating port {} (attempt {}/{})",
                port_name,
                attempt,
                self.config.max_retry_count
            );
            self.state = ConnectionState::Validating;

            match serial_port::check_port(&port_name, self.config.baud_rate).await {
                Some(_) => {
                    log::info!("Port {} validated successfully", port_name);

                    // Add small delay to ensure port is fully released after validation
                    tokio::time::sleep(Duration::from_millis(500)).await;

                    self.state = ConnectionState::Connected;
                    return Ok(port_name);
                }
                None => {
                    log::warn!(
                        "Port validation failed (attempt {}/{})",
                        attempt,
                        self.config.max_retry_count
                    );
                    if attempt < self.config.max_retry_count {
                        log::info!("Retrying in {}ms...", self.config.retry_delay_ms);
                        tokio::time::sleep(Duration::from_millis(self.config.retry_delay_ms)).await;
                    }
                }
            }
        }

        self.state = ConnectionState::Failed;
        anyhow::bail!(
            "Failed to validate port after {} attempts",
            self.config.max_retry_count
        )
    }

    pub async fn maintain_loop(&mut self) -> Result<()> {
        loop {
            // Establish connection
            let port_name = match self.establish().await {
                Ok(name) => name,
                Err(e) => {
                    log::error!("Failed to establish connection: {}", e);
                    return Err(e);
                }
            };

            // Open serial port
            log::info!("Opening serial port: {}", port_name);
            let port_result = tokio_serial::new(&port_name, self.config.baud_rate)
                .timeout(Duration::from_millis(self.config.timeout_ms))
                .open_native_async();

            let port = match port_result {
                Ok(p) => p,
                Err(e) => {
                    log::error!("Failed to open serial port '{}': {}", port_name, e);
                    log::error!("Error details: {:?}", e);
                    anyhow::bail!("Failed to open serial port '{}': {}", port_name, e);
                }
            };

            log::info!("Serial port opened successfully, entering message loop");

            // Start message handling loop
            if let Err(e) = self.handle_messages(port).await {
                log::error!("Message handling error: {}", e);
                self.state = ConnectionState::Reconnecting { attempts: 0 };

                // Reconnect logic
                log::warn!("Connection lost, attempting to reconnect...");
                tokio::time::sleep(Duration::from_millis(self.config.retry_delay_ms)).await;
                continue;
            }
        }
    }

    async fn handle_messages(&mut self, port: SerialStream) -> Result<()> {
        let (reader, mut writer) = tokio::io::split(port);
        let mut reader = BufReader::new(reader);
        let mut line = String::new();

        // Send initial GET_DEVICE_INFO command to verify connection
        log::info!("Sending GET_DEVICE_INFO command to device...");
        if let Err(e) = writer.write_all(b"CMD:GET_DEVICE_INFO\r\n").await {
            log::error!("Failed to send GET_DEVICE_INFO command: {}", e);
        } else {
            log::info!("GET_DEVICE_INFO command sent successfully");
        }

        log::info!("Message handling loop started, waiting for data...");

        loop {
            line.clear();

            // Use timeout to detect if we're stuck waiting
            let read_result =
                tokio::time::timeout(Duration::from_secs(30), reader.read_line(&mut line)).await;

            match read_result {
                Ok(Ok(0)) => {
                    log::warn!("Connection closed (EOF)");
                    anyhow::bail!("Connection closed");
                }
                Ok(Ok(bytes_read)) => {
                    log::info!("Received {} bytes: '{}'", bytes_read, line.trim());
                    log::debug!("Raw bytes: {:?}", line.as_bytes());

                    // Parse message
                    match serial_port::parse_message(&line) {
                        Some(msg) => {
                            log::info!("Successfully parsed message with ID: {}", msg.id);
                            if let Err(e) = self.process_message(msg, &mut writer).await {
                                log::error!("Failed to process message: {}", e);
                                // Continue processing other messages
                            }
                        }
                        None => {
                            log::warn!("Failed to parse message: '{}'", line.trim());
                            log::warn!("Raw bytes: {:?}", line.as_bytes());
                        }
                    }
                }
                Ok(Err(e)) => {
                    log::error!("Read error: {}", e);
                    anyhow::bail!("Read error: {}", e);
                }
                Err(_) => {
                    // Timeout - no data received
                    log::info!("No data received in last 30 seconds, still waiting...");
                    // Continue waiting
                }
            }
        }
    }

    async fn process_message<W: tokio::io::AsyncWriteExt + Unpin>(
        &self,
        msg: ParsedMessage,
        writer: &mut W,
    ) -> Result<()> {
        match msg.message_type {
            MessageType::SmsReceived(payload) => {
                log::info!("SMS received from {}: {}", payload.sender, payload.content);

                // Store in database
                let sms_msg = SmsMessage {
                    id: payload.id.clone(),
                    sender: payload.sender.clone(),
                    content: payload.content.clone(),
                    received_at: payload.received_at,
                    metas: serde_json::to_string(&payload.metas).unwrap_or_default(),
                };

                self.db
                    .insert_sms(&sms_msg)
                    .context("Failed to insert SMS into database")?;

                // Send notification
                let title = format!("SMS from {}", payload.sender);
                let content = &payload.content;

                if let Err(e) = self.notifier.send(&title, content).await {
                    log::warn!("Failed to send notification: {}", e);
                    // Don't fail the whole process if notification fails
                }

                // Send acknowledgment
                serial_port::send_ack(writer, &msg.id)
                    .await
                    .context("Failed to send ACK")?;

                // Mark as acknowledged in database
                self.db
                    .mark_acknowledged(&msg.id)
                    .context("Failed to mark message as acknowledged")?;
            }
            MessageType::DeviceInfo(info) => {
                log::info!(
                    "Device info - IMEI: {}, Number: {}, Status: {}",
                    info.imei,
                    info.number,
                    info.status
                );
            }
            MessageType::SystemInit(data) => {
                log::info!("System init: {:?}", data);
            }
            MessageType::HeartBeat(data) => {
                log::debug!("Heartbeat: {:?}", data);
            }
            MessageType::Unknown(type_name) => {
                log::warn!("Unknown message type: {}", type_name);
            }
        }

        Ok(())
    }

    pub fn get_state(&self) -> &ConnectionState {
        &self.state
    }
}
