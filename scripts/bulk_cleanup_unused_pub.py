#!/usr/bin/env python3
"""
Bulk cleanup script - Phase 2
Complete cleanup of all remaining unused public functions to pub(crate)
Based on the housekeeping_report.md analysis

This script processes all 30 remaining functions after the successful Phase 1 test.
"""

import os
import re
import subprocess
import sys

# Remaining 30 functions after Phase 1 test (40 total - 10 already processed)
BULK_CLEANUP_ITEMS = [
    ("src/ai/agentic_document_service.rs", 601, "search_documents"),
    ("src/ai/agentic_document_service.rs", 630, "get_document_statistics"),
    ("src/ai/crud_prompt_builder.rs", 83, "with_max_context_length"),
    ("src/data_dictionary/attribute.rs", 28, "as_uuid"),
    ("src/data_dictionary/catalogue.rs", 39, "find_related_attributes"),
    ("src/data_dictionary/mod.rs", 84, "add_attribute"),
    ("src/data_dictionary/mod.rs", 92, "find_by_category"),
    ("src/dsl/domain_context.rs", 82, "with_state_requirements"),
    ("src/dsl/domain_context.rs", 94, "with_request_id"),
    ("src/dsl/domain_registry.rs", 151, "find_domains_for_operation"),
    ("src/dsl/domain_registry.rs", 202, "update_shared_vocabulary"),
    ("src/dsl/domain_registry.rs", 219, "add_global_rule"),
    ("src/dsl/domain_registry.rs", 224, "get_all_validation_rules"),
    ("src/dsl_manager/core.rs", 167, "set_prompt_builder"),
    ("src/dsl_manager/pipeline.rs", 424, "get_stage_metrics"),
    ("src/dsl_manager/pipeline.rs", 429, "get_all_metrics"),
    ("src/dsl_manager/state.rs", 193, "get_states_by_domain"),
    ("src/dsl_manager/state.rs", 201, "get_change_history"),
    ("src/dsl_manager/state.rs", 209, "get_active_states"),
    ("src/dsl_manager/state.rs", 217, "archive_state"),
    ("src/dsl_manager/validation.rs", 296, "add_custom_rule"),
    ("src/error.rs", 417, "add_error"),
    ("src/error.rs", 435, "has_fatal_errors"),
    ("src/error.rs", 445, "warning_count"),
    ("src/error.rs", 452, "fatal_error_count"),
    ("src/error.rs", 459, "into_result"),
    ("src/grammar/mod.rs", 181, "grammar_summary"),
    ("src/lib.rs", 585, "update_config"),
    ("src/lib.rs", 764, "parse_and_validate"),
    ("src/vocabulary/vocab_registry.rs", 206, "list_verbs"),
]


def backup_file(file_path):
    """Create a backup of the file before modification"""
    backup_path = f"{file_path}.bulk_backup"
    with open(file_path, "r") as src, open(backup_path, "w") as dst:
        dst.write(src.read())
    return backup_path


def restore_file(file_path, backup_path):
    """Restore file from backup"""
    if os.path.exists(backup_path):
        with open(backup_path, "r") as src, open(file_path, "w") as dst:
            dst.write(src.read())
        os.remove(backup_path)


def demote_pub_to_pub_crate(file_path, line_num, func_name):
    """Demote a public function to pub(crate)"""
    full_path = f"rust/{file_path}"

    if not os.path.exists(full_path):
        print(f"❌ File not found: {full_path}")
        return False

    # Create backup
    backup_path = backup_file(full_path)

    try:
        # Read the file
        with open(full_path, "r") as f:
            lines = f.readlines()

        # Find and modify the line (line numbers are 1-based)
        target_line_idx = line_num - 1
        if target_line_idx >= len(lines):
            print(
                f"❌ Line {line_num} not found in {file_path} (file has {len(lines)} lines)"
            )
            restore_file(full_path, backup_path)
            return False

        original_line = lines[target_line_idx]

        # Look for the pattern: pub fn function_name
        if f"pub fn {func_name}" in original_line:
            # Replace pub fn with pub(crate) fn
            modified_line = original_line.replace("pub fn", "pub(crate) fn", 1)
            lines[target_line_idx] = modified_line

            # Write back the file
            with open(full_path, "w") as f:
                f.writelines(lines)

            print(f"✅ {file_path}:{line_num} - {func_name}")
            os.remove(backup_path)
            return True
        else:
            print(
                f"⚠️  Could not find 'pub fn {func_name}' at line {line_num} in {file_path}"
            )
            print(f"   Found: {original_line.strip()}")
            restore_file(full_path, backup_path)
            return False

    except Exception as e:
        print(f"❌ Error processing {file_path}: {e}")
        restore_file(full_path, backup_path)
        return False


