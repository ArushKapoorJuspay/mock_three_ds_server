# Certificate Structure for 3DS Mock Server

This directory shows the expected certificate structure for the 3DS Mock Server. **The actual certificates are generated using the `generate-certs.sh` script and are NOT stored in version control for security reasons.**

## Required Files

The mock server expects these certificate files in the `certs/` directory:

```
certs/
├── acs-cert.pem      # ACS certificate (public key)
└── acs-private-key.pem # ACS private key (NEVER commit to Git)
```

## Certificate Generation

To generate certificates for development:

```bash
# Run the certificate generation script
./generate-certs.sh

# The script will create:
# - certs/acs-cert.pem (self-signed certificate)
# - certs/acs-private-key.pem (RSA private key)
```

## Certificate Requirements

### ACS Certificate (`acs-cert.pem`)
- **Format**: X.509 PEM format
- **Key Size**: 2048-bit RSA minimum
- **Validity**: 365 days (for development)
- **Subject**: Should include appropriate organizational details
- **Extensions**: 
  - Subject Alternative Names (SAN) for localhost
  - Key usage: digitalSignature, keyEncipherment, dataEncipherment
  - Extended key usage: serverAuth, clientAuth

### ACS Private Key (`acs-private-key.pem`)
- **Format**: PKCS#8 or PKCS#1 PEM format
- **Key Size**: 2048-bit RSA minimum
- **Permissions**: 600 (readable only by owner)
- **Security**: NEVER commit to version control

## Example Certificate Information

When certificates are generated, they will have structure similar to:

```
Certificate:
    Data:
        Version: 3 (0x2)
        Serial Number: <random>
        Signature Algorithm: sha256WithRSAEncryption
        Issuer: C=US, ST=Development, L=Local, O=Mock3DS, OU=ACS, CN=localhost, emailAddress=dev@mock3ds.local
        Validity
            Not Before: <generation_date>
            Not After : <expiry_date>
        Subject: C=US, ST=Development, L=Local, O=Mock3DS, OU=ACS, CN=localhost, emailAddress=dev@mock3ds.local
        Subject Public Key Info:
            Public Key Algorithm: rsaEncryption
                RSA Public-Key: (2048 bit)
        X509v3 extensions:
            X509v3 Key Usage:
                Digital Signature, Key Encipherment, Data Encipherment
            X509v3 Extended Key Usage:
                TLS Web Server Authentication, TLS Web Client Authentication
            X509v3 Subject Alternative Name:
                DNS:localhost, DNS:*.localhost, DNS:mock-acs-server.local, DNS:*.mock-acs-server.local, IP:127.0.0.1, IP:::1
```

## JWT Signing Process

The certificates are used for:

1. **Loading**: Certificate and private key are loaded from PEM files
2. **JWT Header**: Certificate is included in x5c header (base64-encoded, no PEM headers)
3. **JWT Signing**: Private key signs the JWT using PS256 algorithm
4. **Payload**: Contains `acsTransID`, `acsRefNumber`, `acsURL`, `acsEphemPubKey`

## Security Notes

### Development
- ✅ Self-signed certificates are acceptable
- ✅ Generated locally by each developer
- ✅ Not shared between environments
- ⚠️ Only for localhost/development use

### Production
- ❌ Never use self-signed certificates
- ✅ Use certificates from trusted Certificate Authority
- ✅ Implement proper certificate rotation
- ✅ Use Hardware Security Modules (HSM) for private keys
- ✅ Monitor certificate expiration

## Troubleshooting

### Certificate Not Found Errors
If you see errors like "Failed to load certificates", run:
```bash
./generate-certs.sh
```

### Certificate Validation Errors
If certificates exist but validation fails:
```bash
# Check certificate validity
openssl x509 -in certs/acs-cert.pem -text -noout

# Check private key
openssl rsa -in certs/acs-private-key.pem -check

# Verify they match
openssl x509 -in certs/acs-cert.pem -noout -modulus | openssl md5
openssl rsa -in certs/acs-private-key.pem -noout -modulus | openssl md5
# The two MD5 hashes should match
```

### Permission Errors
Ensure proper permissions:
```bash
chmod 644 certs/acs-cert.pem      # Read-only for certificate
chmod 600 certs/acs-private-key.pem  # Owner-only for private key
```

## Integration with 3DS Protocol

The certificates enable:

1. **Dynamic ACS Signed Content**: JWT generation for mobile friction flows
2. **3DS 2.2.0 Compliance**: Proper PS256 signing and x5c certificate chains
3. **Ephemeral Key Security**: Certificate-based validation of ephemeral keys
4. **EMVCo Standards**: Following industry best practices for 3DS implementations

For more details, see `DYNAMIC_ACS_SIGNED_CONTENT.md` in the project root.
