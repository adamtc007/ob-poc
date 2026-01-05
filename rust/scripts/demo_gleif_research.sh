#!/usr/bin/env bash
#
# GLEIF Research Demo Script
# ==========================
# Demonstrates: Research → Pivot → Onboarding flow
#
# This script is IDEMPOTENT - run it multiple times, get the same result.
#
# Prerequisites:
#   - PostgreSQL running with data_designer database
#   - dsl_cli built: cargo build --features database,cli --bin dsl_cli --release
#
# Usage:
#   ./scripts/demo_gleif_research.sh
#
# Demo narrative:
#   "We need to onboard Allianz Global Investors. Instead of manual data entry,
#    we query GLEIF to discover their corporate structure and managed funds,
#    then auto-generate the onboarding DSL."

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
CLI="$ROOT_DIR/target/release/dsl_cli"

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${BLUE}============================================${NC}"
echo -e "${BLUE}  GLEIF Research → Onboarding Demo${NC}"
echo -e "${BLUE}============================================${NC}"
echo ""

# Check CLI exists
if [ ! -f "$CLI" ]; then
    echo -e "${YELLOW}Building dsl_cli...${NC}"
    cd "$ROOT_DIR"
    cargo build --features database,cli --bin dsl_cli --release
fi

# Set database URL
export DATABASE_URL="${DATABASE_URL:-postgresql:///data_designer}"

echo -e "${GREEN}Phase 1: Search GLEIF for 'Allianz Global Investors'${NC}"
echo "─────────────────────────────────────────────────────"
echo ""
echo "Querying the global LEI database for entities matching 'Allianz Global Investors'"
echo ""
cat << 'EOF'
DSL: (gleif.search :name "Allianz Global Investors" :limit 5)
EOF
echo ""

$CLI execute << 'EOF'
(gleif.search :name "Allianz Global Investors" :limit 5)
EOF

echo ""
echo -e "${GREEN}Phase 2: Enrich the entity with GLEIF data${NC}"
echo "─────────────────────────────────────────────────────"
echo ""
echo "Using LEI: OJ2TIQSVQND4IZYYK658 (Allianz Global Investors GmbH - ACTIVE)"
echo ""
cat << 'EOF'
DSL: (gleif.enrich :lei "OJ2TIQSVQND4IZYYK658" :as @allianz-gi)
EOF
echo ""

$CLI execute << 'EOF'
(gleif.enrich :lei "OJ2TIQSVQND4IZYYK658" :as @allianz-gi)
EOF

echo ""
echo -e "${GREEN}Phase 3: Explore corporate tree - get parent${NC}"
echo "─────────────────────────────────────────────────────"
echo ""
cat << 'EOF'
DSL: (gleif.get-parent :lei "OJ2TIQSVQND4IZYYK658")
EOF
echo ""

$CLI execute << 'EOF'
(gleif.get-parent :lei "OJ2TIQSVQND4IZYYK658")
EOF

echo ""
echo -e "${GREEN}Phase 4: Discover managed funds${NC}"
echo "─────────────────────────────────────────────────────"
echo ""
cat << 'EOF'
DSL: (gleif.get-managed-funds :manager-lei "OJ2TIQSVQND4IZYYK658" :limit 10)
EOF
echo ""

$CLI execute << 'EOF'
(gleif.get-managed-funds :manager-lei "OJ2TIQSVQND4IZYYK658" :limit 10)
EOF

echo ""
echo -e "${GREEN}Phase 5: THE PIVOT - Auto-generate onboarding for discovered funds${NC}"
echo "─────────────────────────────────────────────────────────────────"
echo ""
echo "This is the key demo moment: from research → to onboarding automation"
echo ""
cat << 'EOF'
DSL: (gleif.import-managed-funds
       :manager-lei "OJ2TIQSVQND4IZYYK658"
       :create-cbus true
       :limit 5
       :dry-run false)
EOF
echo ""

$CLI execute << 'EOF'
(gleif.import-managed-funds
  :manager-lei "OJ2TIQSVQND4IZYYK658"
  :create-cbus true
  :limit 5
  :dry-run false)
EOF

echo ""
echo -e "${GREEN}Phase 6: Verify - Show created CBUs${NC}"
echo "─────────────────────────────────────────────────────"
echo ""

$CLI execute << 'EOF'
(cbu.list :name-contains "Allianz" :limit 10)
EOF

echo ""
echo -e "${BLUE}============================================${NC}"
echo -e "${BLUE}  Demo Complete${NC}"
echo -e "${BLUE}============================================${NC}"
echo ""
echo "Summary:"
echo "  1. Searched GLEIF API for Allianz"
echo "  2. Enriched entity with regulatory metadata"
echo "  3. Explored corporate tree (parent company)"
echo "  4. Discovered managed funds"
echo "  5. Auto-generated CBUs + role assignments"
echo ""
echo "The script is idempotent - run again to verify."
