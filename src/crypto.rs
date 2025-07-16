use aes::cipher::{block_padding::Pkcs7, BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use aes::Aes128;
use aes_gcm::{
    aead::{Aead, AeadInPlace, KeyInit},
    Aes128Gcm, Key, Nonce,
};
use base64::{engine::general_purpose, Engine as _};
use cbc::{Decryptor, Encryptor};
use hmac::{Hmac, Mac};
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use p256::elliptic_curve::sec1::ToEncodedPoint;
use p256::SecretKey;
use rand_core::OsRng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;
use uuid::Uuid;

type Aes128CbcDec = Decryptor<Aes128>;
type Aes128CbcEnc = Encryptor<Aes128>;
type HmacSha256 = Hmac<Sha256>;

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
    let x_bytes = &coords[1..33]; // 32 bytes for x
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
    println!("🔐 Creating ACS signed content JWT");
    println!(
        "  📋 Input acsTransID: {} (length: {})",
        acs_trans_id,
        acs_trans_id.to_string().len()
    );

    // Load certificate and private key
    let cert_base64 = load_certificate(cert_path)?;
    let encoding_key = load_private_key(key_path)?;

    // Create JWT header with x5c certificate chain
    let mut header = Header::new(Algorithm::PS256);
    header.typ = Some("JWT".to_string());
    header.x5c = Some(vec![cert_base64]);

    // Create payload
    let acs_trans_id_str = acs_trans_id.to_string();
    let payload = AcsSignedContentPayload {
        acs_trans_id: acs_trans_id_str.clone(),
        acs_ref_number: acs_ref_number.to_string(),
        acs_url: acs_url.to_string(),
        acs_ephem_pub_key: ephemeral_keys.public_key.clone(),
    };

    println!(
        "  📋 Payload acsTransID: {} (length: {})",
        acs_trans_id_str,
        acs_trans_id_str.len()
    );

    // Sign and encode JWT
    let jwt = encode(&header, &payload, &encoding_key)?;

    println!("  ✅ Generated JWT length: {} characters", jwt.len());

    Ok(jwt)
}

/// Create ACS URL for mobile challenge flows
pub fn create_acs_url(base_url: &str) -> String {
    format!("{}/challenge", base_url.trim_end_matches('/'))
}

