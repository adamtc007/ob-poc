#!/usr/bin/env python3
"""
Populate phase_tags in verb YAML metadata blocks.

Adds phase_tags: [...] to verb metadata based on domain→phase mapping.
Only adds where phase_tags is not already present.

Usage:
    cd rust/
    python3 scripts/populate_phase_tags.py              # Normal run
    python3 scripts/populate_phase_tags.py --dry-run     # Preview without writing
    python3 scripts/populate_phase_tags.py --verbose      # Show per-verb detail
"""

import sys
import os
import re

# ─── Domain → phase_tags mapping ───────────────────────────────────────────

DOMAIN_PHASE_MAP = {
    # ── onboarding ──
    "cbu": ["onboarding"],
    "cbu-role": ["onboarding"],
    "cbu.role": ["onboarding"],
    "entity": ["onboarding"],
    "entity-role": ["onboarding"],
    "fund": ["onboarding"],
    "fund-compartment": ["onboarding"],
    "share-class": ["onboarding"],
    "client-group": ["onboarding"],
    "client": ["onboarding"],
    "client-type": ["onboarding"],
    "legal-entity": ["onboarding"],
    "onboarding": ["onboarding"],
    "product": ["onboarding"],
    "product-subscription": ["onboarding"],
    "manco": ["onboarding"],
    "capital": ["onboarding"],
    "person": ["onboarding"],
    "partnership": ["onboarding"],

    # ── kyc ──
    "kyc": ["kyc"],
    "kyc-case": ["kyc"],
    "kyc-agreement": ["kyc"],
    "screening": ["kyc"],
    "ubo": ["kyc"],
    "document": ["kyc"],
    "requirement": ["kyc"],
    "control": ["kyc"],
    "bods": ["kyc"],
    "ownership": ["kyc"],
    "evidence": ["kyc"],
    "observation": ["kyc"],
    "verify": ["kyc"],
    "case": ["kyc"],
    "case-event": ["kyc"],
    "case-screening": ["kyc"],
    "case-type": ["kyc"],
    "allegation": ["kyc"],
    "board": ["kyc"],
    "discrepancy": ["kyc"],
    "doc-request": ["kyc"],
    "docs-bundle": ["kyc"],
    "entity-workstream": ["kyc"],
    "outcome": ["kyc"],
    "rationale": ["kyc"],
    "readiness": ["kyc"],
    "red-flag": ["kyc"],
    "risk-rating": ["kyc"],
    "skeleton": ["kyc"],
    "tollgate": ["kyc"],
    "workstream": ["kyc"],
    "request": ["kyc"],
    "verification": ["kyc"],
    "regulatory.registration": ["kyc"],
    "regulatory.status": ["kyc"],

    # ── deal ──
    "deal": ["deal"],
    "contract": ["deal"],
    "contract-pack": ["deal"],
    "billing": ["deal"],
    "sla": ["deal"],
    "pricing-config": ["deal"],

    # ── trading ──
    "trading-profile": ["trading"],
    "custody": ["trading"],
    "isda": ["trading"],
    "settlement-chain": ["trading"],
    "settlement-type": ["trading"],
    "ssi-type": ["trading"],
    "investment-manager": ["trading"],
    "booking-location": ["trading"],
    "booking-principal": ["trading"],
    "service-availability": ["trading"],
    "client-principal-relationship": ["trading"],
    "investor": ["trading"],
    "investor-role": ["trading"],
    "holding": ["trading"],
    "fund-vehicle": ["trading"],
    "instruction-profile": ["trading"],
    "cash-sweep": ["trading"],
    "cbu-custody": ["trading"],
    "corporate-action": ["trading"],
    "delivery": ["trading"],
    "economic-exposure": ["trading"],
    "entity-settlement": ["trading"],
    "issuer-control-config": ["trading"],
    "matrix-overlay": ["trading"],
    "movement": ["trading"],
    "provisioning": ["trading"],
    "subcustodian": ["trading"],
    "trade-gateway": ["trading"],
    "trust": ["trading"],
    "tax-config": ["trading"],
    "delegation": ["trading"],

    # ── monitoring ──
    "lifecycle": ["monitoring"],
    "temporal": ["monitoring"],
    "bpmn": ["monitoring"],

    # ── stewardship ──
    "attribute": ["stewardship"],
    "attributes": ["stewardship"],
    "semantic": ["stewardship"],
    "graph": ["stewardship"],
    "rule": ["stewardship"],
    "rule-field": ["stewardship"],
    "ruleset": ["stewardship"],
    "registry": ["stewardship"],
    "changeset": ["stewardship"],
    "governance": ["stewardship"],
    "audit": ["stewardship"],
    "maintenance": ["stewardship"],
    "focus": ["stewardship"],
    "schema": ["stewardship"],
    "edge": ["stewardship"],
    "identifier": ["stewardship"],
    "coverage": ["stewardship"],
    "discovery": ["stewardship"],
    "effects": ["stewardship"],
    "service": ["stewardship"],
    "service-intent": ["stewardship"],
    "service-resource": ["stewardship"],

    # ── navigation ──
    "session": ["navigation"],
    "view": ["navigation"],
    "agent": ["navigation"],

    # ── administration ──
    "team": ["administration"],
    "user": ["administration"],
    "batch": ["administration"],
    "template": ["administration"],
    "pack": ["administration"],
    "pipeline": ["administration"],

    # ── research (cross-cutting) ──
    "gleif": ["onboarding", "kyc"],
    "research": ["onboarding"],

    # ── reference data → stewardship ──
    "refdata": ["stewardship"],
    "jurisdiction": ["stewardship"],
    "currency": ["stewardship"],
    "nationality": ["stewardship"],
    "role": ["stewardship"],
    "screening-type": ["stewardship"],
    "security-type": ["stewardship"],
    "instrument-class": ["stewardship"],
    "market": ["stewardship"],
    "reason": ["stewardship"],
    "priority": ["stewardship"],
    "percentage": ["stewardship"],
    "title": ["stewardship"],
}

