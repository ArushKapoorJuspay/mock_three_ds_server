# API Specifications

## Complete API Flow

### Implementation Context
These API specifications were derived from the original user requirements for a 3DS mock server. Each endpoint was carefully designed to simulate real 3DS authentication flows while maintaining educational clarity.

### 1. Version Check Endpoint
**URL:** `POST /3ds/version`
**Purpose:** Initialize transaction and check card capabilities

**Request:**
```json
{
  "cardNumber": "5155016800000000000"
}
```

**Response:**
```json
{
  "threeDSServerTransID": "29bf9634-b810-420d-bd8e-25072ce602f5",
  "cardRanges": [
    {
      "acsInfoInd": ["01", "02"],
      "startRange": "5155010000000000",
      "acsEndProtocolVersion": "2.2.0",
      "acsStartProtocolVersion": "2.2.0",
      "endRange": "5155019999999999"
    }
  ]
}
```

**Business Logic:**
- Generate UUID for `threeDSServerTransID` using Rust's uuid crate
- If card starts with "515501" → specific range (5155010000000000-5155019999999999)
- Otherwise → default range (4000000000000000-4999999999999999)
- Transaction ID persisted in Arc<Mutex<HashMap>> state for subsequent calls

**Implementation Notes:**
- UUID v4 ensures uniqueness across all transactions
- State not stored at this stage - just transaction ID generation
- Card range logic demonstrates business rule implementation

### 2. Authentication Endpoint
**URL:** `POST /3ds/authenticate`
**Purpose:** Start authentication process with challenge/frictionless decision

**Key Request Fields:**
```json
{
  "threeDSServerTransID": "{{from version call}}",
  "cardholderAccount": {
    "acctNumber": "4000400040004001",
    "cardExpiryDate": "3107",
    "cardSecurityCode": "166"
  },
  "purchase": {
    "purchaseAmount": 100,
    "purchaseCurrency": "356",
    "purchaseDate": "20240919034416"
  },
  "cardholder": {
    "cardholderName": "John Doe",
    "email": "netcetera@example.com"
  }
}
```

**Response Structure:**
```json
{
  "purchaseDate": "20240919034416",
  "base64EncodedChallengeRequest": "eyJ...",
  "acsURL": "https://...",
  "threeDSServerTransID": "...",
  "authenticationResponse": { /* ARes data */ },
  "challengeRequest": { /* CReq data */ },
  "acsChallengeMandated": "Y",
  "transStatus": "C",
  "authenticationRequest": { /* Complete request data */ }
}
```

**Business Logic:**
- Card ending in "4001" → Challenge flow (`transStatus: "C"`)
- Card ending in "4000" → Frictionless flow (`transStatus: "Y"`)
- Generate `acs_trans_id`, `ds_trans_id`, `sdk_trans_id`
- Store complete transaction data in state
- Create base64-encoded challenge request
- Build comprehensive authentication response

### 3. Results Endpoint
**URL:** `POST /3ds/results`
**Purpose:** Submit authentication results after challenge completion

**Request:**
```json
{
  "acsTransID": "c9fc760d-2278-4bb9-8e43-d6818eff7146",
  "messageCategory": "01",
  "eci": "02",
  "messageType": "RReq",
  "acsRenderingType": {
    "acsUiTemplate": "01",
    "acsInterface": "02"
  },
  "dsTransID": "5f4497a7-d9a8-4429-be80-5f349db2a83a",
  "authenticationMethod": "10",
  "authenticationType": "02",
  "messageVersion": "2.2.0",
  "sdkTransID": "6b3793c1-090c-4160-803e-9594ca9413e8",
  "interactionCounter": "01",
  "authenticationValue": "xgQYYgZVAAAAAAAAAAAAAAAAAAAA",
  "transStatus": "Y",
  "threeDSServerTransID": "{{transaction_id}}"
}
```

**Response:**
```json
{
  "dsTransID": "5f4497a7-d9a8-4429-be80-5f349db2a83a",
  "messageType": "RRes",
  "threeDSServerTransID": "69bf64ce-a30e-4904-8d14-f521f0928a27",
  "acsTransID": "c9fc760d-2278-4bb9-8e43-d6818eff7146",
  "sdkTransID": "6b3793c1-090c-4160-803e-9594ca9413e8",
  "resultsStatus": "01",
  "messageVersion": "2.2.0"
}
```

**Business Logic:**
- Find transaction by `threeDSServerTransID`
- Update transaction state with results request
- Return confirmation response
- Error if transaction not found

### 4. Final Endpoint
**URL:** `POST /3ds/final`
**Purpose:** Retrieve complete authentication package

**Request:**
```json
{
  "threeDSServerTransID": "69bf64ce-a30e-4904-8d14-f521f0928a27"
}
```

**Response:**
```json
{
  "eci": "02",
  "authenticationValue": "xgQYYgZVAAAAAAAAAAAAAAAAAAAA",
  "threeDSServerTransID": "69bf64ce-a30e-4904-8d14-f521f0928a27",
  "resultsResponse": {
    "dsTransID": "5f4497a7-d9a8-4429-be80-5f349db2a83a",
    "messageType": "RRes",
    "resultsStatus": "01",
    "messageVersion": "2.2.0"
  },
  "resultsRequest": {
    "eci": "02",
    "authenticationValue": "xgQYYgZVAAAAAAAAAAAAAAAAAAAA",
    "messageType": "RReq",
    "transStatus": "Y",
    "messageVersion": "2.2.0"
  },
  "transStatus": "Y"
}
```

**Business Logic:**
- Find transaction by ID
- Verify results were submitted
- Combine authentication and results data
- Return complete authentication package

## Error Responses

### Transaction Not Found
```json
{
  "error": "Transaction not found"
}
```
**HTTP Status:** 400 Bad Request

### Results Not Available
```json
{
  "error": "Results not found for this transaction"
}
```
**HTTP Status:** 400 Bad Request

## State Transitions

```
Version → Authentication → Results → Final
   ↓           ↓            ↓         ↓
Generate    Store Data   Update     Retrieve
Trans ID    in State     Results    Complete
```

## Testing Scenarios

### Challenge Flow Test
1. Use card ending in "4001"
2. Expect `transStatus: "C"` in authentication
3. Submit results with `transStatus: "Y"`
4. Get final package with complete data

### Frictionless Flow Test
1. Use card ending in "4000"
2. Expect `transStatus: "Y"` in authentication
3. Can skip directly to final (or submit results)
4. Get final package

### Card Range Test
1. Use card starting with "515501"
2. Expect specific range in version response
3. All other cards get default range

## HTTP Headers
- **Content-Type:** `application/json`
- **Accept:** `application/json`

## Response Codes
- **200 OK:** Successful operation
- **400 Bad Request:** Invalid request or missing transaction
- **500 Internal Server Error:** Server error (shouldn't happen)

## Request Validation
- JSON structure validation via Serde
- Required fields enforced at compile time
- UUID format validation automatic
- No additional business validation (mock server)
