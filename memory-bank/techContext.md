# Tech Context - Technologies, Setup & Dependencies

## Technologies Used

### Core Technology Stack

#### Programming Language: Rust (Edition 2021)
**Why Chosen:**
- Memory safety without garbage collection
- Excellent performance characteristics  
- Strong type system prevents runtime errors
- Growing ecosystem for web development
- Educational value for systems programming concepts

**Version:** Latest stable (1.75+)
**Key Features Used:**
- Ownership and borrowing system
- Async/await for concurrent programming
- Derive macros for code generation
- Pattern matching and Option/Result types

#### Web Framework: Actix-web 4.x
**Why Chosen:**
- High-performance async web framework
- Mature ecosystem with middleware support
- Excellent JSON handling with Serde integration
- Clear documentation and examples
- Production-ready with good error handling

**Key Features Used:**
- HTTP server and routing
- JSON request/response handling
- Middleware for logging
- Dependency injection for shared state
- Async handler functions

#### Serialization: Serde 1.0
**Why Chosen:**
- De facto standard for Rust serialization
- Zero-cost abstractions via derive macros
- Excellent JSON support
- Flexible field naming and transformation
- Compile-time validation of data structures

**Features Used:**
- Automatic JSON ↔ Struct conversion
- Field renaming (camelCase ↔ snake_case)
- Optional fields and default values
- Custom serialization for complex types

### Supporting Libraries

#### UUID Generation: uuid 1.6
**Purpose:** Unique transaction ID generation
**Features:** Version 4 (random) UUIDs with Serde support
**Usage:** `Uuid::new_v4()` for transaction identification

#### Base64 Encoding: base64 0.21  
**Purpose:** Challenge request encoding for 3DS protocol
**Usage:** Encode binary challenge data for JSON transport

#### Logging: env_logger 0.10
**Purpose:** Development and debugging support
**Features:** Configurable log levels, environment-based configuration
**Integration:** Actix-web middleware for request logging

#### Date/Time: chrono 0.4
**Purpose:** Timestamp handling for 3DS protocol fields
**Features:** RFC3339 formatting, timezone support
**Usage:** Transaction timestamps and date formatting

#### Async Runtime: tokio 1.x (via Actix-web)
**Purpose:** Async execution and concurrency
**Features:** Multi-threaded runtime, async I/O
**Integration:** Automatic via Actix-web framework

## Development Setup

### Prerequisites
```bash
# Install Rust toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Verify installation
rustc --version
cargo --version
```

### Project Setup
```bash
# Clone/create project
git clone <repository> # or cargo new mock_three_ds_server
cd mock_three_ds_server

# Install dependencies
cargo build

# Run development server
cargo run

# Run with logging
RUST_LOG=info cargo run
```

### Development Workflow
```bash
# Check code formatting
cargo fmt --check

# Run linter
cargo clippy

# Run tests
cargo test

# Build for release
cargo build --release
```

### IDE Configuration
**Recommended:** Visual Studio Code with rust-analyzer extension
**Features:**
- Syntax highlighting and error detection
- Code completion and navigation
- Integrated debugging support
- Format on save with rustfmt

## Technical Constraints

### Performance Constraints
- **Memory Usage:** Limited by available system RAM for in-memory state
- **Concurrency:** Single mutex serializes all state access
- **Throughput:** Adequate for development/testing, not production scale
- **Latency:** Sub-millisecond for simple operations, 1-5ms for complex JSON

### Scalability Constraints  
- **Horizontal Scaling:** Requires external state store
- **State Persistence:** Lost on server restart
- **Connection Limits:** Default Actix-web limits (varies by system)
- **Memory Growth:** Linear with number of active transactions

### Educational Constraints
- **Complexity Limitations:** Simplified for learning purposes
- **Feature Completeness:** Mock implementation, not production 3DS
- **Error Handling:** Basic patterns, not comprehensive production cases
- **Security:** No authentication, encryption, or audit logging

### Runtime Constraints
- **Platform:** Cross-platform (Windows, macOS, Linux)
- **Dependencies:** Minimal system dependencies
- **Network:** Requires available port 8080
- **Permissions:** Standard user permissions sufficient

## Dependencies Analysis

### Core Dependencies
```toml
[dependencies]
actix-web = "4"                    # Web framework
serde = { version = "1.0", features = ["derive"] }  # Serialization
serde_json = "1.0"                 # JSON support
uuid = { version = "1.6", features = ["v4", "serde"] }  # ID generation
chrono = { version = "0.4", features = ["serde"] }      # Date/time
tokio = { version = "1", features = ["full"] }          # Async runtime
base64 = "0.21"                    # Base64 encoding
env_logger = "0.10"                # Logging
```

