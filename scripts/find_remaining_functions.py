#!/usr/bin/env python3
"""
Find current line numbers for the remaining 21 functions that need to be deleted.
This script searches for function definitions and reports their current locations.
"""

import os
import re
from pathlib import Path

# The 21 remaining functions that couldn't be deleted in the first pass
REMAINING_FUNCTIONS = [
    ("src/ai/agentic_document_service.rs", "search_documents"),
    ("src/ai/agentic_document_service.rs", "get_document_statistics"),
    ("src/data_dictionary/catalogue.rs", "find_related_attributes"),
    ("src/dsl/domain_context.rs", "with_state_requirements"),
    ("src/dsl/domain_context.rs", "with_request_id"),
    ("src/dsl/domain_registry.rs", "update_shared_vocabulary"),
    ("src/dsl/domain_registry.rs", "add_global_rule"),
    ("src/dsl/domain_registry.rs", "get_all_validation_rules"),
    ("src/dsl_manager/pipeline.rs", "get_stage_metrics"),
    ("src/dsl_manager/pipeline.rs", "get_all_metrics"),
    ("src/dsl_manager/state.rs", "get_change_history"),
    ("src/dsl_manager/state.rs", "get_active_states"),
    ("src/dsl_manager/state.rs", "archive_state"),
    ("src/dsl_manager/validation.rs", "add_custom_rule"),
    ("src/error.rs", "has_fatal_errors"),
    ("src/error.rs", "warning_count"),
    ("src/error.rs", "fatal_error_count"),
    ("src/error.rs", "into_result"),
    ("src/grammar/mod.rs", "grammar_summary"),
    ("src/lib.rs", "parse_and_validate"),
    ("src/vocabulary/vocab_registry.rs", "list_verbs"),
]


def find_function_line(file_path, func_name):
    """Find the current line number of a function in a file"""
    full_path = Path("rust") / file_path

    if not full_path.exists():
        return None, f"File not found: {full_path}"

    try:
        with open(full_path, "r") as f:
            lines = f.readlines()

        for i, line in enumerate(lines):
            # Look for pub fn function_name or pub(crate) fn function_name
            if f"pub fn {func_name}" in line or f"pub(crate) fn {func_name}" in line:
                return i + 1, line.strip()  # Return 1-based line number

        return None, f"Function '{func_name}' not found"

    except Exception as e:
        return None, f"Error reading file: {e}"


def main():
    print("🔍 FINDING REMAINING FUNCTIONS")
    print("=" * 50)
    print(f"Searching for {len(REMAINING_FUNCTIONS)} remaining functions...")
    print()

    found_functions = []
    missing_functions = []

    for file_path, func_name in REMAINING_FUNCTIONS:
        print(f"Searching for {func_name} in {file_path}...", end=" ")

        line_num, result = find_function_line(file_path, func_name)

        if line_num:
            found_functions.append((file_path, line_num, func_name))
            print(f"✅ Found at line {line_num}")
        else:
            missing_functions.append((file_path, func_name, result))
            print(f"❌ {result}")

    print(f"\n📊 SEARCH RESULTS")
    print(f"Found: {len(found_functions)} functions")
    print(f"Missing: {len(missing_functions)} functions")

    if found_functions:
        print(f"\n✅ FOUND FUNCTIONS (ready for deletion):")
        print("# Updated function list for deletion script")
        print("REMAINING_FUNCTIONS_TO_DELETE = [")
        for file_path, line_num, func_name in found_functions:
            print(f'    ("{file_path}", {line_num}, "{func_name}"),')
        print("]")

    if missing_functions:
        print(f"\n❌ MISSING FUNCTIONS:")
        for file_path, func_name, reason in missing_functions:
            print(f"  • {func_name} in {file_path}: {reason}")

    return len(found_functions)


if __name__ == "__main__":
    found_count = main()

    if found_count > 0:
        print(f"\n🚀 NEXT STEP:")
        print(f"Copy the function list above into the deletion script and execute!")
    else:
        print(
            f"\n⚠️  No functions found - they may have been renamed or already deleted"
        )
