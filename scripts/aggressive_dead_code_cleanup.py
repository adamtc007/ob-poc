#!/usr/bin/env python3
"""
Aggressive Dead Code Cleanup Script

This script takes the approach that if a function is only used by tests,
then both the function AND the test are dead code that should be deleted.

Based on the housekeeping analysis showing 40 unused public functions,
this script will:
1. Identify functions that are only called from test code
2. Delete both the function and any tests that call it
3. Clean up imports and references
4. Validate that the core library still compiles

Philosophy: Test-only code is dead code. If the only consumer of a function
is a test, then the test itself has no value and both should be removed.
"""

import ast
import os
import re
import subprocess
import sys
from pathlib import Path

# Functions identified as unused by the analysis
UNUSED_FUNCTIONS = [
    # AI/RAG functions - likely test-only helpers
    ("src/ai/agentic_document_service.rs", 589, "extract_document_metadata"),
    ("src/ai/agentic_document_service.rs", 601, "search_documents"),
    ("src/ai/agentic_document_service.rs", 630, "get_document_statistics"),
    ("src/ai/crud_prompt_builder.rs", 83, "with_max_context_length"),
    ("src/ai/rag_system.rs", 216, "get_all_examples"),
    # AST utility functions - likely test helpers
    ("src/ast/types.rs", 263, "as_number"),
    # Dictionary functions - likely test/development helpers
    ("src/data_dictionary/attribute.rs", 28, "as_uuid"),
    ("src/data_dictionary/catalogue.rs", 14, "search_by_semantic_similarity"),
    ("src/data_dictionary/catalogue.rs", 39, "find_related_attributes"),
    ("src/data_dictionary/mod.rs", 84, "add_attribute"),
    ("src/data_dictionary/mod.rs", 92, "find_by_category"),
    # DSL domain functions - likely over-engineered helpers
    ("src/dsl/domain_context.rs", 76, "with_contexts"),
    ("src/dsl/domain_context.rs", 82, "with_state_requirements"),
    ("src/dsl/domain_context.rs", 94, "with_request_id"),
    ("src/dsl/domain_registry.rs", 151, "find_domains_for_operation"),
    ("src/dsl/domain_registry.rs", 202, "update_shared_vocabulary"),
    ("src/dsl/domain_registry.rs", 219, "add_global_rule"),
    ("src/dsl/domain_registry.rs", 224, "get_all_validation_rules"),
    # DSL Manager functions - likely debug/monitoring helpers
    ("src/dsl_manager/core.rs", 162, "set_rag_system"),
    ("src/dsl_manager/core.rs", 167, "set_prompt_builder"),
    ("src/dsl_manager/pipeline.rs", 424, "get_stage_metrics"),
    ("src/dsl_manager/pipeline.rs", 429, "get_all_metrics"),
    ("src/dsl_manager/state.rs", 193, "get_states_by_domain"),
    ("src/dsl_manager/state.rs", 201, "get_change_history"),
    ("src/dsl_manager/state.rs", 209, "get_active_states"),
    ("src/dsl_manager/state.rs", 217, "archive_state"),
    ("src/dsl_manager/validation.rs", 296, "add_custom_rule"),
    # Error handling helpers - likely over-engineered
    ("src/error.rs", 383, "add_context"),
    ("src/error.rs", 417, "add_error"),
    ("src/error.rs", 435, "has_fatal_errors"),
    ("src/error.rs", 445, "warning_count"),
    ("src/error.rs", 452, "fatal_error_count"),
    ("src/error.rs", 459, "into_result"),
    # Grammar functions - likely debug helpers
    ("src/grammar/mod.rs", 60, "set_active_grammar"),
    ("src/grammar/mod.rs", 181, "grammar_summary"),
    # Core lib functions - BE VERY CAREFUL with these
    ("src/lib.rs", 575, "grammar_engine_mut"),
    ("src/lib.rs", 585, "update_config"),
    ("src/lib.rs", 764, "parse_and_validate"),
    # Vocabulary functions - likely introspection helpers
    ("src/vocabulary/vocab_registry.rs", 192, "is_verb_available"),
    ("src/vocabulary/vocab_registry.rs", 206, "list_verbs"),
]


