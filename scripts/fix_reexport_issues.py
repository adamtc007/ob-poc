#!/usr/bin/env python3
"""
Quick Fix for Re-export Visibility Issues

This script fixes the visibility issues that arise when items are made pub(crate)
but are still being re-exported in pub use statements. It identifies the specific
items that need to remain public and reverts them back to pub.
"""

import re
import subprocess
from pathlib import Path

# Items that are being re-exported and need to stay public
REEXPORT_ITEMS = {
    "src/dsl_manager/core.rs": [
        "CanonicalDslResponse",
        "ComprehensiveHealthStatus",
        "DslInstanceSummary",
        "DslManager",
        "DslManagerConfig",
        "ExecutionDetails",
        "HealthMetrics",
        "AgenticCrudRequest",
        "AiOnboardingRequest",
        "AiOnboardingResponse",
        "AiValidationResult",
        "CbuGenerator",
    ],
    "src/dsl_manager/backend.rs": [
        "BackendOperation",
        "BackendResult",
        "DslBackend",
    ],
    "src/dsl_manager/compiler.rs": [
        "CompilationResult",
        "DslCompiler",
        "ExecutionContext",
    ],
    "src/dsl_manager/pipeline.rs": [
        "DslPipeline",
        "DslPipelineStage",
        "PipelineResult",
    ],
    "src/dsl_manager/state.rs": [
        "DslState",
        "DslStateManager",
        "StateChangeEvent",
    ],
    "src/dsl_manager/validation.rs": [
        "DslValidationEngine",
        "ValidationLevel",
        "ValidationReport",
    ],
    "src/parser/idiomatic_parser.rs": [
        "parse_verb_form",
        "parse_form",
        "parse_identifier",
        "parse_string_literal",
        "parse_value",
    ],
}


def fix_visibility_in_file(file_path, items):
    """Fix visibility for specific items in a file"""
    rust_file = Path("rust") / file_path
    if not rust_file.exists():
        print(f"⚠️  File not found: {rust_file}")
        return 0

    with open(rust_file, "r") as f:
        content = f.read()

    original_content = content
    changes_made = 0

    for item in items:
        # Pattern to match pub(crate) declarations for this item
        patterns = [
            rf"pub\(crate\) fn {item}",
            rf"pub\(crate\) struct {item}",
            rf"pub\(crate\) enum {item}",
            rf"pub\(crate\) trait {item}",
            rf"pub\(crate\) type {item}",
            rf"pub\(crate\) const {item}",
            rf"pub\(crate\) mod {item}",
        ]

        for pattern in patterns:
            replacement = pattern.replace("pub(crate)", "pub")
            if re.search(pattern, content):
                content = re.sub(pattern, replacement, content)
                changes_made += 1
                print(f"  ✅ Fixed visibility for {item}")
                break

    if changes_made > 0:
        with open(rust_file, "w") as f:
            f.write(content)
        print(f"✅ Updated {file_path}: {changes_made} changes")
    else:
        print(f"ℹ️  No changes needed in {file_path}")

    return changes_made


def check_compilation():
    """Check if the code compiles after fixes"""
    print("\n🔧 Checking compilation...")
    result = subprocess.run(
        ["cargo", "+1.91", "check", "--workspace", "--lib"],
        cwd="rust",
        capture_output=True,
        text=True,
    )

    if result.returncode == 0:
        print("✅ Compilation successful!")
        return True
    else:
        print("❌ Compilation failed:")
        # Show only the first few errors
        errors = result.stderr.split("\n")[:10]
        for error in errors:
            if error.strip():
                print(f"   {error}")
        return False


def main():
    print("🔧 Fixing Re-export Visibility Issues")
    print("=" * 40)

    total_changes = 0

    for file_path, items in REEXPORT_ITEMS.items():
        print(f"\n📄 Processing {file_path}...")
        changes = fix_visibility_in_file(file_path, items)
        total_changes += changes

    print(f"\n📊 Total changes made: {total_changes}")

    if total_changes > 0:
        if check_compilation():
            print("\n🎉 All re-export issues fixed successfully!")
            print(
                "The public API surface reduction preserved necessary public interfaces."
            )
        else:
            print("\n⚠️  Some compilation issues remain - manual review needed")
    else:
        print("\n ℹ️  No changes were needed")


if __name__ == "__main__":
    main()
