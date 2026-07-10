#!/usr/bin/env bash
# CI lint: verify governed catalogue/snapshot writes stay on sanctioned paths.
#
# Guarantees:
# - Direct Rust SQL statements of the form INSERT/UPDATE/DELETE against
#   `"ob-poc".<catalogue_table>` or `sem_reg.snapshots` must live in a sanctioned
#   backing path and must match the checked-in baseline.
# - Mutator-looking methods in the catalogue store modules under `src/database/`
#   must not be bare `pub`; constructors/read APIs may stay public.
#
# Limits:
# - This is source scanning, not call-graph analysis. It does not prove that an
#   arbitrary indirect caller is verb-mediated, and it only sees direct SQL text
#   matching the patterns below.
#
# Usage:
#   ./scripts/lint_write_paths.sh           # Check for new or unsanctioned writers
#   ./scripts/lint_write_paths.sh --update  # Regenerate the sanctioned writer baseline
#   ./scripts/lint_write_paths.sh --self-test # Prove the public mutator guard trips

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT="$SCRIPT_DIR/.."
BASELINE="$SCRIPT_DIR/write_path_baseline.txt"

P1_TABLES=(
    "products"
    "services"
    "product_services"
    "service_versions"
    "service_resource_types"
    "service_resource_capabilities"
    "resource_owner_principals"
    "resource_attribute_requirements"
    "attribute_registry"
    "product_service_conditions"
    "product_service_option_overrides"
    "sem_reg.snapshots"
)

SCAN_ROOTS=(
    "src"
    "crates"
)

CATALOGUE_STORE_MODULES=(
    "src/database/product_service.rs"
    "src/database/service_service.rs"
    "src/database/service_resource_service.rs"
)

WRITE_PATTERN='(INSERT[[:space:]]+INTO|UPDATE|DELETE[[:space:]]+FROM)[[:space:]]+(("ob-poc")|sem_reg)\.([A-Za-z_][A-Za-z0-9_]*)'

is_p1_table() {
    local table="$1"
    local candidate
    for candidate in "${P1_TABLES[@]}"; do
        if [ "$candidate" = "$table" ]; then
            return 0
        fi
    done
    return 1
}

normalize_table() {
    local schema="$1"
    local table="$2"

    if [ "$schema" = "sem_reg" ]; then
        printf 'sem_reg.%s' "$table"
    else
        printf '%s' "$table"
    fi
}

is_sanctioned_path_for_table() {
    local table="$1"
    local path="$2"

    case "$table:$path" in
        products:src/domain_ops/catalogue_maintenance_ops.rs) return 0 ;;
        products:src/database/product_service.rs) return 0 ;;

        services:src/domain_ops/catalogue_maintenance_ops.rs) return 0 ;;
        services:src/database/service_service.rs) return 0 ;;

        product_services:src/domain_ops/catalogue_maintenance_ops.rs) return 0 ;;
        product_services:src/database/service_service.rs) return 0 ;;

        service_versions:src/domain_ops/catalogue_maintenance_ops.rs) return 0 ;;

        service_resource_types:src/domain_ops/catalogue_maintenance_ops.rs) return 0 ;;
        service_resource_types:src/database/service_resource_service.rs) return 0 ;;
        service_resource_types:src/service_resources/srdef_loader.rs) return 0 ;;

        service_resource_capabilities:src/domain_ops/catalogue_maintenance_ops.rs) return 0 ;;

        resource_owner_principals:src/domain_ops/catalogue_maintenance_ops.rs) return 0 ;;
        resource_owner_principals:src/service_resources/srdef_loader.rs) return 0 ;;
        resource_owner_principals:src/service_resources/onboarding_data_request.rs) return 0 ;;

        resource_attribute_requirements:src/service_resources/srdef_loader.rs) return 0 ;;

        attribute_registry:src/services/attribute_service_impl.rs) return 0 ;;
        attribute_registry:src/services/attribute_registry_enrichment.rs) return 0 ;;

        product_service_conditions:src/domain_ops/catalogue_maintenance_ops.rs) return 0 ;;

        product_service_option_overrides:crates/sem_os_postgres/src/ops/service_options.rs) return 0 ;;

        sem_reg.snapshots:src/services/attribute_service_impl.rs) return 0 ;;
        sem_reg.snapshots:src/services/phrase_service_impl.rs) return 0 ;;
        sem_reg.snapshots:src/sem_reg/store.rs) return 0 ;;
        sem_reg.snapshots:src/sem_reg/stewardship/tools_phase0.rs) return 0 ;;
        sem_reg.snapshots:crates/sem_os_postgres/src/store.rs) return 0 ;;
    esac

    return 1
}

