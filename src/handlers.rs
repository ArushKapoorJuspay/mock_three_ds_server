use actix_web::{web, HttpResponse, Result};
use base64::{engine::general_purpose, Engine as _};
use log::{debug, error, info, warn};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use uuid::Uuid;

use crate::config::Settings;
use crate::crypto::{
    calculate_derived_key, create_acs_signed_content, create_acs_url, decrypt_challenge_request,
    encrypt_challenge_response, generate_ephemeral_key_pair,
};
use crate::models::*;
use crate::state_store::{StateStore, TransactionData};

// Helper functions for generating authentication values
fn generate_authentic_auth_value() -> String {
    // Generate 20 bytes for CAVV (Cardholder Authentication Verification Value)
    let mut cavv_bytes = vec![0u8; 20];

    // Mock data that looks authentic following 3DS specification patterns
    cavv_bytes[0] = 0x02; // Version indicator
    cavv_bytes[1] = 0x01; // Authentication method indicator

    // Fill rest with deterministic pseudo-random data for consistency
    for i in 2..20 {
        cavv_bytes[i] = ((i * 17 + 13 + 0x4A) % 256) as u8;
    }

    general_purpose::STANDARD.encode(&cavv_bytes)
}

fn generate_failed_auth_value() -> String {
    // For failed authentication, use a pattern indicating failure
    "AAAAAAAAAAAAAAAAAAAAAA==".to_string()
}

pub async fn version_handler(req: web::Json<VersionRequest>) -> Result<HttpResponse> {
    // Generate a new transaction ID for this session
    let trans_id = Uuid::new_v4();

    // Check if card is in the supported range (5155010000000000 - 5155019999999999)
    let card_range = if req.card_number.starts_with("515501") {
        CardRange {
            acs_info_ind: vec!["01".to_string(), "02".to_string()],
            start_range: "5155010000000000".to_string(),
            acs_end_protocol_version: "2.2.0".to_string(),
            acs_start_protocol_version: "2.2.0".to_string(),
            end_range: "5155019999999999".to_string(),
        }
    } else {
        // Default range for other cards
        CardRange {
            acs_info_ind: vec!["01".to_string(), "02".to_string()],
            start_range: "4000000000000000".to_string(),
            acs_end_protocol_version: "2.2.0".to_string(),
            acs_start_protocol_version: "2.2.0".to_string(),
            end_range: "4999999999999999".to_string(),
        }
    };

    let response = VersionResponse {
        three_ds_server_trans_id: trans_id,
        card_ranges: vec![card_range],
    };

    Ok(HttpResponse::Ok().json(response))
}

