use actix_web::{web, HttpResponse, Result};
use uuid::Uuid;
use base64::{Engine as _, engine::general_purpose};
use std::sync::Arc;
use std::path::Path;

use crate::models::*;
use crate::state_store::{StateStore, TransactionData};
use crate::crypto::{generate_ephemeral_key_pair, create_acs_signed_content, create_acs_url};
use crate::config::Settings;

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
    let sdk_trans_id = Uuid::new_v4();
    
    // Enhanced flow decision logic
    let card_number = &req.cardholder_account.acct_number;
    let challenge_indicator = &req.three_ds_requestor.three_ds_requestor_challenge_ind;
    let is_mobile = req.device_channel == "01"; // Mobile should be "01" based on requirement
    
    // Determine if challenge is required based on challenge indicator and card number
    let should_challenge = match challenge_indicator.as_str() {
        "04" => true,  // Challenge mandated - force challenge even for frictionless cards
        "05" => false, // No challenge requested - skip challenge even for friction cards
        _ => card_number.ends_with("4001"), // Default card-based logic
    };
    
    let trans_status = if should_challenge { "C" } else { "Y" };
    let acs_challenge_mandated = if should_challenge { "Y" } else { "N" };
    
    // Determine ACS configuration based on challenge indicator and flow type
    let (acs_operator_id, acs_reference_number) = match challenge_indicator.as_str() {
        "05" => ("MOCK_ACS_NEW", "issuer2"), // Exemption flow
        _ => ("MOCK_ACS", "issuer1"),        // Default flow
    };

    // Generate ephemeral keys and ACS signed content for mobile friction flows
    let (ephemeral_keys, dynamic_acs_signed_content) = if is_mobile && should_challenge {
        // Generate ephemeral keys for mobile friction flow
        match generate_ephemeral_key_pair() {
            Ok(keys) => {
                // Create ACS URL for mobile challenge
                let acs_url = create_acs_url("https://mock-acs-server.local");
                
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
                        println!("✅ Generated dynamic ACS signed content for mobile friction flow");
                        (Some(keys), Some(signed_content))
                    }
                    Err(e) => {
                        println!("⚠️  Failed to generate ACS signed content: {}, falling back to hardcoded", e);
                        // Fall back to hardcoded value if cert loading fails
                        (Some(keys), None)
                    }
                }
            }
            Err(e) => {
                println!("⚠️  Failed to generate ephemeral keys: {}, using hardcoded content", e);
                (None, None)
            }
        }
    } else {
        (None, None)
    };

    // Create authentication request data for the response
    let auth_request_json = serde_json::json!({
        "shipAddrLine3": req.cardholder.ship_addr_line3,
        "browserColorDepth": req.browser_information.browser_color_depth,
        "purchaseCurrency": req.purchase.purchase_currency,
        "email": req.cardholder.email,
        "shipAddrPostCode": req.cardholder.ship_addr_post_code,
        "billAddrLine2": req.cardholder.bill_addr_line2,
        "browserScreenHeight": req.browser_information.browser_screen_height.to_string(),
        "merchantCountryCode": req.merchant.merchant_country_code,
        "acquirerBIN": req.acquirer.acquirer_bin,
        "purchaseDate": &req.purchase.purchase_date,
        "threeDSRequestorName": req.merchant.three_ds_requestor_name,
        "deviceRenderOptions": {
            "sdkUiType": req.device_render_options.sdk_ui_type,
            "sdkInterface": req.device_render_options.sdk_interface
        },
        "browserIP": req.browser_information.browser_ip,
        "acquirerMerchantID": req.acquirer.acquirer_merchant_id,
        "browserJavaEnabled": req.browser_information.browser_java_enabled,
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
        "browserScreenWidth": req.browser_information.browser_screen_width.to_string(),
        "shipAddrLine1": req.cardholder.ship_addr_line1,
        "notificationURL": req.merchant.notification_url,
        "browserLanguage": req.browser_information.browser_language,
        "threeDSServerRefNumber": "3DS_LOA_SER_JTPL_020200_00841",
        "threeDSServerOperatorID": "10073246",
        "shipAddrCountry": req.cardholder.ship_addr_country,
        "browserUserAgent": req.browser_information.browser_user_agent,
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
        "browserTZ": req.browser_information.browser_tz.to_string(),
        "browserJavascriptEnabled": req.browser_information.browser_javascript_enabled,
        "transType": req.purchase.trans_type,
        "billAddrPostCode": req.cardholder.bill_addr_post_code,
        "mcc": req.merchant.mcc,
        "recurringFrequency": req.purchase.recurring_frequency.to_string(),
        "purchaseExponent": req.purchase.purchase_exponent.to_string(),
        "homePhone": {
            "subscriber": req.cardholder.home_phone.subscriber,
            "cc": req.cardholder.home_phone.cc
        },
        "browserAcceptHeader": req.browser_information.browser_accept_header,
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

    // Store transaction data in state
    let transaction_data = TransactionData {
        authenticate_request: req.into_inner(),
        acs_trans_id,
        ds_trans_id,
        sdk_trans_id,
        results_request: None,
        ephemeral_keys: ephemeral_keys.clone(),
    };
    
    if let Err(e) = state.insert(three_ds_server_trans_id, transaction_data).await {
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
            acs_signed_content: dynamic_acs_signed_content.or_else(|| Some("eyJhbGciOiJQUzI1NiIsIng1YyI6WyJNSUlFRURDQ0F2aWdBd0lCQWdJSWFYL2RWZE9CM0hnd0RRWUpLb1pJaHZjTkFRRUxCUUF3ZERFTE1Ba0dBMVVFQmhNQ1EwZ3hDekFKQmdOVkJBZ1RBbHBJTVE4d0RRWURWUVFIRXdaYWRYSnBZMmd4RlRBVEJnTlZCQW9UREU1bGRHTmxkR1Z5WVNCQlJ6RWJNQmtHQTFVRUN4TVNRV054ZFdseWFXNW5JRkJ5YjJSMVkzUnpNUk13RVFZRFZRUURFd296WkhOemNISmxka05CTUI0WERUSTBNVEV4TVRFMU1EZ3dNRm9YRFRNek1URXhNVEUxTURnd01Gb3dnWk14Q3pBSkJnTlZCQVlUQWtOSU1ROHdEUVlEVlFRSURBWmFkWEpwWTJneEN6QUpCZ05WQkFjTUFscElNUlV3RXdZRFZRUUtEQXhPWlhSalpYUmxjbUZnUVVjeElEQWVCZ05WQkFzTUYxTmxZM1Z5WlNCRWFXZHBkR0ZzSUZCaGVXMWxiblJ6TVMwd0t3WURWUVFERENSall6WmhOelUwWWkwMU9EQTNMVFF6TkRNdFlUbGxNQzA0TjJNMlpqTTNaREJqTURVd2dnRWlNQTBHQ1NxR1NJYjNEUUVCQVFVQUE0SUJEd0F3Z2dFK0FvSUJBUUMvLzdpT3RaK0ljS1E3MWtnNENxS2hmR2ZqdXVDV3N2OUh1d1c0WXgyMkFGa0RyRGpNOURsZmJkaVo2VEpyNFFjaU9hcm95QkJONTRTT25LclE2MjRJbytpdCtXRWZ0cFhKNDg1V2xydUF3TUdFU1lrTmtnRmdNOEtFbEdIOU54UlJUR2MxQnd0WHdTdjZwbnE4TTRXc0s4SXpVWlZodU9RQ3ZYOEVsK3UzM2RrSEsrbnFLTGpENEZtUlZBeHdKTFJTcWJBeitUMlJmQWJtOHVWUVQvSlVLb3h5cVhGZlFOSVhCZGRLZEdyQXhYdUJUMTBsbjZtYlkwcE9GQi93enAxVnlxdlYvckNLL3dVSVpnTVNXRzN5djVUSnREV2ZHZmk0TVg4ajg3Tit0VlFia1J4d2d3bmgrWWVFaW10cm9VbEV0aUsxc04zYURWbExGK0JoME8rdkFnTUJBQUdqZ1lVd2dZSXdIZ1lKWUlaSUFZYjRRZ0VOQkJFV0QzaGpZU0JqWlhKMGFXWnBZMkYwWlRBTUJnTlZIUk1CQWY4RUFqQUFNQjBHQTFVZERnUVdCQlRRb0hrUG1qcEkydnVIYkFwVit1ekVWbXhRTWpBTEJnTlZIUThFQkFNQ0E3Z3dFd1lEVlIwbEJBd3dDZ1lJS3dZQkJRVUhBd0l3RVFZSllJWklBWWI0UWdFQkJBUURBZ1dnTUEwR0NTcUdTSWIzRFFFQkN3VUFBNElCQVFBQnB4V0piM1ZTL242MmVNeUNhOEZJNnJCY0FFOXZndkYxZ2xJczg4Vk5ENGYyUXpJODFzenBKejZlTjJWRWFnL2VyaEpLTUVoTkk2ZzllRXVMS3ZZbmVxMUFVd3BYdG1rczVhSlhVRC95d0RWS2w1eXpwMzB4WWxZamtlWVQzbVRhNkVsU3BRQ2h5UnBjbkNDVDlCRDRncXFnOVRGZ2NTb1BqcDRKbXJ0TnpsODJIV2NFRUZLbE1DMXVwbDY1M0xzZUprdFduekxlbkg5dkt2OGFyZWRDSVNibllsV05pT2ZoVEZmUmZ3dE5qMmRaNXlZSFMycnl4TVBzUTh5a3EwYllaSjlHOVd3UXJQM0w5RUtEalV6WEJjL29hR0RMY3k2akZ3TnBvaldKSVh4alBTSWRkZTNNUGx4SXY4YWQyYjBhbS9LWi9IMk1xOHhURFhUbGNrcTAiLCJNSUlEdkRDQ0FxU2dBd0lCQWdJSVh4MUhUcVhOaTljd0RRWUpLb1pJaHZjTkFRRU1CUUF3ZERFTE1Ba0dBMVVFQmhNQ1EwZ3hDekFKQmdOVkJBZ1RBbHBJTVE4d0RRWURWUVFIRXdaYWRYSnBZMmd4RlRBVEJnTlZCQW9UREU1bGRHTmxkR1Z5WVNCQlJ6RWJNQmtHQTFVRUN4TVNRV054ZFdseWFXNW5JRkJ5YjJSMVkzUnpNUk13RVFZRFZRUURFd296WkhOemNISmxka05CTUI0WERUSTBNRGd3TnpFeU1EUXdNRm9YRFRNME1EZ3dOekV5TURRd01Gb3dkREVMTUFrR0ExVUVCaE1DUTBneEN6QUpCZ05WQkFnVEFscElNUTh3RFFZRFZRUUhFd1phZFhKcFkyZ3hGVEFUQmdOVkJBb1RERTVsZEdObGRHVnlZU0JCUnpFYk1Ca0dBMVVFQ3hNU1FXTnhkV2x5YVc1bklGQnliMlIxWTNSek1STXdFUVlEVlFRREV3b3paSE56Y0hKbGRrTkJNSUlCSWpBTkJna3Foa2lHOXcwQkFRRUZBQU9DQVE4QU1JSUJDZ0tDQVFFQTM1NTMwMG5lR0tFSkpsY1pCSDdOMEs3dCtVWC82Y2dZUkd4amx2ak9hRzdFaHV5QllyLzdodUxidGtVc2YwRVl4dWJnNWkwWlI3ZlREa1U1czZKZ1JoZmVFTlZndXNUTVB5WU5rN2lPS3NFWHdhaHY0VW11dGh2T3RZaWRWL1d3OXkrZHZjRWIxNHBsWDY4NXE5bnpOcGp4eHdnMFBBdkJJQzNhOWU2Yi82WXgvQ0UwZm5iR05wU1FETFl4QzhBL3ZCMXk4RStBaFJpR0F4Tk5CdHkva3VVdkJsRDVLZCtmc0ZqSnRVK2hJMERFV0VYVmVlR3FVME9aeWZMb2JxUFVNbk5KWVRTR2o3SG5YVWtCYkZLWE9LN0V5MURnc3hCdUFsaUlsV0x4YXRBQ245QnpzdXN2cU51U3Z5L2lxZHdOUVp2MENBcnQ0TWZsV3RNSW5kd1JHUUlEQVFBQm8xSXdVREFQQmdOVkhSTUJBZjhFQlRBREFRSC9NQjBHQTFVZERnUVdCQlRKMk9aSWpLcS9TSTM3SXNValhzYlI4cEZQN2pBTEJnTlZIUThFQkFNQ0FRWXdFUVlKWUlaSUFZYjRRZ0VCQkFRREFnQUhNQTBHQ1NxR1NJYjNEUUVCREFVQUE0SUJBUUFmZWQ5T3RWdHhQR3B6TS9KS3g4TUsrWm5XYjI0c0VGMlNXaERhNzhmZ2syRWFzbmsrT1hrTzd1R0tTdm5uTFB1UXFEbzBiZmhhS1lwQ1QvTjNTYk8vbXZ0NHdORzBwc0EySGUvYXBvWEUzdEN1eFdYaGg4SnVkTnlEQzBCaHRTVXE2WDliWVlNOFhzUnE0LzQ1ejY3bnlCYlowVHExRVdMQ1BKci82b1FxTXlNTTFvVFp3WnFETlpjMzd3TkFCelVpVnVtNGdUQW9YZTdwNDJVdVNsdXJZS3hmTzZpeExoaFNOWWtta2lhRHl2QlNwVFFJWitBc2REcUI5OFdIeDBiYUlqSmpCZU1tZSt1L1BpZmVGRjNDUUxSa28wSWxKUWdldDczZlBZNGFzWXhwekx0M1lQNE1lbmg2dTVLVHFucTJWUGd2aGZMTGp1R3libkNDLytERSJdfQ.eyJhY3NVUkwiOiJodHRwczovL25kbS1wcmV2LjNkc3Mtbm9uLXByb2QuY2xvdWQubmV0Y2V0ZXJhLmNvbS9hY3MvY2hhbGxlbmdlIiwiYWNzRXBoZW1QdWJLZXkiOnsia3R5IjoiRUMiLCJ4IjoiMlh6VnhTa25rOGVfbmdoSTNTSTRuOW9hT2xrZ1VYV2pCOWhpU2x6cHFiYyIsInkiOiI4YnFySHF5T29hODRTVTJlYzMxTEFINU9VSXA4dWNOamlOQm5FWm11M2N3IiwiY3J2IjoiUC0yNTYifSwic2RrRXBoZW1QdWJLZXkiOnsia3R5IjoiRUMiLCJjcnYiOiJQLTI1NiIsIngiOiJMWjJxMXJOMFlYUGo2V3J3VVFrUEFva0dWMksxdGthUV9iMVo4alRxQldRIiwieSI6IlV1cDRDaFZtcHlKem84NW5MekMwamZuSzNLRHBNU2lERGE2cmlscmFleWMifX0.u63PWhL6WRZy4d5EXlAlrjXCt3wN_4QH6fEZo32yCpULrlNkBZvOyajM3FnOmZ8NunGRFnCpJCtOWcGU9EbBBtHkK2chdUKZQ0kxFx6YHKotZS7-_Hldw3JpsgILHI7ZfPv2uTM54SQpGyeUET3-j-kOhX-KKEaEXun9FcaGA4o576lL0xvsmfUjmj2Xo8EkIEc4MH2d2DgE-AXFc53UtWboCRB6OXeTCCh7svufVcU4LOK4FsyUUQWgU985NtPNhn3KLhl6LzMWlrvMzQ3vdwrOC6phJya2lcccpf2scDiqf-pKFNuOTLShwk8BscY7jBkMZVhtElNOhbOcKE65jA".to_string())),
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
            sdk_trans_id: Some(sdk_trans_id),
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
        purchase_date: auth_request_json["purchaseDate"].as_str().unwrap().to_string(),
        base64_encoded_challenge_request: if should_challenge { Some(base64_encoded_challenge_request) } else { None },
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

pub async fn acs_trigger_otp_handler(
    form: web::Form<AcsTriggerOtpRequest>,
    settings: web::Data<Settings>,
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
    
    // Build dynamic URLs using server configuration
    let server_url = format!("http://{}:{}", settings.server.host, settings.server.port);
    let fallback_redirect_url = server_url.clone();
    let pay_endpoint = format!("{}/processor/mock/acs/verify-otp?redirectUrl=https://juspay.api.in.end", server_url);

    // Load and populate the HTML template
    let template_content = include_str!("../templates/acs-challenge.html");
    let html_content = template_content
        .replace("{{FALLBACK_REDIRECT_URL}}", &fallback_redirect_url)
        .replace("{{THREE_DS_SERVER_TRANS_ID}}", &three_ds_server_trans_id.to_string())
        .replace("{{PAY_ENDPOINT}}", &pay_endpoint);

    Ok(HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html_content))
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
            if let Err(e) = state.update(&three_ds_server_trans_id, transaction_data.clone()).await {
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
        Ok(None) => {
            Ok(HttpResponse::BadRequest().json(serde_json::json!({
                "error": "Transaction not found"
            })))
        }
        Err(e) => {
            Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to retrieve transaction data: {}", e)
            })))
        }
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
        Ok(None) => {
            Ok(HttpResponse::BadRequest().json(serde_json::json!({
                "error": "Transaction not found"
            })))
        }
        Err(e) => {
            Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to retrieve transaction data: {}", e)
            })))
        }
    }
}
