use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;
use serde::{Serialize, Deserialize};

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

// In-memory implementation (existing pattern)
pub struct InMemoryStore {
    data: Arc<Mutex<HashMap<Uuid, TransactionData>>>,
}

impl InMemoryStore {
    pub fn new() -> Self {
        Self {
            data: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl StateStore for InMemoryStore {
    async fn insert(&self, key: Uuid, data: TransactionData) -> Result<(), StateError> {
        let mut guard = self.data.lock().unwrap();
        guard.insert(key, data);
        Ok(())
    }

    async fn get(&self, key: &Uuid) -> Result<Option<TransactionData>, StateError> {
        let guard = self.data.lock().unwrap();
        Ok(guard.get(key).cloned())
    }

    async fn update(&self, key: &Uuid, data: TransactionData) -> Result<(), StateError> {
        let mut guard = self.data.lock().unwrap();
        if guard.contains_key(key) {
            guard.insert(*key, data);
            Ok(())
        } else {
            Err(StateError::NotFound)
        }
    }

    async fn delete(&self, key: &Uuid) -> Result<(), StateError> {
        let mut guard = self.data.lock().unwrap();
        guard.remove(key);
        Ok(())
    }
}

// Redis implementation
pub struct RedisStore {
    client: redis::Client,
    ttl_seconds: u64,
}

impl RedisStore {
    pub async fn new(redis_url: &str, ttl_seconds: u64) -> Result<Self, StateError> {
        let client = redis::Client::open(redis_url)
            .map_err(|e| StateError::Connection(format!("Failed to create Redis client: {}", e)))?;
        
        // Test the connection
        let mut conn = client.get_async_connection().await
            .map_err(|e| StateError::Connection(format!("Failed to connect to Redis: {}", e)))?;
        
        // Simple ping test
        let _: String = redis::cmd("PING").query_async(&mut conn).await?;

        Ok(Self {
            client,
            ttl_seconds,
        })
    }

    fn make_key(&self, key: &Uuid) -> String {
        format!("3ds_transaction:{}", key)
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

// Factory function to create the appropriate store based on configuration
pub async fn create_state_store() -> Result<Box<dyn StateStore>, StateError> {
    let use_redis = std::env::var("USE_REDIS")
        .unwrap_or_else(|_| "false".to_string())
        .parse::<bool>()
        .unwrap_or(false);

    if use_redis {
        let redis_url = std::env::var("REDIS_URL")
            .unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
        
        let ttl_seconds = std::env::var("TRANSACTION_TTL_SECONDS")
            .unwrap_or_else(|_| "1800".to_string()) // 30 minutes default
            .parse::<u64>()
            .unwrap_or(1800);

        println!("Initializing Redis state store at {}", redis_url);
        let redis_store = RedisStore::new(&redis_url, ttl_seconds).await?;
        Ok(Box::new(redis_store))
    } else {
        println!("Initializing in-memory state store");
        Ok(Box::new(InMemoryStore::new()))
    }
}
