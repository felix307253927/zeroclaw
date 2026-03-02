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
use futures_util::{stream::SplitSink, SinkExt};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, Mutex};
use tokio_tungstenite::{accept_async, tungstenite::Message, WebSocketStream};
use uuid::Uuid;

type TxType = mpsc::Sender<ChannelMessage>;
// type WsSink = tokio::sync::mpsc::Sender<Result<Message, tokio_tungstenite::tungstenite::Error>>;
type WsSink = SplitSink<WebSocketStream<TcpStream>, Message>;

pub struct UnibiChannel {
    port: u16,
    clients: Arc<Mutex<HashMap<String, WsSink>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UnibiConfig {
    pub port: u16,
}

impl UnibiChannel {
    pub fn new(config: UnibiConfig) -> Self {
        Self {
            port: config.port,
            clients: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl ChannelConfig for UnibiConfig {
    fn name() -> &'static str {
        "Unibi"
    }
    fn desc() -> &'static str {
        "Unibi Channel, a simple websocket channel"
    }
}

#[async_trait]
impl Channel for UnibiChannel {
    fn name(&self) -> &str {
        "unibi"
    }

    async fn send(&self, message: &SendMessage) -> anyhow::Result<()> {
        let payload = serde_json::json!({
            "type": "chunk",
            "content": message.content
        });
        let msg = Message::Text(payload.to_string().into());

        let mut clients = self.clients.lock().await;
        tracing::debug!("Unibi: send message: {:?}, clients: {}", message, clients.len());
        for sink in clients.values_mut() {
            if let Err(err) = sink.send(msg.clone()).await {
                tracing::error!("Unibi: send message error: {}", err);
            }
        }
        Ok(())
    }

    async fn listen(&self, tx: mpsc::Sender<ChannelMessage>) -> anyhow::Result<()> {
        let addr = format!("127.0.0.1:{}", self.port);
        let listener = TcpListener::bind(&addr).await?;
        tracing::info!("Unibi WebSocket server listening on {}", addr);

        let clients = self.clients.clone();

        loop {
            tokio::select! {
                result = listener.accept() => {
                    match result {
                        Ok((stream, peer_addr)) => {
                            tracing::info!("Unibi: new connection from {}", peer_addr);
                            let clients = clients.clone();
                            let tx_clone = tx.clone();
                            tokio::spawn(async move {
                                use futures_util::StreamExt;
                                use tokio_tungstenite::WebSocketStream;

                                let ws_stream: WebSocketStream<TcpStream> = match accept_async(stream).await
                                {
                                    Ok(ws) => ws,
                                    Err(e) => {
                                        tracing::error!("Unibi: websocket accept error: {}", e);
                                        return;
                                    }
                                };
                                let client_id = Uuid::new_v4().to_string();
                                let (write, mut read) = ws_stream.split();
                                {
                                    let mut clients = clients.lock().await;
                                    clients.insert(client_id.clone(), write);
                                    tracing::debug!("Unibi: add client: {}， len:{}", client_id, clients.len());
                                }

                                while let Some(msg) = read.next().await {
                                    let msg = match msg {
                                        Ok(Message::Text(text)) => text,
                                        Ok(Message::Close(_)) => break,
                                        Err(_) => break,
                                        _ => continue,
                                    };

                                    let line = msg.trim().to_string();
                                    if line.is_empty() {
                                        continue;
                                    }
                                    tracing::debug!("Unibi: received message: {}", line);

                                    let channel_msg = ChannelMessage {
                                        id: Uuid::new_v4().to_string(),
                                        sender: "user".to_string(),
                                        reply_target: "user".to_string(),
                                        content: line,
                                        channel: "unibi".to_string(),
                                        timestamp: std::time::SystemTime::now()
                                            .duration_since(std::time::UNIX_EPOCH)
                                            .unwrap_or_default()
                                            .as_secs(),
                                        thread_ts: None,
                                    };

                                    if tx_clone.send(channel_msg).await.is_err() {
                                        break;
                                    }
                                }

                                {
                                    let mut clients = clients.lock().await;
                                    tracing::debug!("Unibi: remove client: {}", client_id);
                                    clients.remove(&client_id);
                                }
                            });
                        }
                        Err(e) => {
                            tracing::error!("Unibi: accept error: {}", e);
                        }
                    }
                }
            }
        }
    }
}
