# Complete Guide to Understanding the Rust 3DS Mock Server

## Table of Contents
1. [What is Rust?](#what-is-rust)
2. [What is 3D Secure (3DS)?](#what-is-3d-secure-3ds)
3. [Project Overview](#project-overview)
4. [File-by-File Breakdown](#file-by-file-breakdown)
5. [Understanding the Data Flow](#understanding-the-data-flow)
6. [Key Rust Concepts Explained](#key-rust-concepts-explained)
7. [How the APIs Work Together](#how-the-apis-work-together)

---

## What is Rust?

Rust is a systems programming language that focuses on **safety**, **speed**, and **concurrency**. Think of it as a language that:
- Prevents crashes and security vulnerabilities at compile time
- Runs as fast as C/C++ but is much safer
- Has excellent support for building web servers and APIs

### Key Rust Concepts You'll See:
- **Structs**: Like classes in other languages, they group related data together
- **Enums**: Types that can be one of several variants
- **Traits**: Like interfaces, they define what a type can do
- **Ownership**: Rust's unique way of managing memory safely
- **Modules**: Ways to organize code into separate files

---

## What is 3D Secure (3DS)?

3D Secure is a security protocol for online credit card payments. Think of it like this:

1. **Customer** tries to pay with their card
2. **Merchant** needs to verify the card is legitimate
3. **3DS Server** acts as a middleman to authenticate the transaction
4. **Bank** (ACS - Access Control Server) verifies the customer

Our mock server simulates the **3DS Server** part of this flow.

---

## Project Overview

Our project creates a **mock 3DS server** that pretends to be a real payment authentication system. This is useful for:
- Testing payment systems without involving real banks
- Development and integration testing
- Understanding how 3DS works

### What Our Server Does:
1. **Version Check**: "What authentication methods does this card support?"
2. **Authentication**: "Let's start authenticating this payment"
3. **Results**: "Here are the authentication results"
4. **Final**: "Give me the final authentication proof"

---

## File-by-File Breakdown

### 1. `Cargo.toml` - The Project Configuration

```toml
[package]
name = "mock_three_ds_server"
version = "0.1.0"
edition = "2021"

[dependencies]
actix-web = "4"
serde = { version = "1.0", features = ["derive"] }
# ... more dependencies
```

**What this does:**
- `[package]`: Basic info about our project
- `[dependencies]`: External libraries we're using
  - `actix-web`: Web framework for building HTTP APIs
  - `serde`: For converting between JSON and Rust data structures
  - `uuid`: For generating unique IDs
  - `chrono`: For handling dates and times

**Think of it like:** A recipe list for building our project

---

### 2. `src/models.rs` - The Data Structures

This file defines all the **data shapes** our API uses. In Rust, we use `struct` to define these shapes.

#### Example: Version Request
```rust
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionRequest {
    pub card_number: String,
}
```

**Breaking this down:**
- `#[derive(Debug, Deserialize)]`: Automatically generates code for:
  - `Debug`: Lets us print this struct for debugging
  - `Deserialize`: Converts JSON into this Rust struct
- `#[serde(rename_all = "camelCase")]`: Converts `card_number` to `cardNumber` in JSON
- `pub struct`: A public data structure (other files can use it)
- `pub card_number: String`: A public field that holds text

#### More Complex Example: Authentication Request
```rust
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AuthenticateRequest {
    pub three_ds_server_trans_id: Uuid,
    pub device_channel: String,
    pub cardholder_account: CardholderAccount,
    pub purchase: Purchase,
    // ... many more fields
}
```

**What's happening:**
- `Clone`: Allows us to make copies of this struct
- `Uuid`: A unique identifier type (like `f47ac10b-58cc-4372-a567-0e02b2c3d479`)
- `CardholderAccount`, `Purchase`: These are other structs defined in the same file

**Think of models as:** Templates that define what data looks like, similar to forms with specific fields

---

### 3. `src/state.rs` - Managing Memory Between Requests

```rust
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct TransactionData {
    pub authenticate_request: AuthenticateRequest,
    pub acs_trans_id: Uuid,
    pub ds_trans_id: Uuid,
    pub sdk_trans_id: Uuid,
    pub results_request: Option<ResultsRequest>,
}

pub type AppState = Arc<Mutex<HashMap<Uuid, TransactionData>>>;
```

**What this does:**
- `HashMap<Uuid, TransactionData>`: Like a dictionary where we store transaction data using the transaction ID as the key
- `Mutex`: Ensures only one request can modify the data at a time (thread safety)
- `Arc`: Allows multiple parts of our program to share the same data safely
- `Option<ResultsRequest>`: This field might contain a `ResultsRequest` or might be empty (`None`)

**Think of it like:** A shared notebook where we write down transaction details, but only one person can write in it at a time

---

### 4. `src/handlers.rs` - The Business Logic

This file contains the actual functions that handle each API endpoint.

#### Version Handler (Simplest Example)
```rust
pub async fn version_handler(req: web::Json<VersionRequest>) -> Result<HttpResponse> {
    // Generate a new transaction ID
    let trans_id = Uuid::new_v4();
    
    // Check card number and create appropriate response
    let card_range = if req.card_number.starts_with("515501") {
        CardRange {
            acs_info_ind: vec!["01".to_string(), "02".to_string()],
            start_range: "5155010000000000".to_string(),
            // ... more fields
        }
    } else {
        // Default range for other cards
        CardRange { /* ... */ }
    };

    let response = VersionResponse {
        three_ds_server_trans_id: trans_id,
        card_ranges: vec![card_range],
    };

    Ok(HttpResponse::Ok().json(response))
}
```

**Breaking this down:**
- `pub async fn`: A public asynchronous function (can handle multiple requests at once)
- `web::Json<VersionRequest>`: Automatically converts incoming JSON to our `VersionRequest` struct
- `Uuid::new_v4()`: Generates a random unique ID
- `req.card_number.starts_with("515501")`: Checks if the card number starts with those digits
- `vec![]`: Creates a list/array
- `HttpResponse::Ok().json(response)`: Sends back a successful HTTP response with JSON

**Think of handlers as:** Restaurant cooks who take orders (requests) and prepare specific dishes (responses)

#### Authentication Handler (More Complex)
```rust
pub async fn authenticate_handler(
    req: web::Json<AuthenticateRequest>,
    state: web::Data<AppState>,
) -> Result<HttpResponse> {
    let card_number = &req.cardholder_account.acct_number;
    let is_challenge = card_number.ends_with("4001");
    let trans_status = if is_challenge { "C" } else { "Y" };
    
    // Store transaction data for later use
    let transaction_data = TransactionData { /* ... */ };
    state.lock().unwrap().insert(three_ds_server_trans_id, transaction_data);
    
    // Create and return response
    let response = AuthenticateResponse { /* ... */ };
    Ok(HttpResponse::Ok().json(response))
}
```

**Key concepts:**
- `&req.cardholder_account.acct_number`: Gets a reference to the card number (borrowing, not taking ownership)
- `is_challenge`: Boolean (true/false) that determines if we need extra authentication
- `state.lock().unwrap()`: Safely access our shared transaction storage
- `.insert()`: Add new transaction data to our storage

---

### 5. `src/main.rs` - The Server Setup

```rust
#![recursion_limit = "256"]

mod handlers;
mod models;
mod state;

use actix_web::{middleware, web, App, HttpServer};
use state::create_app_state;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Initialize logging
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    // Create shared application state
    let app_state = create_app_state();

    println!("Starting 3DS Mock Server on http://localhost:8080");

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(app_state.clone()))
            .wrap(middleware::Logger::default())
            .route("/3ds/version", web::post().to(handlers::version_handler))
            .route("/3ds/authenticate", web::post().to(handlers::authenticate_handler))
            .route("/3ds/results", web::post().to(handlers::results_handler))
            .route("/3ds/final", web::post().to(handlers::final_handler))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
```

**What this does:**
- `mod`: Imports other files in our project
- `#[actix_web::main]`: Tells Rust this is our main async function
- `env_logger::init_from_env()`: Sets up logging so we can see what's happening
- `create_app_state()`: Creates our shared transaction storage
- `HttpServer::new()`: Creates a new web server
- `.route()`: Maps URLs to handler functions
- `.bind("127.0.0.1:8080")`: Listen on localhost port 8080
- `.run().await`: Start the server and wait forever

**Think of main.rs as:** The restaurant manager who sets up the kitchen, assigns cooks to stations, and opens the restaurant

---

## Understanding the Data Flow

Let's trace what happens when someone calls our API:

### 1. Version Call Flow
```
POST /3ds/version
{
  "cardNumber": "5155016800000000000"
}
```

**Step by step:**
1. **HTTP Request arrives** at our server
2. **Actix-web framework** sees it's a POST to `/3ds/version`
3. **Routes to version_handler** function
4. **JSON gets converted** to `VersionRequest` struct automatically
5. **Handler logic runs**:
   - Generate new transaction ID
   - Check card number prefix
   - Create appropriate card range
6. **Response created** and converted to JSON
7. **HTTP Response sent** back to client

### 2. Authentication Call Flow
```
POST /3ds/authenticate
{
  "threeDSServerTransID": "...",
  "cardholderAccount": {
    "acctNumber": "4000400040004001"
  },
  // ... lots more data
}
```

**Step by step:**
1. **Request arrives** with transaction ID and card details
2. **Routes to authenticate_handler**
3. **JSON converted** to `AuthenticateRequest` struct
4. **Business logic**:
   - Check if card ends in "4001" (challenge) or "4000" (frictionless)
   - Generate various IDs (acs_trans_id, ds_trans_id, etc.)
   - Create complex response with authentication details
   - **Store transaction data** in shared state for later use
5. **Response sent** with authentication details

### 3. Results Call Flow
```
POST /3ds/results
{
  "threeDSServerTransID": "...",
  "transStatus": "Y",
  "authenticationValue": "...",
  // ... more result data
}
```

**Step by step:**
1. **Request arrives** with results from authentication
2. **Routes to results_handler**
3. **Look up transaction** in shared state using transaction ID
4. **Store results data** in the transaction record
5. **Send back confirmation** that results were received

### 4. Final Call Flow
```
POST /3ds/final
{
  "threeDSServerTransID": "..."
}
```

**Step by step:**
1. **Request arrives** asking for final authentication proof
2. **Routes to final_handler**
3. **Look up transaction** and its stored results
4. **Combine all data** from authentication and results
5. **Send back complete** authentication package

---

## Key Rust Concepts Explained

### 1. Ownership and Borrowing
```rust
let card_number = &req.cardholder_account.acct_number;  // Borrowing
let trans_id = Uuid::new_v4();  // Ownership
```
- `&` means "borrow" - look at the data but don't take it
- Without `&`, we would "move" the data (take ownership)

### 2. Option Types
```rust
pub results_request: Option<ResultsRequest>,
```
- `Option<T>` means "maybe has a value, maybe doesn't"
- `Some(value)` when there is a value
- `None` when there isn't
- This prevents null pointer errors!

### 3. Result Types
```rust
-> Result<HttpResponse>
```
- `Result<T, E>` means "either success (Ok) or error (Err)"
- Forces you to handle errors explicitly
- Much safer than exceptions

### 4. Pattern Matching
```rust
if let Some(transaction_data) = state_guard.get(&three_ds_server_trans_id) {
    // Use transaction_data
} else {
    // Handle case where transaction not found
}
```
- Check what variant of an enum/Option we have
- Extract values safely

### 5. Async/Await
```rust
pub async fn version_handler(...) -> Result<HttpResponse> {
```
- `async`: This function can be paused and resumed
- Allows handling many requests simultaneously
- Like JavaScript's async/await

---

## How the APIs Work Together

### The Complete 3DS Flow

```
1. Client calls /3ds/version
   ↓
   Server generates transaction ID
   ↓
   Client receives transaction ID + card ranges

2. Client calls /3ds/authenticate with transaction ID
   ↓
   Server checks card number:
   - Ends in 4001? → Challenge required (status "C")
   - Ends in 4000? → Frictionless (status "Y")
   ↓
   Server stores transaction data
   ↓
   Client receives authentication response

3. (If challenge required) Client calls /3ds/results
   ↓
   Server finds transaction by ID
   ↓
   Server updates transaction with results
   ↓
   Client receives confirmation

4. Client calls /3ds/final
   ↓
   Server finds transaction + results
   ↓
   Server combines all data
   ↓
   Client receives final authentication package
```

### State Management Across Calls

```rust
// Call 1: Store initial data
state.insert(trans_id, TransactionData { ... });

// Call 2: Retrieve and update
if let Some(transaction) = state.get_mut(&trans_id) {
    transaction.results_request = Some(results);
}

// Call 3: Retrieve everything
if let Some(transaction) = state.get(&trans_id) {
    // Use all stored data
}
```

### Why This Design?

1. **Stateful**: We remember transaction details between calls
2. **Thread-safe**: Multiple requests can't corrupt each other's data
3. **Type-safe**: Rust prevents us from mixing up data types
4. **Error-safe**: We handle missing transactions gracefully

---

## Real-World Parallels

Think of our 3DS server like a **restaurant reservation system**:

1. **Version call**: "Do you have tables for 4 people?" (Check capabilities)
2. **Authenticate call**: "I'd like to make a reservation" (Start the process)
3. **Results call**: "I've confirmed with my party" (Provide additional info)
4. **Final call**: "Give me my confirmation details" (Get final proof)

The **shared state** is like the restaurant's reservation book - it remembers all the details between phone calls.

The **handlers** are like different restaurant staff who specialize in different types of requests.

The **models** are like the forms the restaurant uses to record consistent information.

---

## Summary

This Rust 3DS mock server demonstrates several important concepts:

1. **Web API design**: Clear endpoints with specific purposes
2. **Data modeling**: Structured representations of complex payment data
3. **State management**: Remembering information across multiple API calls
4. **Type safety**: Rust's type system prevents many common bugs
5. **Concurrency**: Handling multiple requests safely and efficiently

The server simulates a real 3DS authentication flow while being completely safe for testing and development purposes. It shows how Rust's features like ownership, type safety, and pattern matching create robust, reliable systems.
