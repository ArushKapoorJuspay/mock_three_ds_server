use async_trait::async_trait;
use uuid::Uuid;
use serde::{Serialize, Deserialize};

use crate::config::Settings;
use crate::models::{AuthenticateRequest, ResultsRequest};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionData {
    pub authenticate_request: AuthenticateRequest,
    pub acs_trans_id: Uuid,
    pub ds_trans_id: Uuid,
    pub sdk_trans_id: Uuid,
    pub results_request: Option<ResultsRequest>,
}

#[derive(Debug, thiserror::Error)]
pub enum StateError {
    #[error("Transaction not found")]
    NotFound,
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Redis error: {0}")]
    Redis(#[from] redis::RedisError),
    #[error("Connection error: {0}")]
    Connection(String),
}

#[async_trait]
pub trait StateStore: Send + Sync {
    async fn insert(&self, key: Uuid, data: TransactionData) -> Result<(), StateError>;
    async fn get(&self, key: &Uuid) -> Result<Option<TransactionData>, StateError>;
    async fn update(&self, key: &Uuid, data: TransactionData) -> Result<(), StateError>;
    async fn delete(&self, key: &Uuid) -> Result<(), StateError>;
}

// Redis implementation (Redis-only state store)
pub struct RedisStore {
    client: redis::Client,
    ttl_seconds: u64,
    key_prefix: String,
}

impl RedisStore {
    pub async fn new(settings: &Settings) -> Result<Self, StateError> {
        let client = redis::Client::open(settings.redis.url.as_str())
            .map_err(|e| StateError::Connection(format!("Failed to create Redis client: {}", e)))?;
        
        // Test the connection
        let mut conn = client.get_async_connection().await
            .map_err(|e| StateError::Connection(format!("Failed to connect to Redis: {}", e)))?;
        
        // Simple ping test
        let _: String = redis::cmd("PING").query_async(&mut conn).await?;

        println!("✅ Redis connection established: {}", settings.redis.url);
        println!("📝 Transaction TTL: {} seconds", settings.redis.ttl_seconds);
        println!("🔑 Key prefix: {}", settings.redis.key_prefix);

        Ok(Self {
            client,
            ttl_seconds: settings.redis.ttl_seconds,
            key_prefix: settings.redis.key_prefix.clone(),
        })
    }

    fn make_key(&self, key: &Uuid) -> String {
        format!("{}:{}", self.key_prefix, key)
    }
}

#[async_trait]
impl StateStore for RedisStore {
    async fn insert(&self, key: Uuid, data: TransactionData) -> Result<(), StateError> {
        let mut conn = self.client.get_async_connection().await?;
        let redis_key = self.make_key(&key);
        let serialized_data = serde_json::to_string(&data)?;
        
        redis::cmd("SETEX")
            .arg(&redis_key)
            .arg(self.ttl_seconds)
            .arg(&serialized_data)
            .query_async(&mut conn)
            .await?;
        
        Ok(())
    }

    async fn get(&self, key: &Uuid) -> Result<Option<TransactionData>, StateError> {
        let mut conn = self.client.get_async_connection().await?;
        let redis_key = self.make_key(key);
        
        let result: Option<String> = redis::cmd("GET")
            .arg(&redis_key)
            .query_async(&mut conn)
            .await?;
        
        match result {
            Some(data_str) => {
                let data: TransactionData = serde_json::from_str(&data_str)?;
                Ok(Some(data))
            }
            None => Ok(None),
        }
    }

    async fn update(&self, key: &Uuid, data: TransactionData) -> Result<(), StateError> {
        let mut conn = self.client.get_async_connection().await?;
        let redis_key = self.make_key(key);
        
        // Check if key exists first
        let exists: bool = redis::cmd("EXISTS")
            .arg(&redis_key)
            .query_async(&mut conn)
            .await?;
        
        if !exists {
            return Err(StateError::NotFound);
        }
        
        let serialized_data = serde_json::to_string(&data)?;
        
        redis::cmd("SETEX")
            .arg(&redis_key)
            .arg(self.ttl_seconds)
            .arg(&serialized_data)
            .query_async(&mut conn)
            .await?;
        
        Ok(())
    }

    async fn delete(&self, key: &Uuid) -> Result<(), StateError> {
        let mut conn = self.client.get_async_connection().await?;
        let redis_key = self.make_key(key);
        
        redis::cmd("DEL")
            .arg(&redis_key)
            .query_async(&mut conn)
            .await?;
        
        Ok(())
    }
}

// Factory function to create Redis store from settings
pub async fn create_redis_store(settings: &Settings) -> Result<RedisStore, StateError> {
    RedisStore::new(settings).await
}
