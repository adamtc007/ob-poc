#!/usr/bin/env python3
"""
Generate comprehensive Allianz DSL for full ETL load.

Sources:
- data/derived/gleif/allianzgi_ownership_chain.json (ownership hierarchy)
- data/external/allianzgi/*_comprehensive.json (funds + share classes)

Output:
- data/derived/dsl/allianz_full_etl.dsl

Verbs used:
- entity.ensure-limited-company (companies with LEI)
- fund.ensure-umbrella / fund.ensure-subfund (fund entities)  
- fund.ensure-share-class (share classes with ISIN)
- ubo.add-ownership (ownership relationships)
- cbu.ensure (CBU creation)
- cbu.assign-role (role assignments)
"""

import json
import re
import hashlib
from pathlib import Path
from datetime import datetime

GLEIF_PATH = Path("data/derived/gleif/allianzgi_ownership_chain.json")
FUNDS_DIR = Path("data/external/allianzgi")
OUTPUT_PATH = Path("data/derived/dsl/allianz_full_etl.dsl")

def slugify(text: str, max_len: int = 30) -> str:
    original = text
    text = text.lower()
    text = re.sub(r"[^a-z0-9]+", "_", text)
    text = text.strip("_")
    if not text:
        digest = hashlib.sha1(original.encode()).hexdigest()[:10]
        return f"entity_{digest}"
    if len(text) > max_len:
        digest = hashlib.sha1(original.encode()).hexdigest()[:6]
        text = text[:max_len-7] + "_" + digest
    return text

def escape(s: str) -> str:
    if s is None:
        return ""
    return s.replace('"', '\\"').replace('\n', ' ').strip()

def load_gleif():
    if not GLEIF_PATH.exists():
        return None
    return json.loads(GLEIF_PATH.read_text())

def load_funds():
    all_funds = []
    for path in sorted(FUNDS_DIR.glob("*_comprehensive.json")):
        region = path.stem.split("_")[0].upper()
        data = json.loads(path.read_text())
        manco = data.get("metadata", {}).get("managementCompany", {})
        for fund in data.get("funds", []):
            fund["_region"] = region
            fund["_manco"] = manco
            all_funds.append(fund)
    return all_funds