/// Calculate derived key for mobile challenge flow using ECDH
/// Implements proper ECDH with ConcatKDF following EMVCo 3DS specification
pub fn calculate_derived_key(
    sdk_public_key_jwk: &str,
    our_private_key: &str,
    platform: &str, // "android" or "ios"
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    println!("🔐 ECDH: Starting shared secret derivation");

    // Parse SDK public key from JWK format
    let sdk_jwk: serde_json::Value = serde_json::from_str(sdk_public_key_jwk)?;
    println!(
        "  - Curve: {}",
        sdk_jwk["crv"].as_str().unwrap_or("unknown")
    );

    let x_b64 = sdk_jwk["x"]
        .as_str()
        .ok_or("Missing x coordinate in SDK public key")?;
    let y_b64 = sdk_jwk["y"]
        .as_str()
        .ok_or("Missing y coordinate in SDK public key")?;

    // Decode x and y coordinates
    let x_bytes = general_purpose::URL_SAFE_NO_PAD.decode(x_b64)?;
    let y_bytes = general_purpose::URL_SAFE_NO_PAD.decode(y_b64)?;
    println!("  - X coordinate length: {} bytes", x_bytes.len());
    println!("  - Y coordinate length: {} bytes", y_bytes.len());

    // Decode our private key from base64url
    let our_private_key_bytes = general_purpose::URL_SAFE_NO_PAD.decode(our_private_key)?;

    // Create our private key from the decoded bytes (32-byte array for P-256)
    if our_private_key_bytes.len() != 32 {
        return Err(format!(
            "Invalid private key length: {} (expected 32)",
            our_private_key_bytes.len()
        )
        .into());
    }
    let mut key_array = [0u8; 32];
    key_array.copy_from_slice(&our_private_key_bytes);

    let our_secret_key = SecretKey::from_bytes(&key_array.into())
        .map_err(|e| format!("Failed to create private key: {}", e))?;

    // Build uncompressed public key: 0x04 || x || y
    let mut public_key_bytes = Vec::with_capacity(65);
    public_key_bytes.push(0x04); // Uncompressed point indicator
    public_key_bytes.extend_from_slice(&x_bytes);
    public_key_bytes.extend_from_slice(&y_bytes);

    // Create SDK public key from the uncompressed bytes
    let sdk_public_key = p256::PublicKey::from_sec1_bytes(&public_key_bytes)
        .map_err(|e| format!("Failed to parse SDK public key: {}", e))?;

    // Perform ECDH to get shared secret (Z)
    let shared_secret = p256::ecdh::diffie_hellman(
        our_secret_key.to_nonzero_scalar(),
        sdk_public_key.as_affine(),
    );
    let shared_secret_bytes = shared_secret.raw_secret_bytes();

    println!("  - Shared Secret: {}", hex::encode(&shared_secret_bytes));

    // Build ConcatKDF OtherInfo per EMVCo spec
    // algorithmID: 4-byte zeros
    let algorithm_id = [0u8; 4];

    // partyUInfo: 4-byte zeros
    let party_u_info = [0u8; 4];

    // partyVInfo: 4-byte big-endian length + sdkReferenceNumber (platform-specific)
    let sdk_reference_number = match platform.to_lowercase().as_str() {
        "android" => "3DS_LOA_SDK_JTPL_020200_00788",
        "ios" => "3DS_LOA_SDK_JTPL_020200_00805",
        _ => {
            return Err(format!(
                "Unsupported platform: {} (supported: android, ios)",
                platform
            )
            .into())
        }
    };

    println!("  - Platform: {}", platform);
    println!("  - SDK Reference Number: {}", sdk_reference_number);
    let mut party_v_info = Vec::new();
    party_v_info.extend_from_slice(&(sdk_reference_number.len() as u32).to_be_bytes());
    party_v_info.extend_from_slice(sdk_reference_number.as_bytes());

    // suppPubInfo: 4-byte big-endian representation of 256 (key length in bits)
    let supp_pub_info = [0u8, 0u8, 0x01, 0x00]; // 256 in big-endian

    // Concatenate OtherInfo: algorithmID || partyUInfo || partyVInfo || suppPubInfo
    let mut other_info = Vec::new();
    other_info.extend_from_slice(&algorithm_id);
    other_info.extend_from_slice(&party_u_info);
    other_info.extend_from_slice(&party_v_info);
    other_info.extend_from_slice(&supp_pub_info);

    println!("  - OtherInfo: {}", hex::encode(&other_info));

    // ConcatKDF counter: 4-byte big-endian integer with value 1
    let counter = [0u8, 0u8, 0u8, 0x01]; // 1 in big-endian

    // Build the full KDF input: counter || sharedSecret || OtherInfo
    let mut kdf_input = Vec::new();
    kdf_input.extend_from_slice(&counter);
    kdf_input.extend_from_slice(&shared_secret_bytes);
    kdf_input.extend_from_slice(&other_info);

    println!("  - KDF Input: {}", hex::encode(&kdf_input));

    // Derive the key by computing SHA-256 hash of the KDF input
    let derived_key_bytes = Sha256::digest(&kdf_input);

    // Take first 32 bytes for AES-256 or first 16 bytes for AES-128
    let derived_key = &derived_key_bytes[0..32]; // Use full 32 bytes for more robust key

    println!("  - Derived Key: {}", hex::encode(derived_key));
    println!("  ✅ Derived key length: {} bytes", derived_key.len());

    Ok(derived_key.to_vec())
}