pub async fn authenticate_handler(
    req: web::Json<AuthenticateRequest>,
    state: web::Data<Arc<Box<dyn StateStore>>>,
    settings: web::Data<Settings>,
) -> Result<HttpResponse> {
    let three_ds_server_trans_id = req.three_ds_server_trans_id;
    let acs_trans_id = Uuid::new_v4();
    let ds_trans_id = Uuid::new_v4();
    let sdk_trans_id = req.sdk_trans_id;

    // Enhanced flow decision logic
    let card_number = &req.cardholder_account.acct_number;
    let challenge_indicator = &req.three_ds_requestor.three_ds_requestor_challenge_ind;
    let is_mobile = req.device_channel == "01"; // Mobile should be "01" based on requirement

    info!("🔐 /3ds/authenticate - Processing authentication request");
    info!("  - Transaction ID: {}", three_ds_server_trans_id);
    info!(
        "  - Device Channel: {} ({})",
        req.device_channel,
        if is_mobile { "Mobile" } else { "Browser" }
    );
    info!("  - Challenge Indicator: {}", challenge_indicator);
    debug!(
        "  - Card Number: ***{}****{}",
        &card_number[..4],
        &card_number[card_number.len() - 4..]
    );

    // Validate sdk_trans_id presence for mobile flows
    if is_mobile && sdk_trans_id.is_none() {
        error!("Missing sdk_trans_id for mobile flow (deviceChannel=01)");
        return Ok(HttpResponse::BadRequest().json(serde_json::json!({
            "error": "sdkTransId is required for mobile flows (deviceChannel=01)"
        })));
    }

    // Determine if challenge is required based on challenge indicator and card number
    let should_challenge = match challenge_indicator.as_str() {
        "04" => true,  // Challenge mandated - force challenge even for frictionless cards
        "05" => false, // No challenge requested - skip challenge even for friction cards
        _ => card_number.ends_with("4001"), // Default card-based logic
    };

    let trans_status = if should_challenge { "C" } else { "Y" };
    let acs_challenge_mandated = if should_challenge { "Y" } else { "N" };

    info!(
        "  - Flow Decision: {} ({})",
        trans_status,
        if should_challenge {
            "Challenge Required"
        } else {
            "Frictionless"
        }
    );

    // Determine ACS configuration based on challenge indicator and flow type
    let (acs_operator_id, acs_reference_number) = match challenge_indicator.as_str() {
        "05" => ("MOCK_ACS_NEW", "issuer2"), // Exemption flow
        _ => ("MOCK_ACS", "issuer1"),        // Default flow
    };

    // Generate ephemeral keys and ACS signed content for mobile friction flows
    let (ephemeral_keys, dynamic_acs_signed_content) = if is_mobile && should_challenge {
        info!(
            "🔑 Mobile friction flow detected - generating ephemeral keys and ACS signed content"
        );
        // Generate ephemeral keys for mobile friction flow
        match generate_ephemeral_key_pair() {
            Ok(keys) => {
                info!("  - Ephemeral key pair generated successfully");
                // Create ACS URL for mobile challenge - use our server URL
                let server_url =
                    format!("http://{}:{}", settings.server.host, settings.server.port);
                let acs_url = create_acs_url(&server_url);

                // Attempt to create dynamic ACS signed content
                let cert_path = Path::new("certs/acs-cert.pem");
                let key_path = Path::new("certs/acs-private-key.pem");

                match create_acs_signed_content(
                    acs_trans_id,
                    acs_reference_number,
                    &acs_url,
                    &keys,
                    cert_path,
                    key_path,
                ) {
                    Ok(signed_content) => {
                        info!("  - Dynamic ACS signed content generated successfully");
                        debug!("  - ACS Trans ID: {}", acs_trans_id);
                        debug!("  - ACS Reference Number: {}", acs_reference_number);
                        (Some(keys), Some(signed_content))
                    }
                    Err(e) => {
                        warn!("  - Failed to generate ACS signed content: {}, falling back to hardcoded", e);
                        // Fall back to hardcoded value if cert loading fails
                        (Some(keys), None)
                    }
                }
            }
            Err(e) => {
                warn!(
                    "  - Failed to generate ephemeral keys: {}, using hardcoded content",
                    e
                );
                (None, None)
            }
        }
    } else {
        debug!("  - Skipping ephemeral key generation (not mobile friction flow)");
        (None, None)
    };

    // Create authentication request data for the response with proper browser information handling
    let mut auth_request_json = serde_json::json!({
        "shipAddrLine3": req.cardholder.ship_addr_line3,
        "purchaseCurrency": req.purchase.purchase_currency,
        "email": req.cardholder.email,
        "shipAddrPostCode": req.cardholder.ship_addr_post_code,
        "billAddrLine2": req.cardholder.bill_addr_line2,
        "merchantCountryCode": req.merchant.merchant_country_code,
        "acquirerBIN": req.acquirer.acquirer_bin,
        "purchaseDate": &req.purchase.purchase_date,
        "threeDSRequestorName": req.merchant.three_ds_requestor_name,
        "deviceRenderOptions": {
            "sdkUiType": req.device_render_options.sdk_ui_type,
            "sdkInterface": req.device_render_options.sdk_interface
        },
        "acquirerMerchantID": req.acquirer.acquirer_merchant_id,
        "billAddrLine3": req.cardholder.bill_addr_line3,
        "threeDSRequestorChallengeInd": req.three_ds_requestor.three_ds_requestor_challenge_ind,
        "shipAddrLine2": req.cardholder.ship_addr_line2,
        "acctType": req.cardholder_account.acct_type,
        "workPhone": {
            "subscriber": req.cardholder.work_phone.subscriber,
            "cc": req.cardholder.work_phone.cc
        },
        "merchantName": req.merchant.merchant_name,
        "threeDSRequestorID": req.merchant.three_ds_requestor_id,
        "billAddrCountry": req.cardholder.bill_addr_country,
        "addrMatch": req.cardholder.addr_match,
        "messageType": "AReq",
        "deviceChannel": req.device_channel,
        "threeDSServerTransID": three_ds_server_trans_id,
        "threeDSRequestorAuthenticationInd": req.three_ds_requestor.three_ds_requestor_authentication_ind,
        "shipAddrLine1": req.cardholder.ship_addr_line1,
        "notificationURL": req.merchant.notification_url,
        "threeDSServerRefNumber": "3DS_LOA_SER_JTPL_020200_00841",
        "threeDSServerOperatorID": "10073246",
        "shipAddrCountry": req.cardholder.ship_addr_country,
        "mobilePhone": {
            "subscriber": req.cardholder.mobile_phone.subscriber,
            "cc": req.cardholder.mobile_phone.cc
        },
        "threeDSServerURL": "https://visa.3ds.certification.juspay.in/3ds/results",
        "billAddrCity": req.cardholder.bill_addr_city,
        "cardExpiryDate": req.cardholder_account.card_expiry_date,
        "billAddrLine1": req.cardholder.bill_addr_line1,
        "cardSecurityCode": req.cardholder_account.card_security_code,
        "purchaseAmount": req.purchase.purchase_amount.to_string(),
        "transType": req.purchase.trans_type,
        "billAddrPostCode": req.cardholder.bill_addr_post_code,
        "mcc": req.merchant.mcc,
        "recurringFrequency": req.purchase.recurring_frequency.to_string(),
        "purchaseExponent": req.purchase.purchase_exponent.to_string(),
        "homePhone": {
            "subscriber": req.cardholder.home_phone.subscriber,
            "cc": req.cardholder.home_phone.cc
        },
        "threeDSCompInd": req.three_ds_comp_ind,
        "threeDSRequestorAuthenticationInfo": {
            "threeDSReqAuthMethod": req.three_ds_requestor.three_ds_requestor_authentication_info.three_ds_req_auth_method,
            "threeDSReqAuthTimestamp": req.three_ds_requestor.three_ds_requestor_authentication_info.three_ds_req_auth_timestamp
        },
        "messageCategory": req.message_category,
        "cardholderName": req.cardholder.cardholder_name,
        "recurringExpiry": req.purchase.recurring_expiry,
        "threeDSRequestorURL": req.merchant.notification_url,
        "acctNumber": req.cardholder_account.acct_number,
        "shipAddrCity": req.cardholder.ship_addr_city,
        "messageVersion": "2.2.0"
    });

    // Add browser information if present (browser flow)
    if let Some(browser_info) = &req.browser_information {
        auth_request_json["browserColorDepth"] =
            serde_json::Value::String(browser_info.browser_color_depth.clone());
        auth_request_json["browserScreenHeight"] =
            serde_json::Value::String(browser_info.browser_screen_height.to_string());
        auth_request_json["browserIP"] = serde_json::Value::String(browser_info.browser_ip.clone());
        auth_request_json["browserJavaEnabled"] =
            serde_json::Value::Bool(browser_info.browser_java_enabled);
        auth_request_json["browserScreenWidth"] =
            serde_json::Value::String(browser_info.browser_screen_width.to_string());
        auth_request_json["browserLanguage"] =
            serde_json::Value::String(browser_info.browser_language.clone());
        auth_request_json["browserUserAgent"] =
            serde_json::Value::String(browser_info.browser_user_agent.clone());
        auth_request_json["browserTZ"] =
            serde_json::Value::String(browser_info.browser_tz.to_string());
        auth_request_json["browserJavascriptEnabled"] =
            serde_json::Value::Bool(browser_info.browser_javascript_enabled);
        auth_request_json["browserAcceptHeader"] =
            serde_json::Value::String(browser_info.browser_accept_header.clone());
    }

    // Add SDK ephemeral public key if present (mobile flow) - check both old nested and new top-level format
    if let Some(sdk_key) = &req.sdk_ephemeral_public_key {
        auth_request_json["sdkEphemeralPublicKey"] = serde_json::json!({
            "kty": sdk_key.kty,
            "crv": sdk_key.crv,
            "x": sdk_key.x,
            "y": sdk_key.y
        });
    } else if req.kty.is_some() && req.crv.is_some() && req.x.is_some() && req.y.is_some() {
        // New format: top-level fields
        auth_request_json["sdkEphemeralPublicKey"] = serde_json::json!({
            "kty": req.kty.as_ref().unwrap(),
            "crv": req.crv.as_ref().unwrap(),
            "x": req.x.as_ref().unwrap(),
            "y": req.y.as_ref().unwrap()
        });
    }

    // Extract redirect URL from the notification URL or use default
    let redirect_url = req.merchant.notification_url.clone();

    // Extract SDK ephemeral public key if this is a mobile flow - check both old nested and new top-level format
    let sdk_ephemeral_public_key = if is_mobile {
        if let Some(sdk_key) = &req.sdk_ephemeral_public_key {
            // Old nested format
            let sdk_public_key_jwk = serde_json::json!({
                "kty": sdk_key.kty,
                "crv": sdk_key.crv,
                "x": sdk_key.x,
                "y": sdk_key.y
            });
            debug!("🔑 Extracting SDK ephemeral public key from nested format");
            info!("📱 Mobile flow detected - storing SDK ephemeral key for future ECDH");
            Some(serde_json::to_string(&sdk_public_key_jwk).unwrap_or_default())
        } else if req.kty.is_some() && req.crv.is_some() && req.x.is_some() && req.y.is_some() {
            // New top-level format
            let sdk_public_key_jwk = serde_json::json!({
                "kty": req.kty.as_ref().unwrap(),
                "crv": req.crv.as_ref().unwrap(),
                "x": req.x.as_ref().unwrap(),
                "y": req.y.as_ref().unwrap()
            });
            debug!("🔑 Extracting SDK ephemeral public key from top-level fields");
            info!("📱 Mobile flow detected - storing SDK ephemeral key for future ECDH");
            Some(serde_json::to_string(&sdk_public_key_jwk).unwrap_or_default())
        } else {
            warn!("⚠️  Mobile flow but no SDK ephemeral public key provided");
            None
        }
    } else {
        None
    };

    println!("===> sdkEphemeralKey : {:?}", sdk_ephemeral_public_key);
    // Store transaction data in state
    let transaction_data = TransactionData {
        authenticate_request: req.into_inner(),
        acs_trans_id,
        ds_trans_id,
        sdk_trans_id,
        results_request: None,
        ephemeral_keys: ephemeral_keys.clone(),
        redirect_url: Some(redirect_url),
        sdk_ephemeral_public_key,
    };

    info!("📦 Storing transaction data");
    debug!("  - ACS Trans ID: {}", acs_trans_id);
    debug!("  - threeDSServerTransID: {}", three_ds_server_trans_id);
    if transaction_data.sdk_ephemeral_public_key.is_some() {
        debug!("🔐 SDK ephemeral public key stored for mobile challenge flow");
    }

    if let Err(e) = state
        .insert(three_ds_server_trans_id, transaction_data)
        .await
    {
        error!("Failed to store transaction data: {}", e);
        return Ok(HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("Failed to store transaction data: {}", e)
        })));
    }

    // Create challenge request (used when challenge is required)
    let challenge_request = ChallengeRequest {
        message_type: "CReq".to_string(),
        three_ds_server_trans_id,
        acs_trans_id,
        challenge_window_size: "01".to_string(),
        message_version: "2.2.0".to_string(),
    };

    // Encode challenge request to base64
    let challenge_request_json = serde_json::to_string(&challenge_request)?;
    let base64_encoded_challenge_request = general_purpose::STANDARD.encode(challenge_request_json);

    // Build dynamic ACS URL using server configuration
    let server_url = format!("http://{}:{}", settings.server.host, settings.server.port);

    // Create authentication response based on flow type (mobile vs browser)
    let authentication_response = if is_mobile {
        // Mobile flow - includes SDK-specific fields
        AuthenticationResponse {
            three_ds_requestor_app_url_ind: Some("N".to_string()),
            acs_operator_id: acs_operator_id.to_string(),
            ds_reference_number: "MOCK_DS".to_string(),
            eci: "05".to_string(),
            acs_signed_content: dynamic_acs_signed_content,
            ds_trans_id,
            acs_rendering_type: Some(AcsRenderingTypeResponse {
                device_user_interface_mode: "01".to_string(),
                acs_interface: "01".to_string(),
                acs_ui_template: "01".to_string(),
            }),
            message_type: "ARes".to_string(),
            three_ds_server_trans_id,
            acs_trans_id,
            broad_info: Some(BroadInfo {
                category: "01".to_string(),
                severity: "04".to_string(),
                source: "03".to_string(),
                recipients: vec!["02".to_string(), "01".to_string(), "03".to_string()],
                description: BroadInfoDescription {
                    message: "TLS 1.x will be turned off starting summer 2019".to_string(),
                },
                exp_date: "20241231".to_string(),
            }),
            authentication_method: Some("02".to_string()),
            trans_status_reason: Some("15".to_string()),
            device_info_recognised_version: Some("1.3".to_string()),
            acs_challenge_mandated: acs_challenge_mandated.to_string(),
            authentication_type: "02".to_string(),
            sdk_trans_id: sdk_trans_id,
            authentication_value: "QWErty123+/ABCD5678ghijklmn==".to_string(),
            trans_status: trans_status.to_string(),
            message_version: "2.2.0".to_string(),
            acs_reference_number: acs_reference_number.to_string(),
            acs_url: None, // Mobile flow doesn't use acsURL
        }
    } else {
        // Browser flow - traditional response
        AuthenticationResponse {
            three_ds_requestor_app_url_ind: None,
            acs_operator_id: acs_operator_id.to_string(),
            ds_reference_number: "MOCK_DS".to_string(),
            eci: "05".to_string(),
            acs_signed_content: None,
            ds_trans_id,
            acs_rendering_type: None,
            message_type: "ARes".to_string(),
            three_ds_server_trans_id,
            acs_trans_id,
            broad_info: None,
            authentication_method: None,
            trans_status_reason: None,
            device_info_recognised_version: None,
            acs_challenge_mandated: acs_challenge_mandated.to_string(),
            authentication_type: "02".to_string(),
            sdk_trans_id: None,
            authentication_value: "QWErty123+/ABCD5678ghijklmn==".to_string(),
            trans_status: trans_status.to_string(),
            message_version: "2.2.0".to_string(),
            acs_reference_number: acs_reference_number.to_string(),
            acs_url: if should_challenge {
                Some(format!("{}/processor/mock/acs/trigger-otp", server_url))
            } else {
                None
            },
        }
    };

    // Create response structure
    let response = AuthenticateResponse {
        purchase_date: auth_request_json["purchaseDate"]
            .as_str()
            .unwrap()
            .to_string(),
        base64_encoded_challenge_request: if should_challenge {
            Some(base64_encoded_challenge_request)
        } else {
            None
        },
        acs_url: if should_challenge && !is_mobile {
            Some(format!("{}/processor/mock/acs/trigger-otp", server_url))
        } else {
            None
        },
        three_ds_server_trans_id,
        authentication_response,
        challenge_request,
        acs_challenge_mandated: acs_challenge_mandated.to_string(),
        trans_status: trans_status.to_string(),
        authentication_request: auth_request_json,
    };

    Ok(HttpResponse::Ok().json(response))
}

