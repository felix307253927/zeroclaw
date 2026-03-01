/*
 * @Author             : Felix
 * @Email              : 307253927@qq.com
 * @Date               : 2026-02-28 18:18:27
 * @LastEditors        : Felix
 * @LastEditTime       : 2026-02-28 18:42:42
 */
use super::traits::{Channel, ChannelMessage, SendMessage};
use crate::config::traits::ChannelConfig;
use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tokio::io::{self, AsyncBufReadExt, BufReader};
use uuid::Uuid;

/// CLI channel — stdin/stdout, always available, zero deps
pub struct UnibiChannel {
    base_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UnibiConfig {
    base_url: String,
}

impl UnibiChannel {
    pub fn new(config: UnibiConfig) -> Self {
        Self {
            base_url: config.base_url,
        }
    }
}

impl ChannelConfig for UnibiConfig {
    fn name() -> &'static str {
        "Unibi"
    }
    fn desc() -> &'static str {
        "Unibi Channel"
    }
}

#[async_trait]
impl Channel for UnibiChannel {
    fn name(&self) -> &str {
        "unibi"
    }

    async fn send(&self, message: &SendMessage) -> anyhow::Result<()> {
        println!("{}", message.content);
        Ok(())
    }

    async fn listen(&self, tx: tokio::sync::mpsc::Sender<ChannelMessage>) -> anyhow::Result<()> {
        let stdin = io::stdin();
        let reader = BufReader::new(stdin);
        let mut lines = reader.lines();

        while let Ok(Some(line)) = lines.next_line().await {
            let line = line.trim().to_string();
            if line.is_empty() {
                continue;
            }

            let msg = ChannelMessage {
                id: Uuid::new_v4().to_string(),
                sender: "user".to_string(),
                reply_target: "user".to_string(),
                content: line,
                channel: "cli".to_string(),
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
                thread_ts: None,
            };

            if tx.send(msg).await.is_err() {
                break;
            }
        }
        Ok(())
    }
}
