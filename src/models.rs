use serde::{Deserialize, Serialize};
use uuid::Uuid;

// Version API Models
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionRequest {
    pub card_number: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionResponse {
    pub three_ds_server_trans_id: Uuid,
    pub card_ranges: Vec<CardRange>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CardRange {
    pub acs_info_ind: Vec<String>,
    pub start_range: String,
    pub acs_end_protocol_version: String,
    pub acs_start_protocol_version: String,
    pub end_range: String,
}

// Authenticate API Models
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AuthenticateRequest {
    pub three_ds_server_trans_id: Uuid,
    pub device_channel: String,
    pub message_category: String,
    pub preferred_protocol_version: String,
    pub enforce_preferred_protocol_version: bool,
    pub three_ds_comp_ind: String,
    pub three_ds_requestor: ThreeDSRequestor,
    pub cardholder_account: CardholderAccount,
    pub cardholder: Cardholder,
    pub purchase: Purchase,
    pub acquirer: Acquirer,
    pub merchant: Merchant,
    pub browser_information: BrowserInformation,
    pub device_render_options: DeviceRenderOptions,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ThreeDSRequestor {
    pub three_ds_requestor_authentication_ind: String,
    pub three_ds_requestor_authentication_info: ThreeDSRequestorAuthenticationInfo,
    pub three_ds_requestor_challenge_ind: String,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ThreeDSRequestorAuthenticationInfo {
    pub three_ds_req_auth_method: String,
    pub three_ds_req_auth_timestamp: String,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CardholderAccount {
    pub acct_type: String,
    pub card_expiry_date: String,
    pub scheme_id: String,
    pub acct_number: String,
    pub card_security_code: String,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Cardholder {
    pub addr_match: String,
    pub bill_addr_city: String,
    pub bill_addr_country: String,
    pub bill_addr_line1: String,
    pub bill_addr_line2: String,
    pub bill_addr_line3: String,
    pub bill_addr_post_code: String,
    pub email: String,
    pub home_phone: Phone,
    pub mobile_phone: Phone,
    pub work_phone: Phone,
    pub cardholder_name: String,
    pub ship_addr_city: String,
    pub ship_addr_country: String,
    pub ship_addr_line1: String,
    pub ship_addr_line2: String,
    pub ship_addr_line3: String,
    pub ship_addr_post_code: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Phone {
    pub cc: String,
    pub subscriber: String,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Purchase {
    pub purchase_instal_data: u32,
    pub purchase_amount: u64,
    pub purchase_currency: String,
    pub purchase_exponent: u32,
    pub purchase_date: String,
    pub recurring_expiry: String,
    pub recurring_frequency: u32,
    pub trans_type: String,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Acquirer {
    pub acquirer_bin: String,
    pub acquirer_merchant_id: String,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Merchant {
    pub mcc: String,
    pub merchant_country_code: String,
    pub three_ds_requestor_id: String,
    pub three_ds_requestor_name: String,
    pub merchant_name: String,
    pub results_response_notification_url: String,
    pub notification_url: String,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BrowserInformation {
    pub browser_accept_header: String,
    #[serde(rename = "browserIP")]
    pub browser_ip: String,
    pub browser_language: String,
    pub browser_color_depth: String,
    pub browser_screen_height: u32,
    pub browser_screen_width: u32,
    #[serde(rename = "browserTZ")]
    pub browser_tz: u32,
    pub browser_user_agent: String,
    pub challenge_window_size: String,
    pub browser_java_enabled: bool,
    pub browser_javascript_enabled: bool,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DeviceRenderOptions {
    pub sdk_interface: String,
    pub sdk_ui_type: Vec<String>,
    pub sdk_authentication_type: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthenticateResponse {
    pub purchase_date: String,
    pub base64_encoded_challenge_request: String,
    pub acs_url: String,
    pub three_ds_server_trans_id: Uuid,
    pub authentication_response: AuthenticationResponse,
    pub challenge_request: ChallengeRequest,
    pub acs_challenge_mandated: String,
    pub trans_status: String,
    pub authentication_request: serde_json::Value, // Will be dynamically created
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthenticationResponse {
    #[serde(rename = "acsOperatorID")]
    pub acs_operator_id: String,
    pub ds_reference_number: String,
    pub acs_url: String,
    pub ds_trans_id: Uuid,
    pub message_type: String,
    pub three_ds_server_trans_id: Uuid,
    pub acs_trans_id: Uuid,
    pub acs_challenge_mandated: String,
    pub authentication_type: String,
    pub trans_status: String,
    pub message_version: String,
    pub acs_reference_number: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChallengeRequest {
    pub message_type: String,
    pub three_ds_server_trans_id: Uuid,
    pub acs_trans_id: Uuid,
    pub challenge_window_size: String,
    pub message_version: String,
}

// Results API Models
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ResultsRequest {
    pub acs_trans_id: Uuid,
    pub message_category: String,
    pub eci: String,
    pub message_type: String,
    pub acs_rendering_type: AcsRenderingType,
    pub ds_trans_id: Uuid,
    pub authentication_method: String,
    pub authentication_type: String,
    pub message_version: String,
    pub sdk_trans_id: Uuid,
    pub interaction_counter: String,
    pub authentication_value: String,
    pub trans_status: String,
    pub three_ds_server_trans_id: Uuid,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AcsRenderingType {
    pub acs_ui_template: String,
    pub acs_interface: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultsResponse {
    pub ds_trans_id: Uuid,
    pub message_type: String,
    pub three_ds_server_trans_id: Uuid,
    pub acs_trans_id: Uuid,
    pub sdk_trans_id: Uuid,
    pub results_status: String,
    pub message_version: String,
}

// Final API Models
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinalRequest {
    pub three_ds_server_trans_id: Uuid,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FinalResponse {
    pub eci: String,
    pub authentication_value: String,
    pub three_ds_server_trans_id: Uuid,
    pub results_response: ResultsResponse,
    pub results_request: ResultsRequest,
    pub trans_status: String,
}