/// Mobile challenge endpoint - handles encrypted JWE requests from SDK
pub async fn challenge_handler(
    req: web::Bytes,
    state: web::Data<Arc<Box<dyn StateStore>>>,
) -> Result<HttpResponse> {
    info!("📱 /challenge - Processing mobile challenge request");
    debug!("  - Request body length: {} bytes", req.len());
    // let body_str = String::from_utf8(req.to_vec())
    // .unwrap_or_else(|_| "Invalid UTF-8".to_string());
    // debug!("  - Request body: {}", body_str);

    // Convert bytes to string
    let jwe_data = match String::from_utf8(req.to_vec()) {
        Ok(s) => s,
        Err(_) => {
            error!("Invalid request body encoding");
            return Ok(HttpResponse::BadRequest().json(serde_json::json!({
                "errorCode": "400",
                "errorDescription": "Invalid request body encoding"
            })));
        }
    };

    println!("===> Raw Request Body: {}", jwe_data);
    println!("📊 Raw Request Analysis:");
    println!("  - Length: {} characters", jwe_data.len());
    println!(
        "  - First 100 chars: {}",
        if jwe_data.len() > 100 {
            &jwe_data[0..100]
        } else {
            &jwe_data
        }
    );

    // Check if this looks like a JSON error response instead of a JWE
    if jwe_data.trim().starts_with('{') && jwe_data.trim().ends_with('}') {
        println!("⚠️  Received JSON instead of JWE - this might be an error response from SDK");
        if let Ok(json_error) = serde_json::from_str::<serde_json::Value>(&jwe_data) {
            println!(
                "📋 JSON Error Response: {}",
                serde_json::to_string_pretty(&json_error).unwrap_or_default()
            );
            return Ok(HttpResponse::BadRequest().json(serde_json::json!({
                "errorCode": "400",
                "errorDescription": "Received JSON error response instead of JWE"
            })));
        }
    }

    // If it looks like a JWE, log the structure
    if jwe_data.contains('.') && jwe_data.matches('.').count() >= 4 {
        println!("📋 JWE Structure Analysis:");
        let parts: Vec<&str> = jwe_data.split('.').collect();
        println!("  - Total parts: {}", parts.len());
        for (i, part) in parts.iter().enumerate() {
            println!("  - Part {}: {} chars", i + 1, part.len());
        }
        if parts.len() >= 1 {
            // Try to decode and log the header
            if let Ok(header_bytes) = general_purpose::URL_SAFE_NO_PAD.decode(parts[0]) {
                if let Ok(header_str) = String::from_utf8(header_bytes) {
                    println!("  - Decoded header: {}", header_str);
                }
            }
        }
    }

    // Extract JWE Header to get kid (acsTransID)
    let parts: Vec<&str> = jwe_data.split('.').collect();
    if parts.len() != 5 {
        error!("Invalid JWE format - expected 5 parts, got {}", parts.len());
        return Ok(HttpResponse::BadRequest().json(serde_json::json!({
            "errorCode": "400",
            "errorDescription": "Invalid JWE format"
        })));
    }

    let header_data = match general_purpose::URL_SAFE_NO_PAD.decode(parts[0]) {
        Ok(data) => data,
        Err(_) => {
            error!("Invalid JWE header encoding");
            return Ok(HttpResponse::BadRequest().json(serde_json::json!({
                "errorCode": "400",
                "errorDescription": "Invalid JWE header encoding"
            })));
        }
    };

    let header_json: serde_json::Value = match serde_json::from_slice(&header_data) {
        Ok(json) => json,
        Err(_) => {
            error!("Invalid JWE header JSON");
            return Ok(HttpResponse::BadRequest().json(serde_json::json!({
                "errorCode": "400",
                "errorDescription": "Invalid JWE header JSON"
            })));
        }
    };

    debug!("🔍 Extracted JWE header: {:?}", header_json);

    let acs_trans_id_str = match header_json["kid"].as_str() {
        Some(kid) => kid,
        None => {
            error!("Missing kid in JWE header");
            return Ok(HttpResponse::BadRequest().json(serde_json::json!({
                "errorCode": "400",
                "errorDescription": "Missing kid in JWE header"
            })));
        }
    };

    debug!("🔍 Processing kid from JWE header");
    debug!("  - Value: '{}'", acs_trans_id_str);
    debug!("  - Length: {} characters", acs_trans_id_str.len());

    // Check if it looks like a truncated UUID
    if acs_trans_id_str.len() == 35 {
        warn!(
            "Detected truncated UUID (35 chars instead of 36): {}",
            acs_trans_id_str
        );
        debug!("  - Expected format: xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx");
        debug!("  - Actual format:   {}", acs_trans_id_str);

        // Check if we can identify where truncation occurred
        let segments: Vec<&str> = acs_trans_id_str.split('-').collect();
        debug!("  - Segments: {:?}", segments);
        for (i, segment) in segments.iter().enumerate() {
            debug!(
                "    Segment {}: '{}' (length: {})",
                i + 1,
                segment,
                segment.len()
            );
        }
    }

    let acs_trans_id = match Uuid::parse_str(acs_trans_id_str) {
        Ok(id) => id,
        Err(e) => {
            error!(
                "Invalid kid format: {}, UUID parse error: {}",
                acs_trans_id_str, e
            );
            return Ok(HttpResponse::BadRequest().json(serde_json::json!({
                "errorCode": "400",
                "errorDescription": format!("Invalid kid format: {}", acs_trans_id_str)
            })));
        }
    };

    info!("  - ACS Transaction ID extracted: {}", acs_trans_id);

    // Find transaction by acsTransID
    let (three_ds_server_trans_id, transaction_data) =
        match state.find_by_acs_trans_id(&acs_trans_id).await {
            Ok(Some((trans_id, data))) => {
                println!(
                    "✅ Found transaction - threeDSServerTransID: {}, sdkTransID: {:?}",
                    trans_id, data.sdk_trans_id
                );
                (trans_id, data)
            }
            Ok(None) => {
                println!("❌ Transaction not found for acsTransID: {}", acs_trans_id);
                return Ok(HttpResponse::NotFound().json(serde_json::json!({
                    "errorCode": "404",
                    "errorDescription": "Transaction not found"
                })));
            }
            Err(e) => {
                println!("⚠️  Error searching for transaction: {}", e);
                return Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                    "errorCode": "500",
                    "errorDescription": "Internal server error"
                })));
            }
        };

    // Extract SDK ephemeral public key and our private key for ECDH
    let (sdk_public_key, our_private_key) = match (
        &transaction_data.sdk_ephemeral_public_key,
        &transaction_data.ephemeral_keys,
    ) {
        (Some(sdk_key), Some(our_keys)) => (sdk_key.clone(), our_keys.private_key.clone()),
        _ => {
            println!("⚠️  Missing ephemeral keys for ECDH derivation");
            return Ok(HttpResponse::BadRequest().json(serde_json::json!({
                "errorCode": "400",
                "errorDescription": "Missing ephemeral keys for ECDH"
            })));
        }
    };

    // Detect platform from JWE header encryption algorithm
    let platform = match header_json["enc"].as_str().unwrap_or("unknown") {
        "A128CBC-HS256" => "android",
        "A128GCM" => "ios",
        _ => {
            println!(
                "⚠️  Unsupported encryption algorithm: {}",
                header_json["enc"].as_str().unwrap_or("unknown")
            );
            return Ok(HttpResponse::BadRequest().json(serde_json::json!({
                "errorCode": "400",
                "errorDescription": "Unsupported encryption algorithm"
            })));
        }
    };

    println!("  - Detected platform: {}", platform);

    // Derive shared secret using ECDH with platform-specific SDK reference number
    let derived_key = match calculate_derived_key(&sdk_public_key, &our_private_key, platform) {
        Ok(key) => key,
        Err(e) => {
            println!("⚠️  Failed to derive shared key: {}", e);
            return Ok(HttpResponse::BadRequest().json(serde_json::json!({
                "errorCode": "400",
                "errorDescription": "Failed to derive shared key"
            })));
        }
    };

    // Decrypt JWE challenge request
    let challenge_request = match decrypt_challenge_request(&jwe_data, &derived_key).await {
        Ok(request) => {
            println!("📋 Decrypted challenge request: {:?}", request);
            request
        }
        Err(e) => {
            println!("⚠️  Failed to decrypt challenge request: {}", e);
            return Ok(HttpResponse::BadRequest().json(serde_json::json!({
                "errorCode": "400",
                "errorDescription": "Failed to decrypt challenge request"
            })));
        }
    };

    // Validate the decrypted challenge request format
    println!("📋 Validating challenge request format:");
    println!(
        "  - messageType: {}",
        challenge_request
            .get("messageType")
            .and_then(|v| v.as_str())
            .unwrap_or("missing")
    );
    println!(
        "  - messageVersion: {}",
        challenge_request
            .get("messageVersion")
            .and_then(|v| v.as_str())
            .unwrap_or("missing")
    );
    println!(
        "  - sdkCounterStoA: {}",
        challenge_request
            .get("sdkCounterStoA")
            .and_then(|v| v.as_str())
            .unwrap_or("missing")
    );
    println!(
        "  - challengeWindowSize: {}",
        challenge_request
            .get("challengeWindowSize")
            .and_then(|v| v.as_str())
            .unwrap_or("missing")
    );
    println!(
        "  - challengeNoEntry: {}",
        challenge_request
            .get("challengeNoEntry")
            .and_then(|v| v.as_str())
            .unwrap_or("missing")
    );
    println!(
        "  - challengeDataEntry: {}",
        challenge_request
            .get("challengeDataEntry")
            .map(|_v| "present")
            .unwrap_or("missing")
    );

    // Check if this is an OTP submission or initial challenge (matching Node.js behavior)
    let response_data = if let Some(challenge_data_entry) =
        challenge_request.get("challengeDataEntry")
    {
        // Second request: OTP submission
        let user_otp = challenge_data_entry.as_str().unwrap_or("");
        let sdk_counter = challenge_request
            .get("sdkCounterStoA")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let is_valid_otp = user_otp == "1234";

        println!("📲 OTP submission detected - processing final authentication");
        println!("  🔢 OTP value: {}", user_otp);
        println!("  📊 SDK Counter: {}", sdk_counter);
        println!(
            "  ✅ Validation result: {}",
            if is_valid_otp { "PASS" } else { "FAIL" }
        );

        // Validate expected counter for OTP submission
        if sdk_counter != "001" {
            println!(
                "  ⚠️  Unexpected SDK counter for OTP submission: {} (expected: 001)",
                sdk_counter
            );
        }

        // Update transaction with final status and call results handler
        let (trans_status, eci, authentication_value) = if is_valid_otp {
            ("Y", "02", generate_authentic_auth_value())
        } else {
            ("N", "07", generate_failed_auth_value())
        };

        // Create results request to update transaction
        let results_request = ResultsRequest {
            acs_trans_id: transaction_data.acs_trans_id,
            message_category: "01".to_string(),
            eci: eci.to_string(),
            message_type: "RReq".to_string(),
            acs_rendering_type: AcsRenderingType {
                acs_ui_template: "01".to_string(),
                acs_interface: "01".to_string(),
            },
            ds_trans_id: transaction_data.ds_trans_id,
            authentication_method: "02".to_string(),
            authentication_type: "02".to_string(),
            message_version: challenge_request["messageVersion"]
                .as_str()
                .unwrap_or("2.2.0")
                .to_string(),
            sdk_trans_id: transaction_data.sdk_trans_id,
            interaction_counter: "01".to_string(),
            authentication_value: authentication_value.clone(),
            trans_status: trans_status.to_string(),
            three_ds_server_trans_id,
        };

        // Update transaction state internally
        match results_handler(web::Json(results_request), state.clone()).await {
            Ok(_) => {
                println!("✅ Successfully updated transaction with results");
            }
            Err(e) => {
                println!("⚠️  Failed to call results handler: {:?}", e);
            }
        }

        // Final response
        serde_json::json!({
            "acsCounterAtoS": "001",
            "acsTransID": acs_trans_id_str,
            "challengeCompletionInd": "Y",
            "messageType": "CRes",
            "messageVersion": challenge_request["messageVersion"].as_str().unwrap_or("2.2.0"),
            "sdkTransID": transaction_data.sdk_trans_id.map_or_else(|| "".to_string(), |id| id.to_string()),
            "threeDSServerTransID": three_ds_server_trans_id.to_string(),
            "transStatus": trans_status
        })
    } else {
        // First request: Initial challenge (matching Node.js behavior - no challengeDataEntry means initial challenge)
        let sdk_counter = challenge_request
            .get("sdkCounterStoA")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        println!("📲 Initial challenge request - preparing OTP form");
        println!("  📊 SDK Counter: {}", sdk_counter);

        // Validate expected counter for initial challenge
        if sdk_counter != "000" {
            println!(
                "  ⚠️  Unexpected SDK counter for initial challenge: {} (expected: 000)",
                sdk_counter
            );
        }

        serde_json::json!({
            "acsTransID": acs_trans_id_str,
            "acsCounterAtoS": "000",
            "acsUiType": "01",
            "challengeCompletionInd": "N",
            "challengeInfoHeader": "Authentication Required",
            "challengeInfoLabel": "Enter OTP:",
            "messageType": "CRes",
            "messageVersion": "2.2.0",
            "sdkTransID": transaction_data.sdk_trans_id.map_or_else(|| "".to_string(), |id| id.to_string()),
            "threeDSServerTransID": three_ds_server_trans_id.to_string(),
            "submitAuthenticationLabel": "Submit",
            // "transStatus": "C"
        })
    };

    println!("📝 Creating challenge response:");
    println!(
        "  - Message Type: {}",
        response_data["messageType"].as_str().unwrap_or("unknown")
    );
    println!(
        "  - Trans Status: {}",
        response_data["transStatus"].as_str().unwrap_or("unknown")
    );
    println!(
        "  - Challenge Completion: {}",
        response_data["challengeCompletionInd"]
            .as_str()
            .unwrap_or("unknown")
    );

    // Encrypt the response using the same platform that was detected during decryption
    let platform = match header_json["enc"].as_str().unwrap_or("unknown") {
        "A128CBC-HS256" => "android",
        "A128GCM" => "ios",
        _ => "android", // Default to android for unknown encryption types
    };

    let encrypted_response =
        match encrypt_challenge_response(&response_data, acs_trans_id_str, &derived_key, platform)
            .await
        {
            Ok(jwe) => jwe,
            Err(e) => {
                println!("⚠️  Failed to encrypt response: {}", e);
                return Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                    "errorCode": "500",
                    "errorDescription": "Failed to encrypt response"
                })));
            }
        };

    println!("✅ Mobile challenge flow completed successfully");
    println!("  - Transaction ID: {}", three_ds_server_trans_id);
    println!("  - ACS Trans ID: {}", acs_trans_id);
    println!(
        "  - Final Status: {}",
        response_data["transStatus"].as_str().unwrap_or("unknown")
    );

    // Return encrypted JWE response
    Ok(HttpResponse::Ok()
        .content_type("application/jose")
        .body(encrypted_response))
}

