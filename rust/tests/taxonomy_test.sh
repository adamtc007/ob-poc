#!/bin/bash
# Taxonomy Test Harness
# Tests product/service/resource taxonomy verbs and cbu.add-product

set -e

CLI="./target/debug/dsl_cli"
DB_URL="${DATABASE_URL:-postgresql:///data_designer}"

echo "=== Taxonomy Test Harness ==="
echo ""

# Build CLI if needed
if [ ! -f "$CLI" ]; then
    echo "Building CLI..."
    cargo build --features cli,database --bin dsl_cli
fi

echo "1. Testing product.list..."
echo '(product.list)' | $CLI execute 2>&1 | grep -E "(rows|error)" || echo "   PASS"

echo ""
echo "2. Testing service.list..."
echo '(service.list)' | $CLI execute 2>&1 | grep -E "(rows|error)" || echo "   PASS"

echo ""
echo "3. Testing service.list-by-product (Custody)..."
PRODUCT_ID=$(psql -d data_designer -t -c "SELECT product_id FROM \"ob-poc\".products WHERE name='Custody' LIMIT 1" | tr -d ' ')
echo "(service.list-by-product :product-id \"$PRODUCT_ID\")" | $CLI execute 2>&1 | grep -E "(rows|error)" || echo "   PASS"

echo ""
echo "4. Testing service-resource.list..."
echo '(service-resource.list)' | $CLI execute 2>&1 | grep -E "(rows|error)" || echo "   PASS"

echo ""
echo "5. Adding products to existing CBUs..."
# Get all CBU IDs
CBUS=$(psql -d data_designer -t -c "SELECT cbu_id FROM \"ob-poc\".cbus")

for CBU_ID in $CBUS; do
    CBU_ID=$(echo $CBU_ID | tr -d ' ')
    if [ -n "$CBU_ID" ]; then
        CBU_NAME=$(psql -d data_designer -t -c "SELECT name FROM \"ob-poc\".cbus WHERE cbu_id='$CBU_ID'" | tr -d ' ' | head -1)
        echo "   Adding Custody to $CBU_NAME..."
        echo "(cbu.add-product :cbu-id \"$CBU_ID\" :product \"Custody\")" | $CLI execute 2>&1 | grep -E "(rows|error|Affected)" | head -1
    fi
done

echo ""
echo "6. Verifying idempotency (running again)..."
for CBU_ID in $CBUS; do
    CBU_ID=$(echo $CBU_ID | tr -d ' ')
    if [ -n "$CBU_ID" ]; then
        CBU_NAME=$(psql -d data_designer -t -c "SELECT name FROM \"ob-poc\".cbus WHERE cbu_id='$CBU_ID'" | tr -d ' ' | head -1)
        RESULT=$(echo "(cbu.add-product :cbu-id \"$CBU_ID\" :product \"Custody\")" | $CLI execute 2>&1 | grep -oE "[0-9]+ rows" | head -1)
        if [ "$RESULT" = "0 rows" ]; then
            echo "   $CBU_NAME: IDEMPOTENT (0 new rows)"
        else
            echo "   $CBU_NAME: $RESULT"
        fi
    fi
done

echo ""
echo "7. Checking service_delivery_map..."
psql -d data_designer -c "
SELECT c.name as cbu, p.name as product, count(*) as services
FROM \"ob-poc\".service_delivery_map sdm
JOIN \"ob-poc\".cbus c ON sdm.cbu_id = c.cbu_id
JOIN \"ob-poc\".products p ON sdm.product_id = p.product_id
GROUP BY c.name, p.name
ORDER BY c.name;"

echo ""
echo "8. Checking cbu_resource_instances..."
psql -d data_designer -c "
SELECT c.name as cbu, count(*) as resource_instances
FROM \"ob-poc\".cbu_resource_instances cri
JOIN \"ob-poc\".cbus c ON cri.cbu_id = c.cbu_id
GROUP BY c.name
ORDER BY c.name;"

echo ""
echo "9. Verifying no duplicate resources (SWIFT should appear once per CBU)..."
psql -d data_designer -c "
SELECT c.name as cbu, srt.name as resource_type, count(*) as count
FROM \"ob-poc\".cbu_resource_instances cri
JOIN \"ob-poc\".cbus c ON cri.cbu_id = c.cbu_id
JOIN \"ob-poc\".service_resource_types srt ON cri.resource_type_id = srt.resource_id
GROUP BY c.name, srt.name
HAVING count(*) > 1
ORDER BY c.name, srt.name;"

echo ""
echo "=== Test Complete ==="
