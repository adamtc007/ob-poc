#!/usr/bin/env python3
"""
Enhanced Dead Code Report Generator
Parses cargo-workspace-unused-pub output and generates professional reports
"""

import datetime
import json
import os
import re
from collections import defaultdict

OUT = "target/housekeeping"
os.makedirs(OUT, exist_ok=True)


def read(path):
    """Read file content safely"""
    try:
        with open(path, "r", encoding="utf-8", errors="ignore") as f:
            return f.read()
    except FileNotFoundError:
        return ""


def load_json(path):
    """Load JSON file safely"""
    try:
        with open(path, "r", encoding="utf-8") as f:
            return json.load(f)
    except Exception:
        return None


def parse_udeps(path):
    """Parse cargo-udeps JSON output"""
    data = load_json(path)
    unused = []
    if isinstance(data, dict):
        for pkg in data.get("packages", []):
            for d in pkg.get("unused_deps", []):
                if isinstance(d, dict):
                    unused.append(
                        {
                            "crate": pkg.get("name"),
                            "dep": d.get("name") or d.get("package") or str(d),
                        }
                    )
                else:
                    unused.append({"crate": pkg.get("name"), "dep": str(d)})
    elif isinstance(data, list):
        for pkg in data:
            for d in pkg.get("unused_deps", []):
                if isinstance(d, dict):
                    unused.append(
                        {
                            "crate": pkg.get("name"),
                            "dep": d.get("name") or d.get("package") or str(d),
                        }
                    )
                else:
                    unused.append({"crate": pkg.get("name"), "dep": str(d)})
    else:
        # Fallback to text parsing
        txt = read(path)
        for line in txt.splitlines():
            m = re.search(r"(\S+)\s+has\s+unused\s+dependencies:\s+(.*)", line)
            if m:
                crate = m.group(1)
                deps = [x.strip() for x in m.group(2).split(",")]
                for dep in deps:
                    if dep:
                        unused.append({"crate": crate, "dep": dep})
    return unused


def parse_machete(path):
    """Parse cargo-machete text output"""
    txt = read(path)
    unused = []
    for line in txt.splitlines():
        m = re.search(r"([\\w\\-]+):\\s+unused dependency\\s+\'([^\']+)\'", line)
        if m:
            unused.append({"crate": m.group(1), "dep": m.group(2)})
    return unused


def parse_cargo_workspace_unused_pub(path):
    """Parse cargo-workspace-unused-pub output format"""
    txt = read(path)
    items = []
    current_file = ""

    for line in txt.splitlines():
        line = line.strip()

        # Skip log lines
        if line.startswith("[") and (
            "INFO" in line or "WARN" in line or "ERROR" in line
        ):
            continue

        # File path lines (src/...)
        if line.startswith("src/") and line.endswith(".rs"):
            current_file = line
            continue

        # Function/method lines with line numbers and pub declarations
        # Format: "589      pub fn extract_document_metadata("
        match = re.match(
            r"^(\d+)\s+pub\s+(fn|struct|enum|mod|type|const|static)\s+([^(< ]+)", line
        )
        if match:
            line_num = match.group(1)
            item_kind = match.group(2)
            item_name = match.group(3)

            # Create a meaningful symbol name
            if current_file:
                # Convert file path to module-like name
                module_path = (
                    current_file.replace("src/", "")
                    .replace(".rs", "")
                    .replace("/", "::")
                )
                full_name = f"{module_path}::{item_name}"
            else:
                full_name = item_name

            items.append(
                {
                    "symbol": full_name,
                    "raw": line.strip(),
                    "file": current_file,
                    "kind": item_kind,
                    "line_num": line_num,
                    "name": item_name,
                }
            )

    return items


def parse_lcov(path):
    """Parse LCOV coverage data"""
    txt = read(path)
    files = {}
    cur = None

    for line in txt.splitlines():
        if line.startswith("SF:"):
            cur = line[3:].strip()
            files[cur] = {"lines_total": 0, "lines_hit": 0}
        elif line.startswith("DA:") and cur:
            parts = line[3:].split(",")
            if len(parts) == 2:
                files[cur]["lines_total"] += 1
                if int(parts[1]) > 0:
                    files[cur]["lines_hit"] += 1
        elif line.startswith("end_of_record"):
            cur = None

    zero_cov = [
        f
        for f, stats in files.items()
        if stats["lines_total"] > 0 and stats["lines_hit"] == 0
    ]
    return files, zero_cov