pub async fn acs_trigger_otp_handler(
    query: web::Query<HashMap<String, String>>,
    form: web::Form<AcsTriggerOtpRequest>,
    settings: web::Data<Settings>,
    state: web::Data<Arc<Box<dyn StateStore>>>,
) -> Result<HttpResponse> {
    // Parse the creq JSON directly (already decoded)
    let challenge_request: ChallengeRequest = match serde_json::from_str(&form.creq) {
        Ok(req) => req,
        Err(_) => {
            return Ok(HttpResponse::BadRequest().json(serde_json::json!({
                "error": "Invalid JSON in challenge request"
            })));
        }
    };

    // Extract threeDSServerTransID from the challenge request
    let three_ds_server_trans_id = challenge_request.three_ds_server_trans_id;

    // Determine redirect URL: priority is query parameter > stored transaction data > default fallback
    let redirect_url = if let Some(query_redirect_url) = query.get("redirectUrl") {
        // Use redirect URL from query parameter if provided
        println!(
            "📌 Using redirect URL from query parameter: {}",
            query_redirect_url
        );
        query_redirect_url.clone()
    } else {
        // Fall back to stored redirect URL from transaction data
        match state.get(&three_ds_server_trans_id).await {
            Ok(Some(transaction_data)) => {
                let stored_url = transaction_data
                    .redirect_url
                    .unwrap_or_else(|| "https://juspay.api.in.end".to_string());
                println!(
                    "📌 Using stored redirect URL from transaction data: {}",
                    stored_url
                );
                stored_url
            }
            _ => {
                println!("📌 Using default fallback redirect URL");
                "https://juspay.api.in.end".to_string() // Fallback if transaction not found
            }
        }
    };

    // Build dynamic URLs using server configuration
    let server_url = format!("http://{}:{}", settings.server.host, settings.server.port);
    let fallback_redirect_url = server_url.clone();
    let pay_endpoint = format!(
        "{}/processor/mock/acs/verify-otp?redirectUrl={}",
        server_url,
        urlencoding::encode(&redirect_url)
    );

    // Load and populate the HTML template
    let template_content = include_str!("../templates/acs-challenge.html");
    let html_content = template_content
        .replace("{{FALLBACK_REDIRECT_URL}}", &fallback_redirect_url)
        .replace(
            "{{THREE_DS_SERVER_TRANS_ID}}",
            &three_ds_server_trans_id.to_string(),
        )
        .replace("{{PAY_ENDPOINT}}", &pay_endpoint);

    Ok(HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html_content))
}

