#!/usr/bin/env python3
"""
Add explicit subject_kinds: [] to verbs in specified domains
that currently rely on the domain heuristic fallback.

Only modifies verbs that don't already have subject_kinds in their metadata.
"""

import sys
import os
import re

# Domains where verbs should have subject_kinds: [] (no entity filter)
NO_ENTITY_FILTER_DOMAINS = {
    # Navigation & session management
    "session", "view",
    # Agent operations
    "agent",
    # Meta-operations
    "batch", "template", "pack", "pipeline", "graph", "semantic", "temporal",
    "coverage", "discovery",
    # Stewardship & registry
    "focus", "maintenance", "audit", "governance", "changeset", "registry", "schema",
}


def process_file(filepath):
    """Process a single YAML file and add subject_kinds: [] where needed."""
    with open(filepath, 'r') as f:
        lines = f.readlines()

    # First pass: find which domain(s) are in this file
    current_domain = None
    domain_for_this_file = set()
    for line in lines:
        stripped = line.rstrip()
        indent = len(line) - len(line.lstrip())
        # Domain declarations at various indents depending on file structure
        # Most files: domains:\n  domain_name:\n    verbs:
        # So domain name is at indent 2 under "domains:" at indent 0
        if indent == 2 and stripped.endswith(':') and not stripped.lstrip().startswith('#') and not stripped.lstrip().startswith('-'):
            dname = stripped.strip().rstrip(':')
            if dname not in ('description', 'verbs', 'version'):
                domain_for_this_file.add(dname)

    # Check if any domain in this file is in our target list
    target_domains = domain_for_this_file & NO_ENTITY_FILTER_DOMAINS
    if not target_domains:
        return 0

    # Second pass: find metadata blocks and add subject_kinds: []
    changes = 0
    new_lines = []
    i = 0
    in_target_domain = False

    while i < len(lines):
        line = lines[i]
        stripped = line.rstrip()
        indent = len(line) - len(line.lstrip()) if stripped else 0

        # Track domain context
        if indent == 2 and stripped.endswith(':') and not stripped.lstrip().startswith('#') and not stripped.lstrip().startswith('-'):
            dname = stripped.strip().rstrip(':')
            if dname in target_domains:
                in_target_domain = True
            elif dname not in ('description', 'verbs', 'version'):
                in_target_domain = False

        # Look for metadata: at verb field level (indent 8)
        if in_target_domain and indent == 8 and stripped.strip() == 'metadata:':
            new_lines.append(line)
            i += 1

            # Collect all metadata fields
            has_subject_kinds = False
            metadata_lines = []

            while i < len(lines):
                mline = lines[i]
                mstripped = mline.rstrip()
                if mstripped == '':
                    # Empty line could be inside or between blocks
                    metadata_lines.append(mline)
                    i += 1
                    continue
                mindent = len(mline) - len(mline.lstrip())
                if mindent > 8:
                    # Still inside metadata block
                    if 'subject_kinds' in mstripped:
                        has_subject_kinds = True
                    metadata_lines.append(mline)
                    i += 1
                else:
                    # Exited metadata block
                    break

            # Strip trailing empty lines from metadata block
            while metadata_lines and metadata_lines[-1].strip() == '':
                metadata_lines.pop()

            # Add subject_kinds: [] if not present
            if not has_subject_kinds:
                new_lines.extend(metadata_lines)
                new_lines.append('          subject_kinds: []\n')
                changes += 1
            else:
                new_lines.extend(metadata_lines)

            continue

        new_lines.append(line)
        i += 1

    if changes > 0:
        with open(filepath, 'w') as f:
            f.writelines(new_lines)
        print(f"  {os.path.basename(filepath)}: {changes} verbs updated")

    return changes


def main():
    script_dir = os.path.dirname(os.path.abspath(__file__))
    config_dir = os.path.join(script_dir, '..', 'config', 'verbs')

    total_changes = 0

    # Process all YAML files
    for root, dirs, files in os.walk(config_dir):
        for fname in sorted(files):
            if fname.endswith('.yaml') and not fname.startswith('_'):
                filepath = os.path.join(root, fname)
                total_changes += process_file(filepath)

    print(f"\nTotal verbs updated: {total_changes}")


if __name__ == '__main__':
    main()
