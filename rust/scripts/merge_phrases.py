#!/usr/bin/env python3
"""
Merge invocation phrases from draft/extension YAML files into domain YAML files.

Draft/Extension structure:
  domain_key:
    verb_key:
      invocation_phrases:
        - "phrase 1"
        - "phrase 2"

Domain YAML structure:
  domains:
    domain_key:
      verbs:
        verb_key:
          invocation_phrases:
            - "existing phrase"
"""

import yaml
import os
import re
import sys

VERBS_DIR = os.path.join(os.path.dirname(__file__), "..", "config", "verbs")

# Maps domain keys from draft/extension files to their YAML file paths
DOMAIN_FILE_MAP = {
    "view": "view.yaml",
    "ownership": "ownership.yaml",
    "fund": "fund.yaml",
    "ubo": "ubo.yaml",
    "trading-profile": "trading-profile.yaml",
    "entity": "entity.yaml",
    "control": "control.yaml",
    "gleif": "gleif.yaml",
    "cbu": "cbu.yaml",
    "graph": "graph.yaml",
    "bods": "bods.yaml",
    "session": "session.yaml",
    "cbu-role-v2": "cbu-role-v2.yaml",
    "client-group": "client-group.yaml",
    "kyc-case": "kyc/kyc-case.yaml",
}


def load_phrase_file(path):
    """Load a phrase draft/extension file."""
    with open(path) as f:
        return yaml.safe_load(f) or {}


def load_domain_yaml(path):
    """Load the raw text of a domain YAML file."""
    with open(path) as f:
        return f.read()


def get_existing_phrases(yaml_text, domain_key, verb_key):
    """Parse YAML and extract existing invocation_phrases for a verb."""
    data = yaml.safe_load(yaml_text)
    if not data or "domains" not in data:
        return []
    domain = data.get("domains", {}).get(domain_key, {})
    verbs = domain.get("verbs", {})
    verb = verbs.get(verb_key, {})
    return verb.get("invocation_phrases", []) or []


def find_verb_block_end(lines, verb_key, domain_key):
    """
    Find the line range for a verb definition block within a domain YAML.
    Returns (verb_start_line, verb_indent, next_verb_or_section_line).
    """
    # Find the verb key line
    verb_pattern = re.compile(r"^(\s+)" + re.escape(verb_key) + r":\s*$")
    in_verbs_section = False
    verbs_indent = None

    for i, line in enumerate(lines):
        # Find "verbs:" section
        if re.match(r"^\s+verbs:\s*$", line):
            in_verbs_section = True
            verbs_indent = len(line) - len(line.lstrip())
            continue

        if in_verbs_section:
            m = verb_pattern.match(line)
            if m:
                verb_indent = len(m.group(1))
                verb_start = i

                # Find where this verb block ends (next verb at same indent or less)
                for j in range(i + 1, len(lines)):
                    stripped = lines[j].rstrip()
                    if not stripped or stripped.lstrip().startswith("#"):
                        continue
                    line_indent = len(stripped) - len(stripped.lstrip())
                    if line_indent <= verb_indent:
                        return verb_start, verb_indent, j
                return verb_start, verb_indent, len(lines)

    return None, None, None


def find_invocation_phrases_block(lines, verb_start, verb_end, verb_indent):
    """
    Find existing invocation_phrases block within a verb definition.
    Returns (phrases_start, phrases_end) or (None, None) if not found.
    """
    phrases_pattern = re.compile(
        r"^" + " " * (verb_indent + 2) + r"invocation_phrases:\s*$"
    )

    for i in range(verb_start, verb_end):
        if phrases_pattern.match(lines[i]):
            phrases_start = i
            # Find end of phrase list
            for j in range(i + 1, verb_end):
                stripped = lines[j].rstrip()
                if not stripped:
                    continue
                line_indent = len(stripped) - len(stripped.lstrip())
                if line_indent <= verb_indent + 2 and not stripped.lstrip().startswith(
                    "- "
                ):
                    return phrases_start, j
                if line_indent <= verb_indent + 2 and stripped.lstrip().startswith(
                    "- "
                ):
                    # Still in the list
                    continue
            return phrases_start, verb_end
    return None, None