pub async fn acs_verify_otp_handler(
    query: web::Query<HashMap<String, String>>,
    form: web::Form<AcsVerifyOtpRequest>,
    state: web::Data<Arc<Box<dyn StateStore>>>,
) -> Result<HttpResponse> {
    // Extract redirect URL from query parameters
    let redirect_url = query
        .get("redirectUrl")
        .cloned()
        .unwrap_or_else(|| "https://juspay.api.in.end".to_string());

    // Default error response - still redirect as requested
    let error_redirect = format!("{}?transStatus=U&error=processing_error", redirect_url);

    // Parse transaction ID
    let three_ds_server_trans_id = match Uuid::parse_str(&form.three_ds_server_trans_id) {
        Ok(id) => id,
        Err(_) => {
            println!(
                "⚠️  Invalid transaction ID format: {}",
                form.three_ds_server_trans_id
            );
            return Ok(HttpResponse::Found()
                .append_header(("Location", error_redirect))
                .finish());
        }
    };

    // Get transaction data from state
    match state.get(&three_ds_server_trans_id).await {
        Ok(Some(transaction_data)) => {
            // Validate OTP and determine authentication status
            let (trans_status, eci, authentication_value) = if form.otp == "1234" {
                ("Y", "02", generate_authentic_auth_value())
            } else {
                ("N", "07", generate_failed_auth_value())
            };

            println!(
                "✅ OTP validation - OTP: {}, Status: {}, ECI: {}",
                form.otp, trans_status, eci
            );

            // Create results request to update the transaction
            let results_request = ResultsRequest {
                acs_trans_id: transaction_data.acs_trans_id,
                message_category: "01".to_string(),
                eci: eci.to_string(),
                message_type: "RReq".to_string(),
                acs_rendering_type: AcsRenderingType {
                    acs_ui_template: "01".to_string(),
                    acs_interface: "01".to_string(),
                },
                ds_trans_id: transaction_data.ds_trans_id,
                authentication_method: "02".to_string(),
                authentication_type: "02".to_string(),
                message_version: "2.2.0".to_string(),
                sdk_trans_id: transaction_data.sdk_trans_id,
                interaction_counter: "01".to_string(),
                authentication_value: authentication_value.clone(),
                trans_status: trans_status.to_string(),
                three_ds_server_trans_id,
            };

            // Call results handler internally to update transaction state
            match results_handler(web::Json(results_request), state.clone()).await {
                Ok(_) => {
                    println!("✅ Successfully updated transaction with results");
                }
                Err(e) => {
                    println!("⚠️  Failed to call results handler: {:?}", e);
                    // Continue with redirect even if results call failed
                }
            }

            // Build redirect URL with status parameters
            let redirect_with_params = format!(
                "{}?transStatus={}&threeDSServerTransID={}&eci={}&authenticationValue={}",
                redirect_url,
                trans_status,
                three_ds_server_trans_id,
                eci,
                urlencoding::encode(&authentication_value)
            );

            println!("🔄 Redirecting to: {}", redirect_with_params);

            Ok(HttpResponse::Found()
                .append_header(("Location", redirect_with_params))
                .finish())
        }
        Ok(None) => {
            println!(
                "⚠️  Transaction not found for ID: {}",
                three_ds_server_trans_id
            );
            Ok(HttpResponse::Found()
                .append_header(("Location", error_redirect))
                .finish())
        }
        Err(e) => {
            println!("⚠️  Error retrieving transaction data: {}", e);
            Ok(HttpResponse::Found()
                .append_header(("Location", error_redirect))
                .finish())
        }
    }
}

