use config::{Config, ConfigError, Environment, File};
use serde::{Deserialize, Serialize};
use std::env;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Settings {
    pub server: ServerConfig,
    pub redis: RedisConfig,
    pub performance: PerformanceConfig,
    pub monitoring: MonitoringConfig,
    pub retry: RetryConfig,
    pub cache: CacheConfig,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub log_level: String,
    pub workers: Option<usize>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PerformanceConfig {
    pub enable_compression: bool,
    pub enable_metrics: bool,
    pub cache_size: usize,
    pub rate_limit_per_second: u32,
    pub max_connections: usize,
    pub client_timeout_ms: u64,
    pub keep_alive_seconds: u64,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct MonitoringConfig {
    pub metrics_endpoint: String,
    pub health_endpoint: String,
    pub enable_tracing: bool,
    pub request_timeout_seconds: u64,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct RetryConfig {
    pub max_attempts: u32,
    pub initial_delay_ms: u64,
    pub max_delay_ms: u64,
    pub multiplier: f64,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CacheConfig {
    pub card_range_ttl_seconds: u64,
    pub challenge_decision_ttl_seconds: u64,
    pub static_response_ttl_seconds: u64,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct RedisConfig {
    pub url: String,
    pub ttl_seconds: u64,
    pub key_prefix: String,
    pub connection: ConnectionConfig,
    pub pool: PoolConfig,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ConnectionConfig {
    pub timeout_ms: u64,
    pub max_retries: u32,
    pub retry_delay_ms: u64,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PoolConfig {
    pub max_size: u32,
    pub min_idle: u32,
    pub connection_timeout_seconds: u64,
    pub idle_timeout_seconds: u64,
}

impl Settings {
    pub fn new() -> Result<Self, ConfigError> {
        let run_mode = env::var("RUN_MODE").unwrap_or_else(|_| "development".into());

        let s = Config::builder()
            // Load environment-specific configuration (required)
            .add_source(File::with_name(&format!("config/{}", run_mode)))
            // Add environment variables (with prefix "APP")
            // E.g., `APP_REDIS__URL=redis://custom:6379` would override redis.url
            .add_source(Environment::with_prefix("APP").separator("__"))
            .build()?;

        // Deserialize into Settings struct
        s.try_deserialize()
    }

    pub fn validate(&self) -> Result<(), String> {
        // Validate Redis URL format
        if !self.redis.url.starts_with("redis://") && !self.redis.url.starts_with("rediss://") {
            return Err("Redis URL must start with redis:// or rediss://".to_string());
        }

        // Validate port range
        if self.server.port == 0 {
            return Err("Server port must be greater than 0".to_string());
        }

        // Validate pool settings
        if self.redis.pool.max_size == 0 {
            return Err("Redis pool max_size must be greater than 0".to_string());
        }

        if self.redis.pool.min_idle > self.redis.pool.max_size {
            return Err("Redis pool min_idle cannot be greater than max_size".to_string());
        }

        // Validate TTL
        if self.redis.ttl_seconds == 0 {
            return Err("Redis TTL must be greater than 0".to_string());
        }

        Ok(())
    }

    pub fn server_address(&self) -> String {
        format!("{}:{}", self.server.host, self.server.port)
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self::new().expect("Failed to load default configuration")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_settings_validation() {
        let settings = Settings {
            server: ServerConfig {
                host: "127.0.0.1".to_string(),
                port: 8080,
                log_level: "info".to_string(),
                workers: Some(1),
            },
            redis: RedisConfig {
                url: "redis://127.0.0.1:6379".to_string(),
                ttl_seconds: 1800,
                key_prefix: "test".to_string(),
                connection: ConnectionConfig {
                    timeout_ms: 5000,
                    max_retries: 3,
                    retry_delay_ms: 1000,
                },
                pool: PoolConfig {
                    max_size: 10,
                    min_idle: 2,
                    connection_timeout_seconds: 10,
                    idle_timeout_seconds: 300,
                },
            },
            performance: PerformanceConfig {
                enable_compression: false,
                enable_metrics: true,
                cache_size: 1000,
                rate_limit_per_second: 100,
                max_connections: 1000,
                client_timeout_ms: 60000,
                keep_alive_seconds: 60,
            },
            monitoring: MonitoringConfig {
                metrics_endpoint: "/metrics".to_string(),
                health_endpoint: "/health".to_string(),
                enable_tracing: false,
                request_timeout_seconds: 30,
            },
            retry: RetryConfig {
                max_attempts: 3,
                initial_delay_ms: 100,
                max_delay_ms: 5000,
                multiplier: 2.0,
            },
            cache: CacheConfig {
                card_range_ttl_seconds: 3600,
                challenge_decision_ttl_seconds: 300,
                static_response_ttl_seconds: 86400,
            },
        };

        assert!(settings.validate().is_ok());
    }

    #[test]
    fn test_invalid_redis_url() {
        let mut settings = Settings {
            server: ServerConfig {
                host: "127.0.0.1".to_string(),
                port: 8080,
                log_level: "info".to_string(),
                workers: Some(1),
            },
            redis: RedisConfig {
                url: "invalid://url".to_string(),
                ttl_seconds: 1800,
                key_prefix: "test".to_string(),
                connection: ConnectionConfig {
                    timeout_ms: 5000,
                    max_retries: 3,
                    retry_delay_ms: 1000,
                },
                pool: PoolConfig {
                    max_size: 10,
                    min_idle: 2,
                    connection_timeout_seconds: 10,
                    idle_timeout_seconds: 300,
                },
            },
            performance: PerformanceConfig {
                enable_compression: false,
                enable_metrics: true,
                cache_size: 1000,
                rate_limit_per_second: 100,
                max_connections: 1000,
                client_timeout_ms: 60000,
                keep_alive_seconds: 60,
            },
            monitoring: MonitoringConfig {
                metrics_endpoint: "/metrics".to_string(),
                health_endpoint: "/health".to_string(),
                enable_tracing: false,
                request_timeout_seconds: 30,
            },
            retry: RetryConfig {
                max_attempts: 3,
                initial_delay_ms: 100,
                max_delay_ms: 5000,
                multiplier: 2.0,
            },
            cache: CacheConfig {
                card_range_ttl_seconds: 3600,
                challenge_decision_ttl_seconds: 300,
                static_response_ttl_seconds: 86400,
            },
        };

        assert!(settings.validate().is_err());
    }

    #[test]
    fn test_server_address() {
        let settings = Settings {
            server: ServerConfig {
                host: "127.0.0.1".to_string(),
                port: 8080,
                log_level: "info".to_string(),
                workers: Some(1),
            },
            redis: RedisConfig {
                url: "redis://127.0.0.1:6379".to_string(),
                ttl_seconds: 1800,
                key_prefix: "test".to_string(),
                connection: ConnectionConfig {
                    timeout_ms: 5000,
                    max_retries: 3,
                    retry_delay_ms: 1000,
                },
                pool: PoolConfig {
                    max_size: 10,
                    min_idle: 2,
                    connection_timeout_seconds: 10,
                    idle_timeout_seconds: 300,
                },
            },
            performance: PerformanceConfig {
                enable_compression: false,
                enable_metrics: true,
                cache_size: 1000,
                rate_limit_per_second: 100,
                max_connections: 1000,
                client_timeout_ms: 60000,
                keep_alive_seconds: 60,
            },
            monitoring: MonitoringConfig {
                metrics_endpoint: "/metrics".to_string(),
                health_endpoint: "/health".to_string(),
                enable_tracing: false,
                request_timeout_seconds: 30,
            },
            retry: RetryConfig {
                max_attempts: 3,
                initial_delay_ms: 100,
                max_delay_ms: 5000,
                multiplier: 2.0,
            },
            cache: CacheConfig {
                card_range_ttl_seconds: 3600,
                challenge_decision_ttl_seconds: 300,
                static_response_ttl_seconds: 86400,
            },
        };
        assert_eq!(settings.server_address(), "127.0.0.1:8080");
    }
}