# Directory-based fallback for unmapped domains
DIR_PHASE_MAP = {
    "kyc": ["kyc"],
    "custody": ["trading"],
    "research": ["onboarding"],
    "refdata": ["stewardship"],
    "reference": ["stewardship"],
    "registry": ["trading"],         # investor register etc.
    "templates": ["kyc"],            # template workflows
    "observation": ["kyc"],
    "verification": ["kyc"],
    "admin": ["administration"],
    "sem-reg": ["stewardship"],
}


def get_phase_tags(domain_name, rel_dir):
    """Get phase_tags for a domain, using explicit map first, then directory fallback."""
    if domain_name in DOMAIN_PHASE_MAP:
        return DOMAIN_PHASE_MAP[domain_name]

    # Directory-based fallback
    for dir_key, tags in DIR_PHASE_MAP.items():
        if dir_key in rel_dir:
            return tags

    return None


def format_phase_tags(tags):
    """Format phase_tags in YAML flow style: [tag1, tag2]"""
    return "[" + ", ".join(tags) + "]"


def process_file(filepath, config_dir, dry_run=False, verbose=False):
    """Process a single YAML file and add phase_tags where needed."""
    with open(filepath, 'r') as f:
        lines = f.readlines()

    # Compute relative directory for fallback mapping
    rel_path = os.path.relpath(filepath, config_dir)
    rel_dir = os.path.dirname(rel_path)

    # First pass: find domains in this file
    domains_in_file = {}
    current_domain = None
    for line in lines:
        stripped = line.rstrip()
        indent = len(line) - len(line.lstrip()) if stripped else 0
        trimmed = stripped.strip()

        if indent == 2 and trimmed.endswith(':') and not trimmed.startswith('#') and not trimmed.startswith('-'):
            dname = trimmed.rstrip(':')
            if dname not in ('description', 'verbs', 'version'):
                current_domain = dname
                tags = get_phase_tags(dname, rel_dir)
                if tags:
                    domains_in_file[dname] = tags

    if not domains_in_file:
        # Check if there are unmapped domains
        unmapped = []
        for line in lines:
            stripped = line.rstrip()
            indent = len(line) - len(line.lstrip()) if stripped else 0
            trimmed = stripped.strip()
            if indent == 2 and trimmed.endswith(':') and not trimmed.startswith('#') and not trimmed.startswith('-'):
                dname = trimmed.rstrip(':')
                if dname not in ('description', 'verbs', 'version'):
                    unmapped.append(dname)
        if unmapped:
            print(f"  WARN: No phase mapping for domains in {rel_path}: {unmapped}")
        return 0, []

    # Second pass: find metadata blocks and add phase_tags
    changes = 0
    changed_verbs = []
    new_lines = []
    i = 0
    current_domain = None
    current_verb = None

    while i < len(lines):
        line = lines[i]
        stripped = line.rstrip()
        indent = len(line) - len(line.lstrip()) if stripped else 0
        trimmed = stripped.strip()

        # Track domain context (indent 2)
        if indent == 2 and trimmed.endswith(':') and not trimmed.startswith('#') and not trimmed.startswith('-'):
            dname = trimmed.rstrip(':')
            if dname not in ('description', 'verbs', 'version'):
                current_domain = dname

        # Track verb context (indent 6)
        if indent == 6 and trimmed.endswith(':') and not trimmed.startswith('#') and not trimmed.startswith('-'):
            vname = trimmed.rstrip(':')
            if vname not in ('description', 'behavior', 'crud', 'plugin', 'metadata',
                             'args', 'returns', 'template', 'produces',
                             'invocation_phrases', 'lifecycle', 'durable'):
                current_verb = vname

        # Look for metadata: at verb field level (indent 8)
        if current_domain in domains_in_file and indent == 8 and trimmed == 'metadata:':
            new_lines.append(line)
            i += 1

            # Collect all metadata fields
            has_phase_tags = False
            metadata_lines = []
            last_field_idx = -1  # Track last non-empty metadata line

            while i < len(lines):
                mline = lines[i]
                mstripped = mline.rstrip()
                if mstripped == '':
                    metadata_lines.append(mline)
                    i += 1
                    continue
                mindent = len(mline) - len(mline.lstrip())
                if mindent > 8:
                    # Still inside metadata block
                    if 'phase_tags' in mstripped:
                        has_phase_tags = True
                    metadata_lines.append(mline)
                    last_field_idx = len(metadata_lines) - 1
                    i += 1
                else:
                    # Exited metadata block
                    break

            # Strip trailing empty lines from metadata block
            while metadata_lines and metadata_lines[-1].strip() == '':
                metadata_lines.pop()

            # Add phase_tags if not present
            if not has_phase_tags:
                tags = domains_in_file[current_domain]
                tag_line = f'          phase_tags: {format_phase_tags(tags)}\n'
                new_lines.extend(metadata_lines)
                new_lines.append(tag_line)
                changes += 1
                fqn = f"{current_domain}.{current_verb}" if current_verb else current_domain
                changed_verbs.append(fqn)
                if verbose:
                    print(f"    {fqn} → phase_tags: {format_phase_tags(tags)}")
            else:
                new_lines.extend(metadata_lines)

            continue

        new_lines.append(line)
        i += 1

    if changes > 0 and not dry_run:
        with open(filepath, 'w') as f:
            f.writelines(new_lines)

    return changes, changed_verbs