def main():
    OUTPUT_PATH.parent.mkdir(parents=True, exist_ok=True)
    
    gleif = load_gleif()
    funds = load_funds()
    
    lines = []
    
    # Header
    lines.append(f";; ============================================================================")
    lines.append(f";; Allianz Global Investors - Full ETL Load")
    lines.append(f";; Generated: {datetime.now().isoformat()}")
    lines.append(f";; ============================================================================")
    lines.append(f";;")
    lines.append(f";; GLEIF entities: {len(gleif['ownership_chain']) if gleif else 0}")
    lines.append(f";; Subsidiaries: {len(gleif.get('subsidiaries', [])) if gleif else 0}")
    lines.append(f";; Funds: {len(funds)}")
    lines.append(f";;")
    lines.append("")
    
    # =========================================================================
    # PHASE 1: OWNERSHIP CHAIN ENTITIES
    # =========================================================================
    lines.append(";; " + "=" * 70)
    lines.append(";; PHASE 1: Ownership Chain (GLEIF)")
    lines.append(";; " + "=" * 70)
    lines.append("")
    
    if gleif:
        # Build LEI → binding map
        lei_bindings = {}
        
        # Ultimate parent first (Allianz SE)
        for entity in reversed(gleif["ownership_chain"]):
            lei = entity["lei"]
            name = escape(entity["legal_name"])
            jurisdiction = entity.get("jurisdiction", "DE")
            reg_num = entity.get("registration_number", "")
            binding = slugify(name)
            lei_bindings[lei] = binding
            
            is_public = entity.get("ubo_terminus", False)
            
            lines.append(f'(entity.ensure-limited-company')
            lines.append(f'  :name "{name}"')
            lines.append(f'  :jurisdiction "{jurisdiction}"')
            if reg_num:
                lines.append(f'  :company-number "{reg_num}"')
            lines.append(f'  :as @{binding})')
            lines.append("")
        
        # Subsidiaries
        if gleif.get("subsidiaries"):
            lines.append(";; Subsidiaries")
            for sub in gleif["subsidiaries"]:
                lei = sub["lei"]
                name = escape(sub["legal_name"])
                jurisdiction = sub.get("jurisdiction", "")[:2]
                reg_num = sub.get("registration_number", "")
                binding = slugify(name)
                lei_bindings[lei] = binding
                
                lines.append(f'(entity.ensure-limited-company')
                lines.append(f'  :name "{name}"')
                lines.append(f'  :jurisdiction "{jurisdiction}"')
                if reg_num:
                    lines.append(f'  :company-number "{reg_num}"')
                lines.append(f'  :as @{binding})')
                lines.append("")
    
    # =========================================================================
    # PHASE 2: OWNERSHIP RELATIONSHIPS
    # =========================================================================
    lines.append(";; " + "=" * 70)
    lines.append(";; PHASE 2: Ownership Relationships")
    lines.append(";; " + "=" * 70)
    lines.append("")
    
    if gleif:
        for rel in gleif.get("relationships", []):
            child_lei = rel["child_lei"]
            parent_lei = rel["parent_lei"]
            
            child_binding = lei_bindings.get(child_lei)
            parent_binding = lei_bindings.get(parent_lei)
            
            if child_binding and parent_binding:
                lines.append(f'(ubo.add-ownership')
                lines.append(f'  :owner-entity-id @{parent_binding}')
                lines.append(f'  :owned-entity-id @{child_binding}')
                lines.append(f'  :ownership-type "DIRECT"')
                lines.append(f'  :percentage 100.0)')
                lines.append("")
    
    # =========================================================================
    # PHASE 3: CBU
    # =========================================================================
    lines.append(";; " + "=" * 70)
    lines.append(";; PHASE 3: CBU (Client Business Unit)")
    lines.append(";; " + "=" * 70)
    lines.append("")
    
    lines.append('(cbu.ensure')
    lines.append('  :name "Allianz Global Investors"')
    lines.append('  :jurisdiction "DE"')
    lines.append('  :as @cbu_agi)')
    lines.append("")
    
    # Assign ManCo role
    lines.append('(cbu.assign-role')
    lines.append('  :cbu-id @cbu_agi')
    lines.append('  :entity-id @allianz_global_investors_gmbh')
    lines.append('  :role "MANAGEMENT_COMPANY")')
    lines.append("")
    
    # =========================================================================
    # PHASE 4: FUNDS
    # =========================================================================
    lines.append(";; " + "=" * 70)
    lines.append(f";; PHASE 4: Funds ({len(funds)} total)")
    lines.append(";; " + "=" * 70)
    lines.append("")
    
    # Dedupe funds by name+jurisdiction
    seen_funds = set()
    unique_funds = []
    for fund in funds:
        key = (fund.get("fundName", ""), fund.get("_region", ""))
        if key not in seen_funds and key[0]:
            seen_funds.add(key)
            unique_funds.append(fund)
    
    fund_count = 0
    share_class_count = 0
    
    for fund in unique_funds:
        fund_name = escape(fund.get("fundName", "Unknown"))
        jurisdiction = fund.get("_region", "LU")
        legal_structure = fund.get("legalStructure", "")
        sfdr = fund.get("sfdrCategory", "")
        binding = slugify(fund_name + "_" + jurisdiction)
        
        # Determine fund type and verb
        if legal_structure in ("SICAV", "OEIC"):
            verb = "fund.ensure-umbrella"
            fund_type = "SICAV"
        else:
            verb = "fund.ensure-subfund"
            fund_type = legal_structure or "FCP"
        
        lines.append(f'({verb}')
        lines.append(f'  :name "{fund_name}"')
        lines.append(f'  :jurisdiction "{jurisdiction}"')
        lines.append(f'  :regulatory-status "UCITS"')
        lines.append(f'  :as @{binding})')
        lines.append("")
        
        fund_count += 1
        
        # Assign to CBU
        lines.append(f'(cbu.assign-role')
        lines.append(f'  :cbu-id @cbu_agi')
        lines.append(f'  :entity-id @{binding}')
        lines.append(f'  :role "FUND")')
        lines.append("")
        
        # Share classes (limit to first 50 per fund for performance)
        share_classes = fund.get("shareClasses", [])
        if share_classes:
            for sc in share_classes[:50]:
                isin = sc.get("isin", "")
                sc_name = escape(sc.get("shareClassName", ""))
                currency = sc.get("currency", "")
                
                if isin:
                    sc_binding = slugify(isin)
                    lines.append(f'(fund.ensure-share-class')
                    lines.append(f'  :fund-entity-id @{binding}')
                    lines.append(f'  :isin "{isin}"')
                    lines.append(f'  :name "{sc_name}"')
                    lines.append(f'  :currency "{currency}"')
                    lines.append(f'  :as @{sc_binding})')
                    lines.append("")
                    share_class_count += 1
            
            if len(share_classes) > 50:
                lines.append(f";; ... and {len(share_classes) - 50} more share classes (truncated)")
                lines.append("")
    
    # =========================================================================
    # SUMMARY
    # =========================================================================
    lines.append(";; " + "=" * 70)
    lines.append(";; ETL SUMMARY")
    lines.append(";; " + "=" * 70)
    lines.append(f";; Ownership chain entities: {len(gleif['ownership_chain']) if gleif else 0}")
    lines.append(f";; Subsidiaries: {len(gleif.get('subsidiaries', [])) if gleif else 0}")
    lines.append(f";; Funds created: {fund_count}")
    lines.append(f";; Share classes created: {share_class_count}")
    lines.append(";; " + "=" * 70)
    
    # Write output
    OUTPUT_PATH.write_text("\n".join(lines))
    
    print(f"[ok] Generated: {OUTPUT_PATH}")
    print(f"     Ownership entities: {len(gleif['ownership_chain']) if gleif else 0}")
    print(f"     Subsidiaries: {len(gleif.get('subsidiaries', [])) if gleif else 0}")
    print(f"     Funds: {fund_count}")
    print(f"     Share classes: {share_class_count}")
    print(f"     Total lines: {len(lines)}")

if __name__ == "__main__":
    main()
