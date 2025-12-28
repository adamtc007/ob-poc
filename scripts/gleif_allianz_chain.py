#!/usr/bin/env python3
"""
GLEIF Ownership Chain Tracer for Allianz Global Investors

Traces the accounting consolidation parent chain from AllianzGI
up to the ultimate parent (Allianz SE).

GLEIF API provides:
- Level 1: Entity reference data (name, address, LEI, jurisdiction)
- Level 2: Accounting consolidation parents (direct + ultimate)

Note: This is NOT shareholding percentage data - it's consolidation
hierarchy (implies 100% control for accounting purposes).
"""

import json
import requests
import time
from typing import Optional, Dict, Any, List

GLEIF_API = "https://api.gleif.org/api/v1"

# Known Allianz LEIs
KNOWN_LEIS = {
    "OJ2TIQSVQND4IZYYK658": "Allianz Global Investors GmbH",
    "529900K9B0N5BT694847": "Allianz SE",
}

def fetch_lei_record(lei: str) -> Optional[Dict[str, Any]]:
    """Fetch a single LEI record from GLEIF API."""
    url = f"{GLEIF_API}/lei-records/{lei}"
    print(f"  Fetching: {lei}")
    
    try:
        resp = requests.get(url, timeout=30)
        if resp.status_code == 404:
            print(f"    NOT FOUND: {lei}")
            return None
        resp.raise_for_status()
        return resp.json().get("data")
    except requests.RequestException as e:
        print(f"    ERROR: {e}")
        return None

def fetch_direct_parent(lei: str) -> Optional[str]:
    """Fetch direct accounting consolidating parent LEI."""
    url = f"{GLEIF_API}/lei-records/{lei}/direct-parent"
    
    try:
        resp = requests.get(url, timeout=30)
        if resp.status_code == 404:
            return None
        resp.raise_for_status()
        data = resp.json().get("data")
        if data:
            # The parent LEI is in the relationships
            return data.get("id")
    except requests.RequestException:
        pass
    return None

def fetch_ultimate_parent(lei: str) -> Optional[str]:
    """Fetch ultimate accounting consolidating parent LEI."""
    url = f"{GLEIF_API}/lei-records/{lei}/ultimate-parent"
    
    try:
        resp = requests.get(url, timeout=30)
        if resp.status_code == 404:
            return None
        resp.raise_for_status()
        data = resp.json().get("data")
        if data:
            return data.get("id")
    except requests.RequestException:
        pass
    return None

def fetch_direct_children(lei: str, max_results: int = 100) -> List[str]:
    """Fetch entities that report this LEI as their direct parent."""
    url = f"{GLEIF_API}/lei-records"
    params = {
        "filter[entity.registeredAt.id]": "",
        "filter[registration.status]": "ISSUED",
        "page[size]": max_results,
    }
    # Use relationship filter
    url = f"{GLEIF_API}/lei-records/{lei}/direct-children"
    
    try:
        resp = requests.get(url, timeout=30)
        if resp.status_code == 404:
            return []
        resp.raise_for_status()
        data = resp.json().get("data", [])
        return [record.get("id") for record in data if record.get("id")]
    except requests.RequestException:
        return []

def search_by_name(name: str, max_results: int = 20) -> List[Dict[str, Any]]:
    """Search for entities by name."""
    url = f"{GLEIF_API}/lei-records"
    params = {
        "filter[entity.legalName]": name,
        "page[size]": max_results,
    }
    
    try:
        resp = requests.get(url, params=params, timeout=30)
        resp.raise_for_status()
        return resp.json().get("data", [])
    except requests.RequestException as e:
        print(f"Search error: {e}")
        return []

def extract_entity_info(record: Dict[str, Any]) -> Dict[str, Any]:
    """Extract key information from a LEI record."""
    if not record:
        return {}
    
    attrs = record.get("attributes", {})
    entity = attrs.get("entity", {})
    registration = attrs.get("registration", {})
    
    legal_name = entity.get("legalName", {})
    legal_address = entity.get("legalAddress", {})
    
    return {
        "lei": record.get("id") or attrs.get("lei"),
        "legal_name": legal_name.get("name") if isinstance(legal_name, dict) else legal_name,
        "jurisdiction": entity.get("jurisdiction"),
        "category": entity.get("category"),
        "legal_form": entity.get("legalForm", {}).get("id"),
        "status": entity.get("status"),
        "registration_status": registration.get("status"),
        "country": legal_address.get("country"),
        "city": legal_address.get("city"),
    }

