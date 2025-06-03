use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use crate::models::{AuthenticateRequest, ResultsRequest};

#[derive(Debug, Clone)]
pub struct TransactionData {
    pub authenticate_request: AuthenticateRequest,
    pub acs_trans_id: Uuid,
    pub ds_trans_id: Uuid,
    pub sdk_trans_id: Uuid,
    pub results_request: Option<ResultsRequest>,
}

pub type AppState = Arc<Mutex<HashMap<Uuid, TransactionData>>>;

pub fn create_app_state() -> AppState {
    Arc::new(Mutex::new(HashMap::new()))
}