def main():
    dry_run = '--dry-run' in sys.argv
    verbose = '--verbose' in sys.argv or '-v' in sys.argv

    script_dir = os.path.dirname(os.path.abspath(__file__))
    config_dir = os.path.join(script_dir, '..', 'config', 'verbs')
    config_dir = os.path.abspath(config_dir)

    if dry_run:
        print("=== DRY RUN MODE (no files will be modified) ===\n")

    total_changes = 0
    total_files = 0
    all_changed_verbs = []

    # Process all YAML files
    for root, dirs, files in os.walk(config_dir):
        dirs.sort()
        for fname in sorted(files):
            if fname.endswith('.yaml') and not fname.startswith('_'):
                filepath = os.path.join(root, fname)
                rel_path = os.path.relpath(filepath, config_dir)
                changes, changed_verbs = process_file(
                    filepath, config_dir, dry_run=dry_run, verbose=verbose
                )
                if changes > 0:
                    total_files += 1
                    total_changes += changes
                    all_changed_verbs.extend(changed_verbs)
                    prefix = "[DRY] " if dry_run else ""
                    print(f"  {prefix}{rel_path}: {changes} verbs updated")

    print(f"\n{'[DRY RUN] ' if dry_run else ''}Total: {total_changes} verbs across {total_files} files")

    if verbose and all_changed_verbs:
        print(f"\nPhase distribution:")
        phase_counts = {}
        for fqn in all_changed_verbs:
            domain = fqn.split('.')[0]
            tags = get_phase_tags(domain, "")
            if tags:
                for tag in tags:
                    phase_counts[tag] = phase_counts.get(tag, 0) + 1
        for phase, count in sorted(phase_counts.items()):
            print(f"  {phase}: {count} verbs")


if __name__ == '__main__':
    main()
