// k6 Load Testing Script for 3DS Mock Server
// Run with: k6 run --vus 100 --duration 30s load-test.js

import http from 'k6/http';
import { check, sleep } from 'k6';
import { Rate } from 'k6/metrics';

// Custom metrics
const errorRate = new Rate('errors');

export const options = {
  stages: [
    { duration: '10s', target: 50 },   // Ramp up to 50 VUs
    { duration: '20s', target: 100 },  // Stay at 100 VUs
    { duration: '10s', target: 0 },    // Ramp down
  ],
  thresholds: {
    http_req_duration: ['p(95)<500'], // 95% of requests must complete below 500ms
    http_req_failed: ['rate<0.1'],    // Error rate must be below 10%
    errors: ['rate<0.1'],
  },
};

const BASE_URL = 'http://localhost:8080';

// Sample test data
const versionRequest = {
  card_number: "5155010000000001",
  merchant_name: "Test Merchant"
};

const authenticateRequest = {
  threeDSServerTransID: "12345678-1234-1234-1234-123456789abc",
  deviceChannel: "02",
  messageCategory: "01",
  threeDSCompInd: "Y",
  cardholder: {
    cardholderName: "John Doe",
    email: "john.doe@example.com",
    billAddrLine1: "123 Main St",
    billAddrCity: "Anytown",
    billAddrPostCode: "12345",
    billAddrCountry: "US",
    shipAddrLine1: "123 Main St",
    shipAddrCity: "Anytown", 
    shipAddrPostCode: "12345",
    shipAddrCountry: "US",
    addrMatch: "Y",
    homePhone: { cc: "1", subscriber: "5551234567" },
    mobilePhone: { cc: "1", subscriber: "5559876543" },
    workPhone: { cc: "1", subscriber: "5555555555" }
  },
  cardholderAccount: {
    acctNumber: "5155010000000001",
    acctType: "02",
    cardExpiryDate: "2512",
    cardSecurityCode: "123"
  },
  purchase: {
    purchaseAmount: 10000,
    purchaseExponent: 2,
    purchaseCurrency: "840",
    purchaseDate: "20241201120000",
    transType: "01",
    recurringExpiry: "20251201",
    recurringFrequency: 1
  },
  merchant: {
    merchantName: "Test Merchant",
    merchantCountryCode: "840",
    threeDSRequestorId: "test_requestor",
    threeDSRequestorName: "Test Requestor",
    notificationURL: "https://example.com/notification",
    mcc: "5812"
  },
  acquirer: {
    acquirerBIN: "123456",
    acquirerMerchantID: "merchant123"
  },
  threeDSRequestor: {
    threeDSRequestorChallengeInd: "01",
    threeDSRequestorAuthenticationInd: "01",
    threeDSRequestorAuthenticationInfo: {
      threeDSReqAuthMethod: "02",
      threeDSReqAuthTimestamp: "202412011200"
    }
  },
  browserInformation: {
    browserAcceptHeader: "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
    browserIP: "192.168.1.1",
    browserJavaEnabled: true,
    browserJavascriptEnabled: true,
    browserLanguage: "en-US",
    browserColorDepth: "24",
    browserScreenHeight: 1080,
    browserScreenWidth: 1920,
    browserTZ: -300,
    browserUserAgent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36"
  },
  deviceRenderOptions: {
    sdkInterface: "03",
    sdkUiType: ["01", "02", "03", "04", "05"]
  }
};

