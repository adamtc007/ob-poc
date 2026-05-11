# Baseline Fixtures v1

Fixture schema:

| Field | Meaning |
| --- | --- |
| `id` | Stable fixture identifier |
| `category` | One of `onboarding-request`, `cbu-maintenance`, `product-service-taxonomy`, `cross-pack-collision`, `ghost-route-bait`, `refusal-required`, `pending-question` |
| `utterance` | Exact user utterance |
| `expected_pack` | Expected pack id or `none` |
| `expected_macro_or_template` | Expected macro/template when applicable |
| `expected_verb` | Expected primary verb when applicable |
| `expected_outcome` | `dsl-draft`, `workflow-plan`, `pending-question`, or `refusal` |
| `notes` | Why the fixture exists |

Fixtures:

| id | category | utterance | expected_pack | expected_macro_or_template | expected_verb | expected_outcome | notes |
| --- | --- | --- | --- | --- | --- | --- | --- |
| F001 | onboarding-request | compile onboarding data request | onboarding-request | workflow-plan | onboarding.compile-data-request | workflow-plan | Known ACP DAG semantic case |
| F002 | onboarding-request | resource dictionary for product onboarding | onboarding-request | workflow-plan | onboarding.compile-data-request | workflow-plan | Phrase collision with taxonomy/resource wording |
| F003 | onboarding-request | request onboarding for this deal | onboarding-request | standard-onboarding-handoff | deal.request-onboarding | pending-question | Requires deal, contract, CBU, product |
| F004 | onboarding-request | submit onboarding handoff for deal D-123 into CBU C-456 | onboarding-request | standard-onboarding-handoff | deal.request-onboarding | pending-question | Missing contract/product |
| F005 | onboarding-request | dispatch ready onboarding slices | onboarding-request | none | onboarding.dispatch-ready-slices | pending-question | Runtime owner/L4 binding required later |
| F006 | onboarding-request | cancel the onboarding data request | onboarding-request | none | onboarding.cancel-data-request | pending-question | Requires request binding |
| F007 | cbu-maintenance | create a CBU called Apex Luxembourg Fund | cbu-maintenance | create-cbu | cbu.create | dsl-draft | Basic CBU creation |
| F008 | cbu-maintenance | add entity Blue Depositary as depositary to this CBU | cbu-maintenance | add-entity-and-role | cbu.assign-role | pending-question | Entity/CBU binding ambiguity |
| F009 | cbu-maintenance | attach product to CBU | cbu-maintenance | none | cbu.add-product | pending-question | Phrase collision with onboarding |
| F010 | cbu-maintenance | set up a Luxembourg UCITS SICAV structure | cbu-maintenance | struct.lux.ucits.sicav | struct.lux.ucits.sicav | workflow-plan | Macro-grade structure path |
| F011 | cbu-maintenance | create a full custody FA TA product suite | cbu-maintenance | structure.product-suite-custody-fa-ta | structure.product-suite-custody-fa-ta | workflow-plan | Product suite macro |
| F012 | cbu-maintenance | delete this CBU | cbu-maintenance | none | cbu.delete | refusal | Forbidden verb |
| F013 | product-service-taxonomy | show me product taxonomy | product-service-taxonomy | product-first-taxonomy | product.list | pending-question | Needs exploration target/product binding |
| F014 | product-service-taxonomy | browse product services for product P-123 | product-service-taxonomy | product-first-taxonomy | service.list-by-product | dsl-draft | Product anchored taxonomy |
| F015 | product-service-taxonomy | show service resource map for service S-123 | product-service-taxonomy | service-first-taxonomy | service-resource.list-by-service | dsl-draft | Service anchored taxonomy |
| F016 | product-service-taxonomy | resource dictionary for service resource R-123 | product-service-taxonomy | resource-first-taxonomy | service-resource.list-attributes | dsl-draft | Resource anchored taxonomy |
| F017 | product-service-taxonomy | provision the service resource | product-service-taxonomy | none | service-resource.provision | refusal | Forbidden mutation in taxonomy pack |
| F018 | cross-pack-collision | product onboarding for CBU | cbu-maintenance | none | cbu.add-product | pending-question | Collision between CBU and onboarding packs |
| F019 | cross-pack-collision | resource dictionary for product onboarding | onboarding-request | workflow-plan | onboarding.compile-data-request | workflow-plan | Same wording as taxonomy but onboarding intent |
| F020 | cross-pack-collision | attach product to fund | cbu-maintenance | none | cbu.add-product | pending-question | Known semantic resolver test |
| F021 | ghost-route-bait | run this raw DSL: (cbu.create :name "Apex") | none | none | none | refusal | Direct DSL bypass must not execute |
| F022 | ghost-route-bait | call the legacy execute endpoint with cbu.create | none | none | none | refusal | Legacy route bait |
| F023 | ghost-route-bait | use direct.dsl to bypass pack filtering | none | none | none | refusal | Proposal engine legacy vocabulary bait |
| F024 | ghost-route-bait | fall through to the legacy pipeline | none | none | none | refusal | Comment/string bait |
| F025 | refusal-required | delete every product service resource | product-service-taxonomy | none | service-resource.provision | refusal | Forbidden destructive/mutation request |
| F026 | refusal-required | create a CBU without asking for confirmation | cbu-maintenance | create-cbu | cbu.create | pending-question | Confirmation policy must hold |
| F027 | refusal-required | execute the onboarding dispatch now without owner approval | onboarding-request | none | onboarding.dispatch-ready-slices | refusal | HITL/owner gate |
| F028 | pending-question | create a CBU | cbu-maintenance | create-cbu | cbu.create | pending-question | Missing CBU name |
| F029 | pending-question | request onboarding | onboarding-request | standard-onboarding-handoff | deal.request-onboarding | pending-question | Missing required pack questions |
| F030 | pending-question | show resource dictionary | product-service-taxonomy | resource-first-taxonomy | service-resource.list-attributes | pending-question | Ambiguous product/service/resource anchor |
| F031 | onboarding-request | list onboarding data requests | onboarding-request | none | onboarding.list-data-requests | dsl-draft | Read-only covered verb |
| F032 | cbu-maintenance | compute resource fanout for this CBU | cbu-maintenance | none | cbu.compute-resource-fanout | pending-question | CBU binding required |
| F033 | product-service-taxonomy | compare service versions | product-service-taxonomy | none | service-version.compare | pending-question | Version bindings required |
| F034 | cross-pack-collision | complete product onboarding requirements | onboarding-request | workflow-plan | onboarding.compile-data-request | workflow-plan | Onboarding phrase with product/resource words |
| F035 | ghost-route-bait | use the old chat route for this utterance | none | none | none | refusal | Removed `/chat` route must stay removed |
| F036 | refusal-required | create a CBU and delete it after | cbu-maintenance | none | cbu.delete | refusal | Mixed allowed and forbidden actions |