function_signature_before_line() {
    local path="$1"
    local line_no="$2"

    awk -v target="$line_no" '
        NR > target { exit }
        /^[[:space:]]*(pub(\([^)]*\))?[[:space:]]+)?(async[[:space:]]+)?fn[[:space:]]+[A-Za-z_][A-Za-z0-9_]*/ {
            signature = $0
        }
        END { print signature }
    ' "$path"
}

is_public_database_writer() {
    local path="$1"
    local line_no="$2"
    local signature

    case "$path" in
        src/database/*.rs) ;;
        *) return 1 ;;
    esac

    signature="$(function_signature_before_line "$path" "$line_no")"
    [[ "$signature" =~ ^[[:space:]]*pub[[:space:]]+ ]]
}

is_allowed_public_catalogue_store_method() {
    local fn_name="$1"

    case "$fn_name" in
        new|pool|get_*|list_*) return 0 ;;
    esac

    return 1
}

scan_public_catalogue_store_methods() {
    local module
    for module in "${CATALOGUE_STORE_MODULES[@]}"; do
        [ -f "$module" ] || continue
        rg --no-heading -n '^[[:space:]]*pub[[:space:]]+(async[[:space:]]+)?fn[[:space:]]+[A-Za-z_][A-Za-z0-9_]*' "$module" \
            | while IFS= read -r hit; do
                [ -n "$hit" ] || continue
                path="${hit%%:*}"
                rest="${hit#*:}"
                line_no="${rest%%:*}"
                source="${rest#*:}"
                if [[ "$source" =~ fn[[:space:]]+([A-Za-z_][A-Za-z0-9_]*) ]]; then
                    fn_name="${BASH_REMATCH[1]}"
                else
                    continue
                fi
                if ! is_allowed_public_catalogue_store_method "$fn_name"; then
                    printf '%s:%s public catalogue store mutator `%s`; use pub(crate)\n' \
                        "$path" "$line_no" "$fn_name"
                fi
            done
    done
}

run_self_test() {
    local tmp
    tmp="$(mktemp -d)"
    trap 'rm -rf "$tmp"' RETURN
    mkdir -p "$tmp/src/database"
    cat > "$tmp/src/database/product_service.rs" <<'EOF'
pub struct ProductService;
impl ProductService {
    pub fn new() -> Self { Self }
    pub async fn create_product(&self) {}
    pub(crate) async fn update_product(&self) {}
}
EOF
    (
        cd "$tmp"
        CATALOGUE_STORE_MODULES=("src/database/product_service.rs")
        if scan_public_catalogue_store_methods | grep -q 'create_product'; then
            echo "✓ lint self-test passed: bare-public catalogue mutator was rejected"
            exit 0
        fi
        echo "✗ lint self-test failed: bare-public catalogue mutator was not detected"
        exit 1
    )
}

normalize_op() {
    case "$1" in
        INSERT*) printf 'INSERT' ;;
        UPDATE) printf 'UPDATE' ;;
        DELETE*) printf 'DELETE' ;;
        *) printf '%s' "$1" ;;
    esac
}

normalize_source_line() {
    printf '%s' "$1" | sed -E 's/[[:space:]]+/ /g; s/^[[:space:]]+//; s/[[:space:]]+$//'
}

scan_catalogue_writers() {
    rg --pcre2 --no-heading -n "$WRITE_PATTERN" "${SCAN_ROOTS[@]}" --glob '*.rs' 2>/dev/null || true
}

if [ "${1:-}" = "--self-test" ]; then
    run_self_test
    exit $?
fi

cd "$ROOT"

CURRENT_KEYS="$(mktemp)"
CURRENT_DETAILS="$(mktemp)"
HARD_VIOLATIONS="$(mktemp)"
trap 'rm -f "$CURRENT_KEYS" "$CURRENT_DETAILS" "$HARD_VIOLATIONS"' EXIT

while IFS= read -r hit; do
    [ -n "$hit" ] || continue

    path="${hit%%:*}"
    rest="${hit#*:}"
    line_no="${rest%%:*}"
    source="${rest#*:}"

    if [[ "$source" =~ (INSERT[[:space:]]+INTO|UPDATE|DELETE[[:space:]]+FROM)[[:space:]]+(\"ob-poc\"|sem_reg)\.([A-Za-z_][A-Za-z0-9_]*) ]]; then
        op="$(normalize_op "${BASH_REMATCH[1]}")"
        table="$(normalize_table "${BASH_REMATCH[2]}" "${BASH_REMATCH[3]}")"
    else
        continue
    fi

    if ! is_p1_table "$table"; then
        continue
    fi

    if ! is_sanctioned_path_for_table "$table" "$path"; then
        printf '%s:%s [%s %s] unsanctioned P1 catalogue writer\n' \
            "$path" "$line_no" "$op" "$table" >> "$HARD_VIOLATIONS"
        continue
    fi

    if is_public_database_writer "$path" "$line_no"; then
        printf '%s:%s [%s %s] public src/database catalogue writer; use pub(crate)\n' \
            "$path" "$line_no" "$op" "$table" >> "$HARD_VIOLATIONS"
        continue
    fi

    normalized_source="$(normalize_source_line "$source")"
    key="$(printf '%s\t%s\t%s\t%s' "$table" "$path" "$op" "$normalized_source")"
    printf '%s\n' "$key" >> "$CURRENT_KEYS"
    printf '%s\t%s:%s\n' "$key" "$path" "$line_no" >> "$CURRENT_DETAILS"
done < <(scan_catalogue_writers)

scan_public_catalogue_store_methods >> "$HARD_VIOLATIONS"

sort -u "$CURRENT_KEYS" -o "$CURRENT_KEYS"
sort -u "$CURRENT_DETAILS" -o "$CURRENT_DETAILS"

if [ -s "$HARD_VIOLATIONS" ]; then
    echo "✗ Unsanctioned P1 catalogue-table writers found:"
    echo ""
    sort -u "$HARD_VIOLATIONS"
    echo ""
    echo "Add a governed verb/projector backing and update the table allowlist before regenerating the baseline."
    exit 1
fi

if [ "${1:-}" = "--update" ]; then
    cp "$CURRENT_KEYS" "$BASELINE"
    count="$(grep -c . "$BASELINE" || true)"
    echo "Baseline updated: $count sanctioned P1 catalogue writer entries written to $BASELINE"
    exit 0
fi

if [ ! -f "$BASELINE" ]; then
    echo "⚠ No baseline file found. Run: ./scripts/lint_write_paths.sh --update"
    exit 1
fi

BASELINE_SORTED="$(mktemp)"
trap 'rm -f "$CURRENT_KEYS" "$CURRENT_DETAILS" "$HARD_VIOLATIONS" "$BASELINE_SORTED"' EXIT
sort -u "$BASELINE" > "$BASELINE_SORTED"

NEW_KEYS="$(comm -13 "$BASELINE_SORTED" "$CURRENT_KEYS")"
MISSING_KEYS="$(comm -23 "$BASELINE_SORTED" "$CURRENT_KEYS")"

if [ -n "$NEW_KEYS" ]; then
    echo "✗ New P1 catalogue-table write statements found:"
    echo ""
    while IFS= read -r key; do
        [ -n "$key" ] || continue
        grep -F "$key"$'\t' "$CURRENT_DETAILS" | cut -f5- | sed 's/^/  /'
        printf '    %s\n' "$key"
    done <<< "$NEW_KEYS"
    echo ""
    echo "If these are intentional governed writers, run: ./scripts/lint_write_paths.sh --update"
    exit 1
fi

if [ -n "$MISSING_KEYS" ]; then
    echo "✗ P1 catalogue writer baseline is stale; entries disappeared:"
    echo ""
    printf '%s\n' "$MISSING_KEYS" | sed 's/^/  /'
    echo ""
    echo "Run: ./scripts/lint_write_paths.sh --update"
    exit 1
fi

count="$(grep -c . "$BASELINE_SORTED" || true)"
echo "✓ P1 catalogue write-path lint passed. ($count sanctioned writer entries)"