def rank_items(unused_pub, zero_cov_files):
    """Rank items by deletion priority"""
    scores = defaultdict(int)

    for item in unused_pub:
        sym = item["symbol"]
        scores[sym] += 2  # Base score for unused public API

        # Bonus scoring
        if item.get("kind") == "fn":
            scores[sym] += 1  # Functions more likely to be truly unused
        if "test" in item.get("file", "").lower():
            scores[sym] += 1  # Test-related items
        if item.get("name", "").startswith("get_") or item.get("name", "").startswith(
            "set_"
        ):
            scores[sym] += 0.5  # Accessor methods often unused

    # Cross-reference with zero coverage files
    for f in zero_cov_files:
        # Add score to items in zero-coverage files
        for item in unused_pub:
            if f in item.get("file", ""):
                scores[item["symbol"]] += 1

    ranked = sorted(scores.items(), key=lambda kv: (-kv[1], kv[0]))
    return ranked


def generate_summary_stats(machete, udeps, unused_pub, zero_cov):
    """Generate summary statistics"""
    stats = {
        "unused_deps_machete": len(machete),
        "unused_deps_udeps": len(udeps),
        "unused_pub_items": len(unused_pub),
        "zero_coverage_files": len(zero_cov),
        "high_priority_items": 0,
        "functions": 0,
        "structs": 0,
        "other": 0,
    }

    for item in unused_pub:
        kind = item.get("kind", "fn")
        if kind == "fn":
            stats["functions"] += 1
        elif kind in ["struct", "enum"]:
            stats["structs"] += 1
        else:
            stats["other"] += 1

    return stats