### Dependency Security
- **All dependencies:** From crates.io official registry
- **Vulnerability Scanning:** `cargo audit` for security issues
- **License Compatibility:** All MIT/Apache 2.0 licensed
- **Maintenance Status:** All actively maintained projects

### Build Dependencies
- **Compilation:** Requires Rust 1.75+ for edition 2021 features
- **Build Time:** ~2-3 minutes for clean build on modern hardware
- **Binary Size:** ~10-15MB for release build
- **Cross Compilation:** Supported via Rust toolchain

## Tool Usage Patterns

### Development Tools

#### Cargo (Build System)
```bash
cargo check          # Fast compilation check
cargo build          # Development build
cargo build --release # Optimized build
cargo run            # Build and run
cargo test           # Run test suite
cargo fmt            # Format code
cargo clippy         # Lint code
```

#### Testing Tools
```bash
# Unit tests
cargo test

# Integration tests  
cargo test --test integration

# Test specific module
cargo test handlers::tests

# Test with output
cargo test -- --nocapture
```

#### Debugging Tools
```bash
# Debug build with symbols
cargo build

# Run with debugger (VS Code)
# Use integrated debugging with breakpoints

# Environment debugging
RUST_LOG=debug cargo run
RUST_BACKTRACE=1 cargo run
```

### Production Tools (Future)

#### Monitoring
- **Metrics:** Prometheus integration patterns documented
- **Logging:** Structured JSON logging for production
- **Tracing:** Distributed tracing preparation

#### Deployment
- **Docker:** Containerization examples provided
- **Kubernetes:** Deployment manifests documented
- **Health Checks:** Endpoint patterns established

## Environment Configuration

### Development Environment
```bash
# Required environment variables
RUST_LOG=info                    # Logging level
PORT=8080                        # Server port (optional)

# Optional development settings
RUST_BACKTRACE=1                 # Stack traces on panic
CARGO_INCREMENTAL=1              # Faster rebuilds
```

### Testing Environment
```bash
# Test configuration
RUST_LOG=debug                   # Verbose logging for tests
TEST_PORT=8081                   # Avoid conflicts with dev server
```

### Production Environment (Future)
```bash
# Production settings (examples)
RUST_LOG=warn                    # Minimal logging
DATABASE_URL=postgresql://...    # External storage
REDIS_URL=redis://...           # Session storage
BIND_ADDRESS=0.0.0.0:8080       # Network binding
```

## Technology Decision Rationale

### Why Rust for Web Development?
1. **Safety:** Memory safety prevents common web vulnerabilities
2. **Performance:** Near C-level performance for high-throughput APIs
3. **Concurrency:** Excellent async support for handling many connections
4. **Ecosystem:** Mature web frameworks and libraries available
5. **Education:** Teaches systems-level concepts while building web services

**Development Experience Validation:**
- Compile-time error checking prevented entire categories of runtime bugs
- Ownership system made concurrent programming intuitive and safe
- Type system helped catch JSON field mapping errors during development
- Zero-cost abstractions provided both safety and performance

### Why Actix-web over Alternatives?
**Alternatives Considered:**
- **Axum:** Newer, but less mature ecosystem
- **Rocket:** Good ergonomics, but requires nightly Rust
- **Warp:** Functional style, but steeper learning curve

**Actix-web Chosen For:**
- Mature, battle-tested in production
- Excellent performance characteristics
- Rich middleware ecosystem
- Clear documentation and examples
- Stable API with good backward compatibility

**Implementation Experience:**
- JSON integration with Serde worked seamlessly
- Middleware system simplified logging implementation
- Dependency injection pattern made state sharing elegant
- Error handling integrated well with Rust Result types

### Why In-Memory State over Database?
**Trade-offs:**
- **Simplicity:** Easier to understand and debug
- **Setup:** No external dependencies required
- **Educational:** Focus on Rust concepts, not database integration
- **Performance:** Fastest possible for learning/testing scenarios

**Development Insights:**
- Arc<Mutex<HashMap>> pattern provided excellent learning opportunity
- Thread safety achieved through Rust ownership system
- State persistence across requests demonstrated web application concepts
- Simple to reason about for educational purposes

**Future Migration Path:**
- State management abstracted behind clear interface
- Database integration examples documented
- Migration path clearly defined

## Maintenance and Updates

### Dependency Updates
```bash
# Check for updates
cargo outdated

# Update dependencies
cargo update

# Update to latest versions
# Edit Cargo.toml, then cargo update
```

### Security Updates
```bash
# Security audit
cargo install cargo-audit
cargo audit

# Fix security issues
cargo audit fix
```

### Rust Toolchain Updates
```bash
# Update Rust
rustup update

# Update to specific version
rustup install 1.75.0
rustup default 1.75.0
