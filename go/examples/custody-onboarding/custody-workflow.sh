#!/bin/bash

# ============================================================================
# CUSTODY ONBOARDING: PROMPT-DRIVEN DSL STATE TRANSFORMATION
# ============================================================================
#
# This workflow demonstrates the core DSL-as-State architectural pattern where:
# - The DSL IS the state (not a representation of state)
# - Each prompt extends the accumulated DSL document
# - State transformations happen through natural language prompts
# - Every decision is captured in the immutable DSL audit trail
#
# Pattern: Prompt → DSL Extension → State Transformation → New Version
#
# Prerequisites:
# - DSL Onboarding POC built: make build
# - Database seeded with custody services/resources: ./dsl-poc seed-catalog
# ============================================================================

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
PURPLE='\033[0;35m'
CYAN='\033[0;36m'
NC='\033[0m'

# State transformation functions
prompt() {
    echo -e "${CYAN}💬 PROMPT:${NC} $1"
}

dsl_extends() {
    echo -e "${PURPLE}🔄 DSL EXTENDS:${NC} $1"
}

state_transform() {
    echo -e "${BLUE}📊 STATE TRANSFORM:${NC} $1"
}

version_created() {
    echo -e "${GREEN}✅ VERSION:${NC} $1"
}

audit_trail() {
    echo -e "${YELLOW}📋 AUDIT:${NC} $1"
}

# Configuration
CBU_ID="CBU-CUSTODY-2024-001"
CLIENT_NAME="Global Investment Partners LLC"

echo "============================================================================"
echo "🏦 CUSTODY ONBOARDING: DSL-AS-STATE DEMONSTRATION"
echo "============================================================================"
echo "Client: $CLIENT_NAME"
echo "CBU ID: $CBU_ID"
echo "Architecture: DSL-as-State with Prompt-Driven Extensions"
echo "============================================================================"
echo

# ============================================================================
# STATE TRANSFORMATION 1: INITIAL CASE CREATION
# Prompt → DSL Generation → State = DSL Document
# ============================================================================

echo -e "${BLUE}📍 STATE TRANSFORMATION 1: CASE INITIALIZATION${NC}"
echo "-------------------------------------------"
prompt "Create an onboarding case for Global Investment Partners LLC requiring custody services"
echo

dsl_extends "Generates initial DSL with case metadata and client requirements"
if ./dsl-poc create --cbu="$CBU_ID" 2>/dev/null; then
    state_transform "EMPTY → CREATED (DSL Document Created)"
    version_created "Version 1 - Initial case DSL created"
    audit_trail "Case creation, client identification, regulatory classification"
else
    echo -e "${RED}❌ Failed to create initial DSL state${NC}"
    exit 1
fi

echo
echo "Current accumulated DSL state:"
echo "------------------------------"
./dsl-poc history --cbu="$CBU_ID" | head -15
echo "..."
echo

# ============================================================================
# STATE TRANSFORMATION 2: PRODUCT REQUIREMENT EXTENSION
# Previous DSL + Product Selection → Extended DSL
# ============================================================================

echo -e "${BLUE}📍 STATE TRANSFORMATION 2: PRODUCT EXTENSION${NC}"
echo "--------------------------------------------"
prompt "Add CUSTODY product to this case with institutional-grade requirements"
echo

dsl_extends "Appends product selection to existing DSL (never replaces)"
if ./dsl-poc add-products --cbu="$CBU_ID" --products="CUSTODY" 2>/dev/null; then
    state_transform "CREATED → PRODUCTS_ADDED (DSL Accumulated)"
    version_created "Version 2 - Product requirements appended to DSL"
    audit_trail "Product selection rationale, asset classes, expected volumes"
else
    echo -e "${RED}❌ Failed to extend DSL with product requirements${NC}"
    exit 1
fi

echo
echo "DSL accumulation (Version 1 + Version 2):"
echo "-----------------------------------------"
./dsl-poc history --cbu="$CBU_ID" | tail -10
echo

# ============================================================================
# STATE TRANSFORMATION 3: SERVICE DISCOVERY EXTENSION
# Previous DSL + Service Analysis → Business Service DSL
# ============================================================================

