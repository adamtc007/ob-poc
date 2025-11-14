#!/usr/bin/env python3
"""Generate updated KYC attribute definitions with UUIDs."""

import sys

# Read the UUID mapping
uuid_map = {}
with open('attribute_uuid_map.txt', 'r') as f:
    for line in f:
        if '|' in line:
            semantic_id, uuid = line.strip().split('|')
            uuid_map[semantic_id] = uuid

# Print UUID constants for the relevant attributes
print("// UUID constants from database - Auto-generated")
print()

identity_attrs = [
    ('attr.identity.legal_name', 'LEGAL_NAME_UUID'),
    ('attr.identity.first_name', 'FIRST_NAME_UUID'),
    ('attr.identity.last_name', 'LAST_NAME_UUID'),
    ('attr.identity.date_of_birth', 'DATE_OF_BIRTH_UUID'),
    ('attr.identity.nationality', 'NATIONALITY_UUID'),
    ('attr.identity.passport_number', 'PASSPORT_NUMBER_UUID'),
    ('attr.identity.registration_number', 'REGISTRATION_NUMBER_UUID'),
    ('attr.identity.incorporation_date', 'INCORPORATION_DATE_UUID'),
]

for semantic_id, const_name in identity_attrs:
    if semantic_id in uuid_map:
        print(f'pub const {const_name}: &str = "{uuid_map[semantic_id]}";')

print()
print("// For pasting into kyc.rs attribute definitions:")
print()

# Generate example for first few attributes
examples = [
    ('LegalEntityName', 'attr.identity.legal_name'),
    ('FirstName', 'attr.identity.first_name'),
    ('LastName', 'attr.identity.last_name'),
]

for struct_name, semantic_id in examples:
    if semantic_id in uuid_map:
        print(f'// {struct_name}:')
        print(f'//   uuid = "{uuid_map[semantic_id]}",')
        print()

