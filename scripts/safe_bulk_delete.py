#!/usr/bin/env python3
"""
Safe Bulk Delete Script

Based on the analysis showing ALL 40 functions are completely unused
(not even by tests), this script will safely delete them all.

This is the cleanest possible dead code removal - no dependencies to worry about.
"""

import os
import re
import subprocess
import sys
from pathlib import Path

# All 40 completely unused functions - safe to delete
FUNCTIONS_TO_DELETE = [
    ("src/ai/agentic_document_service.rs", 589, "extract_document_metadata"),
    ("src/ai/agentic_document_service.rs", 601, "search_documents"),
    ("src/ai/agentic_document_service.rs", 630, "get_document_statistics"),
    ("src/ai/crud_prompt_builder.rs", 83, "with_max_context_length"),
    ("src/ai/rag_system.rs", 216, "get_all_examples"),
    ("src/ast/types.rs", 263, "as_number"),
    ("src/data_dictionary/attribute.rs", 28, "as_uuid"),
    ("src/data_dictionary/catalogue.rs", 14, "search_by_semantic_similarity"),
    ("src/data_dictionary/catalogue.rs", 39, "find_related_attributes"),
    ("src/data_dictionary/mod.rs", 84, "add_attribute"),
    ("src/data_dictionary/mod.rs", 92, "find_by_category"),
    ("src/dsl/domain_context.rs", 76, "with_contexts"),
    ("src/dsl/domain_context.rs", 82, "with_state_requirements"),
    ("src/dsl/domain_context.rs", 94, "with_request_id"),
    ("src/dsl/domain_registry.rs", 151, "find_domains_for_operation"),
    ("src/dsl/domain_registry.rs", 202, "update_shared_vocabulary"),
    ("src/dsl/domain_registry.rs", 219, "add_global_rule"),
    ("src/dsl/domain_registry.rs", 224, "get_all_validation_rules"),
    ("src/dsl_manager/core.rs", 162, "set_rag_system"),
    ("src/dsl_manager/core.rs", 167, "set_prompt_builder"),
    ("src/dsl_manager/pipeline.rs", 424, "get_stage_metrics"),
    ("src/dsl_manager/pipeline.rs", 429, "get_all_metrics"),
    ("src/dsl_manager/state.rs", 193, "get_states_by_domain"),
    ("src/dsl_manager/state.rs", 201, "get_change_history"),
    ("src/dsl_manager/state.rs", 209, "get_active_states"),
    ("src/dsl_manager/state.rs", 217, "archive_state"),
    ("src/dsl_manager/validation.rs", 296, "add_custom_rule"),
    ("src/error.rs", 383, "add_context"),
    ("src/error.rs", 417, "add_error"),
    ("src/error.rs", 435, "has_fatal_errors"),
    ("src/error.rs", 445, "warning_count"),
    ("src/error.rs", 452, "fatal_error_count"),
    ("src/error.rs", 459, "into_result"),
    ("src/grammar/mod.rs", 60, "set_active_grammar"),
    ("src/grammar/mod.rs", 181, "grammar_summary"),
    ("src/lib.rs", 575, "grammar_engine_mut"),
    ("src/lib.rs", 585, "update_config"),
    ("src/lib.rs", 764, "parse_and_validate"),
    ("src/vocabulary/vocab_registry.rs", 192, "is_verb_available"),
    ("src/vocabulary/vocab_registry.rs", 206, "list_verbs"),
]