/// Decrypt JWE challenge request from SDK
/// This implementation supports both Android (A128CBC-HS256) and iOS (A128GCM) platforms
pub async fn decrypt_challenge_request(
    jwe_string: &str,
    derived_key_buffer: &[u8],
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    println!("🔓 Attempting to decrypt challenge request...");

    // Extract JWE parts
    let jwe_parts: Vec<&str> = jwe_string.split('.').collect();
    if jwe_parts.len() != 5 {
        return Err("Invalid JWE structure, expected 5 parts.".into());
    }

    // Get header information for platform detection
    let header_data = general_purpose::URL_SAFE_NO_PAD.decode(jwe_parts[0])?;
    let header_json: serde_json::Value = serde_json::from_slice(&header_data)?;
    let encryption = header_json["enc"].as_str().unwrap_or("unknown");

    // Detect platform based on encryption algorithm
    let platform = match encryption {
        "A128CBC-HS256" => "Android",
        "A128GCM" => "iOS",
        _ => "Unknown",
    };

    println!("🔍 Platform Detection:");
    println!("  - Encryption Algorithm: {}", encryption);
    println!("  - Detected Platform: {}", platform);
    println!("  - Derived Key Length: {} bytes", derived_key_buffer.len());

    // For logging: decode Base64Url parts
    let iv = general_purpose::URL_SAFE_NO_PAD.decode(jwe_parts[2])?;
    let ciphertext = general_purpose::URL_SAFE_NO_PAD.decode(jwe_parts[3])?;
    let auth_tag = general_purpose::URL_SAFE_NO_PAD.decode(jwe_parts[4])?;

    println!("📋 JWE Components:");
    println!("  - IV: {}", hex::encode(&iv));
    println!("  - Ciphertext Length: {} bytes", ciphertext.len());
    println!("  - Authentication Tag: {}", hex::encode(&auth_tag));

    // Perform platform-specific decryption
    let plaintext = match encryption {
        "A128CBC-HS256" => {
            println!("🤖 Android Decryption: Using A128CBC-HS256");

            // Android uses the full 32-byte derived key (16 for HMAC, 16 for AES per JWE spec)
            if derived_key_buffer.len() != 32 {
                return Err(format!(
                    "Invalid derived key length for Android: {} (expected 32)",
                    derived_key_buffer.len()
                )
                .into());
            }

            let hmac_key = &derived_key_buffer[0..16];
            let aes_key = &derived_key_buffer[16..32];

            println!("  - HMAC Key: {} bytes", hmac_key.len());
            println!("  - AES Key: {} bytes", aes_key.len());
            println!("  - Android HMAC Key: {}", hex::encode(hmac_key));
            println!("  - Android AES Key: {}", hex::encode(aes_key));

            // Verify HMAC tag according to JWE spec (RFC 7516)
            let mut mac = <HmacSha256 as Mac>::new_from_slice(hmac_key)
                .map_err(|e| format!("HMAC initialization failed: {}", e))?;

            // The HMAC input for A128CBC-HS256 must follow the JWE specification:
            // HMAC input = AAD || IV || Ciphertext || AAD Length
            // where AAD is the JWE Protected Header (base64url encoded)

            // 1. AAD (Additional Authenticated Data) - the base64url encoded header
            let aad = jwe_parts[0].as_bytes();
            mac.update(aad);

            // 2. IV - raw bytes
            mac.update(&iv);

            // 3. Ciphertext - raw bytes
            mac.update(&ciphertext);

            // 4. AAD Length - 64-bit big-endian representation of the length of AAD in bits
            let aad_bits = (aad.len() * 8) as u64;
            let aad_bits_be = aad_bits.to_be_bytes(); // Convert to big-endian byte array
            mac.update(&aad_bits_be);

            let computed_hmac = mac.finalize().into_bytes();

            // Check if the first 16 bytes of the computed HMAC match the auth tag
            let truncated_hmac = &computed_hmac[0..16];
            if truncated_hmac != auth_tag.as_slice() {
                return Err("HMAC verification failed - authentication tag does not match".into());
            }

            // Decrypt with AES-128-CBC
            let cipher = Aes128CbcDec::new(aes_key.into(), iv.as_slice().into());
            let mut buffer = ciphertext.clone();

            let plaintext_len = cipher
                .decrypt_padded_mut::<Pkcs7>(&mut buffer)
                .map_err(|e| format!("AES-CBC decryption failed: {}", e))?
                .len();

            buffer.truncate(plaintext_len);
            buffer
        }
        "A128GCM" => {
            println!("🍎 iOS Decryption: Using A128GCM");

            // iOS uses only the first 16 bytes of the derived key (matching JavaScript implementation)
            if derived_key_buffer.len() < 16 {
                return Err(format!(
                    "Insufficient key material for iOS: {} bytes (need at least 16)",
                    derived_key_buffer.len()
                )
                .into());
            }

            let ios_key = &derived_key_buffer[0..16];
            println!(
                "  - Using key slice: {} bytes (first 16 bytes of derived key)",
                ios_key.len()
            );
            println!("  - iOS Key: {}", hex::encode(ios_key));

            // For A128GCM in JWE, we need to include AAD (Additional Authenticated Data)
            // AAD is the ASCII bytes of the base64url-encoded JWE Protected Header
            let aad = jwe_parts[0].as_bytes();
            println!("  - AAD (JWE Header): {}", String::from_utf8_lossy(aad));
            println!("  - AAD length: {} bytes", aad.len());

            // Check IV length - should be 12 bytes for GCM
            if iv.len() != 12 {
                println!(
                    "  ⚠️  Warning: IV length is {} bytes, expected 12 for GCM",
                    iv.len()
                );
                if iv.len() > 12 {
                    println!("  - Truncating IV to first 12 bytes");
                } else if iv.len() < 12 {
                    return Err(
                        format!("IV too short for GCM: {} bytes (need 12)", iv.len()).into(),
                    );
                }
            }

            // Use first 12 bytes of IV for GCM nonce
            let nonce_bytes = if iv.len() >= 12 { &iv[0..12] } else { &iv };

            // Create cipher with iOS key (16 bytes)
            let key = Key::<Aes128Gcm>::from_slice(ios_key);
            let cipher = Aes128Gcm::new(key);
            let nonce = Nonce::from_slice(nonce_bytes);

            // For A128GCM, we need to use decrypt_in_place_detached with AAD
            let mut ciphertext_buffer = ciphertext.clone();

            cipher
                .decrypt_in_place_detached(
                    nonce,
                    aad,
                    &mut ciphertext_buffer,
                    auth_tag.as_slice().into(),
                )
                .map_err(|e| format!("iOS A128GCM decryption failed: {}", e))?;

            ciphertext_buffer
        }
        _ => {
            return Err(format!("Unsupported encryption algorithm: {} (supported: A128GCM for iOS, A128CBC-HS256 for Android)", encryption).into());
        }
    };

    // Parse JSON
    let decrypted_payload = serde_json::from_slice(&plaintext)?;
    println!("✅ {} Decryption Successful!", platform);
    println!(
        "📋 Decrypted Payload: {}",
        serde_json::to_string(&decrypted_payload)?
    );

    Ok(decrypted_payload)
}