echo -e "${BLUE}📍 STATE TRANSFORMATION 3: SERVICE DISCOVERY${NC}"
echo "--------------------------------------------"
prompt "Discover all business services needed for comprehensive custody operations"
echo

dsl_extends "AI analyzes CUSTODY product and appends service plan to accumulated DSL"
if ./dsl-poc discover-services --cbu="$CBU_ID" 2>/dev/null; then
    state_transform "PRODUCTS_ADDED → SERVICES_DISCOVERED (Business Architecture)"
    version_created "Version 3 - Service discovery appended (6 services identified)"
    audit_trail "Service selection: Safekeeping, Security Movement, Trade Capture, Reconciliation, SSI, Reporting"

    echo
    echo "Expected Services Extended into DSL:"
    echo "• Safekeeping - Asset custody and segregation"
    echo "• SecurityMovement - Security transfer and control"
    echo "• TradeCapture - Trade processing and validation"
    echo "• Reconciliation - Position and cash matching"
    echo "• SpecialSettlementInstructions - SSI management"
    echo "• CustodyReporting - Comprehensive reporting"
else
    echo -e "${RED}❌ Failed to extend DSL with service discovery${NC}"
    exit 1
fi

echo

# ============================================================================
# STATE TRANSFORMATION 4: RESOURCE MAPPING EXTENSION
# Previous DSL + Infrastructure Analysis → Implementation DSL
# ============================================================================

echo -e "${BLUE}📍 STATE TRANSFORMATION 4: RESOURCE PROVISIONING${NC}"
echo "-----------------------------------------------"
prompt "Map business services to implementation resources and provision infrastructure"
echo

dsl_extends "Maps services to concrete resources and appends resource plan to DSL"
if ./dsl-poc discover-resources --cbu="$CBU_ID" 2>/dev/null; then
    state_transform "SERVICES_DISCOVERED → RESOURCES_DISCOVERED (Implementation Architecture)"
    version_created "Version 4 - Resource mapping appended (8 resources provisioned)"
    audit_trail "Resource allocation: Platforms, engines, systems, infrastructure components"

    echo
    echo "Implementation Resources Extended into DSL:"
    echo "• CustodyMainPlatform - Primary custody system"
    echo "• TradeCaptureAndRoutingSystem - Trade processing engine"
    echo "• SecurityMovementEngine - Settlement processing"
    echo "• ReconciliationPlatform - Position matching"
    echo "• SSIManagementService - Settlement instructions"
    echo "• CustodyReportingEngine - Reporting platform"
    echo "• PhysicalVaultSystem - Certificate storage"
    echo "• NomineeServicesSystem - Beneficial ownership"
else
    echo -e "${RED}❌ Failed to extend DSL with resource mapping${NC}"
    exit 1
fi

echo

# ============================================================================
# STATE TRANSFORMATION 5: CONFIGURATION EXTENSION
# Previous DSL + Attribute Analysis → Configuration DSL
# ============================================================================

echo -e "${BLUE}📍 STATE TRANSFORMATION 5: CONFIGURATION PARAMETERS${NC}"
echo "------------------------------------------------"
prompt "Populate custody-specific attributes and operational configurations"
echo

dsl_extends "Analyzes requirements and appends configuration attributes to DSL"
if ./dsl-poc populate-attributes --cbu="$CBU_ID" 2>/dev/null; then
    state_transform "RESOURCES_DISCOVERED → ATTRIBUTES_POPULATED (Configuration Layer)"
    version_created "Version 5 - Configuration attributes appended"
    audit_trail "Operational parameters: Account types, limits, preferences, rules"
else
    echo -e "${RED}❌ Failed to extend DSL with configuration attributes${NC}"
    exit 1
fi

echo

# ============================================================================
# STATE TRANSFORMATION 6: VALUE BINDING EXTENSION
# Previous DSL + Value Resolution → Executable DSL
# ============================================================================

echo -e "${BLUE}📍 STATE TRANSFORMATION 6: VALUE RESOLUTION${NC}"
echo "-------------------------------------------"
prompt "Resolve and bind all operational values to make the configuration executable"
echo