pub async fn results_handler(
    req: web::Json<ResultsRequest>,
    state: web::Data<Arc<Box<dyn StateStore>>>,
) -> Result<HttpResponse> {
    let three_ds_server_trans_id = req.three_ds_server_trans_id;

    // Get the existing transaction data
    match state.get(&three_ds_server_trans_id).await {
        Ok(Some(mut transaction_data)) => {
            // Update the transaction data with results request
            transaction_data.results_request = Some(req.into_inner());

            // Store the updated transaction data
            if let Err(e) = state
                .update(&three_ds_server_trans_id, transaction_data.clone())
                .await
            {
                return Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": format!("Failed to update transaction data: {}", e)
                })));
            }

            let response = ResultsResponse {
                ds_trans_id: transaction_data.ds_trans_id,
                message_type: "RRes".to_string(),
                three_ds_server_trans_id,
                acs_trans_id: transaction_data.acs_trans_id,
                sdk_trans_id: transaction_data.sdk_trans_id,
                results_status: "01".to_string(),
                message_version: "2.2.0".to_string(),
            };

            Ok(HttpResponse::Ok().json(response))
        }
        Ok(None) => Ok(HttpResponse::BadRequest().json(serde_json::json!({
            "error": "Transaction not found"
        }))),
        Err(e) => Ok(HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("Failed to retrieve transaction data: {}", e)
        }))),
    }
}

