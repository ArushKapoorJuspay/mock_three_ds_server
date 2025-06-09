#!/bin/bash

# 3DS Mock Server Certificate Generation Script
# This script generates self-signed certificates for development use only
# WARNING: These certificates are NOT suitable for production use

set -e  # Exit on any error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
CERTS_DIR="certs"
CERT_FILE="$CERTS_DIR/acs-cert.pem"
KEY_FILE="$CERTS_DIR/acs-private-key.pem"
VALIDITY_DAYS=365

# Certificate details
COUNTRY="US"
STATE="Development"
CITY="Local"
ORGANIZATION="Mock3DS"
ORGANIZATIONAL_UNIT="ACS"
COMMON_NAME="localhost"
EMAIL="dev@mock3ds.local"

print_header() {
    echo -e "${BLUE}"
    echo "==============================================="
    echo "  3DS Mock Server Certificate Generator"
    echo "==============================================="
    echo -e "${NC}"
    echo
}

print_step() {
    echo -e "${GREEN}[STEP]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

check_openssl() {
    if ! command -v openssl &> /dev/null; then
        print_error "OpenSSL is not installed. Please install it first:"
        echo "  macOS: brew install openssl"
        echo "  Ubuntu/Debian: sudo apt-get install openssl"
        echo "  CentOS/RHEL: sudo yum install openssl"
        exit 1
    fi
    
    local openssl_version=$(openssl version)
    print_step "Found OpenSSL: $openssl_version"
}

check_existing_certificates() {
    if [[ -f "$CERT_FILE" && -f "$KEY_FILE" ]]; then
        print_warning "Certificates already exist:"
        echo "  Certificate: $CERT_FILE"
        echo "  Private Key: $KEY_FILE"
        echo
        
        # Check certificate validity
        local expiry_date=$(openssl x509 -in "$CERT_FILE" -noout -enddate 2>/dev/null | cut -d= -f2)
        if [[ -n "$expiry_date" ]]; then
            echo "  Current certificate expires: $expiry_date"
            
            # Check if certificate expires within 30 days
            local expiry_epoch=$(date -d "$expiry_date" +%s 2>/dev/null || date -j -f "%b %d %H:%M:%S %Y %Z" "$expiry_date" +%s 2>/dev/null || echo "0")
            local current_epoch=$(date +%s)
            local days_until_expiry=$(( (expiry_epoch - current_epoch) / 86400 ))
            
            if [[ $days_until_expiry -lt 30 ]]; then
                print_warning "Certificate expires in $days_until_expiry days!"
            fi
        fi
        
        echo
        read -p "Do you want to regenerate the certificates? [y/N]: " -n 1 -r
        echo
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            print_step "Keeping existing certificates"
            exit 0
        fi
        
        print_step "Removing existing certificates..."
        rm -f "$CERT_FILE" "$KEY_FILE"
    fi
}

create_certs_directory() {
    if [[ ! -d "$CERTS_DIR" ]]; then
        print_step "Creating certificates directory: $CERTS_DIR"
        mkdir -p "$CERTS_DIR"
    fi
}

generate_certificates() {
    print_step "Generating RSA private key (2048 bits)..."
    
    # Generate private key
    openssl genrsa -out "$KEY_FILE" 2048
    
    # Set restrictive permissions on private key
    chmod 600 "$KEY_FILE"
    
    print_step "Generating self-signed certificate..."
    
    # Generate certificate with Subject Alternative Names
    openssl req -new -x509 -key "$KEY_FILE" -out "$CERT_FILE" -days $VALIDITY_DAYS \
        -subj "/C=$COUNTRY/ST=$STATE/L=$CITY/O=$ORGANIZATION/OU=$ORGANIZATIONAL_UNIT/CN=$COMMON_NAME/emailAddress=$EMAIL" \
        -extensions v3_req \
        -config <(cat <<EOF
[req]
distinguished_name = req_distinguished_name
req_extensions = v3_req
prompt = no

[req_distinguished_name]

[v3_req]
keyUsage = keyEncipherment, dataEncipherment, digitalSignature
extendedKeyUsage = serverAuth, clientAuth
subjectAltName = @alt_names

[alt_names]
DNS.1 = localhost
DNS.2 = *.localhost
DNS.3 = mock-acs-server.local
DNS.4 = *.mock-acs-server.local
IP.1 = 127.0.0.1
IP.2 = ::1
EOF
)
    
    # Set appropriate permissions on certificate
    chmod 644 "$CERT_FILE"
}

validate_certificates() {
    print_step "Validating generated certificates..."
    
    # Verify private key
    if ! openssl rsa -in "$KEY_FILE" -check -noout > /dev/null 2>&1; then
        print_error "Generated private key is invalid!"
        exit 1
    fi
    
    # Verify certificate
    if ! openssl x509 -in "$CERT_FILE" -noout > /dev/null 2>&1; then
        print_error "Generated certificate is invalid!"
        exit 1
    fi
    
    # Verify key and certificate match
    local key_hash=$(openssl rsa -in "$KEY_FILE" -noout -modulus 2>/dev/null | openssl md5)
    local cert_hash=$(openssl x509 -in "$CERT_FILE" -noout -modulus 2>/dev/null | openssl md5)
    
    if [[ "$key_hash" != "$cert_hash" ]]; then
        print_error "Private key and certificate do not match!"
        exit 1
    fi
    
    print_success "Certificate validation passed!"
}

display_certificate_info() {
    echo
    echo -e "${BLUE}Certificate Information:${NC}"
    echo "============================================"
    
    # Display certificate details
    openssl x509 -in "$CERT_FILE" -noout -text | grep -A 10 "Subject:"
    echo
    openssl x509 -in "$CERT_FILE" -noout -dates
    echo
    
    # Display certificate fingerprint
    local fingerprint=$(openssl x509 -in "$CERT_FILE" -noout -fingerprint -sha256 | cut -d= -f2)
    echo "SHA256 Fingerprint: $fingerprint"
    echo
}

display_usage_instructions() {
    echo -e "${GREEN}Next Steps:${NC}"
    echo "=========================================="
    echo "1. Start Redis server:"
    echo "   redis-server"
    echo
    echo "2. Run the 3DS mock server:"
    echo "   RUN_MODE=development cargo run"
    echo
    echo "3. Test dynamic ACS signed content generation:"
    echo "   curl -X POST http://localhost:8080/3ds/authenticate \\"
    echo "     -H \"Content-Type: application/json\" \\"
    echo "     -d '{\"deviceChannel\":\"02\",\"threeDSRequestorChallengeInd\":\"04\",...}'"
    echo
    print_warning "IMPORTANT SECURITY NOTES:"
    echo "- These certificates are for DEVELOPMENT ONLY"
    echo "- Never use self-signed certificates in production"
    echo "- Never commit private keys to version control"
    echo "- For production, use certificates from a trusted CA"
    echo
}

cleanup_on_error() {
    print_error "Certificate generation failed!"
    if [[ -f "$KEY_FILE" ]]; then
        rm -f "$KEY_FILE"
    fi
    if [[ -f "$CERT_FILE" ]]; then
        rm -f "$CERT_FILE"
    fi
    exit 1
}

# Main execution
main() {
    # Set up error handling
    trap cleanup_on_error ERR
    
    print_header
    
    # Preflight checks
    check_openssl
    check_existing_certificates
    
    # Certificate generation
    create_certs_directory
    generate_certificates
    validate_certificates
    
    # Success output
    print_success "Certificates generated successfully!"
    display_certificate_info
    display_usage_instructions
}

# Run main function
main "$@"
