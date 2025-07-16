# Enhanced 3DS Authentication Flows

## Implementation Summary

The mock 3DS server has been enhanced to support sophisticated authentication flow logic that considers multiple factors beyond just card numbers. This document details the new capabilities and provides testing examples.

## New Flow Decision Logic

### 1. Challenge Indicator Priority System
The server now respects `threeDSRequestorChallengeInd` values that override default card-based logic:

- **"04" (Challenge Mandated)**: Forces challenge flow even for frictionless cards (4000400040004000)
- **"05" (No Challenge Requested)**: Skips challenge even for friction cards (4000400040004001)  
- **Other values**: Falls back to card-based logic (4001 = challenge, 4000 = frictionless)

### 2. Device Channel Support
- **"01" (Mobile/App)**: Returns mobile-specific response with JWT signed content
- **"02" (Browser)**: Returns traditional browser response with ACS URL

### 3. Dynamic ACS Configuration
- **Exemption flows** (challengeInd: "05"): Uses "MOCK_ACS_NEW" and "issuer2"
- **Default flows**: Uses "MOCK_ACS" and "issuer1"

## Enhanced Response Fields

### New Universal Fields
- `eci`: "05" for all successful authentications
- `authenticationValue`: "QWErty123+/ABCD5678ghijklmn==" (mock value)

### Mobile-Specific Fields (deviceChannel: "01")
- `threeDSRequestorAppURLInd`: "N"
- `acsSignedContent`: JWT with mock keys and challenge data
- `acsRenderingType`: Device interface configuration
- `broadInfo`: TLS deprecation notice
- `authenticationMethod`: "02"
- `transStatusReason`: "15"
- `deviceInfoRecognisedVersion`: "1.3"
- `sdkTransID`: Generated UUID for SDK communication

### Browser-Specific Fields (deviceChannel: "02")
- `acsURL`: Dynamic challenge URL pointing to local ACS endpoint (when challenge required)
- `base64EncodedChallengeRequest`: Base64 encoded challenge data

## ACS Challenge Endpoint (Latest Implementation)

### 4. Web Challenge Flow with Local ACS
The server now includes a complete ACS challenge endpoint for browser-based authentication:

**Endpoint**: `POST /processor/mock/acs/trigger-otp`
**Purpose**: Provides complete HTML challenge form for Web Challenge flow
**Features**:
- Self-contained challenge form with modern UI
- Dynamic URL generation using server configuration
- Template-based HTML rendering
- Proper form data handling for `creq` parameter

### ACS Challenge Form Features
- **Interactive OTP Input**: 4-digit code entry with validation
- **Modern UI**: Responsive design with Juspay Demo Bank branding
- **Help Sections**: Collapsible help with testing instructions
- **JavaScript Integration**: Form validation and submission handling
- **Error Handling**: Graceful failure with fallback redirects

### Template System
The challenge form uses a template system with placeholder substitution:
- `{{FALLBACK_REDIRECT_URL}}`: Dynamic server URL for redirects
- `{{THREE_DS_SERVER_TRANS_ID}}`: Extracted from creq JSON
- `{{PAY_ENDPOINT}}`: Dynamic verify-otp endpoint URL

### Form Data Handling
The endpoint accepts Form POST data with:
- **creq**: JSON string (not base64-encoded) containing challenge request
- **Expected Format**: `{"messageType":"CReq","threeDsServerTransId":"uuid","acsTransId":"uuid","challengeWindowSize":"01","messageVersion":"2.2.0"}`

## Testing Examples

### 1. Frictionless Flow (Default)
```json
{
    "threeDSServerTransID": "{{threeDSServerTransID}}",
    "deviceChannel": "02",
    "threeDSRequestor": {
        "threeDSRequestorChallengeInd": "01"
    },
    "cardholderAccount": {
        "acctNumber": "4000400040004000"
    }
}
```
**Expected**: `transStatus: "Y"`, includes ECI and authenticationValue

### 2. Exemption for Friction Card
```json
{
    "threeDSServerTransID": "{{threeDSServerTransID}}",
    "deviceChannel": "02", 
    "threeDSRequestor": {
        "threeDSRequestorChallengeInd": "05"
    },
    "cardholderAccount": {
        "acctNumber": "4000400040004001"
    }
}
```
**Expected**: `transStatus: "Y"`, uses "MOCK_ACS_NEW" configuration

