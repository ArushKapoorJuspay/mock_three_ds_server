# Project Brief - 3DS Mock Server

## Foundation & Core Requirements

### Project Scope
Build a complete mock 3D Secure (3DS) authentication server in Rust that simulates payment card authentication flows for testing and development purposes.

### Core Requirements
1. **Four API Endpoints:**
   - `POST /3ds/version` - Card range inquiry and transaction initialization
   - `POST /3ds/authenticate` - Authentication process with challenge/frictionless decision
   - `POST /3ds/results` - Authentication outcome submission
   - `POST /3ds/final` - Final authentication package retrieval

2. **State Management:**
   - Maintain transaction data across multiple API calls
   - Thread-safe concurrent access
   - UUID-based transaction tracking
   - Redis-only storage backend (production-ready)
   - TOML configuration with validation and environment overrides
   - Configurable persistence and TTL support

3. **Business Logic:**
   - Challenge flow for cards ending in "4001" (transStatus: "C")
   - Frictionless flow for cards ending in "4000" (transStatus: "Y")
   - Card range detection for cards starting with "515501"
   - Base64 encoded challenge requests

4. **Technical Standards:**
   - JSON request/response format
   - Automatic serialization/deserialization
   - Proper error handling and HTTP status codes
   - Logging and monitoring capabilities

### Success Criteria
- ✅ All 4 endpoints functional and tested
- ✅ Complete transaction flow from version → authenticate → results → final
- ✅ Thread-safe state management
- ✅ Comprehensive documentation for learning and usage
- ✅ Production-ready patterns demonstrating Rust best practices

### Educational Goals
- Demonstrate Rust web development from beginner to advanced
- Teach 3DS protocol understanding
- Show proper error handling and concurrency patterns
- Provide real-world applicable code patterns

### Constraints
- Mock server only (no real payment processing)
- Redis-only state storage (production-ready, no in-memory fallback)
- Focus on educational value while supporting production patterns
- Must be testable via Postman/curl

### Target Audience
- Developers learning Rust web development
- Payment industry professionals understanding 3DS flows
- Teams needing mock 3DS servers for testing
- Educational content for web API design patterns
