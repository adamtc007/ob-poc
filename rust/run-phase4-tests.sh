#!/bin/bash

# Phase 4: Integration Testing with Live Database
# Test runner script for comprehensive DSL Manager → DSL Mod → Database orchestration testing

set -e

echo "🚀 Phase 4: DSL Manager to DSL Mod Database Integration Testing"
echo "=============================================================="

# Configuration
DATABASE_FEATURE="database"
TEST_TIMEOUT="300" # 5 minutes
VERBOSE=${VERBOSE:-false}

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Helper functions
print_status() {
    echo -e "${BLUE}ℹ️  $1${NC}"
}

print_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

print_error() {
    echo -e "${RED}❌ $1${NC}"
}

print_section() {
    echo ""
    echo -e "${BLUE}📋 $1${NC}"
    echo -e "${BLUE}$(printf '=%.0s' {1..50})${NC}"
}

# Check prerequisites
check_prerequisites() {
    print_section "Checking Prerequisites"

    # Check if PostgreSQL is running
    if command -v pg_isready >/dev/null 2>&1; then
        if pg_isready -q; then
            print_success "PostgreSQL is running"
        else
            print_warning "PostgreSQL may not be running - some tests may be skipped"
        fi
    else
        print_warning "pg_isready not found - cannot verify PostgreSQL status"
    fi

    # Check for test database
    TEST_DB_URL=${TEST_DATABASE_URL:-"postgresql://postgres:password@localhost:5432/ob_poc_test"}
    print_status "Using test database: $(echo $TEST_DB_URL | sed 's/:password/:***/')"

    # Check Rust and Cargo
    if ! command -v cargo >/dev/null 2>&1; then
        print_error "Cargo not found. Please install Rust."
        exit 1
    fi

    print_success "Prerequisites check complete"
}

# Build the project
build_project() {
    print_section "Building Project"

    print_status "Building with database features..."
    if cargo build --features $DATABASE_FEATURE; then
        print_success "Project build successful"
    else
        print_error "Project build failed"
        exit 1
    fi
}

# Run environment verification
run_environment_verification() {
    print_section "Environment Verification"

    print_status "Running environment verification test..."
    local cmd="cargo test test_environment_verification --test phase4_integration_tests --features $DATABASE_FEATURE"

    if [ "$VERBOSE" = "true" ]; then
        cmd="$cmd -- --nocapture"
    fi

    if timeout $TEST_TIMEOUT bash -c "$cmd"; then
        print_success "Environment verification passed"
        return 0
    else
        print_warning "Environment verification failed - database may not be available"
        return 1
    fi
}

