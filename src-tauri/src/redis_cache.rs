use redis::{AsyncCommands, Client};
use serde::{de::DeserializeOwned, Serialize};
use std::time::Duration;

pub struct RedisCache {
    client: Option<Client>,
}

impl RedisCache {
    pub fn new() -> Self {
        // Try to connect to Redis on the standard port
        let client = Client::open("redis://127.0.0.1:6379").ok();
        RedisCache { client }
    }

    pub async fn get<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        let client = self.client.as_ref()?;
        let mut conn = client.get_multiplexed_tokio_connection().await.ok()?;
        
        let val: String = conn.get(key).await.ok()?;
        serde_json::from_str(&val).ok()
    }

    pub async fn set<T: Serialize>(&self, key: &str, value: &T, ttl_secs: u64) {
        if let Some(client) = &self.client {
            if let Ok(mut conn) = client.get_multiplexed_tokio_connection().await {
                if let Ok(val) = serde_json::to_string(value) {
                    let _: Result<(), _> = conn.set_ex(key, val, ttl_secs).await;
                }
            }
        }
    }
}
