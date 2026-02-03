use std::sync::Arc;

mod config;
mod connection;
mod database;
mod notification;
mod serial_port;

use config::Config;
use connection::SerialConnection;
use database::Database;
use notification::{BarkNotifier, Notifier};

#[tokio::main]
async fn main() {
    // Initialize logger
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();

    log::info!("=== Air780E UART Server Starting ===");

    // Load configuration
    let config = match Config::load("config.toml") {
        Ok(cfg) => {
            log::info!("Configuration loaded successfully");
            cfg
        }
        Err(e) => {
            eprintln!("Failed to load configuration: {}", e);
            eprintln!("Make sure config.toml exists in the current directory");
            std::process::exit(1);
        }
    };

    // Initialize database
    let db = match Database::new(&config.database.path) {
        Ok(database) => {
            log::info!("Database initialized: {}", config.database.path);
            database
        }
        Err(e) => {
            log::error!("Failed to initialize database: {}", e);
            std::process::exit(1);
        }
    };

    // Print database stats
    if let Ok(total) = db.count_total()
        && let Ok(unack) = db.count_unacknowledged()
    {
        log::info!(
            "Database stats - Total messages: {}, Unacknowledged: {}",
            total,
            unack
        );
    }

    // Initialize notifier
    let notifier: Arc<dyn Notifier> = if config.notification.enabled {
        log::info!("Bark notifications enabled");
        Arc::new(BarkNotifier::new(
            config.notification.bark_server_url.clone(),
            config.notification.bark_device_key.clone(),
        ))
    } else {
        log::warn!("Notifications disabled in config");
        Arc::new(BarkNotifier::new(String::new(), String::new()))
    };

    // Create connection manager
    let mut connection = SerialConnection::new(config.serial.clone(), db.clone(), notifier);

    log::info!("Starting serial connection loop...");
    log::info!(
        "Port: {}, Baud: {}",
        config.serial.port_name,
        config.serial.baud_rate
    );

    // Setup Ctrl+C handler
    let (tx, mut rx) = tokio::sync::mpsc::channel::<()>(1);
    tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to listen for Ctrl+C");
        log::info!("Received Ctrl+C signal, shutting down...");
        let _ = tx.send(()).await;
    });

    // Run connection loop with graceful shutdown
    tokio::select! {
        result = connection.maintain_loop() => {
            match result {
                Ok(_) => log::info!("Connection loop ended normally"),
                Err(e) => log::error!("Connection loop failed: {}", e),
            }
        }
        _ = rx.recv() => {
            log::info!("Shutdown signal received");
        }
    }

    log::info!("=== Air780E UART Server Stopped ===");
}
