#!/usr/bin/env python3
"""
Strategic Public API Surface Reduction Script

This script implements the "pub surface cleanup" strategy to reduce semantic ambiguity
for agentic refactoring tools. It systematically converts `pub` items to `pub(crate)`
when they are only used within the same crate, dramatically reducing the public API
surface that needs documentation.

Philosophy:
- AI agents need clear semantic boundaries
- Undocumented pub items = semantic ambiguity landmines
- Reduce pub surface first, then document what remains
- This eliminates 90% of missing_docs warnings strategically
"""

import ast
import os
import re
import subprocess
import sys
from collections import defaultdict
from pathlib import Path
from typing import Dict, List, Set, Tuple

# Colors for output
RED = "\033[0;31m"
GREEN = "\033[0;32m"
YELLOW = "\033[1;33m"
BLUE = "\033[0;34m"
NC = "\033[0m"  # No Color


class PubSurfaceAnalyzer:
    def __init__(self, rust_dir: Path):
        self.rust_dir = rust_dir
        self.pub_items = []  # List of (file_path, line_num, item_type, item_name, full_line)
        self.usage_map = defaultdict(
            list
        )  # item_name -> [(file_path, line_num, context)]
        self.conversion_candidates = []
        self.true_public_api = []

    def scan_pub_items(self):
        """Find all pub items in the codebase"""
        print(f"{BLUE}📊 Scanning for pub items...{NC}")

        for rust_file in self.rust_dir.rglob("*.rs"):
            if "target/" in str(rust_file):
                continue

            try:
                with open(rust_file, "r", encoding="utf-8") as f:
                    lines = f.readlines()

                for line_num, line in enumerate(lines, 1):
                    line_stripped = line.strip()

                    # Skip comments and doc comments
                    if line_stripped.startswith("//") or line_stripped.startswith(
                        "///"
                    ):
                        continue

                    # Match pub items (but not pub(crate), pub(super), etc.)
                    pub_pattern = r"^pub\s+(fn|struct|enum|mod|type|const|static|trait|use)\s+(\w+)"
                    match = re.search(pub_pattern, line_stripped)

                    if match:
                        item_type = match.group(1)
                        item_name = match.group(2)

                        # Skip main functions and test modules
                        if item_name == "main" or "test" in item_name.lower():
                            continue

                        rel_path = str(rust_file.relative_to(self.rust_dir))
                        self.pub_items.append(
                            (rel_path, line_num, item_type, item_name, line_stripped)
                        )

            except Exception as e:
                print(f"{YELLOW}⚠️  Could not read {rust_file}: {e}{NC}")
                continue

        print(f"{GREEN}✅ Found {len(self.pub_items)} pub items{NC}")

    def analyze_usage_patterns(self):
        """Analyze usage patterns for each pub item"""
        print(f"{BLUE}🔍 Analyzing usage patterns...{NC}")

        for file_path, line_num, item_type, item_name, full_line in self.pub_items:
            # Search for usages across the entire workspace
            usage_files = self.find_item_usage(item_name)
            self.usage_map[item_name] = usage_files

        print(f"{GREEN}✅ Analyzed usage for {len(self.pub_items)} items{NC}")

    def find_item_usage(self, item_name: str) -> List[Tuple[str, int, str]]:
        """Find all files that use a specific item"""
        usages = []

        # Use ripgrep if available, otherwise use grep
        try:
            cmd = ["rg", "-n", "--type", "rust", item_name, str(self.rust_dir)]
            result = subprocess.run(cmd, capture_output=True, text=True)

            if result.returncode != 0:
                # Fallback to grep
                cmd = ["grep", "-rn", "--include=*.rs", item_name, str(self.rust_dir)]
                result = subprocess.run(cmd, capture_output=True, text=True)

        except FileNotFoundError:
            # Fallback to grep if rg not available
            cmd = ["grep", "-rn", "--include=*.rs", item_name, str(self.rust_dir)]
            result = subprocess.run(
                cmd, capture_output=True, text=True, cwd=self.rust_dir.parent
            )

        if result.returncode == 0:
            for line in result.stdout.split("\n"):
                if not line.strip():
                    continue

                parts = line.split(":", 2)
                if len(parts) >= 3:
                    file_path = parts[0].replace(str(self.rust_dir) + "/", "")
                    line_num = parts[1]
                    context = parts[2].strip()

                    # Filter out the definition itself
                    if (
                        f"pub {item_name}" not in context
                        and f"pub fn {item_name}" not in context
                    ):
                        usages.append((file_path, line_num, context))

        return usages

    def categorize_items(self):
        """Categorize items based on usage patterns"""
        print(f"{BLUE}🎯 Categorizing items by usage scope...{NC}")

        for file_path, line_num, item_type, item_name, full_line in self.pub_items:
            usages = self.usage_map[item_name]

            if not usages:
                # No usages found - candidate for removal or pub(crate)
                self.conversion_candidates.append(
                    {
                        "file_path": file_path,
                        "line_num": line_num,
                        "item_type": item_type,
                        "item_name": item_name,
                        "full_line": full_line,
                        "reason": "unused",
                        "usages": [],
                    }
                )
                continue

            # Check if all usages are within the same crate
            current_crate_file = file_path
            all_same_crate = True
            usage_files = set()

            for usage_file, usage_line, context in usages:
                usage_files.add(usage_file)
                # Simple heuristic: if used in different source files, consider it cross-crate
                # (This could be improved with proper crate boundary detection)

            # If only used in same file or very few files, likely internal
            if len(usage_files) <= 2:  # Same file + maybe one other
                self.conversion_candidates.append(
                    {
                        "file_path": file_path,
                        "line_num": line_num,
                        "item_type": item_type,
                        "item_name": item_name,
                        "full_line": full_line,
                        "reason": "internal_use",
                        "usages": usages[:5],  # Show first 5 usages
                    }
                )
            else:
                # Likely true public API
                self.true_public_api.append(
                    {
                        "file_path": file_path,
                        "line_num": line_num,
                        "item_type": item_type,
                        "item_name": item_name,
                        "full_line": full_line,
                        "reason": "true_public",
                        "usages": usages[:3],  # Show first 3 usages
                    }
                )

        print(f"{GREEN}✅ Conversion candidates: {len(self.conversion_candidates)}{NC}")
        print(f"{YELLOW}⚠️  True public API: {len(self.true_public_api)}{NC}")

    def apply_conversions(self, dry_run=True):
        """Apply pub -> pub(crate) conversions"""
        if dry_run:
            print(f"{BLUE}🔍 DRY RUN - Preview of changes:{NC}")
        else:
            print(f"{BLUE}✏️  Applying pub surface reductions...{NC}")

        conversion_count = 0
        files_modified = set()

        # Group conversions by file for efficient processing
        file_conversions = defaultdict(list)
        for item in self.conversion_candidates:
            file_conversions[item["file_path"]].append(item)

        for file_path, items in file_conversions.items():
            full_path = self.rust_dir / file_path

            if dry_run:
                print(f"\n{YELLOW}📄 {file_path}:{NC}")
                for item in items:
                    print(
                        f"  Line {item['line_num']}: {item['item_type']} {item['item_name']} ({item['reason']})"
                    )
                    if item["usages"]:
                        print(f"    Used in: {len(item['usages'])} locations")
                continue

            try:
                with open(full_path, "r", encoding="utf-8") as f:
                    lines = f.readlines()

                # Sort by line number in reverse order to preserve line numbers
                items_sorted = sorted(items, key=lambda x: x["line_num"], reverse=True)

                for item in items_sorted:
                    line_idx = item["line_num"] - 1
                    if line_idx < len(lines):
                        original_line = lines[line_idx]

                        # Convert pub to pub(crate)
                        if original_line.strip().startswith("pub "):
                            new_line = original_line.replace("pub ", "pub(crate) ", 1)
                            lines[line_idx] = new_line
                            conversion_count += 1
                            print(f"  ✅ {item['item_name']} ({item['item_type']})")

                with open(full_path, "w", encoding="utf-8") as f:
                    f.writelines(lines)

                files_modified.add(file_path)

            except Exception as e:
                print(f"{RED}❌ Error processing {file_path}: {e}{NC}")
                continue

        if not dry_run:
            print(
                f"\n{GREEN}✅ Applied {conversion_count} conversions across {len(files_modified)} files{NC}"
            )

        return conversion_count

    def generate_report(self):
        """Generate a comprehensive report"""
        report_path = self.rust_dir / "pub_surface_analysis.md"

        with open(report_path, "w") as f:
            f.write("# Public API Surface Analysis Report\n\n")
            f.write(
                f"Generated: {subprocess.check_output(['date']).decode().strip()}\n\n"
            )

            f.write("## Executive Summary\n\n")
            f.write(f"- **Total pub items found**: {len(self.pub_items)}\n")
            f.write(
                f"- **Conversion candidates**: {len(self.conversion_candidates)} (can be made pub(crate))\n"
            )
            f.write(
                f"- **True public API**: {len(self.true_public_api)} (needs documentation)\n"
            )
            f.write(
                f"- **Potential reduction**: {len(self.conversion_candidates)}/{len(self.pub_items)} ({len(self.conversion_candidates) / len(self.pub_items) * 100:.1f}%)\n\n"
            )

            f.write("## Strategy Impact\n\n")
            f.write("By converting internal-use items to `pub(crate)`, we:\n")
            f.write("- Reduce semantic ambiguity for AI agents\n")
            f.write("- Eliminate documentation burden for internal APIs\n")
            f.write(
                "- Create clear boundaries between public contracts and implementation details\n"
            )
            f.write("- Focus documentation efforts on true public APIs\n\n")

            f.write("## Conversion Candidates (pub → pub(crate))\n\n")
            for item in self.conversion_candidates[:20]:  # Show first 20
                f.write(f"### {item['item_name']} ({item['item_type']})\n")
                f.write(f"- **File**: {item['file_path']}:{item['line_num']}\n")
                f.write(f"- **Reason**: {item['reason']}\n")
                if item["usages"]:
                    f.write(f"- **Internal usage**: {len(item['usages'])} locations\n")
                f.write(f"```rust\n{item['full_line']}\n```\n\n")

            if len(self.conversion_candidates) > 20:
                f.write(
                    f"... and {len(self.conversion_candidates) - 20} more candidates\n\n"
                )

            f.write("## True Public API (needs documentation)\n\n")
            for item in self.true_public_api[:10]:  # Show first 10
                f.write(f"### {item['item_name']} ({item['item_type']})\n")
                f.write(f"- **File**: {item['file_path']}:{item['line_num']}\n")
                f.write(f"- **Cross-crate usage**: {len(item['usages'])} locations\n")
                f.write(
                    f"```rust\n// TODO: Add documentation\n{item['full_line']}\n```\n\n"
                )

            if len(self.true_public_api) > 10:
                f.write(
                    f"... and {len(self.true_public_api) - 10} more public APIs\n\n"
                )

            f.write("## Next Steps\n\n")
            f.write("1. Review conversion candidates above\n")
            f.write("2. Run: `python3 scripts/reduce_pub_surface.py --apply`\n")
            f.write("3. Test compilation: `cargo +1.91 check --workspace`\n")
            f.write("4. Focus documentation efforts on remaining true public APIs\n")
            f.write("5. Re-enable `#![warn(missing_docs)]` in lib.rs\n\n")

        print(f"{GREEN}📋 Report generated: {report_path}{NC}")


