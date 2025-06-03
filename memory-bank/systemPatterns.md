# System Patterns - Architecture & Technical Decisions

## System Architecture

### Overall Design Pattern
**Layered Architecture** with clear separation of concerns:

```
┌─────────────────────────────────────────┐
│             HTTP Layer                  │
│        (main.rs - routing)              │
├─────────────────────────────────────────┤
│          Handler Layer                  │
│     (handlers.rs - business logic)      │
├─────────────────────────────────────────┤
│           Model Layer                   │
│    (models.rs - data structures)        │
├─────────────────────────────────────────┤
│           State Layer                   │
│   (state.rs - memory management)        │
└─────────────────────────────────────────┘
```

### Key Technical Decisions

#### 1. State Management Pattern
**Decision:** Arc<Mutex<HashMap>> for shared state
**Rationale:** 
- Simple to understand and implement
- Thread-safe with clear ownership semantics
- Rust's ownership system prevents data races
- Appropriate for educational/mock server scale

**Alternative Considered:**
- Database storage (rejected for simplicity)
- RwLock (rejected due to complexity for demo)
- Actor pattern (noted for future enhancement)

**Implementation Context:**
- TransactionData struct stores complete state per transaction
- HashMap keyed by UUID (threeDSServerTransID)
- Mutex ensures atomic operations across all endpoints
- Arc enables sharing across Actix-web worker threads
- Data persisted across API calls: version → authenticate → results → final

#### 2. Data Modeling Strategy  
**Decision:** Derive macros for automatic serialization
**Rationale:**
- Zero-boilerplate JSON conversion
- Compile-time safety for field mappings
- Clear separation between Rust naming and JSON naming
- Educational value showing Rust's macro system

**Pattern:**
```rust
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RequestType {
    pub field_name: String,  // → fieldName in JSON
}
```

#### 3. Error Handling Pattern
**Decision:** Result<HttpResponse> with JSON error responses
**Rationale:**
- Consistent with Rust error handling idioms
- Forces explicit error handling at compile time
- HTTP-appropriate error responses
- No exceptions or panics in normal operation

**Pattern:**
```rust
// Success case
Ok(HttpResponse::Ok().json(response))

// Error case  
Ok(HttpResponse::BadRequest().json(serde_json::json!({
    "error": "Description"
})))
```

#### 4. Concurrency Model
**Decision:** Async/await with Actix-web thread pool
**Rationale:**
- Handles many concurrent connections efficiently
- Rust's ownership prevents data races
- Async is the standard for modern web servers
- Educational value for async programming

## Design Patterns in Use

### 1. Repository Pattern (State Management)
```rust
pub type AppState = Arc<Mutex<HashMap<Uuid, TransactionData>>>;
```
- Abstracts data storage implementation
- Could be swapped for database without changing handlers
- Clear interface between business logic and storage

### 2. Handler Pattern (Request Processing)
```rust
pub async fn handler_name(
    req: web::Json<RequestType>,
    state: web::Data<AppState>,
) -> Result<HttpResponse>
```
- Each endpoint has dedicated handler function
- Dependency injection of shared state
- Clear separation of HTTP concerns from business logic

### 3. Builder Pattern (Response Construction)
```rust
let response = AuthenticateResponse {
    purchase_date: req.purchase.purchase_date.clone(),
    three_ds_server_trans_id: trans_id,
    // ... field by field construction
};
```
- Explicit response building
- Compile-time verification of required fields
- Clear data flow from input to output

### 4. Strategy Pattern (Business Logic)
```rust
let is_challenge = card_number.ends_with("4001");
let trans_status = if is_challenge { "C" } else { "Y" };
```
- Different authentication flows based on card number
- Easy to extend with new card number patterns
- Business rules clearly expressed in code

## Component Relationships

### Data Flow Architecture
```
Request → JSON → Struct → Handler → State → Response → JSON
    ↑                                              ↓
    └── Actix-web ← HTTP ← JSON ← Struct ← Handler ←┘
```

### Dependency Graph
```
main.rs
├── handlers.rs
│   ├── models.rs
│   └── state.rs
└── state.rs
    └── models.rs
```

### State Lifecycle
```
1. Version Call:    Generate Transaction ID
2. Authenticate:    Store TransactionData  
3. Results:         Update TransactionData.results_request
4. Final:           Read Complete TransactionData
```

## Critical Implementation Paths

### 1. Transaction State Management
**Critical Path:** UUID generation → State storage → State retrieval → State update

**Risk Mitigation:**
- UUID v4 ensures uniqueness across all transactions
- Mutex prevents concurrent modification corruption
- Error handling for missing transactions
- Clear separation of read vs write operations

### 2. JSON Serialization Chain
**Critical Path:** HTTP Body → JSON → Serde → Rust Struct → Business Logic

**Risk Mitigation:**
- Compile-time field validation via Serde derives
- Automatic error responses for malformed JSON
- Clear error messages for debugging
- Type safety prevents runtime serialization errors

### 3. Concurrent Request Handling
**Critical Path:** Multiple HTTP requests → Shared state access → Response generation

**Risk Mitigation:**
- Rust ownership prevents data races at compile time
- Mutex ensures atomic state operations
- Arc enables safe sharing across threads
- Clear error handling for lock acquisition failures

## Scalability Considerations

### Current Bottlenecks
1. **Single Mutex:** All state access serialized
2. **In-Memory Storage:** Lost on restart, limited by RAM
3. **No Connection Pooling:** Each request creates new connections

### Scaling Strategies (Future)
1. **Read-Write Locks:** Separate read/write access patterns
2. **Database Storage:** Persistent, horizontally scalable storage
3. **Caching Layer:** Redis for session data
4. **Load Balancing:** Multiple server instances

### Design Decisions Supporting Scale
- **Stateless Handlers:** Easy to horizontally scale
- **UUID-based IDs:** No coordination needed for ID generation
- **JSON APIs:** Language-agnostic integration
- **Clear Interfaces:** Easy to extract to microservices