export default function () {
  // Test 1: Version endpoint
  const versionResponse = http.post(`${BASE_URL}/3ds/version`, JSON.stringify(versionRequest), {
    headers: { 'Content-Type': 'application/json' },
  });
  
  const versionCheck = check(versionResponse, {
    'version request successful': (r) => r.status === 200,
    'version response time < 100ms': (r) => r.timings.duration < 100,
  });
  
  errorRate.add(!versionCheck);

  if (versionResponse.status === 200) {
    const versionData = JSON.parse(versionResponse.body);
    
    // Test 2: Authenticate endpoint with transaction ID from version
    const authRequest = {
      ...authenticateRequest,
      threeDSServerTransID: versionData.threeDSServerTransID
    };
    
    const authResponse = http.post(`${BASE_URL}/3ds/authenticate`, JSON.stringify(authRequest), {
      headers: { 'Content-Type': 'application/json' },
    });
    
    const authCheck = check(authResponse, {
      'authenticate request successful': (r) => r.status === 200,
      'authenticate response time < 200ms': (r) => r.timings.duration < 200,
    });
    
    errorRate.add(!authCheck);
    
    if (authResponse.status === 200) {
      const authData = JSON.parse(authResponse.body);
      
      // Test 3: Results endpoint
      const resultsRequest = {
        threeDSServerTransID: authData.threeDSServerTransID,
        acsTransID: authData.authenticationResponse.acsTransID,
        dsTransID: authData.authenticationResponse.dsTransID,
        eci: "05",
        authenticationValue: "test_auth_value",
        transStatus: "Y",
        messageType: "RReq",
        messageVersion: "2.2.0"
      };
      
      const resultsResponse = http.post(`${BASE_URL}/3ds/results`, JSON.stringify(resultsRequest), {
        headers: { 'Content-Type': 'application/json' },
      });
      
      const resultsCheck = check(resultsResponse, {
        'results request successful': (r) => r.status === 200,
        'results response time < 100ms': (r) => r.timings.duration < 100,
      });
      
      errorRate.add(!resultsCheck);
      
      // Test 4: Final endpoint
      const finalRequest = {
        threeDSServerTransID: authData.threeDSServerTransID
      };
      
      const finalResponse = http.post(`${BASE_URL}/3ds/final`, JSON.stringify(finalRequest), {
        headers: { 'Content-Type': 'application/json' },
      });
      
      const finalCheck = check(finalResponse, {
        'final request successful': (r) => r.status === 200,
        'final response time < 100ms': (r) => r.timings.duration < 100,
      });
      
      errorRate.add(!finalCheck);
    }
  }

  // Test health endpoint
  const healthResponse = http.get(`${BASE_URL}/health`);
  check(healthResponse, {
    'health check successful': (r) => r.status === 200,
    'health check fast': (r) => r.timings.duration < 50,
  });

  sleep(0.1); // Small delay between iterations
}

export function handleSummary(data) {
  return {
    'load-test-results.json': JSON.stringify(data, null, 2),
    'stdout': textSummary(data, { indent: ' ', enableColors: true }),
  };
}

function textSummary(data, options) {
  const indent = options.indent || '';
  const enableColors = options.enableColors || false;
  
  let summary = '\n' + indent + '📊 Load Test Summary\n';
  summary += indent + '==================\n\n';
  
  // Request metrics
  if (data.metrics.http_reqs) {
    summary += indent + `Total Requests: ${data.metrics.http_reqs.count}\n`;
    summary += indent + `Request Rate: ${data.metrics.http_reqs.rate.toFixed(2)} req/s\n\n`;
  }
  
  // Response time metrics
  if (data.metrics.http_req_duration) {
    summary += indent + 'Response Times:\n';
    summary += indent + `  Average: ${data.metrics.http_req_duration.avg.toFixed(2)}ms\n`;
    summary += indent + `  p50: ${data.metrics.http_req_duration.p50.toFixed(2)}ms\n`;
    summary += indent + `  p95: ${data.metrics.http_req_duration.p95.toFixed(2)}ms\n`;
    summary += indent + `  p99: ${data.metrics.http_req_duration.p99.toFixed(2)}ms\n`;
    summary += indent + `  Max: ${data.metrics.http_req_duration.max.toFixed(2)}ms\n\n`;
  }
  
  // Error rate
  if (data.metrics.http_req_failed) {
    const errorRate = (data.metrics.http_req_failed.rate * 100).toFixed(2);
    summary += indent + `Error Rate: ${errorRate}%\n\n`;
  }
  
  // Threshold results
  summary += indent + 'Thresholds:\n';
  for (const [name, threshold] of Object.entries(data.thresholds || {})) {
    const status = threshold.ok ? '✅ PASS' : '❌ FAIL';
    summary += indent + `  ${name}: ${status}\n`;
  }
  
  return summary;
}