def merge_phrases_into_file(
    yaml_path, domain_key, verb_phrases, dry_run=False, verbose=False
):
    """
    Merge phrases into a domain YAML file.
    verb_phrases: dict of {verb_key: [new_phrases]}
    """
    with open(yaml_path) as f:
        content = f.read()

    lines = content.split("\n")
    existing_data = yaml.safe_load(content)
    if not existing_data or "domains" not in existing_data:
        print(f"  WARNING: {yaml_path} has no 'domains' key, skipping")
        return 0

    domain = existing_data.get("domains", {}).get(domain_key, {})
    verbs_section = domain.get("verbs", {})

    total_added = 0
    modifications = []  # (verb_key, new_phrases_to_add)

    for verb_key, new_phrases_data in sorted(verb_phrases.items()):
        new_phrases = new_phrases_data.get("invocation_phrases", [])
        if not new_phrases:
            continue

        # Check if verb exists in the domain file
        if verb_key not in verbs_section:
            if verbose:
                print(
                    f"  SKIP: {domain_key}.{verb_key} not found in {yaml_path}"
                )
            continue

        # Get existing phrases and dedupe
        existing = set(
            p.lower().strip()
            for p in (
                verbs_section[verb_key].get("invocation_phrases", []) or []
            )
        )
        to_add = [
            p
            for p in new_phrases
            if p.lower().strip() not in existing
        ]

        if to_add:
            modifications.append((verb_key, to_add))
            total_added += len(to_add)
            if verbose:
                print(
                    f"  {domain_key}.{verb_key}: +{len(to_add)} phrases ({len(existing)} existing)"
                )

    if not modifications:
        return 0

    if dry_run:
        return total_added

    # Apply modifications using YAML round-trip to preserve structure
    # Re-parse and modify the data, then write back
    # We use a simpler approach: load, modify in-memory, dump with careful formatting

    for verb_key, to_add in modifications:
        existing_list = verbs_section[verb_key].get("invocation_phrases", []) or []
        existing_list.extend(to_add)
        verbs_section[verb_key]["invocation_phrases"] = existing_list

    # Write back using line-based approach for better formatting preservation
    # Strategy: for each modified verb, find and replace its invocation_phrases block
    result_lines = list(lines)

    # Process modifications in reverse order (so line numbers don't shift)
    for verb_key, to_add in reversed(modifications):
        verb_start, verb_indent, verb_end = find_verb_block_end(
            result_lines, verb_key, domain_key
        )
        if verb_start is None:
            # Fallback: just note it
            print(f"  WARNING: Could not locate {verb_key} block in {yaml_path}")
            continue

        phrases_start, phrases_end = find_invocation_phrases_block(
            result_lines, verb_start, verb_end, verb_indent
        )

        phrase_indent = " " * (verb_indent + 4)

        if phrases_start is not None:
            # Append to existing phrases block (before phrases_end)
            insert_pos = phrases_end
            # Find actual last phrase line
            for k in range(phrases_end - 1, phrases_start, -1):
                if result_lines[k].strip().startswith("- "):
                    insert_pos = k + 1
                    break

            new_lines = [f'{phrase_indent}- "{p}"' for p in to_add]
            for idx, line in enumerate(new_lines):
                result_lines.insert(insert_pos + idx, line)
        else:
            # No invocation_phrases block exists, insert one after verb description
            # Find where to insert (after description line, or after verb_key line)
            insert_after = verb_start
            desc_indent = " " * (verb_indent + 2)

            # Look for description line
            for k in range(verb_start + 1, verb_end):
                stripped = result_lines[k].strip()
                if stripped.startswith("description:"):
                    insert_after = k
                    break

            new_lines = [f"{desc_indent}invocation_phrases:"]
            new_lines.extend([f'{phrase_indent}- "{p}"' for p in to_add])

            for idx, line in enumerate(new_lines):
                result_lines.insert(insert_after + 1 + idx, line)

    with open(yaml_path, "w") as f:
        f.write("\n".join(result_lines))

    return total_added


def main():
    dry_run = "--dry-run" in sys.argv
    verbose = "--verbose" in sys.argv or "-v" in sys.argv

    draft_path = os.path.join(VERBS_DIR, "_invocation_phrases_draft.yaml")
    ext_path = os.path.join(VERBS_DIR, "_invocation_phrases_extension.yaml")

    # Load phrase sources
    draft = load_phrase_file(draft_path)
    extension = load_phrase_file(ext_path)

    # Merge both sources (extension can overlap with draft)
    all_sources = {}
    for domain, verbs in draft.items():
        all_sources.setdefault(domain, {}).update(verbs)
    for domain, verbs in extension.items():
        for verb_key, verb_data in verbs.items():
            if domain in all_sources and verb_key in all_sources[domain]:
                # Merge phrase lists
                existing = all_sources[domain][verb_key].get(
                    "invocation_phrases", []
                )
                new = verb_data.get("invocation_phrases", [])
                merged = list(existing)
                seen = set(p.lower().strip() for p in existing)
                for p in new:
                    if p.lower().strip() not in seen:
                        merged.append(p)
                        seen.add(p.lower().strip())
                all_sources[domain][verb_key] = {"invocation_phrases": merged}
            else:
                all_sources.setdefault(domain, {})[verb_key] = verb_data

    if dry_run:
        print("=== DRY RUN (no files modified) ===\n")

    grand_total = 0
    for domain_key in sorted(all_sources.keys()):
        file_name = DOMAIN_FILE_MAP.get(domain_key)
        if not file_name:
            print(f"WARNING: No file mapping for domain '{domain_key}', skipping")
            continue

        yaml_path = os.path.join(VERBS_DIR, file_name)
        if not os.path.exists(yaml_path):
            print(f"WARNING: {yaml_path} does not exist, skipping")
            continue

        verb_phrases = all_sources[domain_key]
        added = merge_phrases_into_file(
            yaml_path, domain_key, verb_phrases, dry_run=dry_run, verbose=verbose
        )
        if added > 0:
            print(f"  {domain_key}: +{added} phrases merged into {file_name}")
            grand_total += added

    print(f"\nTotal: +{grand_total} phrases {'would be ' if dry_run else ''}merged")

    if not dry_run and grand_total > 0:
        print("\nDraft files can now be deleted:")
        print(f"  rm {draft_path}")
        print(f"  rm {ext_path}")


if __name__ == "__main__":
    os.chdir(VERBS_DIR)
    main()
