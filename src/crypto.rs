use jsonwebtoken::{encode, Header, EncodingKey, Algorithm};
use p256::SecretKey;
use p256::elliptic_curve::sec1::ToEncodedPoint;
use base64::{Engine as _, engine::general_purpose};
use rand_core::OsRng;
use serde::{Serialize, Deserialize};
use std::fs;
use std::path::Path;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EphemeralKeyPair {
    pub private_key: String, // Base64url encoded d value
    pub public_key: AcsEphemPubKey,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcsEphemPubKey {
    pub kty: String,
    pub crv: String,
    pub x: String,
    pub y: String,
}

#[derive(Debug, Serialize)]
struct AcsSignedContentPayload {
    #[serde(rename = "acsTransID")]
    acs_trans_id: String,
    #[serde(rename = "acsRefNumber")]
    acs_ref_number: String,
    #[serde(rename = "acsURL")]
    acs_url: String,
    #[serde(rename = "acsEphemPubKey")]
    acs_ephem_pub_key: AcsEphemPubKey,
}

/// Generate ephemeral ECDSA P-256 key pair for 3DS transactions
pub fn generate_ephemeral_key_pair() -> Result<EphemeralKeyPair, Box<dyn std::error::Error>> {
    // Generate a new random private key
    let private_key = SecretKey::random(&mut OsRng);
    let public_key = private_key.public_key();

    // Get the encoded point for the public key
    let encoded_point = public_key.to_encoded_point(false); // uncompressed format
    
    // Extract x and y coordinates (skip the first byte which is 0x04 for uncompressed)
    let coords = encoded_point.as_bytes();
    let x_bytes = &coords[1..33];  // 32 bytes for x
    let y_bytes = &coords[33..65]; // 32 bytes for y

    // Encode as base64url
    let x = general_purpose::URL_SAFE_NO_PAD.encode(x_bytes);
    let y = general_purpose::URL_SAFE_NO_PAD.encode(y_bytes);

    // Get the private key scalar (d value)
    let d_bytes = private_key.to_bytes();
    let d = general_purpose::URL_SAFE_NO_PAD.encode(d_bytes.as_slice());

    Ok(EphemeralKeyPair {
        private_key: d,
        public_key: AcsEphemPubKey {
            kty: "EC".to_string(),
            crv: "P-256".to_string(),
            x,
            y,
        },
    })
}

/// Load and format certificate for x5c header
pub fn load_certificate(cert_path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let cert_content = fs::read_to_string(cert_path)?;
    
    // Remove PEM headers and footers, and all whitespace
    let cert_base64 = cert_content
        .lines()
        .filter(|line| !line.starts_with("-----"))
        .collect::<Vec<_>>()
        .join("");
    
    Ok(cert_base64)
}

/// Load private key from PEM file
pub fn load_private_key(key_path: &Path) -> Result<EncodingKey, Box<dyn std::error::Error>> {
    let key_content = fs::read(key_path)?;
    
    // Try to parse as PKCS#8 first
    if let Ok(encoding_key) = EncodingKey::from_rsa_pem(&key_content) {
        return Ok(encoding_key);
    }
    
    // If that fails, try EC key
    if let Ok(encoding_key) = EncodingKey::from_ec_pem(&key_content) {
        return Ok(encoding_key);
    }
    
    // Try PKCS#1 RSA
    EncodingKey::from_rsa_pem(&key_content)
        .map_err(|e| format!("Failed to load private key: {}", e).into())
}

/// Create ACS signed content JWT for mobile flows
pub fn create_acs_signed_content(
    acs_trans_id: Uuid,
    acs_ref_number: &str,
    acs_url: &str,
    ephemeral_keys: &EphemeralKeyPair,
    cert_path: &Path,
    key_path: &Path,
) -> Result<String, Box<dyn std::error::Error>> {
    // Load certificate and private key
    let cert_base64 = load_certificate(cert_path)?;
    let encoding_key = load_private_key(key_path)?;

    // Create JWT header with x5c certificate chain
    let mut header = Header::new(Algorithm::PS256);
    header.typ = Some("JWT".to_string());
    header.x5c = Some(vec![cert_base64]);

    // Create payload
    let payload = AcsSignedContentPayload {
        acs_trans_id: acs_trans_id.to_string(),
        acs_ref_number: acs_ref_number.to_string(),
        acs_url: acs_url.to_string(),
        acs_ephem_pub_key: ephemeral_keys.public_key.clone(),
    };

    // Sign and encode JWT
    let jwt = encode(&header, &payload, &encoding_key)?;
    
    Ok(jwt)
}

/// Create ACS URL for mobile challenge flows
pub fn create_acs_url(base_url: &str) -> String {
    format!("{}/challenge", base_url.trim_end_matches('/'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_generate_ephemeral_key_pair() {
        let result = generate_ephemeral_key_pair();
        assert!(result.is_ok());
        
        let keys = result.unwrap();
        assert_eq!(keys.public_key.kty, "EC");
        assert_eq!(keys.public_key.crv, "P-256");
        assert!(!keys.public_key.x.is_empty());
        assert!(!keys.public_key.y.is_empty());
        assert!(!keys.private_key.is_empty());
    }

    #[test]
    fn test_create_acs_url() {
        assert_eq!(
            create_acs_url("https://example.com"),
            "https://example.com/challenge"
        );
        assert_eq!(
            create_acs_url("https://example.com/"),
            "https://example.com/challenge"
        );
    }
}