def main():
    """Main report generation function"""
    print("Generating enhanced dead code report...")

    # Parse all data sources
    machete = parse_machete(f"{OUT}/machete.txt")
    udeps = parse_udeps(f"{OUT}/udeps.json")
    unused_pub = parse_cargo_workspace_unused_pub(f"{OUT}/warnalyzer.txt")
    files, zero_cov = parse_lcov(f"{OUT}/lcov.info")

    # Rank items by priority
    ranked = rank_items(unused_pub, zero_cov)

    # Generate statistics
    stats = generate_summary_stats(machete, udeps, unused_pub, zero_cov)

    # Generate timestamp
    ts = datetime.datetime.utcnow().isoformat() + "Z"

    # Write the report
    report_path = f"{OUT}/housekeeping_report.md"
    with open(report_path, "w", encoding="utf-8") as w:
        w.write(f"# 🧹 Dead Code Analysis Report\n\n")
        w.write(f"**Generated**: {ts}\n")
        w.write(f"**Tool**: cargo-workspace-unused-pub + enhanced analysis\n\n")

        # Executive Summary
        w.write("## 📊 Executive Summary\n\n")
        w.write(
            f"- **Unused Dependencies**: {stats['unused_deps_machete']} (machete) / {stats['unused_deps_udeps']} (udeps)\n"
        )
        w.write(f"- **Unused Public Items**: {stats['unused_pub_items']} total\n")
        w.write(f"  - Functions: {stats['functions']}\n")
        w.write(f"  - Structs/Enums: {stats['structs']}\n")
        w.write(f"  - Other: {stats['other']}\n")
        w.write(f"- **Zero-Coverage Files**: {stats['zero_coverage_files']}\n\n")

        # Priority Actions
        w.write("## 🎯 Priority Actions (Ranked by Evidence)\n\n")
        if ranked:
            w.write("| Item | Score | Action | Evidence | Location |\n")
            w.write("|------|-------|--------|----------|----------|\n")

            for item_symbol, score in ranked[:50]:  # Top 50
                # Find the item details
                item_details = next(
                    (x for x in unused_pub if x.get("symbol") == item_symbol), {}
                )
                item_kind = item_details.get("kind", "unknown")
                item_file = item_details.get("file", "")
                item_line = item_details.get("line_num", "")
                item_name = item_details.get("name", item_symbol.split("::")[-1])

                # Action recommendation
                if score >= 4:
                    action = "🔴 **DELETE** (high confidence)"
                elif score >= 3:
                    action = "🟡 **Demote to pub(crate)**"
                elif score >= 2:
                    action = "🟢 Investigate further"
                else:
                    action = "⚪ Low priority"

                # Evidence
                evidence = []
                if score >= 2:
                    evidence.append(f"unused {item_kind}")
                if any(item_file in zc for zc in zero_cov):
                    evidence.append("zero coverage")
                if "test" in item_file.lower():
                    evidence.append("test code")

                ev_str = ", ".join(evidence) if evidence else "unused public"
                location = (
                    f"`{item_file}:{item_line}`" if item_file and item_line else "-"
                )

                w.write(
                    f"| `{item_name}` | {score} | {action} | {ev_str} | {location} |\n"
                )
        else:
            w.write("_No unused public items detected._\n\n")

        # Dependencies Section
        if machete or udeps:
            w.write("## 📦 Unused Dependencies\n\n")

            if udeps:
                w.write("### Precise Analysis (cargo-udeps)\n")
                w.write("| Crate | Dependency |\n|-------|------------|\n")
                for u in udeps:
                    w.write(f"| `{u['crate']}` | `{u['dep']}` |\n")
                w.write("\n")

            if machete:
                w.write("### Fast Analysis (cargo-machete)\n")
                w.write("| Crate | Dependency |\n|-------|------------|\n")
                for u in machete:
                    w.write(f"| `{u['crate']}` | `{u['dep']}` |\n")
                w.write("\n")
        else:
            w.write("## 📦 Dependencies: ✅ Clean\n\n")
            w.write("No unused dependencies detected. Good job!\n\n")

        # Detailed Breakdown by File
        if unused_pub:
            w.write("## 📁 Unused Public Items by File\n\n")
            file_groups = defaultdict(list)
            for item in unused_pub:
                file_groups[item.get("file", "unknown")].append(item)

            for file_path, items in sorted(file_groups.items()):
                if file_path and file_path != "unknown":
                    w.write(f"### {file_path}\n")
                    for item in sorted(
                        items, key=lambda x: int(x.get("line_num", "0"))
                    ):
                        kind = item.get("kind", "fn")
                        name = item.get("name", "unknown")
                        line_num = item.get("line_num", "")
                        symbol = item.get("symbol", name)

                        # Get score for this item
                        item_score = next(
                            (score for sym, score in ranked if sym == symbol), 0
                        )
                        priority = (
                            "🔴 HIGH"
                            if item_score >= 4
                            else "🟡 MED"
                            if item_score >= 3
                            else "🟢 LOW"
                        )

                        w.write(
                            f"- **{name}** (`{kind}`) - Line {line_num} - {priority}\n"
                        )
                    w.write("\n")

        # Coverage Analysis
        if zero_cov:
            w.write("## 📈 Zero-Coverage Files\n\n")
            w.write("Files with no test coverage (potential deletion candidates):\n\n")
            for f in sorted(zero_cov):
                w.write(f"- `{f}`\n")
            w.write("\n")

        # Next Steps
        w.write("## 🚀 Recommended Actions\n\n")
        w.write("### Immediate (High Confidence)\n")
        w.write(
            "1. **Review RED items** (score ≥4) - these are prime deletion candidates\n"
        )
        w.write(
            "2. **Remove unused dependencies** confirmed by both machete and udeps\n"
        )
        w.write("3. **Run tests** after each change to ensure nothing breaks\n\n")

        w.write("### Next Phase (Medium Confidence)\n")
        w.write("1. **Demote YELLOW items** (score 3) from `pub` → `pub(crate)`\n")
        w.write(
            "2. **Re-run analysis** to see if they become unused after visibility change\n"
        )
        w.write("3. **Review zero-coverage files** for potential archival\n\n")

        w.write("### Validation Steps\n")
        w.write("```bash\n")
        w.write("# After each cleanup batch\n")
        w.write("cargo check --workspace --all-targets --all-features\n")
        w.write("cargo test --workspace\n")
        w.write("cargo build --examples\n")
        w.write("```\n\n")

        # File locations
        w.write("## 📋 Report Data Sources\n\n")
        w.write(f"- Unused deps (fast): `{OUT}/machete.txt`\n")
        w.write(f"- Unused deps (precise): `{OUT}/udeps.json`\n")
        w.write(f"- Unused public API: `{OUT}/warnalyzer.txt`\n")
        w.write(f"- Coverage data: `{OUT}/lcov.info`\n")
        w.write(f"- This report: `{OUT}/housekeeping_report.md`\n\n")

        w.write("---\n")
        w.write("*Generated by enhanced dead code analysis workflow*\n")

    print(f"✅ Enhanced report written to {report_path}")
    print(f"📊 Found {len(unused_pub)} unused public items")
    print(
        f"🎯 {len([s for _, s in ranked if s >= 4])} high-priority deletion candidates"
    )
    return report_path


if __name__ == "__main__":
    main()
