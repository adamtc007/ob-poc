#!/usr/bin/env python3
"""
AllianzGI CSV to DSL Converter
============================================================================
Converts downloaded AllianzGI fund CSV exports to DSL load commands.

Usage:
    python csv_to_dsl.py out/LU__AGI_LUX__*.csv > lu_funds.dsl

Expected CSV columns (based on AllianzGI export format):
    - Fund Name / Fondsname
    - ISIN
    - Share Class / Anteilsklasse
    - Currency / Währung
    - Asset Class
    - Morningstar Rating
    - SFDR Category
    - NAV Date
    - NAV
    - 1Y Return
    etc.
============================================================================
"""

import csv
import sys
import re
from pathlib import Path
from typing import Dict, List, Optional
from dataclasses import dataclass

@dataclass
class ShareClass:
    """Parsed share class from CSV"""
    fund_name: str
    share_class_name: str
    isin: str
    currency: str
    asset_class: str
    share_class_type: str  # RETAIL, INSTITUTIONAL
    distribution_type: str  # ACC, DIST
    is_hedged: bool
    sfdr_category: str
    
def parse_share_class_type(name: str) -> tuple[str, str, bool]:
    """
    Parse share class designation to extract type, dist, and hedge info.
    Examples:
        "A - EUR" -> RETAIL, ACC, False
        "IT - USD" -> INSTITUTIONAL, ACC, False
        "ADM - EUR" -> RETAIL, DIST, False
        "AT (H2-EUR) - EUR" -> RETAIL, ACC, True
    """
    name_upper = name.upper()
    
    # Determine share class type
    if any(x in name_upper for x in [" IT ", " WT ", " I ", " W ", " PT "]):
        share_type = "INSTITUTIONAL"
    else:
        share_type = "RETAIL"
    
    # Determine distribution type
    if any(x in name_upper for x in ["DM ", "ADM", " D ", " DIS"]):
        dist_type = "DIST"
    else:
        dist_type = "ACC"
    
    # Determine if hedged
    is_hedged = "H2" in name_upper or "(H-" in name_upper or "HEDGED" in name_upper
    
    return share_type, dist_type, is_hedged

def extract_subfund_name(full_name: str) -> str:
    """
    Extract sub-fund name from full share class name.
    "Allianz Global Artificial Intelligence - A - EUR" -> "Allianz Global Artificial Intelligence"
    """
    # Split on " - " and take everything before the share class designation
    parts = full_name.split(" - ")
    if len(parts) >= 2:
        return parts[0].strip()
    return full_name

def sanitize_var_name(name: str) -> str:
    """Convert name to valid DSL variable name"""
    # Remove special chars, convert to lowercase with underscores
    clean = re.sub(r'[^a-zA-Z0-9]+', '_', name.lower())
    clean = re.sub(r'_+', '_', clean).strip('_')
    return clean[:50]  # Truncate if too long

def parse_csv(filepath: Path, jurisdiction: str, manco_code: str) -> List[ShareClass]:
    """Parse AllianzGI CSV export"""
    share_classes = []
    
    with open(filepath, 'r', encoding='utf-8-sig') as f:
        # Try to detect delimiter
        sample = f.read(2000)
        f.seek(0)
        
        if '\t' in sample:
            delimiter = '\t'
        elif ';' in sample:
            delimiter = ';'
        else:
            delimiter = ','
        
        reader = csv.DictReader(f, delimiter=delimiter)
        
        # Normalize column names (handle different languages)
        col_map = {}
        for col in reader.fieldnames or []:
            col_lower = col.lower().strip()
            if 'fund' in col_lower or 'fonds' in col_lower:
                col_map['fund_name'] = col
            elif col_lower == 'isin':
                col_map['isin'] = col
            elif 'share' in col_lower or 'anteils' in col_lower or 'class' in col_lower:
                col_map['share_class'] = col
            elif 'currency' in col_lower or 'währung' in col_lower:
                col_map['currency'] = col
            elif 'asset' in col_lower:
                col_map['asset_class'] = col
            elif 'sfdr' in col_lower:
                col_map['sfdr'] = col
        
        for row in reader:
            try:
                # Get share class name - might be separate or part of fund name
                full_name = row.get(col_map.get('fund_name', ''), '')
                share_class_suffix = row.get(col_map.get('share_class', ''), '')
                
                if share_class_suffix and share_class_suffix not in full_name:
                    share_class_name = f"{full_name} - {share_class_suffix}"
                else:
                    share_class_name = full_name
                
                isin = row.get(col_map.get('isin', ''), '').strip()
                currency = row.get(col_map.get('currency', ''), 'EUR').strip()[:3]
                asset_class = row.get(col_map.get('asset_class', ''), 'EQUITY').strip()
                sfdr = row.get(col_map.get('sfdr', ''), '').strip()
                
                if not isin or not full_name:
                    continue
                
                share_type, dist_type, is_hedged = parse_share_class_type(share_class_name)
                
                sc = ShareClass(
                    fund_name=extract_subfund_name(full_name),
                    share_class_name=share_class_name,
                    isin=isin,
                    currency=currency,
                    asset_class=asset_class,
                    share_class_type=share_type,
                    distribution_type=dist_type,
                    is_hedged=is_hedged,
                    sfdr_category=sfdr
                )
                share_classes.append(sc)
                
            except Exception as e:
                print(f"# Warning: Failed to parse row: {e}", file=sys.stderr)
                continue
    
    return share_classes

