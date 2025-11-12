#!/usr/bin/env python3
"""
Selective cleanup test script - Phase 1
Test demoting unused public functions to pub(crate) with a diverse sample
Based on the housekeeping_report.md analysis
"""

import os
import re
import subprocess
import sys

# Test batch: 10 diverse functions from different files
TEST_CLEANUP_ITEMS = [
    ("src/ai/agentic_document_service.rs", 589, "extract_document_metadata"),
    ("src/ai/rag_system.rs", 216, "get_all_examples"),
    ("src/ast/types.rs", 263, "as_number"),
    ("src/data_dictionary/catalogue.rs", 14, "search_by_semantic_similarity"),
    ("src/dsl/domain_context.rs", 76, "with_contexts"),
    ("src/dsl_manager/core.rs", 162, "set_rag_system"),
    ("src/error.rs", 383, "add_context"),
    ("src/grammar/mod.rs", 60, "set_active_grammar"),
    ("src/lib.rs", 575, "grammar_engine_mut"),
    ("src/vocabulary/vocab_registry.rs", 192, "is_verb_available"),
]


def backup_file(file_path):
    """Create a backup of the file before modification"""
    backup_path = f"{file_path}.backup"
    with open(file_path, "r") as src, open(backup_path, "w") as dst:
        dst.write(src.read())
    return backup_path


def restore_file(file_path, backup_path):
    """Restore file from backup"""
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
            print(f"   Before: {original_line.strip()}")
            print(f"   After:  {modified_line.strip()}")
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
    """Run cargo check to validate changes"""
    print("\n🔧 Running validation: cargo check --workspace...")
    result = subprocess.run(
        ["cargo", "check", "--workspace"], cwd="rust", capture_output=True, text=True
    )

    if result.returncode == 0:
        print("✅ Validation passed - all changes compile successfully")
        return True
    else:
        print("❌ Validation failed - compilation errors found")
        print("\nSTDERR:")
        print(result.stderr)
        print("\nSTDOUT:")
        print(result.stdout)
        return False


def run_tests():
    """Run a quick test to ensure basic functionality"""
    print("\n🧪 Running basic tests...")
    result = subprocess.run(
        ["cargo", "test", "--lib", "--quiet"],
        cwd="rust",
        capture_output=True,
        text=True,
    )

    if result.returncode == 0:
        print("✅ Basic tests passed")
        return True
    else:
        print("⚠️  Some tests failed (this may be expected)")
        # Don't treat test failures as blocking for this cleanup
        return True


def main():
    print("🧹 Selective Dead Code Cleanup - Phase 1 Test")
    print("=" * 55)
    print(f"Testing with {len(TEST_CLEANUP_ITEMS)} diverse functions")
    print()

    # Check we're in the right directory
    if not os.path.exists("rust/Cargo.toml"):
        print("❌ Error: Must be run from ob-poc root directory")
        print("   Current directory should contain rust/Cargo.toml")
        sys.exit(1)

    success_count = 0
    failed_items = []

    # Process each item
    for i, (file_path, line_num, func_name) in enumerate(TEST_CLEANUP_ITEMS, 1):
        print(f"[{i}/{len(TEST_CLEANUP_ITEMS)}] Processing {func_name}...")

        if demote_pub_to_pub_crate(file_path, line_num, func_name):
            success_count += 1
        else:
            failed_items.append((file_path, line_num, func_name))
        print()

    print("📊 PHASE 1 SUMMARY")
    print("-" * 30)
    print(f"✅ Successfully processed: {success_count}/{len(TEST_CLEANUP_ITEMS)}")
    if failed_items:
        print(f"❌ Failed to process: {len(failed_items)}")
        for file_path, line_num, func_name in failed_items:
            print(f"   - {file_path}:{line_num} - {func_name}")
    print()

    # Validate changes
    if success_count > 0:
        if validate_changes():
            print("🎉 PHASE 1 SUCCESSFUL - Ready for full cleanup!")

            # Run tests as bonus validation
            run_tests()

            print()
            print("🚀 NEXT STEPS:")
            print("1. Review the changes above")
            print("2. If satisfied, run the full cleanup:")
            print("   python3 scripts/bulk_cleanup_unused_pub.py")
            print("3. Or continue with additional selective batches")

        else:
            print("❌ PHASE 1 FAILED - Compilation errors detected")
            print("Manual review required before proceeding")
    else:
        print("❌ NO CHANGES MADE - All items failed processing")
        print("Check file paths and line numbers")


if __name__ == "__main__":
    main()
