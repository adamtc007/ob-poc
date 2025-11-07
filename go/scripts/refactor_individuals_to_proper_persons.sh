#!/bin/bash

# Comprehensive refactoring script to rename all "individual/person" references to "Proper Person"
# This script handles database schema, Go code, and configuration files

set -e  # Exit on any error

echo "🔄 Starting comprehensive refactoring: Individual/Person → Proper Person"
echo "=================================================================="

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Check if we're in the right directory
if [[ ! -f "go.mod" ]] || [[ ! -d "sql" ]]; then
    echo -e "${RED}❌ Error: Must run from project root directory${NC}"
    exit 1
fi

# Backup function
backup_file() {
    local file="$1"
    if [[ -f "$file" && "$CREATE_BACKUPS" == "true" ]]; then
        cp "$file" "$file.backup.$(date +%Y%m%d_%H%M%S)"
        echo -e "${BLUE}📝 Backed up: $file${NC}"
    fi
}

# Function to refactor Go files
refactor_go_files() {
    echo -e "${YELLOW}🔧 Refactoring Go source files...${NC}"

    # Find all .go files
    find . -name "*.go" -type f | while read -r file; do
        if [[ -f "$file" ]]; then
            backup_file "$file"

            # Replace struct names and types
            sed -i '' \
                -e 's/entity_individuals/entity_proper_persons/g' \
                -e 's/EntityIndividuals/EntityProperPersons/g' \
                -e 's/individual_id/proper_person_id/g' \
                -e 's/IndividualID/ProperPersonID/g' \
                -e 's/PersonID/ProperPersonID/g' \
                -e 's/person_id/proper_person_id/g' \
                -e 's/"INDIVIDUAL"/"PROPER_PERSON"/g' \
                -e 's/INDIVIDUAL/PROPER_PERSON/g' \
                -e 's/"NATURAL_PERSON"/"PROPER_PERSON"/g' \
                -e 's/NATURAL_PERSON/PROPER_PERSON/g' \
                -e 's/Individual Trustee/Proper Person Trustee/g' \
                -e 's/individual trustee/proper person trustee/g' \
                -e 's/individual or corporate/proper person or corporate/g' \
                -e 's/individual(/proper person(/g' \
                -e 's/Individual(/Proper Person(/g' \
                -e 's/Type of investor (individual/Type of investor (proper person/g' \
                -e 's/single individual/single proper person/g' \
                -e 's/Single individual/Single proper person/g' \
                -e 's/named individual/named proper person/g' \
                -e 's/Named individual/Named proper person/g' \
                -e 's/entity_type.*INDIVIDUAL/entity_type (PROPER_PERSON/g' \
                "$file"

            echo -e "${GREEN}✅ Updated: $file${NC}"
        fi
    done
}

# Function to refactor SQL files
refactor_sql_files() {
    echo -e "${YELLOW}🗄️  Refactoring SQL files...${NC}"

    find sql -name "*.sql" -type f | while read -r file; do
        if [[ -f "$file" ]]; then
            backup_file "$file"

            # Replace table and column references
            sed -i '' \
                -e 's/entity_individuals/entity_proper_persons/g' \
                -e 's/individual_id/proper_person_id/g' \
                -e 's/INDIVIDUAL/PROPER_PERSON/g' \
                -e 's/NATURAL_PERSON/PROPER_PERSON/g' \
                -e 's/Individual/Proper Person/g' \
                -e 's/kyc\.individual\./kyc.proper_person./g' \
                -e 's/kyc_individual/kyc_proper_person/g' \
                -e 's/-- Individual/-- Proper Person/g' \
                -e 's/-- Proper Person (Individual)/-- Proper Person/g' \
                -e 's/Natural Person\/Individual/Natural Person\/Proper Person/g' \
                -e 's/individual,/proper person,/g' \
                -e 's/individual or/proper person or/g' \
                -e 's/Type of investor (individual/Type of investor (proper person/g' \
                -e 's/single individual/single proper person/g' \
                -e 's/named individual/named proper person/g' \
                -e 's/ubo_person_id/ubo_proper_person_id/g' \
                "$file"

            echo -e "${GREEN}✅ Updated: $file${NC}"
        fi
    done
}