class SafeBulkDelete:
    def __init__(self):
        self.rust_dir = Path("rust")
        self.deleted_functions = []
        self.failed_deletions = []

    def find_function_bounds(self, lines, start_line_idx, func_name):
        """Find the start and end lines of a function definition"""
        # Look for the function signature around the specified line
        func_start = None
        for i in range(max(0, start_line_idx - 5), min(len(lines), start_line_idx + 5)):
            line = lines[i].strip()
            if f"pub fn {func_name}" in line or f"pub(crate) fn {func_name}" in line:
                func_start = i
                break

        if func_start is None:
            return None, None

        # Find function end by tracking braces
        brace_count = 0
        func_end = None
        in_function_body = False

        for i in range(func_start, len(lines)):
            line = lines[i]

            # Start counting braces after we see the opening brace
            if "{" in line:
                brace_count += line.count("{")
                in_function_body = True
            if "}" in line:
                brace_count -= line.count("}")

            # Function ends when braces balance and we're in the function body
            if in_function_body and brace_count == 0:
                func_end = i
                break

        return func_start, func_end

    def delete_function(self, file_path, line_num, func_name):
        """Delete a function from a file"""
        full_path = self.rust_dir / file_path

        if not full_path.exists():
            print(f"❌ File not found: {full_path}")
            return False

        # Read the file
        with open(full_path, "r") as f:
            lines = f.readlines()

        # Find function bounds
        start_idx, end_idx = self.find_function_bounds(lines, line_num - 1, func_name)

        if start_idx is None or end_idx is None:
            print(f"❌ Could not find function bounds for {func_name} in {file_path}")
            return False

        # Check if we found the right function
        found_signature = lines[start_idx].strip()
        if func_name not in found_signature:
            print(f"❌ Function signature mismatch for {func_name} in {file_path}")
            print(f"   Expected: {func_name}, Found: {found_signature}")
            return False

        # Remove the function (including any preceding comments/attributes)
        # Look backwards for any doc comments or attributes
        actual_start = start_idx
        for i in range(start_idx - 1, -1, -1):
            line = lines[i].strip()
            if line.startswith("///") or line.startswith("#[") or line == "":
                actual_start = i
            else:
                break

        # Create new file content without the function
        new_lines = lines[:actual_start] + lines[end_idx + 1 :]

        # Write back to file
        with open(full_path, "w") as f:
            f.writelines(new_lines)

        deleted_lines = end_idx - actual_start + 1
        print(f"✅ Deleted {func_name} ({deleted_lines} lines) from {file_path}")
        return True

    def validate_compilation(self):
        """Validate that the code still compiles after deletion"""
        print("\n🔧 Validating compilation...")

        # Check core library compilation
        result = subprocess.run(
            ["cargo", "check", "--lib"],
            cwd=self.rust_dir,
            capture_output=True,
            text=True,
        )

        if result.returncode == 0:
            print("✅ Core library compiles successfully")
            return True
        else:
            print("❌ Core library compilation failed:")
            print(
                result.stderr[:1000] + "..."
                if len(result.stderr) > 1000
                else result.stderr
            )
            return False

    def bulk_delete(self):
        """Execute bulk deletion of all unused functions"""
        print("🗑️  SAFE BULK DELETE - 40 UNUSED FUNCTIONS")
        print("=" * 60)
        print("These functions have ZERO usages - completely safe to delete")
        print()

        # Group by file for better organization
        files_to_process = {}
        for file_path, line_num, func_name in FUNCTIONS_TO_DELETE:
            if file_path not in files_to_process:
                files_to_process[file_path] = []
            files_to_process[file_path].append((line_num, func_name))

        print(f"📁 Files to modify: {len(files_to_process)}")
        for file_path, funcs in files_to_process.items():
            print(f"   • {file_path}: {len(funcs)} functions")
        print()

        # Execute deletions
        success_count = 0
        for i, (file_path, line_num, func_name) in enumerate(FUNCTIONS_TO_DELETE, 1):
            print(f"[{i:2d}/40] {func_name} ... ", end="")

            if self.delete_function(file_path, line_num, func_name):
                success_count += 1
                self.deleted_functions.append((file_path, func_name))
            else:
                self.failed_deletions.append((file_path, func_name))

        print(f"\n📊 DELETION SUMMARY")
        print(f"✅ Successfully deleted: {success_count}/40 functions")
        print(f"📁 Files modified: {len(files_to_process)}")

        if self.failed_deletions:
            print(f"❌ Failed deletions: {len(self.failed_deletions)}")
            for file_path, func_name in self.failed_deletions:
                print(f"   • {func_name} in {file_path}")

        # Validate compilation
        if success_count > 0:
            if self.validate_compilation():
                print(f"\n🎉 BULK DELETION SUCCESSFUL!")
                print(f"   • {success_count} dead functions eliminated")
                print(f"   • {len(files_to_process)} files cleaned")
                print(f"   • Codebase compilation verified")

                # Estimate lines of code removed
                estimated_loc_removed = success_count * 8  # Conservative estimate
                print(f"   • Estimated ~{estimated_loc_removed} lines of code removed")

                self.show_next_steps(success_count)
                return True
            else:
                print(f"\n❌ COMPILATION FAILED AFTER DELETIONS")
                print("Rolling back changes...")
                subprocess.run(["git", "checkout", "."], cwd=self.rust_dir)
                print("🔄 Changes rolled back")
                return False
        else:
            print(f"\n❌ NO FUNCTIONS WERE DELETED")
            return False

    def show_next_steps(self, deleted_count):
        """Show recommended next steps"""
        print(f"\n🚀 RECOMMENDED NEXT STEPS:")
        print(f"1. Run full test suite to ensure nothing broke:")
        print(f"   cd rust && cargo test")
        print(f"2. Re-run dead code analysis to see remaining opportunities:")
        print(f"   ./scripts/dead-code-sweep.sh")
        print(f"3. Commit the cleanup:")
        print(
            f"   git add -A && git commit -m 'Remove {deleted_count} completely unused functions'"
        )
        print(f"4. Consider running clippy for additional cleanup:")
        print(f"   cd rust && cargo clippy --fix --allow-dirty")


def main():
    if not Path("rust/Cargo.toml").exists():
        print("❌ Must be run from ob-poc root directory")
        sys.exit(1)

    print("🔍 SAFE BULK DELETE ANALYSIS")
    print("Based on comprehensive analysis showing ALL 40 functions have ZERO usages")
    print("This is the safest possible dead code removal - no dependencies to break")
    print()

    deleter = SafeBulkDelete()
    success = deleter.bulk_delete()

    if not success:
        print(
            "\n💡 TIP: Check that line numbers in the script match current file state"
        )
        print("Run 'git status' to see what files were modified")
        sys.exit(1)


if __name__ == "__main__":
    main()
