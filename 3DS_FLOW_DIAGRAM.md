# 3DS Authentication Flow Diagrams

## Complete Transaction Flow

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│     Client      │    │   Mock 3DS      │    │   Shared        │
│   (Postman)     │    │    Server       │    │   State         │
└─────────────────┘    └─────────────────┘    └─────────────────┘
         │                       │                       │
         │                       │                       │
    ┌────▼────┐                  │                       │
    │ Step 1: │                  │                       │
    │ Version │                  │                       │
    │  Call   │                  │                       │
    └────┬────┘                  │                       │
         │                       │                       │
         │ POST /3ds/version     │                       │
         │ {"cardNumber": "..."}  │                       │
         ├──────────────────────►│                       │
         │                       │                       │
         │                       ├─ Generate UUID        │
         │                       ├─ Check card prefix    │
         │                       ├─ Create card ranges   │
         │                       │                       │
         │ {"threeDSServerTransID"│                      │
         │  "cardRanges": [...]} │                       │
         │◄──────────────────────┤                       │
         │                       │                       │
    ┌────▼────┐                  │                       │
    │ Step 2: │                  │                       │
    │  Auth   │                  │                       │
    │  Call   │                  │                       │
    └────┬────┘                  │                       │
         │                       │                       │
         │ POST /3ds/authenticate │                       │
         │ {transaction details}  │                       │
         ├──────────────────────►│                       │
         │                       │                       │
         │                       ├─ Check card ending    │
         │                       ├─ Generate IDs         │
         │                       ├─ Create auth response │
         │                       │                       │
         │                       │   Store Transaction   │
         │                       ├──────────────────────►│
         │                       │   {auth_req, IDs}     │
         │                       │                       │
         │ {authentication data,  │                       │
         │  challenge_request}   │                       │
         │◄──────────────────────┤                       │
         │                       │                       │
    ┌────▼────┐                  │                       │
    │ Step 3: │                  │                       │
    │ Results │                  │                       │
    │  Call   │                  │                       │
    └────┬────┘                  │                       │
         │                       │                       │
         │ POST /3ds/results     │                       │
         │ {auth results}        │                       │
         ├──────────────────────►│                       │
         │                       │                       │
         │                       │   Find Transaction    │
         │                       ├──────────────────────►│
         │                       │   by TransID          │
         │                       │                       │
         │                       │   Update with Results │
         │                       ├──────────────────────►│
         │                       │   store results_req   │
         │                       │                       │
         │ {"resultsStatus": "01"}│                      │
         │◄──────────────────────┤                       │
         │                       │                       │
    ┌────▼────┐                  │                       │
    │ Step 4: │                  │                       │
    │ Final   │                  │                       │
    │  Call   │                  │                       │
    └────┬────┘                  │                       │
         │                       │                       │
         │ POST /3ds/final       │                       │
         │ {"threeDSServerTransID"}                      │
         ├──────────────────────►│                       │
         │                       │                       │
         │                       │   Retrieve Complete   │
         │                       ├──────────────────────►│
         │                       │   Transaction Data    │
         │                       │                       │
         │ {final auth package}  │                       │
         │◄──────────────────────┤                       │
         │                       │                       │
```

## Data Structures Flow

### 1. Version Call Data Flow
```
Input JSON → VersionRequest Struct → Handler Logic → VersionResponse Struct → Output JSON

{                    VersionRequest {           Generate UUID        VersionResponse {        {
  "cardNumber":        card_number: String,  ──► Check prefix    ──►   three_ds_server_      "threeDSServerTransID":
  "5155..."          }                           Create ranges         trans_id: Uuid,       "abc-123...",
}                                                                     card_ranges: Vec[]     "cardRanges": [...]
                                                                   }                         }
```

### 2. Authentication Call Data Flow
```
Large JSON → AuthenticateRequest → Business Logic → Store State → AuthenticateResponse → JSON

{              AuthenticateRequest {     Check card ending        TransactionData {      AuthenticateResponse {     {
  "threeDSS...   three_ds_server_      ──► (4001 vs 4000)    ──► auth_request: ...,   ──► purchase_date: ...,    ──► "purchaseDate":
  "cardholder    trans_id: Uuid,          Generate IDs           acs_trans_id: ...,      auth_response: ...,        "transStatus": 
  Account": {    cardholder_account:      Create response        ds_trans_id: ...,       trans_status: ...,         "base64Encoded...
  ...            CardholderAccount,       Base64 encode          ...                     ...                        ...
}              ...                    }                      }                      }                         }
               }
```

### 3. Results Call Data Flow
```
JSON → ResultsRequest → Find Transaction → Update State → ResultsResponse → JSON

{              ResultsRequest {        state.get_mut()        transaction.results_    ResultsResponse {         {
  "acsTransID"   acs_trans_id: Uuid,  ──► (transaction_id) ──► request = Some(req) ──► ds_trans_id: Uuid,    ──► "dsTransID":
  "transStatus"  trans_status: String,                                                 message_type: String,     "messageType":
  ...            ...                                                                   results_status: String,   "resultsStatus"
}              }                                                                     }                         }
```

### 4. Final Call Data Flow
```
Simple JSON → FinalRequest → Retrieve Complete State → Combine Data → FinalResponse → JSON