/// Encrypt JWE challenge response for SDK
/// Supports both Android (A128CBC-HS256) and iOS (A128GCM) platforms
pub async fn encrypt_challenge_response(
    response_data: &serde_json::Value,
    acs_trans_id: &str,
    derived_key: &[u8],
    platform: &str, // "android" or "ios"
) -> Result<String, Box<dyn std::error::Error>> {
    println!("🔒 JWE Encryption: Encrypting challenge response");
    println!("  - Target Platform: {}", platform);

    // Serialize response to JSON
    let plaintext = serde_json::to_vec(response_data)?;
    println!("  - Response size: {} bytes", plaintext.len());

    // Platform-specific encryption
    match platform.to_lowercase().as_str() {
        "android" => {
            println!("🤖 Android Encryption: Using A128CBC-HS256");

            // Android uses the full 32-byte derived key (16 for HMAC, 16 for AES per JWE spec)
            if derived_key.len() != 32 {
                return Err(format!(
                    "Invalid derived key length for Android: {} (expected 32)",
                    derived_key.len()
                )
                .into());
            }

            let hmac_key = &derived_key[0..16]; // First 16 bytes for HMAC (per JWE spec)
            let aes_key = &derived_key[16..32]; // Last 16 bytes for AES-128

            println!("  🔑 Android AES key: {} bytes", aes_key.len());
            println!("  🔑 Android HMAC key: {} bytes", hmac_key.len());

            // Generate random IV (16 bytes for CBC)
            let mut iv = [0u8; 16];
            use rand_core::RngCore;
            OsRng.fill_bytes(&mut iv);

            // Encrypt with AES-128-CBC
            let cipher = Aes128CbcEnc::new(aes_key.into(), iv.as_slice().into());

            // Prepare buffer with space for padding (up to one full block)
            let mut buffer = plaintext.clone();
            buffer.resize(plaintext.len() + 16, 0); // Add space for padding

            // Encrypt with padding
            let ciphertext_slice = cipher
                .encrypt_padded_mut::<Pkcs7>(&mut buffer, plaintext.len())
                .map_err(|e| format!("AES-CBC encryption failed: {}", e))?;

            let ciphertext = ciphertext_slice.to_vec();
            println!(
                "  ✅ Encrypted {} bytes to {} bytes",
                plaintext.len(),
                ciphertext.len()
            );

            // Create JWE header for Android
            let header = serde_json::json!({
                "alg": "dir",
                "enc": "A128CBC-HS256",
                "kid": acs_trans_id
            });

            let header_json_str = serde_json::to_string(&header)?;
            println!("  📋 Android JWE header: {}", header_json_str);

            let header_b64 = general_purpose::URL_SAFE_NO_PAD.encode(&header_json_str);
            let encrypted_key_b64 = ""; // Empty for direct key agreement
            let iv_b64 = general_purpose::URL_SAFE_NO_PAD.encode(&iv);
            let ciphertext_b64 = general_purpose::URL_SAFE_NO_PAD.encode(&ciphertext);

            // Calculate HMAC according to JWE spec (RFC 7516)
            let mut mac = <HmacSha256 as Mac>::new_from_slice(hmac_key)
                .map_err(|e| format!("HMAC initialization failed: {}", e))?;

            // The HMAC input for A128CBC-HS256 must follow the JWE specification:
            // HMAC input = AAD || IV || Ciphertext || AAD Length
            // where AAD is the JWE Protected Header (base64url encoded)

            // 1. AAD (Additional Authenticated Data) - the base64url encoded header
            let aad = header_b64.as_bytes();
            mac.update(aad);

            // 2. IV - raw bytes (not base64 encoded)
            mac.update(&iv);

            // 3. Ciphertext - raw bytes (not base64 encoded)
            mac.update(&ciphertext);

            // 4. AAD Length - 64-bit big-endian representation of the length of AAD in bits
            let aad_bits = (aad.len() * 8) as u64;
            let aad_bits_be = aad_bits.to_be_bytes(); // Convert to big-endian byte array
            mac.update(&aad_bits_be);

            let hmac_result = mac.finalize().into_bytes();
            // For A128CBC-HS256, use truncated HMAC (first 16 bytes)
            let truncated_hmac = &hmac_result[0..16];
            let tag_b64 = general_purpose::URL_SAFE_NO_PAD.encode(truncated_hmac);

            println!(
                "  📋 Android HMAC tag: {} bytes (truncated from 32)",
                truncated_hmac.len()
            );

            // Construct JWE
            let jwe = format!(
                "{}.{}.{}.{}.{}",
                header_b64, encrypted_key_b64, iv_b64, ciphertext_b64, tag_b64
            );

            println!("  ✅ Android encrypted JWE length: {} bytes", jwe.len());
            Ok(jwe)
        }
        "ios" => {
            println!("🍎 iOS Encryption: Using A128GCM");

            // iOS uses the LAST 16 bytes of the derived key for encryption (matching JavaScript implementation)
            // JavaScript: Buffer.from(derivedKey.slice(32), 'hex') = last 16 bytes
            if derived_key.len() < 32 {
                return Err(format!(
                    "Insufficient key material for iOS: {} bytes (need at least 32)",
                    derived_key.len()
                )
                .into());
            }

            let ios_key = &derived_key[16..32]; // Last 16 bytes for encryption
            println!(
                "  🔑 iOS encryption key: {} bytes (last 16 bytes of derived key)",
                ios_key.len()
            );
            println!("  🔑 iOS encryption key: {}", hex::encode(ios_key));

            // Generate random IV (12 bytes for GCM)
            let mut iv = [0u8; 12];
            use rand_core::RngCore;
            OsRng.fill_bytes(&mut iv);

            // Create JWE header for iOS first (needed for AAD)
            let header = serde_json::json!({
                "alg": "dir",
                "enc": "A128GCM",
                "kid": acs_trans_id
            });

            let header_json_str = serde_json::to_string(&header)?;
            println!("  📋 iOS JWE header: {}", header_json_str);

            let header_b64 = general_purpose::URL_SAFE_NO_PAD.encode(&header_json_str);

            // For A128GCM in JWE, we need to include AAD (Additional Authenticated Data)
            // AAD is the ASCII bytes of the base64url-encoded JWE Protected Header
            let aad = header_b64.as_bytes();
            println!("  - AAD (JWE Header): {}", String::from_utf8_lossy(aad));
            println!("  - AAD length: {} bytes", aad.len());

            // Create cipher with iOS key (16 bytes)
            let key = Key::<Aes128Gcm>::from_slice(ios_key);
            let cipher = Aes128Gcm::new(key);
            let nonce = Nonce::from_slice(&iv);

            // Encrypt with A128GCM using AAD
            let mut plaintext_buffer = plaintext.clone();
            let auth_tag = cipher
                .encrypt_in_place_detached(nonce, aad, &mut plaintext_buffer)
                .map_err(|e| format!("iOS A128GCM encryption failed: {}", e))?;

            let ciphertext = plaintext_buffer;
            println!(
                "  ✅ iOS encrypted {} bytes to {} bytes + {} byte tag",
                plaintext.len(),
                ciphertext.len(),
                auth_tag.len()
            );

            let encrypted_key_b64 = ""; // Empty for direct key agreement
            let iv_b64 = general_purpose::URL_SAFE_NO_PAD.encode(&iv);
            let ciphertext_b64 = general_purpose::URL_SAFE_NO_PAD.encode(&ciphertext);
            let tag_b64 = general_purpose::URL_SAFE_NO_PAD.encode(&auth_tag);

            // Construct JWE
            let jwe = format!(
                "{}.{}.{}.{}.{}",
                header_b64, encrypted_key_b64, iv_b64, ciphertext_b64, tag_b64
            );

            println!("  ✅ iOS encrypted JWE length: {} bytes", jwe.len());
            Ok(jwe)
        }
        _ => Err(format!(
            "Unsupported platform: {} (supported: android, ios)",
            platform
        )
        .into()),
    }
}
/// Encrypt JWE challenge response for SDK (Legacy Android-only function)
/// This function is kept for backward compatibility and defaults to Android encryption
/// For new code, use encrypt_challenge_response_for_platform instead
pub async fn encrypt_challenge_response_legacy(
    response_data: &serde_json::Value,
    acs_trans_id: &str,
    derived_key: &[u8],
) -> Result<String, Box<dyn std::error::Error>> {
    encrypt_challenge_response(response_data, acs_trans_id, derived_key, "android").await
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[tokio::test]
    async fn test_a128cbc_hs256_round_trip() {
        // Test A128CBC-HS256 encryption/decryption round trip
        println!("🧪 Testing A128CBC-HS256 round-trip encryption/decryption");

        // Generate two key pairs (simulating SDK and ACS)
        let sdk_keys = generate_ephemeral_key_pair().expect("Failed to generate SDK keys");
        let acs_keys = generate_ephemeral_key_pair().expect("Failed to generate ACS keys");

        // Convert SDK public key to JWK format
        let sdk_public_jwk = serde_json::json!({
            "kty": sdk_keys.public_key.kty,
            "crv": sdk_keys.public_key.crv,
            "x": sdk_keys.public_key.x,
            "y": sdk_keys.public_key.y
        });

        // Perform ECDH key derivation (ACS side) - using Android for this test
        let derived_key_acs = calculate_derived_key(
            &serde_json::to_string(&sdk_public_jwk).unwrap(),
            &acs_keys.private_key,
            "android",
        )
        .expect("Failed to derive key on ACS side");

        // Test data to encrypt
        let test_data = serde_json::json!({
            "messageType": "CRes",
            "messageVersion": "2.2.0",
            "acsTransID": "test-acs-trans-id",
            "challengeCompletionInd": "Y",
            "transStatus": "Y"
        });

        println!("  📋 Original data: {}", test_data);

        // Encrypt the data (test Android encryption)
        let encrypted_jwe = encrypt_challenge_response(
            &test_data,
            "test-acs-trans-id",
            &derived_key_acs,
            "android",
        )
        .await
        .expect("Failed to encrypt data");

        println!("  🔒 Encrypted JWE: {}", encrypted_jwe);

        // Decrypt the data back
        let decrypted_data = decrypt_challenge_request(&encrypted_jwe, &derived_key_acs)
            .await
            .expect("Failed to decrypt data");

        println!("  🔓 Decrypted data: {}", decrypted_data);

        // Verify the round-trip worked
        assert_eq!(
            test_data, decrypted_data,
            "Round-trip encryption/decryption failed"
        );

        println!("  ✅ A128CBC-HS256 round-trip test successful!");
    }

    #[tokio::test]
    async fn test_ios_a128gcm_round_trip() {
        // Test A128GCM encryption/decryption round trip for iOS
        println!("🧪 Testing iOS A128GCM round-trip encryption/decryption");

        // Generate two key pairs (simulating SDK and ACS)
        let sdk_keys = generate_ephemeral_key_pair().expect("Failed to generate SDK keys");
        let acs_keys = generate_ephemeral_key_pair().expect("Failed to generate ACS keys");

        // Convert SDK public key to JWK format
        let sdk_public_jwk = serde_json::json!({
            "kty": sdk_keys.public_key.kty,
            "crv": sdk_keys.public_key.crv,
            "x": sdk_keys.public_key.x,
            "y": sdk_keys.public_key.y
        });

        // Perform ECDH key derivation (ACS side) - using iOS for this test
        let derived_key_acs = calculate_derived_key(
            &serde_json::to_string(&sdk_public_jwk).unwrap(),
            &acs_keys.private_key,
            "ios",
        )
        .expect("Failed to derive key on ACS side");

        // Test data to encrypt
        let test_data = serde_json::json!({
            "messageType": "CRes",
            "messageVersion": "2.2.0",
            "acsTransID": "test-acs-trans-id",
            "challengeCompletionInd": "Y",
            "transStatus": "Y"
        });

        println!("  📋 Original data: {}", test_data);

        // Encrypt the data (test iOS encryption)
        let encrypted_jwe =
            encrypt_challenge_response(&test_data, "test-acs-trans-id", &derived_key_acs, "ios")
                .await
                .expect("Failed to encrypt data");

        println!("  🔒 Encrypted JWE: {}", encrypted_jwe);

        // Decrypt the data back
        let decrypted_data = decrypt_challenge_request(&encrypted_jwe, &derived_key_acs)
            .await
            .expect("Failed to decrypt data");

        println!("  🔓 Decrypted data: {}", decrypted_data);

        // Verify the round-trip worked
        assert_eq!(
            test_data, decrypted_data,
            "iOS round-trip encryption/decryption failed"
        );

        println!("  ✅ iOS A128GCM round-trip test successful!");
    }

    #[tokio::test]
    async fn test_ecdh_consistency() {
        // Test that ECDH produces consistent results
        println!("🧪 Testing ECDH consistency");

        let sdk_keys = generate_ephemeral_key_pair().expect("Failed to generate SDK keys");
        let acs_keys = generate_ephemeral_key_pair().expect("Failed to generate ACS keys");

        // Convert keys to JWK format
        let sdk_public_jwk = serde_json::json!({
            "kty": sdk_keys.public_key.kty,
            "crv": sdk_keys.public_key.crv,
            "x": sdk_keys.public_key.x,
            "y": sdk_keys.public_key.y
        });

        let acs_public_jwk = serde_json::json!({
            "kty": acs_keys.public_key.kty,
            "crv": acs_keys.public_key.crv,
            "x": acs_keys.public_key.x,
            "y": acs_keys.public_key.y
        });

        // Derive keys from both perspectives - using Android for consistency test
        let derived_key_1 = calculate_derived_key(
            &serde_json::to_string(&sdk_public_jwk).unwrap(),
            &acs_keys.private_key,
            "android",
        )
        .expect("Failed to derive key 1");

        let derived_key_2 = calculate_derived_key(
            &serde_json::to_string(&acs_public_jwk).unwrap(),
            &sdk_keys.private_key,
            "android",
        )
        .expect("Failed to derive key 2");

        println!("  🔑 Derived key 1: {}", hex::encode(&derived_key_1));
        println!("  🔑 Derived key 2: {}", hex::encode(&derived_key_2));

        // Both perspectives should produce the same derived key
        assert_eq!(
            derived_key_1, derived_key_2,
            "ECDH should produce same key from both perspectives"
        );

        println!("  ✅ ECDH consistency test successful!");
    }
}
