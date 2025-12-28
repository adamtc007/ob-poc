#!/usr/bin/env python3
"""
GLEIF Verified Ownership Chain Extractor for Allianz Global Investors

This script extracts the GLEIF-verified ownership chain and generates
DSL commands for loading into the ob-poc system.

Key findings:
- AllianzGI GmbH reports DIRECTLY to Allianz SE (no intermediate holding)
- Allianz SE reports NO_KNOWN_PERSON exception (public company - apex)
- AllianzGI has 2 direct subsidiaries (US, Japan)
- AllianzGI manages 50+ funds registered in GLEIF
"""

import json
import requests
import time
from typing import Optional, Dict, Any, List
from datetime import datetime

GLEIF_API = "https://api.gleif.org/api/v1"

class GLEIFExtractor:
    def __init__(self):
        self.entities = {}
        self.relationships = []
        self.fund_management = []
        
    def fetch(self, url: str, params: dict = None) -> Optional[Dict]:
        """Fetch from GLEIF API with rate limiting."""
        try:
            time.sleep(0.3)  # Rate limiting
            resp = requests.get(url, params=params, timeout=30)
            if resp.status_code == 404:
                return None
            resp.raise_for_status()
            return resp.json()
        except requests.RequestException as e:
            print(f"  ERROR: {e}")
            return None
    
    def get_lei_record(self, lei: str) -> Optional[Dict]:
        """Fetch LEI record."""
        url = f"{GLEIF_API}/lei-records/{lei}"
        return self.fetch(url)
    
    def get_direct_parent(self, lei: str) -> Optional[Dict]:
        """Fetch direct parent LEI record."""
        url = f"{GLEIF_API}/lei-records/{lei}/direct-parent"
        return self.fetch(url)
    
    def get_parent_relationship(self, lei: str) -> Optional[Dict]:
        """Fetch direct parent relationship record with details."""
        url = f"{GLEIF_API}/lei-records/{lei}/direct-parent-relationship"
        return self.fetch(url)
    
    def get_parent_exception(self, lei: str) -> Optional[Dict]:
        """Fetch parent reporting exception."""
        url = f"{GLEIF_API}/lei-records/{lei}/direct-parent-reporting-exception"
        return self.fetch(url)
    
    def get_direct_children(self, lei: str, max_pages: int = 5) -> List[Dict]:
        """Fetch all direct children with pagination."""
        all_children = []
        page = 1
        
        while page <= max_pages:
            url = f"{GLEIF_API}/lei-records/{lei}/direct-children"
            params = {"page[size]": 100, "page[number]": page}
            data = self.fetch(url, params)
            
            if not data:
                break
                
            records = data.get("data", [])
            if not records:
                break
                
            all_children.extend(records)
            
            # Check if more pages
            links = data.get("links", {})
            if not links.get("next"):
                break
            page += 1
            
        return all_children
    
    def get_managed_funds(self, lei: str, max_pages: int = 5) -> List[Dict]:
        """Fetch all managed funds with pagination."""
        all_funds = []
        page = 1
        
        while page <= max_pages:
            url = f"{GLEIF_API}/lei-records/{lei}/managed-funds"
            params = {"page[size]": 100, "page[number]": page}
            data = self.fetch(url, params)
            
            if not data:
                break
                
            records = data.get("data", [])
            if not records:
                break
                
            all_funds.extend(records)
            
            links = data.get("links", {})
            if not links.get("next"):
                break
            page += 1
            
        return all_funds
    
    def extract_entity_info(self, record: Dict) -> Dict:
        """Extract key info from LEI record."""
        if not record:
            return {}
            
        data = record.get("data", record)
        attrs = data.get("attributes", {})
        entity = attrs.get("entity", {})
        registration = attrs.get("registration", {})
        
        legal_name = entity.get("legalName", {})
        legal_address = entity.get("legalAddress", {})
        
        return {
            "lei": data.get("id") or attrs.get("lei"),
            "legal_name": legal_name.get("name") if isinstance(legal_name, dict) else str(legal_name),
            "jurisdiction": entity.get("jurisdiction"),
            "category": entity.get("category"),
            "legal_form_code": entity.get("legalForm", {}).get("id"),
            "status": entity.get("status"),
            "registration_number": entity.get("registeredAs"),
            "country": legal_address.get("country"),
            "city": legal_address.get("city"),
            "address": ", ".join(legal_address.get("addressLines", [])),
            "creation_date": entity.get("creationDate"),
            "bic": attrs.get("bic", []),
        }
    
    def trace_ownership_chain(self, start_lei: str) -> List[Dict]:
        """Trace from starting entity up to apex."""
        chain = []
        current_lei = start_lei
        visited = set()
        
        print(f"\n{'='*70}")
        print("TRACING OWNERSHIP CHAIN")
        print(f"{'='*70}\n")
        
        while current_lei and current_lei not in visited:
            visited.add(current_lei)
            
            # Get current entity
            record = self.get_lei_record(current_lei)
            if not record:
                print(f"  Could not fetch {current_lei}")
                break
            
            info = self.extract_entity_info(record)
            chain.append(info)
            self.entities[current_lei] = info
            
            print(f"  [{len(chain)-1}] {info['legal_name']}")
            print(f"      LEI: {info['lei']}")
            print(f"      Jurisdiction: {info['jurisdiction']}")
            print(f"      Registration: {info.get('registration_number', 'N/A')}")
            
            # Get parent relationship details
            rel_record = self.get_parent_relationship(current_lei)
            if rel_record:
                rel_data = rel_record.get("data", {})
                rel_attrs = rel_data.get("attributes", {})
                relationship = rel_attrs.get("relationship", {})
                
                parent_lei = relationship.get("endNode", {}).get("id")
                rel_type = relationship.get("type")
                rel_status = relationship.get("status")
                corr_level = rel_attrs.get("registration", {}).get("corroborationLevel")
                
                if parent_lei:
                    print(f"      Direct Parent: {parent_lei}")
                    print(f"      Relationship: {rel_type} ({rel_status})")
                    print(f"      Corroboration: {corr_level}")
                    
                    self.relationships.append({
                        "child_lei": current_lei,
                        "parent_lei": parent_lei,
                        "type": rel_type,
                        "status": rel_status,
                        "corroboration": corr_level,
                    })
                    
                    current_lei = parent_lei
                    continue
            
            # Check for reporting exception
            exception = self.get_parent_exception(current_lei)
            if exception:
                exc_data = exception.get("data", {})
                exc_attrs = exc_data.get("attributes", {})
                reason = exc_attrs.get("reason")
                
                print(f"      Parent Exception: {reason}")
                info["ubo_terminus"] = True
                info["terminus_reason"] = reason
            else:
                # Try direct parent endpoint
                parent_record = self.get_direct_parent(current_lei)
                if parent_record:
                    parent_info = self.extract_entity_info(parent_record)
                    if parent_info.get("lei"):
                        print(f"      Direct Parent: {parent_info['lei']} ({parent_info['legal_name']})")
                        current_lei = parent_info["lei"]
                        continue
            
            # No more parents
            print(f"      [APEX - No further parents]")
            break
        
        return chain
    
    def get_subsidiary_tree(self, lei: str, depth: int = 0, max_depth: int = 2) -> List[Dict]:
        """Get subsidiaries recursively."""
        if depth >= max_depth:
            return []
        
        children = self.get_direct_children(lei, max_pages=2)
        result = []
        
        for child in children:
            info = self.extract_entity_info({"data": child})
            info["parent_lei"] = lei
            info["depth"] = depth + 1
            result.append(info)
            self.entities[info["lei"]] = info
            
            # Add relationship
            self.relationships.append({
                "child_lei": info["lei"],
                "parent_lei": lei,
                "type": "IS_DIRECTLY_CONSOLIDATED_BY",
                "status": "ACTIVE",
            })
        
        return result

