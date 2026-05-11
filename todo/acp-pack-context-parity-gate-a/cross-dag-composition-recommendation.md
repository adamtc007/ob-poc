# Cross-DAG Composition Recommendation

Status: audit draft for Gate A replan.

Current evidence:

- `rust/config/sem_os_seeds/constellation_families` defines cross-workspace families.
- `rust/config/sem_os_seeds/constellation_maps` defines routeable workspace maps.
- Slice 1 fixtures intentionally include collisions between onboarding, CBU maintenance, and product/service taxonomy.

Recommendation:

For Slice 1, use pack-scoped composition with explicit neighbour hints rather than global semantic blending.

Rules:

1. Resolve pack first from authored pack phrases and workspace hints.
2. Resolve verb/macro/workbook plan only inside the selected pack plus explicitly declared neighbours.
3. Refuse or ask a pending question when two packs tie and no neighbour rule breaks the tie.
4. Never project code-grade, YAML-grade, or research macros into cross-pack context.

Required metadata before Gate B implementation:

- Pack neighbour map for `onboarding-request`, `cbu-maintenance`, and `product-service-taxonomy`.
- Collision policy for `resource dictionary`, `product onboarding`, and `attach product`.
- Diagnostic codes for ambiguous pack, forbidden mutation, and legacy-route bait.