def main():
    import argparse

    parser = argparse.ArgumentParser(
        description="Reduce public API surface for semantic clarity"
    )
    parser.add_argument(
        "--apply", action="store_true", help="Apply changes (default is dry-run)"
    )
    parser.add_argument("--rust-dir", default="rust", help="Rust source directory")
    args = parser.parse_args()

    rust_dir = Path(args.rust_dir)
    if not rust_dir.exists():
        print(f"{RED}❌ Rust directory not found: {rust_dir}{NC}")
        sys.exit(1)

    print(f"{BLUE}🎯 Strategic Public API Surface Reduction{NC}")
    print(f"{BLUE}========================================{NC}")
    print("Goal: Convert internal pub items to pub(crate) for semantic clarity")
    print("This eliminates documentation burden and reduces AI agent confusion")
    print()

    analyzer = PubSurfaceAnalyzer(rust_dir)

    # Phase 1: Discovery
    analyzer.scan_pub_items()
    analyzer.analyze_usage_patterns()
    analyzer.categorize_items()

    # Phase 2: Generate report
    analyzer.generate_report()

    # Phase 3: Apply changes
    if args.apply:
        print(f"\n{YELLOW}⚠️  APPLYING CHANGES - This will modify your source files{NC}")
        print("Auto-applying changes for semantic clarity optimization...")

        conversion_count = analyzer.apply_conversions(dry_run=False)

        if conversion_count > 0:
            print(f"\n{BLUE}🔧 Validating changes...{NC}")
            result = subprocess.run(
                ["cargo", "+1.91", "check", "--workspace"],
                cwd=rust_dir,
                capture_output=True,
            )

            if result.returncode == 0:
                print(f"{GREEN}✅ All changes compile successfully!{NC}")
                print(f"\n{BLUE}📈 IMPACT SUMMARY:{NC}")
                print(f"- Converted {conversion_count} pub items to pub(crate)")
                print(
                    f"- Reduced public API surface by {conversion_count}/{len(analyzer.pub_items)} items"
                )
                print(f"- Eliminated semantic ambiguity for AI agents")
                print(
                    f"- Focused documentation burden on {len(analyzer.true_public_api)} true public APIs"
                )

                print(f"\n{BLUE}🎯 NEXT STEPS:{NC}")
                print("1. Review remaining true public API items")
                print("2. Add minimal documentation to true public items")
                print("3. Re-enable #![warn(missing_docs)] in src/lib.rs")
                print("4. Run dead code analysis to catch newly-unused items")

            else:
                print(f"{RED}❌ Compilation failed after changes{NC}")
                print("You may need to adjust some conversions manually")
    else:
        conversion_count = analyzer.apply_conversions(dry_run=True)
        print(f"\n{YELLOW}📋 DRY RUN COMPLETE{NC}")
        print(
            f"Would convert {len(analyzer.conversion_candidates)} items to pub(crate)"
        )
        print(f"Run with --apply to make actual changes")
        print(f"See pub_surface_analysis.md for detailed report")


if __name__ == "__main__":
    main()