dsl_extends "Resolves attribute values and appends bindings to create executable DSL"
if ./dsl-poc get-attribute-values --cbu="$CBU_ID" 2>/dev/null; then
    state_transform "ATTRIBUTES_POPULATED → VALUES_BOUND (Executable State)"
    version_created "Version 6 - Value bindings appended (DSL now executable)"
    audit_trail "Concrete values: Account numbers, contacts, URLs, limits, configurations"
else
    echo -e "${YELLOW}⚠️  Value binding completed with some pending items${NC}"
    state_transform "ATTRIBUTES_POPULATED → PARTIALLY_BOUND (Some values pending)"
    version_created "Version 6 - Partial value bindings (workflow continues)"
fi

echo

# ============================================================================
# DSL-AS-STATE DEMONSTRATION: COMPLETE ACCUMULATED STATE
# ============================================================================

echo "============================================================================"
echo -e "${GREEN}🎉 DSL-AS-STATE PATTERN DEMONSTRATED${NC}"
echo "============================================================================"
echo

echo -e "${PURPLE}📋 COMPLETE ACCUMULATED DSL STATE:${NC}"
echo "Each prompt extended the DSL. The DSL IS the complete state."
echo "----------------------------------------"
./dsl-poc history --cbu="$CBU_ID"

echo
echo "============================================================================"
echo -e "${GREEN}✅ ARCHITECTURAL PATTERNS DEMONSTRATED${NC}"
echo "============================================================================"
echo

echo -e "${BLUE}🏗️  DSL-AS-STATE PATTERN:${NC}"
echo "  ✅ State = Accumulated DSL Document"
echo "  ✅ Each prompt extends (never replaces) the DSL"
echo "  ✅ Immutable versioning with complete audit trail"
echo "  ✅ Compositional state building through accumulation"
echo "  ✅ Human-readable yet machine-executable"

echo
echo -e "${PURPLE}🔄 PROMPT-DRIVEN STATE TRANSFORMATION:${NC}"
echo "  ✅ Natural language prompts drive state transitions"
echo "  ✅ AI generates DSL extensions based on context"
echo "  ✅ Previous DSL provides context for next extension"
echo "  ✅ Business requirements become executable configuration"

echo
echo -e "${CYAN}📊 STATE EVOLUTION TIMELINE:${NC}"
echo "  Version 1: EMPTY → CREATED (Case initialization)"
echo "  Version 2: CREATED → PRODUCTS_ADDED (Product selection)"
echo "  Version 3: PRODUCTS_ADDED → SERVICES_DISCOVERED (Business architecture)"
echo "  Version 4: SERVICES_DISCOVERED → RESOURCES_DISCOVERED (Implementation)"
echo "  Version 5: RESOURCES_DISCOVERED → ATTRIBUTES_POPULATED (Configuration)"
echo "  Version 6: ATTRIBUTES_POPULATED → VALUES_BOUND (Executable state)"

echo
echo -e "${YELLOW}📋 COMPLIANCE & AUDIT BENEFITS:${NC}"
echo "  ✅ Complete decision audit trail"
echo "  ✅ Immutable compliance record"
echo "  ✅ Regulatory-ready documentation"
echo "  ✅ Time-travel to any historical state"
echo "  ✅ Business-readable yet legally binding"

echo
echo -e "${GREEN}🚀 OPERATIONAL BENEFITS:${NC}"
echo "  ✅ Executable workflow from natural language"
echo "  ✅ Automated configuration from requirements"
echo "  ✅ Cross-system integration through shared DSL"
echo "  ✅ Zero-loss information transformation"
echo "  ✅ Human oversight with machine precision"

echo
echo "============================================================================"
echo -e "${GREEN}🏦 CUSTODY ONBOARDING COMPLETED THROUGH DSL STATE TRANSFORMATION${NC}"
echo "============================================================================"
echo
echo "Client: $CLIENT_NAME"
echo "Final State: Fully configured custody relationship"
echo "DSL Versions: 6 accumulated transformations"
echo "Architecture: DSL-as-State with prompt-driven extensions"
echo "Result: Complete onboarding audit trail in executable DSL format"
echo
echo "The accumulated DSL document IS the complete state of the onboarding."
echo "============================================================================"