# Function to refactor test data and mocks
refactor_test_data() {
    echo -e "${YELLOW}🧪 Refactoring test data and mock files...${NC}"

    # Find test files and mock files
    find . \( -name "*_test.go" -o -name "mock_*.go" -o -name "*.json" \) -type f | while read -r file; do
        if [[ -f "$file" ]]; then
            backup_file "$file"

            sed -i '' \
                -e 's/entity_individuals/entity_proper_persons/g' \
                -e 's/"individual_id"/"proper_person_id"/g' \
                -e 's/"INDIVIDUAL"/"PROPER_PERSON"/g' \
                -e 's/"NATURAL_PERSON"/"PROPER_PERSON"/g' \
                -e 's/Individual Trustee/Proper Person Trustee/g' \
                -e 's/individual or corporate/proper person or corporate/g' \
                -e 's/individual(/proper person(/g' \
                -e 's/Type of investor (individual/Type of investor (proper person/g' \
                "$file"

            echo -e "${GREEN}✅ Updated: $file${NC}"
        fi
    done
}

# Function to update documentation
refactor_documentation() {
    echo -e "${YELLOW}📚 Refactoring documentation...${NC}"

    find . \( -name "*.md" -o -name "*.txt" -o -name "README*" -o -name "CLAUDE.md" \) -type f | while read -r file; do
        if [[ -f "$file" ]]; then
            backup_file "$file"

            sed -i '' \
                -e 's/entity_individuals/entity_proper_persons/g' \
                -e 's/individual_id/proper_person_id/g' \
                -e 's/INDIVIDUAL/PROPER_PERSON/g' \
                -e 's/Individual entities/Proper Person entities/g' \
                -e 's/individual entities/proper person entities/g' \
                -e 's/Individual(/Proper Person(/g' \
                -e 's/individual or corporate/proper person or corporate/g' \
                -e 's/individual trustee/proper person trustee/g' \
                -e 's/Individual Trustee/Proper Person Trustee/g' \
                "$file"

            echo -e "${GREEN}✅ Updated: $file${NC}"
        fi
    done
}

# Function to update configuration files
refactor_config_files() {
    echo -e "${YELLOW}⚙️  Refactoring configuration files...${NC}"

    find . \( -name "*.yaml" -o -name "*.yml" -o -name "*.toml" -o -name "*.env*" \) -type f | while read -r file; do
        if [[ -f "$file" ]]; then
            backup_file "$file"

            sed -i '' \
                -e 's/entity_individuals/entity_proper_persons/g' \
                -e 's/individual_id/proper_person_id/g' \
                -e 's/INDIVIDUAL/PROPER_PERSON/g' \
                "$file"

            echo -e "${GREEN}✅ Updated: $file${NC}"
        fi
    done
}

# Function to create special fixes for complex cases
apply_special_fixes() {
    echo -e "${YELLOW}🎯 Applying special fixes for complex cases...${NC}"

    # Fix specific Go struct definitions that need manual attention
    if [[ -f "internal/datastore/types.go" ]]; then
        backup_file "internal/datastore/types.go"

        # Replace Individual struct name if it exists
        sed -i '' \
            -e 's/type Individual struct/type ProperPerson struct/g' \
            -e 's/\*Individual/\*ProperPerson/g' \
            -e 's/\[\]Individual/\[\]ProperPerson/g' \
            "internal/datastore/types.go"
    fi

    # Fix index names in migration scripts
    find sql/migrations -name "*.sql" -type f | while read -r file; do
        if [[ -f "$file" ]]; then
            sed -i '' \
                -e 's/idx_individuals_/idx_proper_persons_/g' \
                "$file"
        fi
    done

    # Fix enum handling in Go code - need to be more careful here
    find . -name "*.go" -type f -exec grep -l "EntityType" {} \; | while read -r file; do
        if [[ -f "$file" ]]; then
            # Only replace in entity type contexts, not in comments or strings that should remain
            sed -i '' \
                -e 's/EntityTypeIndividual/EntityTypeProperPerson/g' \
                -e 's/ENTITY_TYPE_INDIVIDUAL/ENTITY_TYPE_PROPER_PERSON/g' \
                "$file"
        fi
    done

    echo -e "${GREEN}✅ Applied special fixes${NC}"
}