def trace_parent_chain(start_lei: str, max_depth: int = 10) -> List[Dict[str, Any]]:
    """
    Trace the parent chain from a starting LEI up to the ultimate parent.
    Returns list of entities from child to ultimate parent.
    """
    chain = []
    visited = set()
    current_lei = start_lei
    
    for depth in range(max_depth):
        if current_lei in visited:
            print(f"  Cycle detected at {current_lei}")
            break
        visited.add(current_lei)
        
        # Fetch current entity
        record = fetch_lei_record(current_lei)
        if not record:
            print(f"  Could not fetch {current_lei}")
            break
        
        info = extract_entity_info(record)
        info["depth"] = depth
        chain.append(info)
        
        print(f"  [{depth}] {info['legal_name']} ({info['lei'][:8]}...)")
        print(f"      Jurisdiction: {info['jurisdiction']}, Status: {info['status']}")
        
        # Try to get parent relationships from the record
        relationships = record.get("relationships", {})
        
        # Check for direct parent link
        direct_parent_link = relationships.get("direct-parent", {}).get("links", {}).get("related")
        ultimate_parent_link = relationships.get("ultimate-parent", {}).get("links", {}).get("related")
        
        # Fetch direct parent
        time.sleep(0.3)  # Rate limiting
        
        if direct_parent_link:
            # Extract parent LEI from the relationship
            try:
                parent_resp = requests.get(direct_parent_link, timeout=30)
                if parent_resp.status_code == 200:
                    parent_data = parent_resp.json().get("data")
                    if parent_data:
                        # Get the parent LEI from relationship record
                        rel_attrs = parent_data.get("attributes", {})
                        relationship = rel_attrs.get("relationship", {})
                        end_node = relationship.get("endNode", {})
                        parent_lei = end_node.get("nodeID")
                        
                        if parent_lei and parent_lei != current_lei:
                            print(f"      Direct Parent: {parent_lei}")
                            current_lei = parent_lei
                            continue
            except Exception as e:
                print(f"      Error fetching parent: {e}")
        
        # No more parents found
        print(f"      No further parents (this is the apex)")
        break
    
    return chain

def main():
    print("=" * 70)
    print("GLEIF Ownership Chain Tracer - Allianz Global Investors")
    print("=" * 70)
    print()
    
    # Start with Allianz Global Investors GmbH
    start_lei = "OJ2TIQSVQND4IZYYK658"
    
    print(f"Starting entity: {KNOWN_LEIS.get(start_lei, 'Unknown')}")
    print(f"LEI: {start_lei}")
    print()
    
    print("Searching for Allianz entities first...")
    print("-" * 50)
    
    # Search for Allianz entities to find the hierarchy
    search_results = search_by_name("Allianz Global Investors", max_results=50)
    print(f"Found {len(search_results)} entities matching 'Allianz Global Investors'")
    print()
    
    # Show relevant entities
    print("Key Allianz Global Investors entities:")
    for record in search_results[:15]:
        info = extract_entity_info(record)
        if info.get("registration_status") == "ISSUED":
            print(f"  {info['lei'][:20]}... | {info['jurisdiction']:3} | {info['legal_name'][:50]}")
    
    print()
    print("-" * 50)
    print("Tracing parent chain from AllianzGI GmbH...")
    print("-" * 50)
    print()
    
    chain = trace_parent_chain(start_lei)
    
    print()
    print("=" * 70)
    print("OWNERSHIP CHAIN SUMMARY")
    print("=" * 70)
    print()
    
    if chain:
        # Print in reverse (apex first)
        for i, entity in enumerate(reversed(chain)):
            depth = len(chain) - 1 - i
            indent = "  " * depth
            connector = "└── " if depth > 0 else ""
            print(f"{indent}{connector}{entity['legal_name']}")
            print(f"{indent}    LEI: {entity['lei']}")
            print(f"{indent}    Jurisdiction: {entity['jurisdiction']}")
            if depth > 0:
                print(f"{indent}    (100% accounting consolidation)")
            else:
                print(f"{indent}    [APEX - Ultimate Parent]")
            print()
    
    # Output as JSON for DSL generation
    print()
    print("=" * 70)
    print("JSON OUTPUT (for DSL generation)")
    print("=" * 70)
    print(json.dumps(chain, indent=2))
    
    # Generate DSL
    print()
    print("=" * 70)
    print("GENERATED DSL")
    print("=" * 70)
    print()
    
    if chain:
        print(";; GLEIF-verified ownership chain for Allianz Global Investors")
        print(f";; Generated from GLEIF API - {len(chain)} entities")
        print()
        
        # Create entities (apex first)
        for entity in reversed(chain):
            safe_name = entity['legal_name'].replace('"', '\\"')
            binding = f"@lei_{entity['lei'][:8].lower()}"
            print(f'(entity.ensure-limited-company')
            print(f'    :name "{safe_name}"')
            print(f'    :lei "{entity["lei"]}"')
            print(f'    :jurisdiction "{entity["jurisdiction"]}"')
            print(f'    :as {binding})')
            print()
        
        # Create relationships (parent owns child)
        print(";; Ownership relationships (100% - accounting consolidation)")
        for i in range(len(chain) - 1):
            child = chain[i]
            parent = chain[i + 1]
            child_binding = f"@lei_{child['lei'][:8].lower()}"
            parent_binding = f"@lei_{parent['lei'][:8].lower()}"
            print(f'(cbu.role:assign-ownership')
            print(f'    :owner-entity-id {parent_binding}')
            print(f'    :owned-entity-id {child_binding}')
            print(f'    :percentage 100.0')
            print(f'    :ownership-type "ACCOUNTING_CONSOLIDATION")')
            print()

if __name__ == "__main__":
    main()