def generate_dsl(share_classes: List[ShareClass], jurisdiction: str, manco_code: str) -> str:
    """Generate DSL commands from parsed share classes"""
    
    # Group by sub-fund
    subfunds: Dict[str, List[ShareClass]] = {}
    for sc in share_classes:
        if sc.fund_name not in subfunds:
            subfunds[sc.fund_name] = []
        subfunds[sc.fund_name].append(sc)
    
    lines = []
    lines.append(f"# AllianzGI {jurisdiction} Fund Load")
    lines.append(f"# Generated from CSV export")
    lines.append(f"# ManCo: {manco_code}")
    lines.append(f"# Sub-funds: {len(subfunds)}")
    lines.append(f"# Share classes: {len(share_classes)}")
    lines.append(f"# " + "=" * 70)
    lines.append("")
    
    # Reference to umbrella (assume main SICAV for LU)
    umbrella_var = f"$umbrella_{jurisdiction.lower()}"
    lines.append(f"# Assumes umbrella already exists - use lookup or prior creation")
    lines.append(f'# {umbrella_var} = (lookup umbrella for {jurisdiction})')
    lines.append("")
    
    # Generate sub-fund and share class commands
    for subfund_name, scs in subfunds.items():
        var_name = sanitize_var_name(subfund_name)
        
        # Determine base currency from most common share class currency
        currencies = [sc.currency for sc in scs if not sc.is_hedged]
        base_currency = max(set(currencies), key=currencies.count) if currencies else "EUR"
        
        lines.append(f"# Sub-fund: {subfund_name}")
        lines.append(f"fund.create-subfund \\")
        lines.append(f'  name="{subfund_name}" \\')
        lines.append(f"  umbrella-id={umbrella_var} \\")
        lines.append(f"  base-currency={base_currency} \\")
        lines.append(f"  -> $sf_{var_name}")
        lines.append("")
        
        # Generate share classes
        for sc in scs:
            sc_var = sanitize_var_name(sc.isin)
            lines.append(f"fund.create-share-class \\")
            lines.append(f'  name="{sc.share_class_name}" \\')
            lines.append(f"  subfund-id=$sf_{var_name} \\")
            lines.append(f"  share-class-type={sc.share_class_type} \\")
            lines.append(f"  distribution-type={sc.distribution_type} \\")
            lines.append(f"  currency={sc.currency} \\")
            if sc.is_hedged:
                lines.append(f"  hedged=true \\")
            lines.append(f'  isin="{sc.isin}" \\')
            lines.append(f"  -> $sc_{sc_var}")
            lines.append("")
        
        lines.append("")
    
    return "\n".join(lines)

def main():
    if len(sys.argv) < 2:
        print("Usage: python csv_to_dsl.py <csv_file> [jurisdiction] [manco_code]", file=sys.stderr)
        print("Example: python csv_to_dsl.py out/LU__AGI_LUX__funds.csv LU AGI_LUX", file=sys.stderr)
        sys.exit(1)
    
    filepath = Path(sys.argv[1])
    
    # Try to extract jurisdiction and manco from filename
    filename = filepath.stem
    parts = filename.split("__")
    
    jurisdiction = sys.argv[2] if len(sys.argv) > 2 else (parts[0] if len(parts) >= 1 else "LU")
    manco_code = sys.argv[3] if len(sys.argv) > 3 else (parts[1] if len(parts) >= 2 else "AGI_LUX")
    
    share_classes = parse_csv(filepath, jurisdiction, manco_code)
    dsl = generate_dsl(share_classes, jurisdiction, manco_code)
    print(dsl)

if __name__ == "__main__":
    main()