# Function to fix backward compatibility references
fix_backward_compatibility() {
    echo -e "${YELLOW}🔄 Setting up backward compatibility...${NC}"

    # Create type aliases for backward compatibility in key files
    if [[ -f "internal/datastore/interface.go" ]]; then
        backup_file "internal/datastore/interface.go"

        # Add a comment about backward compatibility
        echo "
// Backward compatibility aliases (deprecated - use ProperPerson instead)
// TODO: Remove these aliases in next major version
type Individual = ProperPerson
const INDIVIDUAL = PROPER_PERSON
const EntityTypeIndividual = EntityTypeProperPerson
" >> "internal/datastore/interface.go"
    fi

    echo -e "${GREEN}✅ Added backward compatibility aliases${NC}"
}

# Main execution
main() {
    echo -e "${BLUE}Starting refactoring process...${NC}"
    if [[ "$CREATE_BACKUPS" == "true" ]]; then
        echo -e "${YELLOW}📁 Creating backup files for safety${NC}"
    else
        echo -e "${YELLOW}⚠️  No backup files will be created${NC}"
    fi
    echo ""

    # 1. SQL files (database schema)
    refactor_sql_files
    echo ""

    # 2. Go source files
    refactor_go_files
    echo ""

    # 3. Test data and mocks
    refactor_test_data
    echo ""

    # 4. Documentation
    refactor_documentation
    echo ""

    # 5. Configuration files
    refactor_config_files
    echo ""

    # 6. Special complex cases
    apply_special_fixes
    echo ""

    # 7. Backward compatibility
    fix_backward_compatibility
    echo ""

    echo -e "${GREEN}🎉 Refactoring completed successfully!${NC}"
    echo ""
    echo -e "${YELLOW}📋 Next steps:${NC}"
    echo "1. Run the database migration: sql/migrations/004_rename_individuals_to_proper_persons.sql"
    echo "2. Update imports and fix any compilation errors"
    echo "3. Run tests to verify everything works: make test"
    echo "4. Update any hardcoded strings in external configuration"
    echo "5. Review and commit changes"
    echo ""
    if [[ "$CREATE_BACKUPS" == "true" ]]; then
        echo -e "${BLUE}💡 Note: Backup files (.backup.*) have been created for all modified files${NC}"
        echo -e "${BLUE}    You can clean them up with: find . -name '*.backup.*' -delete${NC}"
    fi
    echo ""
    echo -e "${RED}⚠️  Important: Review all changes before committing!${NC}"
}

# Set backup preference (default: false to reduce clutter)
CREATE_BACKUPS="${CREATE_BACKUPS:-false}"

# Check for backup flag
if [[ "$1" == "--with-backups" ]]; then
    CREATE_BACKUPS="true"
    shift
fi

# Check for dry run mode
if [[ "$1" == "--dry-run" ]]; then
    echo -e "${YELLOW}🔍 DRY RUN MODE - No files will be modified${NC}"
    echo "This would refactor the following types of files:"
    echo "• SQL files: $(find sql -name "*.sql" | wc -l) files"
    echo "• Go files: $(find . -name "*.go" | wc -l) files"
    echo "• Test files: $(find . -name "*_test.go" | wc -l) files"
    echo "• Documentation: $(find . -name "*.md" | wc -l) files"
    echo ""
    echo "Run without --dry-run to execute the refactoring"
    exit 0
fi

# Confirm before proceeding
if [[ "$1" != "--yes" ]]; then
    echo -e "${YELLOW}⚠️  This will modify many files in the project.${NC}"
    if [[ "$CREATE_BACKUPS" == "true" ]]; then
        echo -e "${YELLOW}   Backup files will be created automatically.${NC}"
    else
        echo -e "${YELLOW}   No backup files will be created (use --with-backups to enable).${NC}"
    fi
    echo ""
    read -p "Continue with refactoring? (y/N): " -n 1 -r
    echo ""
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        echo "Refactoring cancelled."
        exit 0
    fi
fi

# Execute main function
main