def main():
    print("="*70)
    print("GLEIF OWNERSHIP CHAIN EXTRACTOR")
    print(f"Generated: {datetime.now().isoformat()}")
    print("="*70)
    
    extractor = GLEIFExtractor()
    
    # Start with AllianzGI GmbH
    start_lei = "OJ2TIQSVQND4IZYYK658"
    
    # 1. Trace ownership chain UP to apex
    chain = extractor.trace_ownership_chain(start_lei)
    
    # 2. Get AllianzGI's direct subsidiaries
    print(f"\n{'='*70}")
    print("AllianzGI DIRECT SUBSIDIARIES")
    print(f"{'='*70}\n")
    
    subsidiaries = extractor.get_subsidiary_tree(start_lei, max_depth=1)
    for sub in subsidiaries:
        print(f"  {sub['lei'][:20]}... | {sub['jurisdiction']:3} | {sub['legal_name'][:50]}")
    
    # 3. Get managed funds
    print(f"\n{'='*70}")
    print("AllianzGI MANAGED FUNDS (from GLEIF)")
    print(f"{'='*70}\n")
    
    funds = extractor.get_managed_funds(start_lei, max_pages=3)
    print(f"Total funds registered in GLEIF: {len(funds)}")
    print()
    for fund in funds[:20]:
        info = extractor.extract_entity_info({"data": fund})
        print(f"  {info['lei'][:20]}... | {info['jurisdiction']:3} | {info['legal_name'][:50]}")
        extractor.fund_management.append({
            "manco_lei": start_lei,
            "fund_lei": info["lei"],
            "fund_name": info["legal_name"],
            "fund_jurisdiction": info["jurisdiction"],
        })
    
    if len(funds) > 20:
        print(f"  ... and {len(funds) - 20} more")
    
    # 4. Summary
    print(f"\n{'='*70}")
    print("VERIFIED OWNERSHIP STRUCTURE")
    print(f"{'='*70}\n")
    
    # Print chain in hierarchical format
    for i, entity in enumerate(reversed(chain)):
        indent = "  " * i
        is_apex = entity.get("ubo_terminus", False)
        
        print(f"{indent}{'└── ' if i > 0 else ''}{entity['legal_name']}")
        print(f"{indent}    LEI: {entity['lei']}")
        print(f"{indent}    Jurisdiction: {entity['jurisdiction']}")
        
        if is_apex:
            reason = entity.get("terminus_reason", "UNKNOWN")
            print(f"{indent}    Status: UBO TERMINUS ({reason})")
        elif i > 0:
            print(f"{indent}    Ownership: 100% (accounting consolidation)")
    
    # 5. Generate DSL
    print(f"\n{'='*70}")
    print("GENERATED DSL")
    print(f"{'='*70}\n")
    
    dsl_output = []
    dsl_output.append(";; GLEIF-Verified Allianz Global Investors Ownership Structure")
    dsl_output.append(f";; Generated: {datetime.now().isoformat()}")
    dsl_output.append(f";; Source: GLEIF API (api.gleif.org)")
    dsl_output.append(";; Relationship Type: IS_DIRECTLY_CONSOLIDATED_BY (accounting consolidation = 100%)")
    dsl_output.append("")
    
    # Create entities (apex first)
    dsl_output.append(";; === ENTITIES ===")
    dsl_output.append("")
    
    for entity in reversed(chain):
        lei = entity["lei"]
        binding = f"@{lei[:8].lower()}"
        safe_name = entity["legal_name"].replace('"', '\\"')
        
        is_apex = entity.get("ubo_terminus", False)
        
        dsl_output.append(f'(entity.ensure-limited-company')
        dsl_output.append(f'    :name "{safe_name}"')
        dsl_output.append(f'    :lei "{lei}"')
        dsl_output.append(f'    :jurisdiction "{entity["jurisdiction"]}"')
        if entity.get("registration_number"):
            dsl_output.append(f'    :registration-number "{entity["registration_number"]}"')
        if entity.get("city"):
            dsl_output.append(f'    :city "{entity["city"]}"')
        dsl_output.append(f'    :as {binding})')
        dsl_output.append("")
    
    # Create subsidiaries
    if subsidiaries:
        dsl_output.append(";; === SUBSIDIARIES ===")
        dsl_output.append("")
        
        for sub in subsidiaries:
            lei = sub["lei"]
            binding = f"@{lei[:8].lower()}"
            safe_name = sub["legal_name"].replace('"', '\\"')
            
            dsl_output.append(f'(entity.ensure-limited-company')
            dsl_output.append(f'    :name "{safe_name}"')
            dsl_output.append(f'    :lei "{lei}"')
            dsl_output.append(f'    :jurisdiction "{sub["jurisdiction"]}"')
            dsl_output.append(f'    :as {binding})')
            dsl_output.append("")
    
    # Create ownership relationships
    dsl_output.append(";; === OWNERSHIP CHAIN ===")
    dsl_output.append(";; GLEIF reports 'IS_DIRECTLY_CONSOLIDATED_BY' = 100% accounting ownership")
    dsl_output.append("")
    
    for rel in extractor.relationships:
        child_binding = f"@{rel['child_lei'][:8].lower()}"
        parent_binding = f"@{rel['parent_lei'][:8].lower()}"
        
        dsl_output.append(f'(cbu.role:assign-ownership')
        dsl_output.append(f'    :owner-entity-id {parent_binding}')
        dsl_output.append(f'    :owned-entity-id {child_binding}')
        dsl_output.append(f'    :percentage 100.0')
        dsl_output.append(f'    :ownership-type "ACCOUNTING_CONSOLIDATION"')
        dsl_output.append(f'    :source "GLEIF"')
        dsl_output.append(f'    :corroboration "{rel.get("corroboration", "UNKNOWN")}")')
        dsl_output.append("")
    
    # Mark apex as UBO terminus
    apex = chain[-1] if chain else None
    if apex and apex.get("ubo_terminus"):
        apex_binding = f"@{apex['lei'][:8].lower()}"
        reason = apex.get("terminus_reason", "NO_KNOWN_PERSON")
        
        dsl_output.append(";; === UBO TERMINUS ===")
        dsl_output.append(f";; Allianz SE is publicly traded with dispersed ownership")
        dsl_output.append("")
        dsl_output.append(f'(cbu.role:mark-ubo-terminus')
        dsl_output.append(f'    :entity-id {apex_binding}')
        dsl_output.append(f'    :reason "{reason}"')
        dsl_output.append(f'    :notes "GLEIF reporting exception - no consolidating parent")')
        dsl_output.append("")
    
    # Fund management relationships (sample)
    if extractor.fund_management:
        dsl_output.append(";; === FUND MANAGEMENT (sample) ===")
        dsl_output.append(f";; AllianzGI manages {len(funds)} funds registered in GLEIF")
        dsl_output.append("")
        
        for fm in extractor.fund_management[:5]:
            safe_name = fm["fund_name"].replace('"', '\\"')[:50]
            dsl_output.append(f';; Fund: {safe_name}...')
            dsl_output.append(f';; LEI: {fm["fund_lei"]}')
            dsl_output.append("")
    
    # Print DSL
    for line in dsl_output:
        print(line)
    
    # Save to file
    output_file = "/Users/adamtc007/Developer/ob-poc/data/derived/gleif/allianzgi_ownership_chain.dsl"
    import os
    os.makedirs(os.path.dirname(output_file), exist_ok=True)
    
    with open(output_file, "w") as f:
        f.write("\n".join(dsl_output))
    
    print(f"\n{'='*70}")
    print(f"DSL saved to: {output_file}")
    print(f"{'='*70}")
    
    # JSON output
    json_output = {
        "generated": datetime.now().isoformat(),
        "source": "GLEIF API",
        "ownership_chain": chain,
        "relationships": extractor.relationships,
        "subsidiaries": subsidiaries,
        "managed_funds_count": len(funds),
        "managed_funds_sample": extractor.fund_management[:10],
    }
    
    json_file = "/Users/adamtc007/Developer/ob-poc/data/derived/gleif/allianzgi_ownership_chain.json"
    with open(json_file, "w") as f:
        json.dump(json_output, f, indent=2)
    
    print(f"JSON saved to: {json_file}")

if __name__ == "__main__":
    main()