pub async fn final_handler(
    req: web::Json<FinalRequest>,
    state: web::Data<Arc<Box<dyn StateStore>>>,
) -> Result<HttpResponse> {
    let three_ds_server_trans_id = req.three_ds_server_trans_id;

    match state.get(&three_ds_server_trans_id).await {
        Ok(Some(transaction_data)) => {
            if let Some(results_request) = &transaction_data.results_request {
                let results_response = ResultsResponse {
                    ds_trans_id: transaction_data.ds_trans_id,
                    message_type: "RRes".to_string(),
                    three_ds_server_trans_id,
                    acs_trans_id: transaction_data.acs_trans_id,
                    sdk_trans_id: transaction_data.sdk_trans_id,
                    results_status: "01".to_string(),
                    message_version: "2.2.0".to_string(),
                };

                let response = FinalResponse {
                    eci: results_request.eci.clone(),
                    authentication_value: results_request.authentication_value.clone(),
                    three_ds_server_trans_id,
                    results_response,
                    results_request: results_request.clone(),
                    trans_status: results_request.trans_status.clone(),
                };

                Ok(HttpResponse::Ok().json(response))
            } else {
                Ok(HttpResponse::BadRequest().json(serde_json::json!({
                    "error": "Results not found for this transaction"
                })))
            }
        }
        Ok(None) => Ok(HttpResponse::BadRequest().json(serde_json::json!({
            "error": "Transaction not found"
        }))),
        Err(e) => Ok(HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("Failed to retrieve transaction data: {}", e)
        }))),
    }
}
