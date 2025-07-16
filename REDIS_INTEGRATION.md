# Redis Integration Guide

## Overview

The 3DS Mock Server now supports both in-memory and Redis-based state management. This enhancement allows for persistent transaction storage, horizontal scaling, and production-ready deployment scenarios.

## Architecture Changes

### State Management Abstraction

The server now uses a trait-based state management system that supports multiple backends:

```rust
#[async_trait]
pub trait StateStore: Send + Sync {
    async fn insert(&self, key: Uuid, data: TransactionData) -> Result<(), StateError>;
    async fn get(&self, key: &Uuid) -> Result<Option<TransactionData>, StateError>;
    async fn update(&self, key: &Uuid, data: TransactionData) -> Result<(), StateError>;
    async fn delete(&self, key: &Uuid) -> Result<(), StateError>;
}
```

### Two Implementation Options

1. **In-Memory Store** (Default)
   - Uses `Arc<Mutex<HashMap>>` for thread-safe storage
   - Perfect for development and testing
   - Data is lost on server restart

2. **Redis Store** (Optional)
   - Uses Redis for persistent, distributed storage
   - Supports automatic TTL (Time To Live) for transactions
   - Enables horizontal scaling across multiple server instances

## Configuration

### Environment Variables

- `USE_REDIS`: Set to `true` to enable Redis mode (default: `false`)
- `REDIS_URL`: Redis connection string (default: `redis://127.0.0.1:6379`)
- `TRANSACTION_TTL_SECONDS`: TTL for transactions in seconds (default: `1800` = 30 minutes)

### Usage Examples

#### Running with In-Memory State (Default)
```bash
cargo run
```

#### Running with Redis
```bash
USE_REDIS=true cargo run
```

#### Custom Redis Configuration
```bash
USE_REDIS=true \
REDIS_URL=redis://localhost:6380 \
TRANSACTION_TTL_SECONDS=3600 \
cargo run
```

## Redis Setup

### Local Development with Docker

```bash
# Start Redis server
docker run --name redis-3ds -p 6379:6379 -d redis:7-alpine

# Run the application with Redis
USE_REDIS=true cargo run

# Stop Redis when done
docker stop redis-3ds
docker rm redis-3ds
```

### Production Redis

For production environments, consider:
- Redis Cluster for high availability
- Redis Sentinel for automatic failover
- Proper authentication and TLS encryption
- Monitoring and alerting

Example production configuration:
```bash
USE_REDIS=true \
REDIS_URL=rediss://username:password@redis-cluster.example.com:6380 \
TRANSACTION_TTL_SECONDS=1800 \
cargo run
```

## Features

### Automatic TTL (Time To Live)

Redis transactions automatically expire after the configured TTL, preventing memory leaks and ensuring data cleanup:

- Default TTL: 30 minutes (1800 seconds)
- Configurable via `TRANSACTION_TTL_SECONDS`
- Each operation resets the TTL for that transaction

### Transaction Persistence

Transaction data is stored in Redis with the following key pattern:
```
3ds_transaction:{transaction_id}
```

### Error Handling

The system includes comprehensive error handling for Redis operations:
- Connection failures
- Serialization/deserialization errors
- Transaction not found scenarios
- Automatic fallback error responses

### JSON Serialization

Transaction data is automatically serialized to JSON for Redis storage, including all nested structures:
- Authentication requests
- Results requests
- UUIDs and timestamps
- Complex nested objects

## Testing

### Testing In-Memory Mode
```bash
# Default mode - no Redis required
cargo run
```

### Testing Redis Mode
```bash
# Start Redis
docker run --name test-redis -p 6379:6379 -d redis:7-alpine

# Test with Redis
USE_REDIS=true cargo run

# Verify data persistence by restarting the server
# Transactions should survive server restarts
```

### Switching Between Modes

You can switch between modes by simply changing the environment variable:
```bash
# Start with in-memory
cargo run

# Stop and restart with Redis
USE_REDIS=true cargo run
```

## Performance Considerations

### In-Memory Store
- **Pros**: Fastest possible performance, no network overhead
- **Cons**: Limited by server RAM, data lost on restart
- **Best for**: Development, testing, single-instance deployments

### Redis Store
- **Pros**: Persistent storage, horizontal scaling, automatic cleanup
- **Cons**: Network latency, requires Redis infrastructure
- **Best for**: Production deployments, multi-instance setups

### Benchmarks

Typical performance characteristics:
- In-Memory: < 1ms per operation
- Redis (local): 1-5ms per operation
- Redis (remote): 5-50ms per operation (depends on network)

## Implementation Details

### Connection Management

The Redis implementation uses individual connections per operation for simplicity and reliability:
```rust
async fn get(&self, key: &Uuid) -> Result<Option<TransactionData>, StateError> {
    let mut conn = self.client.get_async_connection().await?;
    // ... operation
}
```

### Serialization Strategy

All transaction data is serialized to JSON for human-readable storage and debugging:
```json
{
  "authenticate_request": { /* full request data */ },
  "acs_trans_id": "uuid-here",
  "ds_trans_id": "uuid-here",
  "sdk_trans_id": "uuid-here",
  "results_request": { /* optional results data */ }
}
```

### Error Recovery

The system includes graceful error handling:
- Failed Redis operations return appropriate HTTP error responses
- Clear error messages for debugging
- No silent failures or data corruption

## Migration Guide

### From In-Memory to Redis

1. Install and start Redis
2. Set environment variables
3. Restart the application
4. Verify operation with test transactions

### Rollback Strategy

To rollback from Redis to in-memory:
1. Stop the application
2. Unset `USE_REDIS` environment variable
3. Restart the application

Note: In-memory mode will not have access to transactions stored in Redis.

## Monitoring

### Key Metrics to Monitor

- Redis connection health
- Transaction TTL expiration rates
- Redis memory usage
- Operation latency
- Error rates

### Logging

The application logs state store initialization:
```
Initializing Redis state store at redis://127.0.0.1:6379
```
or
```
Initializing in-memory state store
```

## Security Considerations

### Redis Security
- Use Redis AUTH for authentication
- Enable TLS for encryption in transit
- Use Redis ACLs for fine-grained access control
- Keep Redis server patched and updated

### Data Privacy
- Transaction data contains sensitive payment information
- Consider encryption at rest for Redis data
- Implement proper key rotation strategies
- Monitor access logs for compliance

## Troubleshooting

### Common Issues

1. **Redis Connection Failed**
   ```
   Failed to connect to Redis: Connection refused
   ```
   - Verify Redis is running
   - Check REDIS_URL configuration
   - Verify network connectivity

2. **Serialization Errors**
   ```
   Serialization error: missing field
   ```
   - Check data model compatibility
   - Verify all required fields are present

3. **TTL Too Short**
   - Increase `TRANSACTION_TTL_SECONDS`
   - Monitor transaction lifecycle timing

### Debug Commands

```bash
# Check Redis connectivity
redis-cli ping

# Monitor Redis operations
redis-cli monitor

# Check stored transactions
redis-cli keys "3ds_transaction:*"

# View transaction data
redis-cli get "3ds_transaction:YOUR_UUID_HERE"
```

## Future Enhancements

Potential improvements for the Redis integration:

1. **Connection Pooling**: Use Redis connection pools for better performance
2. **Clustering Support**: Add Redis Cluster support for high availability
3. **Metrics**: Add Prometheus metrics for monitoring
4. **Backup/Restore**: Add transaction backup and restore capabilities
5. **Encryption**: Add client-side encryption for sensitive data
