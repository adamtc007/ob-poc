#!/usr/bin/env python3
import datetime
import json
import os
import re
import sys
from collections import defaultdict

OUT = "target/housekeeping"
os.makedirs(OUT, exist_ok=True)


def read(path):
    try:
        with open(path, "r", encoding="utf-8", errors="ignore") as f:
            return f.read()
    except FileNotFoundError:
        return ""


def load_json(path):
    try:
        with open(path, "r", encoding="utf-8") as f:
            return json.load(f)
    except Exception:
        return None


def parse_udeps(path):
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
    txt = read(path)
    unused = []
    for line in txt.splitlines():
        m = re.search(r"([\\w\\-]+):\\s+unused dependency\\s+'([^']+)'", line)
        if m:
            unused.append({"crate": m.group(1), "dep": m.group(2)})
    return unused


def parse_warnalyzer(path):
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
        if re.match(r"^\d+\s+pub\s+(fn|struct|enum|mod|type|const|static)", line):
            # Extract function/item name
            parts = line.split()
            if len(parts) >= 3:
                item_type = parts[1]  # pub
                item_kind = parts[2]  # fn, struct, etc
                if len(parts) > 3:
                    item_name = (
                        parts[3].split("(")[0].split("<")[0]
                    )  # Remove generics and params
                    full_name = (
                        f"{current_file}::{item_name}" if current_file else item_name
                    )
                    items.append(
                        {
                            "symbol": full_name,
                            "raw": line.strip(),
                            "file": current_file,
                            "kind": item_kind,
                            "line_num": parts[0],
                        }
                    )
    return items


def parse_lcov(path):
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


def rank(unused_pub, zero_cov_files):
    scores = defaultdict(int)
    for item in unused_pub:
        sym = item["symbol"]
        scores[sym] += 2  # Unused public API

        # Bonus points for certain patterns
        if item.get("kind") == "fn":
            scores[sym] += 1  # Functions are more likely to be truly unused
        if "test" in item.get("file", "").lower():
            scores[sym] += 1  # Test-related items

    for f in zero_cov_files:
        scores[f] += 1  # Zero coverage

    ranked = sorted(scores.items(), key=lambda kv: (-kv[1], kv[0]))
    return ranked


def main():
    machete = parse_machete(f"{OUT}/machete.txt")
    udeps = parse_udeps(f"{OUT}/udeps.json")
    warn = parse_warnalyzer(f"{OUT}/warnalyzer.txt")
    files, zero_cov = parse_lcov(f"{OUT}/lcov.info")

    ranked = rank(warn, zero_cov)

    ts = datetime.datetime.utcnow().isoformat() + "Z"
    report_path = f"{OUT}/housekeeping_report.md"
    with open(report_path, "w", encoding="utf-8") as w:
        w.write(f"# Housekeeping Report\\n\\nGenerated: {ts}\\n\\n")
        w.write("## Summary\\n")
        w.write(f"- Unused deps (machete fast): **{len(machete)}**\\n")
        w.write(f"- Unused deps (udeps precise): **{len(udeps)}**\\n")
        w.write(f"- Unused public items (workspace): **{len(warn)}**\\n")
        w.write(f"- Zero-coverage files: **{len(zero_cov)}**\\n\\n")

        w.write("## Action Buckets\\n")
        w.write("### Delete / Demote Candidates (ranked)\n")
        if ranked:
            w.write(
                "| Item | Score | Recommendation | Evidence | Location |\n|---|---:|---|---|---|\n"
            )
            for item, score in ranked[:200]:
                # Find the item details
                item_details = next((x for x in warn if x.get("symbol") == item), {})
                item_kind = item_details.get("kind", "unknown")
                item_file = item_details.get("file", "")
                item_line = item_details.get("line_num", "")

                # Recommendation based on score and type
                if score >= 3:
                    rec = "🔴 **Delete** (high confidence)"
                elif score >= 2:
                    rec = "🟡 Demote to pub(crate)"
                else:
                    rec = "🟢 Investigate"

                # Evidence
                evs = []
                if any(item in (x.get("symbol", "")) for x in warn):
                    evs.append(f"unused {item_kind}")
                if item in zero_cov:
                    evs.append("zero coverage")
                ev = ", ".join(evs) if evs else "-"

                # Location info
                location = (
                    f"{item_file}:{item_line}"
                    if item_file and item_line
                    else item_file or "-"
                )

                w.write(f"| `{item}` | {score} | {rec} | {ev} | {location} |\n")
        else:
            w.write("_No ranked items. Run sweep first._\n")

        w.write("\\n### Unused Dependencies (precise: cargo-udeps)\\n")
        if udeps:
            w.write("| Crate | Dependency |\\n|---|---|\\n")
            for u in udeps:
                w.write(f"| `{u['crate']}` | `{u['dep']}` |\\n")
        else:
            w.write("_None detected or udeps missing._\\n")

        w.write("\\n### Unused Dependencies (fast: cargo-machete)\\n")
        if machete:
            w.write("| Crate | Dependency |\\n|---|---|\\n")
            for u in machete:
                w.write(f"| `{u['crate']}` | `{u['dep']}` |\\n")
        else:
            w.write("_None detected or machete missing._\\n")

        w.write("\n### Unused Public Functions by File\n")
        if warn:
            file_groups = defaultdict(list)
            for item in warn:
                file_groups[item.get("file", "unknown")].append(item)

            for file_path, items in sorted(file_groups.items()):
                w.write(f"\n#### {file_path}\n")
                for item in items:
                    kind = item.get("kind", "fn")
                    symbol = item.get("symbol", "").split("::")[
                        -1
                    ]  # Just the function name
                    line_num = item.get("line_num", "")
                    w.write(f"- **{symbol}** ({kind}) - Line {line_num}\n")

        w.write("\n### Zero-Coverage Files\n")
        if zero_cov:
            for f in zero_cov[:300]:
                w.write(f"- `{f}`\n")
        else:
            w.write("_None detected or coverage missing._\n")

        w.write("\n## Raw Outputs\n")
        w.write(f"- `machete.txt`: {OUT}/machete.txt\n")
        w.write(f"- `udeps.json`: {OUT}/udeps.json\n")
        w.write(
            f"- `warnalyzer.txt`: {OUT}/warnalyzer.txt (cargo-workspace-unused-pub format)\n"
        )
        w.write(f"- `lcov.info`: {OUT}/lcov.info\n")
        w.write("\\n---\\n")
        w.write("### Next Steps\\n")
        w.write(
            "1. For **unused pub**: shrink visibility (`pub(crate)`), re-run sweep; if still unused, delete.\\n"
        )
        w.write(
            "2. For **deps** where machete & udeps agree: remove in `Cargo.toml`, run `cargo clippy --fix`, test.\\n"
        )
        w.write(
            "3. For **zero-coverage** modules: confirm with callgraph; if unreachable & unreferenced, delete or move to benches/examples.\\n"
        )
        w.write("4. Add this workflow to CI to keep the codebase clean.\\n")

    print(f"Wrote {report_path}")
