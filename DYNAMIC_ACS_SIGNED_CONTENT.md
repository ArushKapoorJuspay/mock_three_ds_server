# Dynamic ACS Signed Content Implementation

## Overview

This implementation adds dynamic generation of `acs_signed_content` for mobile friction flows in the 3DS mock server, replacing the previously hardcoded JWT with a dynamically generated one based on the JavaScript reference implementation.

## Implementation Details

### Key Components Added

1. **Crypto Module (`src/crypto.rs`)**
   - `generate_ephemeral_key_pair()`: Generates ECDSA P-256 ephemeral key pairs
   - `create_acs_signed_content()`: Creates JWT with PS256 signing algorithm
   - `load_certificate()` and `load_private_key()`: Load certificates and keys from PEM files
   - `create_acs_url()`: Utility function for ACS URL generation

2. **Enhanced State Management**
   - Added `ephemeral_keys` field to `TransactionData` structure
   - Updated state store to persist ephemeral keys for potential future use in challenge flows

3. **Certificate Infrastructure**
   - Certificate generation script (`generate-certs.sh`) for secure development setup
   - `certs/acs-cert.pem`: Mock ACS certificate (generated locally, not in Git)
   - `certs/acs-private-key.pem`: Private key for JWT signing (generated locally, not in Git)
   - `certs.example/`: Documentation and structure examples

### Flow Logic

The dynamic ACS signed content generation is triggered when:
- `deviceChannel == "01"` (mobile flow)
- `should_challenge == true` (friction flow requiring challenge)

### Implementation Process

1. **Ephemeral Key Generation**
   - Generates fresh ECDSA P-256 key pair for each transaction
   - Extracts x, y coordinates and private key scalar (d)
   - Encodes as base64url per 3DS specification

2. **JWT Creation**
   - **Header**: PS256 algorithm with x5c certificate chain
   - **Payload**: 
     - `acsTransID`: Current transaction ID
     - `acsRefNumber`: ACS reference number ("issuer1" or "issuer2")
     - `acsURL`: ACS challenge URL (https://mock-acs-server.local/challenge)
     - `acsEphemPubKey`: Generated ephemeral public key
   - **Signature**: PS256 using loaded private key

3. **Graceful Fallback**
   - If certificate loading fails, falls back to hardcoded content
   - Logs appropriate warning messages
   - Ensures service availability even with certificate issues

### Testing

To test the dynamic generation:

1. **Mobile Friction Flow Request**:
   ```json
   {
     "deviceChannel": "01",
     "threeDSRequestorChallengeInd": "04", // Force challenge
     "cardholderAccount": {
       "acctNumber": "4000400040004000" // Any card
     }
   }
   ```

2. **Expected Behavior**:
   - Console log: "✅ Generated dynamic ACS signed content for mobile friction flow"
   - Response contains freshly generated JWT in `acs_signed_content`
   - Ephemeral keys stored in transaction state

3. **Fallback Testing**:
   - Remove or corrupt certificate files
   - Should see fallback message and hardcoded content
   - Service continues to operate

### Compliance Features

- **3DS 2.2.0 Protocol**: Follows EMVCo 3-D Secure specification
- **JWT Standards**: Proper PS256 signing with x5c certificate chain
- **Key Management**: Secure ephemeral key generation per transaction
- **Error Handling**: Graceful degradation with fallback mechanisms

### Security Considerations

⚠️ **Important**: This implementation uses mock certificates and is intended for testing only. For production use:

1. Replace mock certificates with proper CA-signed certificates
2. Implement secure key storage (HSM, key vaults)
3. Add certificate validation and revocation checking
4. Use production-grade random number generation
5. Implement proper key rotation policies

### File Structure

```
mock_three_ds_server/
├── src/
│   ├── crypto.rs              # New crypto module
│   ├── handlers.rs             # Updated with dynamic generation
│   ├── state_store.rs          # Enhanced with ephemeral key storage
│   └── ...
├── certs/                      # New certificate directory
│   ├── acs-cert.pem           # Mock ACS certificate
│   └── acs-private-key.pem    # Mock private key
├── Cargo.toml                  # Updated dependencies
└── DYNAMIC_ACS_SIGNED_CONTENT.md
```

### Dependencies Added

```toml
# Cryptography for JWT and key generation
jsonwebtoken = "9.2"
p256 = { version = "0.13", features = ["ecdsa", "jwk"] }
rand_core = { version = "0.6", features = ["std"] }
pem = "3.0"
```

## Usage

The implementation automatically detects mobile friction flows and generates dynamic ACS signed content. No additional configuration is required beyond ensuring the certificate files are present in the `certs/` directory.

For production deployment, replace the mock certificates with proper production certificates and configure appropriate security measures.
