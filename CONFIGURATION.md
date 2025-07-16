# Configuration Guide

## Overview

The 3DS Mock Server now uses TOML configuration files for all settings. Redis is **required** and is the only supported state storage backend.

## Configuration Files

### File Hierarchy
1. `config/{RUN_MODE}.toml` - Environment-specific configuration (required)
2. Environment variables with `APP_` prefix - Highest priority

### Available Configuration Files

#### `config/development.toml`
Complete development configuration including:
- Local server binding (127.0.0.1)
- Debug logging level
- Shorter Redis TTL (5 minutes) for faster testing
- Standard connection pool settings

#### `config/production.toml`
Complete production configuration including:
- Bind to all interfaces (0.0.0.0)
- Warning-level logging
- Standard Redis TTL (30 minutes)
- Larger Redis connection pool for high throughput

## Configuration Structure

```toml
[server]
host = "127.0.0.1"      # Server bind address
port = 8080             # Server port
log_level = "info"      # Logging level: trace, debug, info, warn, error

[redis]
url = "redis://127.0.0.1:6379"  # Redis connection URL
ttl_seconds = 1800              # Transaction TTL (30 minutes)
key_prefix = "3ds_transaction"  # Redis key prefix

[redis.connection]
timeout_ms = 5000       # Connection timeout
max_retries = 3         # Connection retry attempts
retry_delay_ms = 1000   # Delay between retries

[redis.pool]
max_size = 10                     # Maximum connections
min_idle = 2                      # Minimum idle connections
connection_timeout_seconds = 10   # Pool connection timeout
idle_timeout_seconds = 300        # Idle connection timeout
```

## Running with Different Configurations

### Development (Default)
```bash
cargo run
# or explicitly
RUN_MODE=development cargo run
```

### Production
```bash
RUN_MODE=production cargo run
```

### Custom Environment
Create `config/staging.toml` and run:
```bash
RUN_MODE=staging cargo run
```

## Environment Variable Overrides

Override any configuration value using environment variables with `APP_` prefix:

```bash
# Override Redis URL
APP_REDIS__URL=redis://custom-host:6379 cargo run

# Override server port
APP_SERVER__PORT=8081 cargo run

# Override log level
APP_SERVER__LOG_LEVEL=debug cargo run

# Multiple overrides
APP_REDIS__URL=redis://prod:6379 \
APP_SERVER__HOST=0.0.0.0 \
APP_SERVER__PORT=80 \
cargo run
```

## Redis Requirements

### Mandatory Redis
- Redis is **required** - the application will not start without it
- No in-memory fallback is available
- Ensure Redis is running before starting the application

### Starting Redis with Docker
```bash
# Start Redis container
docker run --name redis-3ds -p 6379:6379 -d redis:7-alpine

# Start the application
cargo run

# Stop Redis when done
docker stop redis-3ds && docker rm redis-3ds
```

### Redis Connection Examples

#### Local Redis
```toml
[redis]
url = "redis://127.0.0.1:6379"
```

#### Redis with Auth
```toml
[redis]
url = "redis://:password@127.0.0.1:6379"
```

#### Redis with Username/Password
```toml
[redis]
url = "redis://username:password@127.0.0.1:6379"
```

#### Redis SSL/TLS
```toml
[redis]
url = "rediss://127.0.0.1:6380"
```

## Configuration Validation

The application validates configuration on startup:

- ✅ Redis URL format (must start with `redis://` or `rediss://`)
- ✅ Server port (must be > 0)
- ✅ Pool settings (max_size > 0, min_idle ≤ max_size)
- ✅ TTL values (must be > 0)

Invalid configuration will cause startup failure with clear error messages.

## Development Tips

### Quick Configuration Check
```bash
# Dry run to check configuration
cargo check
```

### Debug Configuration Loading
```bash
# See what configuration is loaded
RUN_MODE=development cargo run 2>&1 | grep "Configuration mode"
```

### Testing Different Redis Instances
```bash
# Test with different Redis
APP_REDIS__URL=redis://localhost:6380 cargo run
```

## Troubleshooting

### Application Won't Start

#### "Failed to load configuration"
- Ensure the appropriate environment config file exists (`config/development.toml` or `config/production.toml`)
- Check TOML syntax for errors
- Verify file permissions
- Ensure RUN_MODE environment variable matches an existing config file

#### "Configuration validation failed"
- Check Redis URL format
- Verify port numbers are valid
- Ensure pool settings are logical

#### "Failed to initialize Redis store"
- Ensure Redis is running
- Check Redis URL is correct
- Verify network connectivity
- Check Redis authentication

### Runtime Issues

#### "Redis connection lost"
- Check Redis server status
- Verify network connectivity
- Review Redis logs

#### "Transaction not found"
- Check Redis TTL settings
- Verify Redis persistence settings
- Review application logs

## Migration from Environment Variables

If migrating from the previous environment variable approach:

| Old Environment Variable | New TOML Configuration |
|--------------------------|------------------------|
| `USE_REDIS=true` | *Always Redis now* |
| `REDIS_URL` | `redis.url` |
| `TRANSACTION_TTL_SECONDS` | `redis.ttl_seconds` |
| `SERVER_PORT` | `server.port` |

The environment variable `USE_REDIS` is no longer needed as Redis is always used.

## Security Considerations

### Sensitive Data
- Never commit production Redis URLs with credentials
- Use environment variables for production secrets:
  ```bash
  APP_REDIS__URL=redis://user:secret@prod-redis:6379 cargo run
  ```

### File Permissions
- Ensure config files have appropriate permissions
- Consider using `config/local.toml` for local secrets (gitignored)

### Redis Security
- Use Redis AUTH in production
- Enable TLS with `rediss://` URLs
- Restrict Redis network access
- Monitor Redis access logs

## Performance Tuning

### Connection Pool Settings
For high-throughput applications:
```toml
[redis.pool]
max_size = 50          # Increase for more concurrent requests
min_idle = 10          # Keep more connections warm
```

### TTL Optimization
```toml
[redis]
ttl_seconds = 600      # Shorter for testing
ttl_seconds = 3600     # Longer for production
```

### Network Timeouts
```toml
[redis.connection]
timeout_ms = 2000      # Faster timeout for local Redis
timeout_ms = 10000     # Longer timeout for remote Redis