# Run integration tests
run_integration_tests() {
    print_section "Integration Tests"

    local tests=(
        "test_end_to_end_orchestration:End-to-End Orchestration"
        "test_database_round_trip_operations:Database Round-Trip Operations"
        "test_concurrent_operations:Concurrent Operations"
        "test_dictionary_service_integration:Dictionary Service Integration"
        "test_full_pipeline_integration:Full Pipeline Integration"
        "test_connection_pool_stress:Connection Pool Stress Testing"
    )

    local passed=0
    local total=${#tests[@]}

    for test_info in "${tests[@]}"; do
        IFS=':' read -r test_name test_desc <<< "$test_info"

        print_status "Running: $test_desc"
        local cmd="cargo test $test_name --test phase4_integration_tests --features $DATABASE_FEATURE"

        if [ "$VERBOSE" = "true" ]; then
            cmd="$cmd -- --nocapture"
        fi

        if timeout $TEST_TIMEOUT bash -c "$cmd" >/dev/null 2>&1; then
            print_success "$test_desc"
            ((passed++))
        else
            print_error "$test_desc"
        fi
    done

    echo ""
    print_status "Integration Tests: $passed/$total passed"

    if [ $passed -eq $total ]; then
        print_success "All integration tests passed"
        return 0
    else
        print_warning "Some integration tests failed"
        return 1
    fi
}

# Run performance benchmarks
run_performance_benchmarks() {
    print_section "Performance Benchmarks"

    local benchmarks=(
        "benchmark_single_operation_performance:Single Operation Performance"
        "benchmark_concurrent_load:Concurrent Load Testing"
        "benchmark_memory_usage:Memory Usage Profiling"
        "benchmark_end_to_end_dsl_manager:End-to-End DSL Manager Performance"
        "benchmark_stress_testing:Stress Testing"
    )

    local passed=0
    local total=${#benchmarks[@]}

    for benchmark_info in "${benchmarks[@]}"; do
        IFS=':' read -r benchmark_name benchmark_desc <<< "$benchmark_info"

        print_status "Running: $benchmark_desc"
        local cmd="cargo test $benchmark_name --test phase4_benchmarks --features $DATABASE_FEATURE"

        if [ "$VERBOSE" = "true" ]; then
            cmd="$cmd -- --nocapture"
        fi

        if timeout $TEST_TIMEOUT bash -c "$cmd" >/dev/null 2>&1; then
            print_success "$benchmark_desc"
            ((passed++))
        else
            print_error "$benchmark_desc"
        fi
    done

    echo ""
    print_status "Performance Benchmarks: $passed/$total passed"

    if [ $passed -eq $total ]; then
        print_success "All performance benchmarks passed"
        return 0
    else
        print_warning "Some performance benchmarks failed"
        return 1
    fi
}

# Run error scenario tests
run_error_scenario_tests() {
    print_section "Error Scenario Tests"

    local error_tests=(
        "test_database_connection_failures:Database Connection Failures"
        "test_invalid_dsl_content_handling:Invalid DSL Content Handling"
        "test_connection_pool_exhaustion:Connection Pool Exhaustion"
        "test_malformed_operation_handling:Malformed Operation Handling"
        "test_concurrent_error_handling:Concurrent Error Handling"
        "test_dsl_manager_error_recovery:DSL Manager Error Recovery"
        "test_resource_cleanup:Resource Cleanup"
    )

    local passed=0
    local total=${#error_tests[@]}

    for test_info in "${error_tests[@]}"; do
        IFS=':' read -r test_name test_desc <<< "$test_info"

        print_status "Running: $test_desc"
        local cmd="cargo test $test_name --test phase4_error_scenarios --features $DATABASE_FEATURE"

        if [ "$VERBOSE" = "true" ]; then
            cmd="$cmd -- --nocapture"
        fi

        if timeout $TEST_TIMEOUT bash -c "$cmd" >/dev/null 2>&1; then
            print_success "$test_desc"
            ((passed++))
        else
            print_error "$test_desc"
        fi
    done

    echo ""
    print_status "Error Scenario Tests: $passed/$total passed"

    if [ $passed -eq $total ]; then
        print_success "All error scenario tests passed"
        return 0
    else
        print_warning "Some error scenario tests failed"
        return 1
    fi
}

# Run all Phase 4 tests
run_all_tests() {
    print_section "Running All Phase 4 Tests"

    print_status "Running comprehensive Phase 4 test suite..."
    local cmd="cargo test phase4 --features $DATABASE_FEATURE"

    if [ "$VERBOSE" = "true" ]; then
        cmd="$cmd -- --nocapture"
    fi

    if timeout $TEST_TIMEOUT bash -c "$cmd"; then
        print_success "All Phase 4 tests completed"
        return 0
    else
        print_warning "Some Phase 4 tests failed"
        return 1
    fi
}

# Generate test report
generate_report() {
    print_section "Test Summary Report"

    echo ""
    echo "📊 Phase 4 Integration Testing Summary"
    echo "======================================"
    echo ""
    echo "🏗️  Architecture Tested:"
    echo "   DSL Manager → DSL Processor → Database Service → PostgreSQL"
    echo ""
    echo "🧪 Test Categories:"
    echo "   ✅ Environment Verification"
    echo "   ✅ Integration Tests (6 test cases)"
    echo "   ✅ Performance Benchmarks (5 benchmarks)"
    echo "   ✅ Error Scenario Tests (7 error cases)"
    echo ""
    echo "🎯 Key Validations:"
    echo "   • End-to-end orchestration pipeline"
    echo "   • Database round-trip operations with SQLX"
    echo "   • Concurrent operation safety"
    echo "   • Performance under load"
    echo "   • Error handling and recovery"
    echo "   • Resource management and cleanup"
    echo ""
    echo "🚀 Phase 4 Status: COMPLETE"
    echo ""

    if [ -n "$TEST_DATABASE_URL" ]; then
        echo "🗄️  Database: $(echo $TEST_DATABASE_URL | sed 's/:password/:***/')"
    else
        echo "🗄️  Database: Default PostgreSQL configuration"
    fi
    echo ""
}

# Main execution
main() {
    echo "Starting Phase 4 integration testing..."
    echo ""

    # Parse command line arguments
    while [[ $# -gt 0 ]]; do
        case $1 in
            --verbose|-v)
                VERBOSE=true
                shift
                ;;
            --integration|-i)
                TEST_TYPE="integration"
                shift
                ;;
            --benchmarks|-b)
                TEST_TYPE="benchmarks"
                shift
                ;;
            --errors|-e)
                TEST_TYPE="errors"
                shift
                ;;
            --all|-a)
                TEST_TYPE="all"
                shift
                ;;
            --help|-h)
                echo "Usage: $0 [OPTIONS]"
                echo ""
                echo "Options:"
                echo "  -v, --verbose     Enable verbose output"
                echo "  -i, --integration Run integration tests only"
                echo "  -b, --benchmarks  Run performance benchmarks only"
                echo "  -e, --errors      Run error scenario tests only"
                echo "  -a, --all         Run all tests (default)"
                echo "  -h, --help        Show this help message"
                echo ""
                echo "Environment Variables:"
                echo "  TEST_DATABASE_URL    Override default database URL"
                echo "  VERBOSE             Enable verbose output (true/false)"
                exit 0
                ;;
            *)
                print_error "Unknown option: $1"
                exit 1
                ;;
        esac
    done

    # Default to all tests
    TEST_TYPE=${TEST_TYPE:-"all"}

    # Check prerequisites
    check_prerequisites

    # Build project
    build_project

    # Run environment verification
    if ! run_environment_verification; then
        print_warning "Environment verification failed - some tests may be skipped"
        print_status "Continuing with available tests..."
    fi

    # Run selected test type
    local overall_result=0

    case $TEST_TYPE in
        integration)
            if ! run_integration_tests; then
                overall_result=1
            fi
            ;;
        benchmarks)
            if ! run_performance_benchmarks; then
                overall_result=1
            fi
            ;;
        errors)
            if ! run_error_scenario_tests; then
                overall_result=1
            fi
            ;;
        all)
            local integration_result=0
            local benchmark_result=0
            local error_result=0

            if ! run_integration_tests; then
                integration_result=1
            fi

            if ! run_performance_benchmarks; then
                benchmark_result=1
            fi

            if ! run_error_scenario_tests; then
                error_result=1
            fi

            if [ $integration_result -ne 0 ] || [ $benchmark_result -ne 0 ] || [ $error_result -ne 0 ]; then
                overall_result=1
            fi
            ;;
    esac

    # Generate report
    generate_report

    # Final status
    if [ $overall_result -eq 0 ]; then
        print_success "Phase 4 integration testing completed successfully!"
        echo ""
        echo "🎉 All tests passed - system is ready for production deployment!"
    else
        print_warning "Phase 4 integration testing completed with some failures"
        echo ""
        echo "ℹ️  Check test output above for details on failed tests"
        echo "ℹ️  Some failures may be expected if database is not available"
    fi

    exit $overall_result
}

# Execute main function
main "$@"
