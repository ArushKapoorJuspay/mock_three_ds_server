use actix_web::{web, HttpResponse, Result};
use uuid::Uuid;
use base64::{Engine as _, engine::general_purpose};

use crate::models::*;
use crate::state::{AppState, TransactionData};

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
    state: web::Data<AppState>,
) -> Result<HttpResponse> {
    let three_ds_server_trans_id = req.three_ds_server_trans_id;
    let acs_trans_id = Uuid::new_v4();
    let ds_trans_id = Uuid::new_v4();
    let sdk_trans_id = Uuid::new_v4();
    
    // Determine if challenge is required based on card number
    let card_number = &req.cardholder_account.acct_number;
    let is_challenge = card_number.ends_with("4001");
    let trans_status = if is_challenge { "C" } else { "Y" };
    let acs_challenge_mandated = if is_challenge { "Y" } else { "N" };

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
    };
    
    state.lock().unwrap().insert(three_ds_server_trans_id, transaction_data);

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

    let authentication_response = AuthenticationResponse {
        acs_operator_id: "MOCK_ACS_NEW".to_string(),
        ds_reference_number: "MOCK_DS".to_string(),
        acs_url: "https://integ-expresscheckout-api.juspay.in/processor/mock/acs/trigger-otp?redirectUrl=https://sandbox.juspay.in".to_string(),
        ds_trans_id,
        message_type: "ARes".to_string(),
        three_ds_server_trans_id,
        acs_trans_id,
        acs_challenge_mandated: acs_challenge_mandated.to_string(),
        authentication_type: "02".to_string(),
        trans_status: trans_status.to_string(),
        message_version: "2.2.0".to_string(),
        acs_reference_number: "issuer2".to_string(),
    };

    let response = AuthenticateResponse {
        purchase_date: auth_request_json["purchaseDate"].as_str().unwrap().to_string(),
        base64_encoded_challenge_request,
        acs_url: authentication_response.acs_url.clone(),
        three_ds_server_trans_id,
        authentication_response,
        challenge_request,
        acs_challenge_mandated: acs_challenge_mandated.to_string(),
        trans_status: trans_status.to_string(),
        authentication_request: auth_request_json,
    };

    Ok(HttpResponse::Ok().json(response))
}

pub async fn results_handler(
    req: web::Json<ResultsRequest>,
    state: web::Data<AppState>,
) -> Result<HttpResponse> {
    let three_ds_server_trans_id = req.three_ds_server_trans_id;
    
    // Store the results request in state
    let mut state_guard = state.lock().unwrap();
    if let Some(transaction_data) = state_guard.get_mut(&three_ds_server_trans_id) {
        transaction_data.results_request = Some(req.into_inner());
        
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
    } else {
        Ok(HttpResponse::BadRequest().json(serde_json::json!({
            "error": "Transaction not found"
        })))
    }
}

pub async fn final_handler(
    req: web::Json<FinalRequest>,
    state: web::Data<AppState>,
) -> Result<HttpResponse> {
    let three_ds_server_trans_id = req.three_ds_server_trans_id;
    
    let state_guard = state.lock().unwrap();
    if let Some(transaction_data) = state_guard.get(&three_ds_server_trans_id) {
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
    } else {
        Ok(HttpResponse::BadRequest().json(serde_json::json!({
            "error": "Transaction not found"
        })))
    }
}