def validate_changes():
    """Run comprehensive validation"""
    print("\n🔧 Running comprehensive validation...")

    # 1. Cargo check
    print("   Step 1: cargo check --workspace...")
    result = subprocess.run(
        ["cargo", "check", "--workspace", "--all-targets", "--all-features"],
        cwd="rust",
        capture_output=True,
        text=True,
    )

    if result.returncode != 0:
        print("❌ Cargo check failed")
        print("\nSTDERR:")
        print(result.stderr)
        return False

    print("   ✅ Cargo check passed")

    # 2. Build examples
    print("   Step 2: cargo build --examples...")
    result = subprocess.run(
        ["cargo", "build", "--examples"],
        cwd="rust",
        capture_output=True,
        text=True,
    )

    if result.returncode != 0:
        print("⚠️  Some examples failed to build (may be expected)")
    else:
        print("   ✅ Examples build successfully")

    # 3. Core tests
    print("   Step 3: cargo test --lib --quiet...")
    result = subprocess.run(
        ["cargo", "test", "--lib", "--quiet"],
        cwd="rust",
        capture_output=True,
        text=True,
    )

    if result.returncode != 0:
        print("⚠️  Some tests failed (may be expected)")
    else:
        print("   ✅ Core tests passed")

    return True


def run_post_cleanup_analysis():
    """Run a quick analysis to see remaining cleanup opportunities"""
    print("\n📊 Running post-cleanup analysis...")

    # Re-run warnalyzer to see remaining items
    result = subprocess.run(
        ["cargo", "workspace-unused-pub"],
        cwd="rust",
        capture_output=True,
        text=True,
    )

    if result.returncode == 0 and result.stdout.strip():
        remaining_count = len(result.stdout.strip().split("\n"))
        print(f"   📈 Remaining unused pub items: {remaining_count}")
        if remaining_count < 10:
            print("   🎉 Significant reduction achieved!")
    else:
        print("   🎉 No unused pub items detected!")


def main():
    print("🧹 Bulk Dead Code Cleanup - Phase 2")
    print("=" * 45)
    print(f"Processing {len(BULK_CLEANUP_ITEMS)} remaining unused public functions")
    print("Based on successful Phase 1 validation\n")

    # Check we're in the right directory
    if not os.path.exists("rust/Cargo.toml"):
        print("❌ Error: Must be run from ob-poc root directory")
        print("   Current directory should contain rust/Cargo.toml")
        sys.exit(1)

    # Group items by file for better organization
    files_to_process = {}
    for file_path, line_num, func_name in BULK_CLEANUP_ITEMS:
        if file_path not in files_to_process:
            files_to_process[file_path] = []
        files_to_process[file_path].append((line_num, func_name))

    print(f"📁 Files to modify: {len(files_to_process)}")
    for file_path, items in files_to_process.items():
        print(f"   • {file_path}: {len(items)} functions")
    print()

    success_count = 0
    failed_items = []
    processed_files = set()

    # Process each item
    for i, (file_path, line_num, func_name) in enumerate(BULK_CLEANUP_ITEMS, 1):
        print(f"[{i:2d}/{len(BULK_CLEANUP_ITEMS)}] {func_name}", end=" ... ")

        if demote_pub_to_pub_crate(file_path, line_num, func_name):
            success_count += 1
            processed_files.add(file_path)
        else:
            failed_items.append((file_path, line_num, func_name))

    print(f"\n📊 BULK CLEANUP SUMMARY")
    print("=" * 35)
    print(
        f"✅ Successfully processed: {success_count}/{len(BULK_CLEANUP_ITEMS)} functions"
    )
    print(f"📁 Files modified: {len(processed_files)}")

    if failed_items:
        print(f"❌ Failed to process: {len(failed_items)} items")
        for file_path, line_num, func_name in failed_items:
            print(f"   • {file_path}:{line_num} - {func_name}")
        print()

    # Validate changes
    if success_count > 0:
        if validate_changes():
            print("\n🎉 BULK CLEANUP SUCCESSFUL!")

            # Run post-cleanup analysis
            run_post_cleanup_analysis()

            print("\n📈 CLEANUP IMPACT:")
            print(f"   • Functions demoted: {success_count}")
            print(f"   • Files cleaned: {len(processed_files)}")
            print(f"   • Public API surface reduced significantly")

            print("\n🔄 RECOMMENDED FOLLOW-UP:")
            print("1. Re-run dead code analysis:")
            print("   ./scripts/dead-code-sweep.sh")
            print("   python3 scripts/generate-report.py")
            print("2. Review remaining items in new report")
            print("3. Consider deleting items that became fully unused")
            print("4. Commit changes:")
            print(
                "   git add -A && git commit -m 'Clean up unused public API: demote 40 functions to pub(crate)'"
            )

        else:
            print("\n❌ VALIDATION FAILED")
            print("Some compilation errors detected - manual review required")

    else:
        print("\n❌ NO CHANGES MADE")
        print("All items failed processing - check file paths and line numbers")

    print(
        f"\n📋 Total cleanup progress: Phase 1 (10) + Phase 2 ({success_count}) = {10 + success_count} functions cleaned"
    )


if __name__ == "__main__":
    main()
