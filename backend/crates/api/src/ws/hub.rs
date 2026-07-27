use futures::StreamExt;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{broadcast, RwLock};

const CHANNEL_CAPACITY: usize = 512;

type Channels = Arc<RwLock<HashMap<String, broadcast::Sender<String>>>>;

/// In-memory WebSocket hub backed by Redis pub/sub.
/// Redis publishes to a channel → Hub broadcasts to subscribed WebSocket connections.
pub struct WsHub {
    redis_url: String,
    channels:  Channels,
}

impl WsHub {
    pub fn new(redis_url: String) -> Self {
        Self {
            redis_url,
            channels: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Subscribe to a channel. Returns a receiver for that channel.
    pub async fn subscribe(&self, channel: &str) -> broadcast::Receiver<String> {
        {
            let channels = self.channels.read().await;
            if let Some(tx) = channels.get(channel) {
                return tx.subscribe();
            }
        }

        let (tx, rx) = broadcast::channel(CHANNEL_CAPACITY);
        self.channels.write().await.insert(channel.to_string(), tx);
        rx
    }

    /// Publish a message to a channel (used internally).
    #[allow(dead_code)]
    pub async fn publish(&self, channel: &str, message: String) {
        let channels = self.channels.read().await;
        if let Some(tx) = channels.get(channel) {
            let _ = tx.send(message);
        }
    }

    /// Start a background task that subscribes to Redis and fans out to in-memory channels.
    pub async fn start_redis_listener(&self) -> anyhow::Result<()> {
        let url      = self.redis_url.clone();
        let channels = Arc::clone(&self.channels);

        tokio::spawn(async move {
            loop {
                if let Err(e) = pubsub_loop(url.clone(), Arc::clone(&channels)).await {
                    tracing::error!("Redis pub/sub error: {}", e);
                }
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            }
        });

        Ok(())
    }
}

async fn pubsub_loop(url: String, channels: Channels) -> anyhow::Result<()> {
    let client = redis::Client::open(url.as_str())?;
    let mut pubsub = client.get_async_pubsub().await?;

    // Subscribe to all per-project event channels + board
    pubsub.psubscribe("project:*:events").await?;
    pubsub.subscribe("board:events").await?;

    let mut msg_stream = pubsub.into_on_message();
    while let Some(msg) = msg_stream.next().await {
        let channel: String = msg.get_channel_name().to_string();
        if let Ok(payload) = msg.get_payload::<String>() {
            let ch = channels.read().await;
            if let Some(tx) = ch.get(&channel) {
                let _ = tx.send(payload);
            }
        }
    }

    Ok(())
}
