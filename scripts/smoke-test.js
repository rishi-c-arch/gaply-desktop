// scripts/smoke-test.js
const https = require('https');
const http = require('http');

const BASE_URL = process.env.TEST_URL || 'https://www.gaply.in';
const TIMEOUT = 10000;

// Security headers to check
const securityHeaders = [
  'strict-transport-security',
  'x-frame-options',
  'x-content-type-options',
  'referrer-policy'
];

// Test cases
const testCases = [
  {
    name: 'Homepage HTML Content',
    path: '/',
    tests: [
      { type: 'title', selector: 'title', expected: 'Gaply' },
      { type: 'meta', selector: 'meta[property="og:title"]', expected: 'Gaply' }
    ]
  },
  {
    name: 'Features Page HTML Content',
    path: '/features',
    tests: [
      { type: 'title', selector: 'title', expected: 'Features' }
    ]
  }
];

function makeRequest(url) {
  return new Promise((resolve, reject) => {
    const client = url.startsWith('https') ? https : http;
    const request = client.get(url, { timeout: TIMEOUT }, (response) => {
      let data = '';
      response.on('data', chunk => data += chunk);
      response.on('end', () => {
        resolve({
          statusCode: response.statusCode,
          headers: response.headers,
          body: data
        });
      });
    });
    
    request.on('error', reject);
    request.on('timeout', () => {
      request.destroy();
      reject(new Error('Request timeout'));
    });
  });
}

function parseHTML(html, selector, type) {
  try {
    if (type === 'title') {
      const match = html.match(/<title[^>]*>(.*?)<\/title>/i);
      return match ? match[1].trim() : null;
    }
    
    if (type === 'meta') {
      const regex = new RegExp(`<meta[^>]*${selector.replace(/[\[\]]/g, '\\$&')}[^>]*content=["']([^"']*)["']`, 'i');
      const match = html.match(regex);
      return match ? match[1].trim() : null;
    }
    
    return null;
  } catch (error) {
    return null;
  }
}

async function testHTMLContent(testCase) {
  const url = `${BASE_URL}${testCase.path}`;
  console.log(`\n🧪 Testing: ${testCase.name}`);
  console.log(`📍 URL: ${url}`);
  
  try {
    const response = await makeRequest(url);
    
    if (response.statusCode !== 200) {
      console.log(`❌ Status Code: ${response.statusCode} (expected 200)`);
      return false;
    }
    
    console.log(`✅ Status Code: ${response.statusCode}`);
    
    if (!response.body || response.body.length < 100) {
      console.log(`❌ HTML content too short: ${response.body.length} characters`);
      return false;
    }
    
    console.log(`✅ HTML Content Length: ${response.body.length} characters`);
    
    let allPassed = true;
    for (const test of testCase.tests) {
      const content = parseHTML(response.body, test.selector, test.type);
      
      if (!content) {
        console.log(`❌ ${test.type} not found: ${test.selector}`);
        allPassed = false;
        continue;
      }
      
      if (content.includes(test.expected)) {
        console.log(`✅ ${test.type}: "${content}"`);
      } else {
        console.log(`❌ ${test.type}: "${content}" (expected to contain: "${test.expected}")`);
        allPassed = false;
      }
    }
    
    return allPassed;
  } catch (error) {
    console.log(`❌ Error: ${error.message}`);
    return false;
  }
}

async function testSecurityHeaders() {
  console.log(`\n🔒 Testing Security Headers`);
  console.log(`📍 URL: ${BASE_URL}`);
  
  try {
    const response = await makeRequest(BASE_URL);
    
    let allPassed = true;
    for (const header of securityHeaders) {
      const value = response.headers[header];
      
      if (value) {
        console.log(`✅ ${header}: ${value}`);
      } else {
        console.log(`❌ ${header}: Missing`);
        allPassed = false;
      }
    }
    
    return allPassed;
  } catch (error) {
    console.log(`❌ Error: ${error.message}`);
    return false;
  }
}

async function runTests() {
  console.log('🚀 Starting Prerender Smoke Tests');
  console.log(`🎯 Target URL: ${BASE_URL}`);
  console.log(`⏱️ Timeout: ${TIMEOUT}ms`);
  
  const results = {
    htmlContent: true,
    securityHeaders: true
  };
  
  // Test HTML content for all pages
  for (const testCase of testCases) {
    const passed = await testHTMLContent(testCase);
    if (!passed) {
      results.htmlContent = false;
    }
  }
  
  // Test security headers
  const securityPassed = await testSecurityHeaders();
  if (!securityPassed) {
    results.securityHeaders = false;
  }
  
  // Summary
  console.log('\n📊 Test Results Summary');
  console.log('========================');
  console.log(`HTML Content: ${results.htmlContent ? '✅ PASS' : '❌ FAIL'}`);
  console.log(`Security Headers: ${results.securityHeaders ? '✅ PASS' : '❌ FAIL'}`);
  
  const overallPassed = results.htmlContent && results.securityHeaders;
  console.log(`\n🎯 Overall Result: ${overallPassed ? '✅ ALL TESTS PASSED' : '❌ SOME TESTS FAILED'}`);
  
  if (!overallPassed) {
    process.exit(1);
  }
}

// Run if called directly
if (require.main === module) {
  runTests().catch(error => {
    console.error('💥 Test runner error:', error);
    process.exit(1);
  });
}

module.exports = { runTests, testHTMLContent, testSecurityHeaders };
