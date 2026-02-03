use crate::serial_port::auto_detect_port;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};
use tokio_serial::SerialPortBuilderExt;

mod serial_port;

#[tokio::main]
async fn main() {
    let port_name = auto_detect_port().await;
    println!("Using serial port '{:?}'", port_name);
}