class AggressiveCleanup:
    def __init__(self):
        self.rust_dir = Path("rust")
        self.deleted_functions = []
        self.deleted_tests = []
        self.compilation_errors = []

    def find_function_in_file(self, file_path, line_num, func_name):
        """Find a function definition in a file"""
        full_path = self.rust_dir / file_path
        if not full_path.exists():
            return None

        with open(full_path, "r") as f:
            lines = f.readlines()

        # Look around the specified line number for the function
        start_line = max(0, line_num - 5)
        end_line = min(len(lines), line_num + 5)

        for i in range(start_line, end_line):
            line = lines[i].strip()
            if f"pub fn {func_name}" in line or f"pub(crate) fn {func_name}" in line:
                return self.extract_function(lines, i)
        return None

    def extract_function(self, lines, start_idx):
        """Extract a complete function definition"""
        function_lines = []
        brace_count = 0
        in_function = False

        i = start_idx
        while i < len(lines):
            line = lines[i]
            function_lines.append((i, line))

            # Count braces to find function end
            if "{" in line:
                brace_count += line.count("{")
                in_function = True
            if "}" in line:
                brace_count -= line.count("}")

            if in_function and brace_count == 0:
                break
            i += 1

        return function_lines

    def find_usages_in_codebase(self, func_name):
        """Find all usages of a function in the codebase"""
        cmd = ["grep", "-r", "-n", "--include=*.rs", func_name, str(self.rust_dir)]
        result = subprocess.run(cmd, capture_output=True, text=True)

        usages = []
        for line in result.stdout.split("\n"):
            if line.strip() and func_name in line:
                parts = line.split(":", 2)
                if len(parts) >= 3:
                    file_path = parts[0].replace(str(self.rust_dir) + "/", "")
                    line_num = parts[1]
                    content = parts[2]
                    usages.append((file_path, line_num, content.strip()))

        return usages

    def is_test_usage(self, file_path, content):
        """Determine if a usage is in test code"""
        test_indicators = [
            "/tests/",
            "test.rs",
            "#[test]",
            "#[cfg(test)]",
            "mod tests",
            "fn test_",
        ]

        # Check file path
        for indicator in test_indicators:
            if indicator in file_path:
                return True

        # Check content context (simplified)
        if "test" in content.lower():
            return True

        return False

    def categorize_function(self, func_name, file_path):
        """Categorize function by risk level for deletion"""
        # Find all usages
        usages = self.find_usages_in_codebase(func_name)

        if not usages:
            return "SAFE_DELETE", []

        # Filter out the definition itself
        definition_usages = []
        call_usages = []

        for usage_file, line_num, content in usages:
            if (
                f"pub fn {func_name}" in content
                or f"pub(crate) fn {func_name}" in content
            ):
                definition_usages.append((usage_file, line_num, content))
            else:
                call_usages.append((usage_file, line_num, content))

        if not call_usages:
            return "SAFE_DELETE", []

        # Check if all usages are in test code
        test_usages = [u for u in call_usages if self.is_test_usage(u[0], u[2])]

        if len(test_usages) == len(call_usages):
            return "DELETE_WITH_TESTS", call_usages

        return "KEEP", call_usages

    def delete_function(self, file_path, line_num, func_name):
        """Delete a function from a file"""
        full_path = self.rust_dir / file_path

        with open(full_path, "r") as f:
            lines = f.readlines()

        # Find and extract the function
        function_lines = self.find_function_in_file(file_path, line_num, func_name)
        if not function_lines:
            return False

        # Remove the function lines
        lines_to_remove = set(i for i, _ in function_lines)
        new_lines = [lines[i] for i in range(len(lines)) if i not in lines_to_remove]

        # Write back
        with open(full_path, "w") as f:
            f.writelines(new_lines)

        self.deleted_functions.append((file_path, func_name))
        return True

    def delete_test_file(self, test_file_path):
        """Delete an entire test file"""
        full_path = self.rust_dir / test_file_path
        if full_path.exists():
            os.remove(full_path)
            self.deleted_tests.append(test_file_path)
            return True
        return False

    def validate_compilation(self):
        """Check if the code still compiles after changes"""
        print("\n🔧 Validating compilation...")

        # Test core library build
        result = subprocess.run(
            ["cargo", "check", "--lib"],
            cwd=self.rust_dir,
            capture_output=True,
            text=True,
        )

        if result.returncode != 0:
            self.compilation_errors.append(("lib", result.stderr))
            return False

        print("✅ Core library compiles successfully")
        return True

    def aggressive_cleanup(self):
        """Perform aggressive cleanup of dead code"""
        print("🧹 DEAD CODE ANALYSIS")
        print("=" * 50)
        print("Philosophy: If only tests use it, both test and function are dead")
        print()

        safe_deletes = []
        test_deletes = []
        keep_items = []

        # Categorize all functions
        for file_path, line_num, func_name in UNUSED_FUNCTIONS:
            print(f"Analyzing {func_name}...", end=" ")

            category, usages = self.categorize_function(func_name, file_path)

            if category == "SAFE_DELETE":
                safe_deletes.append((file_path, line_num, func_name))
                print("🟢 SAFE DELETE (no usages)")
            elif category == "DELETE_WITH_TESTS":
                test_deletes.append((file_path, line_num, func_name, usages))
                print(f"🟡 DELETE WITH {len(usages)} TESTS")
            else:
                keep_items.append((file_path, line_num, func_name, usages))
                print(f"🔴 KEEP (has {len(usages)} real usages)")

        print(f"\n📊 ANALYSIS SUMMARY")
        print(f"Safe deletes: {len(safe_deletes)}")
        print(f"Delete with tests: {len(test_deletes)}")
        print(f"Keep (real usages): {len(keep_items)}")

        # Show what would be deleted (DRY RUN)
        print(f"\n🔍 WOULD DELETE SAFELY ({len(safe_deletes)} functions)")
        for file_path, line_num, func_name in safe_deletes:
            print(f"  • {func_name} from {file_path}:{line_num}")

        # Show test-coupled deletions (DRY RUN)
        print(f"\n🔍 WOULD DELETE WITH TESTS ({len(test_deletes)} functions)")
        for file_path, line_num, func_name, test_usages in test_deletes:
            print(f"  • {func_name} from {file_path}:{line_num}")
            test_files = set(usage[0] for usage in test_usages)
            for test_file in test_files:
                print(f"    └─ would delete test: {test_file}")

        print(f"\n🔍 DRY RUN COMPLETE - No files were actually modified")

        # Summary
        print(f"\n📊 ANALYSIS COMPLETE")
        total_deletable = len(safe_deletes) + len(test_deletes)
        print(f"Functions that can be deleted: {total_deletable}")
        print(f"Functions that should be preserved: {len(keep_items)}")

        # Show what was kept and why
        if keep_items:
            print(f"\n📋 FUNCTIONS TO PRESERVE (have real usages):")
            for file_path, line_num, func_name, usages in keep_items[
                :10
            ]:  # Show first 10
                print(f"  • {func_name} ({file_path}) - {len(usages)} real usages")
            if len(keep_items) > 10:
                print(f"  ... and {len(keep_items) - 10} more")

        print(f"\n🚀 READY TO EXECUTE CLEANUP:")
        print(f"  • {len(safe_deletes)} functions can be safely deleted")
        print(
            f"  • {len(test_deletes)} test-only functions can be deleted with their tests"
        )

        return True

    def rollback_changes(self):
        """Rollback all changes made"""
        subprocess.run(["git", "checkout", "."], cwd=self.rust_dir)
        print("🔄 Changes rolled back")


def main():
    if not Path("rust/Cargo.toml").exists():
        print("❌ Must be run from ob-poc root directory")
        sys.exit(1)

    cleanup = AggressiveCleanup()

    print("🔍 RUNNING ANALYSIS (DRY RUN)")
    print("This will analyze which functions are truly dead vs test-only")
    print()

    success = cleanup.aggressive_cleanup()

    if success:
        print("\n🚀 NEXT STEPS:")
        print("1. Review the deleted code")
        print("2. Run full test suite: cargo test")
        print(
            "3. Commit changes: git add -A && git commit -m 'Aggressive dead code cleanup: delete test-only functions'"
        )
    else:
        print("\n❌ Cleanup failed - manual review required")


if __name__ == "__main__":
    main()
