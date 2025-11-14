#!/bin/bash
# Test script for Attribute Dictionary API

BASE_URL="http://localhost:3000"

echo "🧪 Testing Attribute Dictionary API"
echo "===================================="

# Test 1: Health check
echo -e "\n1️⃣  Testing health endpoint..."
curl -s "${BASE_URL}/api/attributes/health" | jq '.'

# Test 2: Validate DSL with @attr{} references
echo -e "\n2️⃣  Testing DSL validation..."
curl -s -X POST "${BASE_URL}/api/attributes/validate-dsl" \
  -H "Content-Type: application/json" \
  -d '{"dsl": "Test DSL without attributes"}' | jq '.'

# Test 3: Get attributes for a CBU (will be empty but should not error)
echo -e "\n3️⃣  Testing get CBU attributes..."
TEST_CBU="00000000-0000-0000-0000-000000000001"
curl -s "${BASE_URL}/api/attributes/${TEST_CBU}" | jq '.'

echo -e "\n✅ Basic API tests complete!"
echo "Note: Document upload and extraction tests require valid base64 content"
