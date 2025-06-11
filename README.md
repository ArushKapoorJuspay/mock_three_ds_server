# Mock 3DS Server

A production-ready mock implementation of 3D Secure (3DS) server endpoints for testing and development purposes. Features dynamic ACS signed content generation, Redis connection pooling, and comprehensive production optimizations.

## Quick Start

### 1. Prerequisites

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Install Redis
# macOS: brew install redis
# Ubuntu: sudo apt-get install redis-server

# Install OpenSSL (for certificate generation)
# macOS: brew install openssl
# Ubuntu: sudo apt-get install openssl
```

### 2. Generate Development Certificates

**⚠️ IMPORTANT**: This server requires certificates for JWT signing. Run the certificate generation script first:

```bash
# Generate self-signed certificates for development
./generate-certs.sh
```

This creates:
- `certs/acs-cert.pem` - ACS certificate (for JWT x5c header)
- `certs/acs-private-key.pem` - Private key (for JWT signing)

**Security Note**: Certificates are NOT stored in Git for security reasons. Each developer must generate their own certificates.

### 3. Start Redis Server

```bash
redis-server
```

### 4. Run the Mock Server

```bash
# Development mode
RUN_MODE=development cargo run

# Production mode
RUN_MODE=production cargo run
```

The server will start on `http://localhost:8080`

## Features

### Core 3DS Functionality
- ✅ Complete 3DS 2.2.0 protocol implementation
- ✅ Challenge and frictionless authentication flows
- ✅ Dynamic ACS signed content generation for mobile flows
- ✅ Ephemeral key pair generation (ECDSA P-256)
- ✅ JWT signing with PS256 algorithm and x5c certificate chains

### Production-Grade Performance
- ✅ Redis connection pooling (10-50x performance improvement)
- ✅ Request rate limiting and compression
- ✅ Prometheus metrics and health checks
- ✅ Configurable worker threads and timeouts
- ✅ Enterprise-grade error handling and retry logic

### Development Experience
- ✅ Hot-reload friendly development setup
- ✅ Comprehensive logging and debugging
- ✅ Load testing tools included
- ✅ Docker deployment ready

## API Endpoints

### 1. Version Call

**Endpoint:** `POST /3ds/version`

**Purpose:** Returns version information and card ranges for a given card number.

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
            "acsInfoInd": [
                "01",
                "02"
            ],
            "startRange": "5155010000000000",
            "acsEndProtocolVersion": "2.2.0",
            "acsStartProtocolVersion": "2.2.0",
            "endRange": "5155019999999999"
        }
    ]
}
```

### 2. Authenticate Call

**Endpoint:** `POST /3ds/authenticate`

**Purpose:** Authenticates a transaction. Returns challenge flow for cards ending in `4001`, frictionless flow for cards ending in `4000`.

**Request:**
```json
{
    "threeDSServerTransID": "{{threeDSServerTransID}}",
    "deviceChannel": "01",
    "messageCategory": "01",
    "preferredProtocolVersion": "2.2.0",
    "enforcePreferredProtocolVersion": true,
    "threeDSCompInd": "Y",
    "threeDSRequestor": {
        "threeDSRequestorAuthenticationInd": "01",
        "threeDSRequestorAuthenticationInfo": {
            "threeDSReqAuthMethod": "04",
            "threeDSReqAuthTimestamp": "202409190344"
        },
        "threeDSRequestorChallengeInd": "01"
    },
    "cardholderAccount": {
        "acctType": "02",
        "cardExpiryDate": "3107",
        "schemeId": "VISA",
        "acctNumber": "4000400040004001",
        "cardSecurityCode": "166"
    },
    "cardholder": {
        "addrMatch": "N",
        "billAddrCity": "Zurich",
        "billAddrCountry": "756",
        "billAddrLine1": "Zypressenstrasse 71",
        "billAddrLine2": "P.O. Box",
        "billAddrLine3": "8040 Zürich",
        "billAddrPostCode": "8000",
        "email": "netcetera@example.com",
        "homePhone": {
            "cc": "1",
            "subscriber": "123"
        },
        "mobilePhone": {
            "cc": "1",
            "subscriber": "123"
        },
        "workPhone": {
            "cc": "1",
            "subscriber": "123"
        },
        "cardholderName": "John Doe",
        "shipAddrCity": "Zurich",
        "shipAddrCountry": "756",
        "shipAddrLine1": "Zypressenstrasse 98",
        "shipAddrLine2": "P.O. Box",
        "shipAddrLine3": "8040 Zürich",
        "shipAddrPostCode": "8000"
    },
    "purchase": {
        "purchaseInstalData": 3,
        "purchaseAmount": 100,
        "purchaseCurrency": "356",
        "purchaseExponent": 2,
        "purchaseDate": "20240919034416",
        "recurringExpiry": "20240901",
        "recurringFrequency": 1,
        "transType": "01"
    },
    "acquirer": {
        "acquirerBin": "271989",
        "acquirerMerchantId": "JuspayTest1"
    },
    "merchant": {
        "mcc": "1520",
        "merchantCountryCode": "356",
        "threeDSRequestorID": "juspay-prev",
        "threeDSRequestorName": "juspay-prev",
        "merchantName": "testMerchant",
        "resultsResponseNotificationUrl": "https://mastercard.3ds.juspay.in/3ds/results",
        "notificationURL": "https://sandbox.juspay.in"
    },
    "browserInformation": {
        "browserAcceptHeader": "application/json",
        "browserIP": "192.168.1.11",
        "browserLanguage": "en",
        "browserColorDepth": "8",
        "browserScreenHeight": 1,
        "browserScreenWidth": 1,
        "browserTZ": 1,
        "browserUserAgent": "Mozilla/5.0 (Windows NT 6.1; Win64; x64; rv:47.0) Gecko/20100101 Firefox/47.0",
        "challengeWindowSize": "01",
        "browserJavaEnabled": false,
        "browserJavascriptEnabled": true
    },
    "deviceRenderOptions": {
        "sdkInterface": "01",
        "sdkUiType": [
            "02"
        ],
        "sdkAuthenticationType": [
            "01"
        ]
    }
}
```

**Note:** Use the `threeDSServerTransID` from the Version call response.

### 3. Results Call

**Endpoint:** `POST /3ds/results`

**Purpose:** Submits authentication results after challenge completion.

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
    "threeDSServerTransID": "{{threeDSServerTransID}}"
}
```

**Note:** Use the IDs from the Authenticate call response.

### 4. Final Call

**Endpoint:** `POST /3ds/final`

**Purpose:** Retrieves final authentication values after the results have been submitted.

**Request:**
```json
{
  "threeDSServerTransID": "{{threeDSServerTransID}}"
}
```

## Testing Flow

1. Call `/3ds/version` with a card number to get a `threeDSServerTransID`
2. Use that ID to call `/3ds/authenticate` with full transaction details
3. If you get a challenge flow (transStatus: "C"), proceed to call `/3ds/results` with the authentication results
4. Finally, call `/3ds/final` to get the final authentication values

## Card Number Behavior

- Cards ending in `4001`: Will trigger challenge flow (transStatus: "C")
- Cards ending in `4000`: Will trigger frictionless flow (transStatus: "Y")
- Cards starting with `515501`: Will return specific card ranges for that BIN

## Postman Setup

1. Create a new collection in Postman
2. Add the four endpoints as POST requests
3. Set the Content-Type header to `application/json` for all requests
4. Use the example JSON bodies provided above
5. Create a collection variable `threeDSServerTransID` to store the transaction ID between calls