{              FinalRequest {          state.get()            Combine:               FinalResponse {           {
  "threeDSS... three_ds_server_     ──► (transaction_id)  ──► - auth data        ──► eci: String,         ──► "eci": "02",
  TransID":    trans_id: Uuid        }                       - results data         authentication_value:     "authenticationValue"
  "..."      }                                               - all IDs              results_response: ...,     "resultsResponse": {
}                                                                                   results_request: ...,       "resultsRequest": {
                                                                                    trans_status: String        "transStatus": "Y"
                                                                                  }                           }
```

## State Management Visualization

```
┌─────────────────────────────────────────────────────────────────────┐
│                           Shared State                             │
│                    HashMap<Uuid, TransactionData>                  │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  Transaction ID: abc-123-def-456                                   │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │ TransactionData {                                           │   │
│  │   authenticate_request: AuthenticateRequest { ... },       │   │
│  │   acs_trans_id: xyz-789-ghi-012,                          │   │
│  │   ds_trans_id: mno-345-pqr-678,                           │   │
│  │   sdk_trans_id: stu-901-vwx-234,                          │   │
│  │   results_request: None  ← Initially empty                 │   │
│  │ }                                                           │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                                                                     │
│  After /3ds/results call:                                         │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │ TransactionData {                                           │   │
│  │   authenticate_request: AuthenticateRequest { ... },       │   │
│  │   acs_trans_id: xyz-789-ghi-012,                          │   │
│  │   ds_trans_id: mno-345-pqr-678,                           │   │
│  │   sdk_trans_id: stu-901-vwx-234,                          │   │
│  │   results_request: Some(ResultsRequest { ... }) ← Updated │   │
│  │ }                                                           │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

## Threading and Concurrency

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│  Request A  │     │  Request B  │     │  Request C  │
│ (Thread 1)  │     │ (Thread 2)  │     │ (Thread 3)  │
└─────┬───────┘     └─────┬───────┘     └─────┬───────┘
      │                   │                   │
      ▼                   ▼                   ▼
┌─────────────────────────────────────────────────────┐
│                 Actix-Web Server                    │
│              (Handles Multiple Threads)             │
└─────────────────────┬───────────────────────────────┘
                      │
                      ▼
┌─────────────────────────────────────────────────────┐
│              Shared State (Arc<Mutex<...>>)         │
│                                                     │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐ │
│  │   Thread 1  │  │   Thread 2  │  │   Thread 3  │ │
│  │    waits    │  │   accesses  │  │    waits    │ │
│  │   (mutex)   │  │    data     │  │   (mutex)   │ │
│  └─────────────┘  └─────────────┘  └─────────────┘ │
│                                                     │
│  Only one thread can modify state at a time        │
└─────────────────────────────────────────────────────┘
```

## Error Handling Flow

```
Request → Handler → Result<Success, Error>

┌─────────────┐
│   Request   │
└─────┬───────┘
      │
      ▼
┌─────────────┐     ┌─────────────────┐     ┌─────────────────┐
│   Handler   │────►│ Transaction     │────►│ Success:        │
│             │     │ Found?          │     │ HttpResponse    │
└─────────────┘     └─────┬───────────┘     │ ::Ok()          │
                          │                 └─────────────────┘
                          │ No
                          ▼
                    ┌─────────────────┐
                    │ Error:          │
                    │ HttpResponse    │
                    │ ::BadRequest()  │
                    └─────────────────┘
```

## Card Number Decision Logic

```
Card Number → Check Ending → Determine Flow

┌─────────────────┐
│ Card Number     │
│ (last 4 digits)│
└─────┬───────────┘
      │
      ▼
┌─────────────────┐
│ ends_with()?    │
├─────────────────┤
│                 │
│  "4001" ────────┼──► Challenge Flow
│                 │    (transStatus: "C")
│                 │    ┌─────────────────┐
│                 │    │ Requires /3ds/  │
│                 │    │ results call    │
│                 │    └─────────────────┘
│                 │
│  "4000" ────────┼──► Frictionless Flow  
│                 │    (transStatus: "Y")
│                 │    ┌─────────────────┐
│                 │    │ Can skip to     │
│                 │    │ /3ds/final      │
│                 │    └─────────────────┘
│                 │
│  Other ─────────┼──► Challenge Flow
│                 │    (default behavior)
└─────────────────┘
```

## Memory Layout

```
Stack (per request):          Heap (shared):
┌─────────────────┐          ┌─────────────────────────────┐
│ Handler         │          │ Arc<Mutex<HashMap<...>>>    │
│ Variables:      │          │                             │
│ - trans_id      │          │ ┌─────────────────────────┐ │
│ - card_number   │          │ │ Transaction Data        │ │
│ - is_challenge  │          │ │ - UUID: abc-123         │ │
│ - response      │    ────► │ │ - AuthRequest: {...}    │ │
└─────────────────┘          │ │ - IDs: [xyz, mno, stu]  │ │
                             │ │ - Results: Option<...>  │ │
                             │ └─────────────────────────┘ │
                             │                             │
                             │ ┌─────────────────────────┐ │
                             │ │ Transaction Data        │ │
                             │ │ - UUID: def-456         │ │
                             │ │ - AuthRequest: {...}    │ │
                             │ │ - IDs: [ghi, jkl, mno]  │ │
                             │ │ - Results: Some(...)    │ │
                             │ └─────────────────────────┘ │
                             └─────────────────────────────┘