### 3. Force Challenge for Frictionless Card
```json
{
    "threeDSServerTransID": "{{threeDSServerTransID}}",
    "deviceChannel": "02",
    "threeDSRequestor": {
        "threeDSRequestorChallengeInd": "04"
    },
    "cardholderAccount": {
        "acctNumber": "4000400040004000"
    }
}
```
**Expected**: `transStatus: "C"`, includes acsURL and base64EncodedChallengeRequest

### 4. Mobile Flow with Challenge
```json
{
    "threeDSServerTransID": "{{threeDSServerTransID}}",
    "deviceChannel": "01",
    "threeDSRequestor": {
        "threeDSRequestorChallengeInd": "04"
    },
    "cardholderAccount": {
        "acctNumber": "4000400040004000"
    }
}
```
**Expected**: `transStatus: "C"`, includes acsSignedContent JWT, no acsURL

## Implementation Details

### Flow Decision Algorithm
```rust
let should_challenge = match challenge_indicator.as_str() {
    "04" => true,  // Challenge mandated
    "05" => false, // No challenge requested  
    _ => card_number.ends_with("4001"), // Default card logic
};

let (acs_operator_id, acs_reference_number) = match challenge_indicator.as_str() {
    "05" => ("MOCK_ACS_NEW", "issuer2"), // Exemption flow
    _ => ("MOCK_ACS", "issuer1"),        // Default flow
};
```

### Response Structure Differentiation
```rust
let authentication_response = if is_mobile {
    // Mobile flow - includes SDK-specific fields
    AuthenticationResponse {
        acs_signed_content: Some(jwt_content),
        acs_rendering_type: Some(mobile_ui_config),
        // ... mobile-specific fields
        acs_url: None, // Mobile doesn't use browser URL
    }
} else {
    // Browser flow - traditional response
    AuthenticationResponse {
        acs_signed_content: None,
        acs_url: if should_challenge { Some(challenge_url) } else { None },
        // ... browser-specific fields
    }
};
```

## Compliance Features

### 3DS 2.2.0 Protocol Support
- Proper message versioning
- Complete AReq/ARes field mapping
- Challenge flow state management
- JWT signing for mobile SDK integration

### Security Considerations
- Mock JWT signing (not production-ready)
- Transaction ID uniqueness
- State isolation between transactions
- Proper error handling for missing transactions

## Testing Verification

### Response Validation Checklist
- ✅ `transStatus` matches expected flow (Y/C)
- ✅ `eci` present in all responses
- ✅ `authenticationValue` included
- ✅ Mobile responses include `acsSignedContent`
- ✅ Browser responses include `acsURL` when challenging
- ✅ ACS configuration varies by flow type
- ✅ Challenge request base64 encoding valid

### Flow Coverage Matrix
| Card Type | Challenge Ind | Device | Expected Result |
|-----------|---------------|---------|----------------|
| 4000 (Frictionless) | 01 | Browser | transStatus: Y, no challenge |
| 4000 (Frictionless) | 04 | Browser | transStatus: C, with acsURL |
| 4000 (Frictionless) | 04 | Mobile | transStatus: C, with JWT |
| 4001 (Friction) | 05 | Browser | transStatus: Y, exemption |
| 4001 (Friction) | 01 | Browser | transStatus: C, default |

## Future Enhancements

### Potential Extensions
1. **Real JWT Signing**: Implement proper cryptographic signing
2. **Configurable Responses**: Environment-based response customization
3. **Advanced Challenge Types**: OTP, biometric, out-of-band
4. **Risk-Based Logic**: Transaction amount, merchant category considerations
5. **Multiple ACS Simulation**: Different issuer behavior patterns

### Integration Considerations
- Database storage for persistent transaction state
- Monitoring and metrics collection
- Rate limiting and security headers
- Docker containerization for deployment
- Load testing for concurrency validation

This enhanced implementation provides a comprehensive 3DS 2.2.0 mock server suitable for testing complex authentication scenarios across different device types and challenge requirements.
