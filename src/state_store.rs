use async_trait::async_trait;
use uuid::Uuid;
use serde::{Serialize, Deserialize};
use deadpool_redis::{Config, Pool, Runtime};
use std::time::Duration;

use crate::config::Settings;
use crate::models::{AuthenticateRequest, ResultsRequest};
use crate::crypto::EphemeralKeyPair;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionData {
    pub authenticate_request: AuthenticateRequest,
    pub acs_trans_id: Uuid,
    pub ds_trans_id: Uuid,
    pub sdk_trans_id: Option<Uuid>,
    pub results_request: Option<ResultsRequest>,
    pub ephemeral_keys: Option<EphemeralKeyPair>,
    pub redirect_url: Option<String>,
    pub sdk_ephemeral_public_key: Option<String>, // SDK's public key for ECDH shared secret derivation
}

#[derive(Debug, thiserror::Error)]
pub enum StateError {
    #[error("Transaction not found")]
    NotFound,
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Redis error: {0}")]
    Redis(#[from] deadpool_redis::redis::RedisError),
    #[error("Pool error: {0}")]
    Pool(#[from] deadpool_redis::PoolError),
    #[error("Connection error: {0}")]
    Connection(String),
}

#[async_trait]
pub trait StateStore: Send + Sync {
    async fn insert(&self, key: Uuid, data: TransactionData) -> Result<(), StateError>;
    async fn get(&self, key: &Uuid) -> Result<Option<TransactionData>, StateError>;
    async fn update(&self, key: &Uuid, data: TransactionData) -> Result<(), StateError>;
    async fn delete(&self, key: &Uuid) -> Result<(), StateError>;
    async fn find_by_acs_trans_id(&self, acs_trans_id: &Uuid) -> Result<Option<(Uuid, TransactionData)>, StateError>;
}

// Redis implementation with connection pooling (Redis-only state store)
pub struct RedisStore {
    pool: Pool,
    ttl_seconds: u64,
    key_prefix: String,
}

impl RedisStore {
    pub async fn new(settings: &Settings) -> Result<Self, StateError> {
        // Configure connection pool
        let cfg = Config::from_url(&settings.redis.url);
        let pool = cfg
            .builder()
            .map_err(|e| StateError::Connection(format!("Failed to create pool builder: {}", e)))?
            .max_size(settings.redis.pool.max_size as usize)
            .runtime(Runtime::Tokio1)
            .build()
            .map_err(|e| StateError::Connection(format!("Failed to create connection pool: {}", e)))?;
        
        // Test the connection pool
        let mut conn = pool.get().await?;
        
        // Simple ping test
        let _: String = deadpool_redis::redis::cmd("PING")
            .query_async(&mut *conn)
            .await?;

        println!("✅ Redis connection pool established: {}", settings.redis.url);
        println!("📊 Pool size: {} (min idle: {})", settings.redis.pool.max_size, settings.redis.pool.min_idle);
        println!("📝 Transaction TTL: {} seconds", settings.redis.ttl_seconds);
        println!("🔑 Key prefix: {}", settings.redis.key_prefix);

        Ok(Self {
            pool,
            ttl_seconds: settings.redis.ttl_seconds,
            key_prefix: settings.redis.key_prefix.clone(),
        })
    }

    fn make_key(&self, key: &Uuid) -> String {
        format!("{}:{}", self.key_prefix, key)
    }

    // Simple retry mechanism for Redis operations
    async fn with_retry<F, Fut, R>(&self, operation: F) -> Result<R, StateError>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<R, StateError>>,
    {
        const MAX_RETRIES: u32 = 3;
        
        for attempt in 1..=MAX_RETRIES {
            match operation().await {
                Ok(result) => return Ok(result),
                Err(StateError::Redis(_)) | Err(StateError::Pool(_)) if attempt < MAX_RETRIES => {
                    // Wait before retrying
                    tokio::time::sleep(Duration::from_millis(100 * attempt as u64)).await;
                    continue;
                }
                Err(e) => return Err(e),
            }
        }
        
        unreachable!()
    }
}

#[async_trait]
impl StateStore for RedisStore {
    async fn insert(&self, key: Uuid, data: TransactionData) -> Result<(), StateError> {
        let redis_key = self.make_key(&key);
        let ttl_seconds = self.ttl_seconds;
        
        self.with_retry(|| async {
            let mut conn = self.pool.get().await?;
            let serialized_data = serde_json::to_string(&data)?;
            
            deadpool_redis::redis::cmd("SETEX")
                .arg(&redis_key)
                .arg(ttl_seconds)
                .arg(&serialized_data)
                .query_async::<_, ()>(&mut *conn)
                .await?;
            
            println!("📦 Transaction stored in Redis: {} (TTL: {} seconds)", key, ttl_seconds);
            
            Ok(())
        }).await
    }

    async fn get(&self, key: &Uuid) -> Result<Option<TransactionData>, StateError> {
        let redis_key = self.make_key(key);
        
        self.with_retry(|| async {
            let mut conn = self.pool.get().await?;
            
            let result: Option<String> = deadpool_redis::redis::cmd("GET")
                .arg(&redis_key)
                .query_async(&mut *conn)
                .await?;
            
            match result {
                Some(data_str) => {
                    let data: TransactionData = serde_json::from_str(&data_str)?;
                    Ok(Some(data))
                }
                None => Ok(None),
            }
        }).await
    }

    async fn update(&self, key: &Uuid, data: TransactionData) -> Result<(), StateError> {
        let redis_key = self.make_key(key);
        let ttl_seconds = self.ttl_seconds;
        
        self.with_retry(|| async {
            let mut conn = self.pool.get().await?;
            
            // Check if key exists first
            let exists: bool = deadpool_redis::redis::cmd("EXISTS")
                .arg(&redis_key)
                .query_async(&mut *conn)
                .await?;
            
            if !exists {
                return Err(StateError::NotFound);
            }
            
            let serialized_data = serde_json::to_string(&data)?;
            
            deadpool_redis::redis::cmd("SETEX")
                .arg(&redis_key)
                .arg(ttl_seconds)
                .arg(&serialized_data)
                .query_async::<_, ()>(&mut *conn)
                .await?;
            
            Ok(())
        }).await
    }

    async fn delete(&self, key: &Uuid) -> Result<(), StateError> {
        let redis_key = self.make_key(key);
        
        self.with_retry(|| async {
            let mut conn = self.pool.get().await?;
            
            deadpool_redis::redis::cmd("DEL")
                .arg(&redis_key)
                .query_async::<_, ()>(&mut *conn)
                .await?;
            
            Ok(())
        }).await
    }

    async fn find_by_acs_trans_id(&self, acs_trans_id: &Uuid) -> Result<Option<(Uuid, TransactionData)>, StateError> {
        println!("🔍 Searching Redis for transaction by acsTransID: {}", acs_trans_id);
        println!("  📊 Scanning keys with pattern: {}:*", self.key_prefix);
        
        self.with_retry(|| async {
            let mut conn = self.pool.get().await?;
            
            // Get all keys matching our pattern
            let pattern = format!("{}:*", self.key_prefix);
            let keys: Vec<String> = deadpool_redis::redis::cmd("KEYS")
                .arg(&pattern)
                .query_async(&mut *conn)
                .await?;
            
            println!("  📋 Found {} total keys to check", keys.len());
            
            // Search through all transactions
            for key in keys {
                let result: Option<String> = deadpool_redis::redis::cmd("GET")
                    .arg(&key)
                    .query_async(&mut *conn)
                    .await?;
                
                if let Some(data_str) = result {
                    if let Ok(transaction_data) = serde_json::from_str::<TransactionData>(&data_str) {
                        if transaction_data.acs_trans_id == *acs_trans_id {
                            // Extract the threeDSServerTransID from the key
                            if let Some(uuid_str) = key.strip_prefix(&format!("{}:", self.key_prefix)) {
                                if let Ok(three_ds_server_trans_id) = Uuid::parse_str(uuid_str) {
                                    println!("  ✅ Found matching transaction: threeDSServerTransID={}, acsTransID={}", 
                                             three_ds_server_trans_id, acs_trans_id);
                                    return Ok(Some((three_ds_server_trans_id, transaction_data)));
                                }
                            }
                        }
                    }
                }
            }
            
            println!("  ❌ No transaction found with acsTransID: {}", acs_trans_id);
            Ok(None)
        }).await
    }
}

// Factory function to create Redis store from settings
pub async fn create_redis_store(settings: &Settings) -> Result<RedisStore, StateError> {
    RedisStore::new(settings).await
}
