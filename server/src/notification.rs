use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait Notifier: Send + Sync {
    async fn send(&self, title: &str, content: &str) -> Result<()>;
}

pub struct BarkNotifier {
    server_url: String,
    device_key: String,
    client: reqwest::Client,
}

impl BarkNotifier {
    pub fn new(server_url: String, device_key: String) -> Self {
        BarkNotifier {
            server_url,
            device_key,
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl Notifier for BarkNotifier {
    async fn send(&self, title: &str, content: &str) -> Result<()> {
        let url = format!(
            "{}/{}/{}/{}",
            self.server_url.trim_end_matches('/'),
            self.device_key,
            urlencoding::encode(title),
            urlencoding::encode(content)
        );

        log::debug!("Sending Bark notification to: {}", url);

        match self.client.get(&url).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    log::info!("Bark notification sent successfully");
                    Ok(())
                } else {
                    let status = response.status();
                    let body = response.text().await.unwrap_or_default();
                    log::warn!("Bark notification failed with status {}: {}", status, body);
                    anyhow::bail!("Bark notification failed with status: {}", status)
                }
            }
            Err(e) => {
                log::warn!("Failed to send Bark notification: {}", e);
                Err(e.into())
            }
        }
    }
}
