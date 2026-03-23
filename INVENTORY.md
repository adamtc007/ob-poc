# Constellation Audit Inventory

## 1. Constellation Map Files
- File path: `rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml`
- constellation name value: `deal.lifecycle`
- jurisdiction value: `ALL`
- Number of top-level slots: `10`
- Slot names:
  - `deal` | type=`cbu` | cardinality=`root` | entity_kinds=`-` | state_machine=`deal_lifecycle` | depends_on=`-` | verb count=`20`
  - `participant` | type=`entity` | cardinality=`optional` | entity_kinds=`person` | state_machine=`-` | depends_on=`deal` | verb count=`3`
  - `deal_contract` | type=`entity` | cardinality=`optional` | entity_kinds=`contract` | state_machine=`-` | depends_on=`deal` | verb count=`3`
  - `contract` | type=`entity` | cardinality=`optional` | entity_kinds=`contract` | state_machine=`-` | depends_on=`deal` | verb count=`14`
  - `deal_product` | type=`entity` | cardinality=`optional` | entity_kinds=`entity` | state_machine=`-` | depends_on=`deal` | verb count=`4`
  - `rate_card` | type=`entity` | cardinality=`optional` | entity_kinds=`entity` | state_machine=`-` | depends_on=`deal_product` | verb count=`11`
  - `onboarding_request` | type=`entity` | cardinality=`optional` | entity_kinds=`entity` | state_machine=`-` | depends_on=`deal (min_state=contracted)` | verb count=`4`
  - `billing_profile` | type=`entity` | cardinality=`optional` | entity_kinds=`entity` | state_machine=`-` | depends_on=`rate_card` | verb count=`17`
  - `pricing` | type=`entity` | cardinality=`optional` | entity_kinds=`entity` | state_machine=`-` | depends_on=`rate_card` | verb count=`12`
  - `contract_template` | type=`entity` | cardinality=`optional` | entity_kinds=`contract` | state_machine=`-` | depends_on=`contract` | verb count=`2`

- File path: `rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml`
- constellation name value: `fund.administration`
- jurisdiction value: `ALL`
- Number of top-level slots: `10`
- Slot names:
  - `fund` | type=`cbu` | cardinality=`root` | entity_kinds=`-` | state_machine=`fund_lifecycle` | depends_on=`-` | verb count=`7`
  - `umbrella` | type=`entity` | cardinality=`optional` | entity_kinds=`fund` | state_machine=`-` | depends_on=`fund` | verb count=`6`
  - `share_class` | type=`entity` | cardinality=`optional` | entity_kinds=`fund` | state_machine=`-` | depends_on=`fund` | verb count=`2`
  - `feeder` | type=`entity` | cardinality=`optional` | entity_kinds=`fund` | state_machine=`-` | depends_on=`fund` | verb count=`2`
  - `investment` | type=`entity` | cardinality=`optional` | entity_kinds=`entity` | state_machine=`-` | depends_on=`fund` | verb count=`5`
  - `capital` | type=`entity` | cardinality=`optional` | entity_kinds=`fund` | state_machine=`-` | depends_on=`fund` | verb count=`30`
  - `investment_manager` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`-` | depends_on=`fund` | verb count=`7`
  - `manco_group` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`-` | depends_on=`fund` | verb count=`16`
  - `trust` | type=`entity` | cardinality=`optional` | entity_kinds=`entity` | state_machine=`-` | depends_on=`fund` | verb count=`8`
  - `partnership` | type=`entity` | cardinality=`optional` | entity_kinds=`entity` | state_machine=`-` | depends_on=`fund` | verb count=`7`

- File path: `rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml`
- constellation name value: `governance.compliance`
- jurisdiction value: `ALL`
- Number of top-level slots: `9`
- Slot names:
  - `group` | type=`cbu` | cardinality=`root` | entity_kinds=`-` | state_machine=`-` | depends_on=`-` | verb count=`0`
  - `sla` | type=`entity` | cardinality=`optional` | entity_kinds=`contract` | state_machine=`-` | depends_on=`group` | verb count=`22`
  - `access_review` | type=`entity` | cardinality=`optional` | entity_kinds=`entity` | state_machine=`-` | depends_on=`group` | verb count=`21`
  - `regulatory` | type=`entity` | cardinality=`optional` | entity_kinds=`entity` | state_machine=`-` | depends_on=`group` | verb count=`10`
  - `ruleset` | type=`entity` | cardinality=`optional` | entity_kinds=`entity` | state_machine=`-` | depends_on=`group` | verb count=`4`
  - `delegation` | type=`entity` | cardinality=`optional` | entity_kinds=`entity` | state_machine=`-` | depends_on=`group` | verb count=`8`
  - `team` | type=`entity` | cardinality=`optional` | entity_kinds=`person` | state_machine=`-` | depends_on=`group` | verb count=`20`
  - `rule` | type=`entity` | cardinality=`optional` | entity_kinds=`entity` | state_machine=`-` | depends_on=`ruleset` | verb count=`3`
  - `rule_field` | type=`entity` | cardinality=`optional` | entity_kinds=`entity` | state_machine=`-` | depends_on=`ruleset` | verb count=`2`

- File path: `rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml`
- constellation name value: `group.ownership`
- jurisdiction value: `ALL`
- Number of top-level slots: `5`
- Slot names:
  - `client_group` | type=`cbu` | cardinality=`root` | entity_kinds=`-` | state_machine=`client_group_lifecycle` | depends_on=`-` | verb count=`24`
  - `gleif_import` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`-` | depends_on=`client_group` | verb count=`16`
  - `ubo_discovery` | type=`entity_graph` | cardinality=`recursive` | entity_kinds=`person, company` | state_machine=`ubo_epistemic_lifecycle` | depends_on=`gleif_import` | verb count=`32`
  - `control_chain` | type=`entity_graph` | cardinality=`recursive` | entity_kinds=`company` | state_machine=`-` | depends_on=`ubo_discovery` | verb count=`35`
  - `cbu_identification` | type=`cbu` | cardinality=`optional` | entity_kinds=`-` | state_machine=`-` | depends_on=`control_chain` | verb count=`34`

- File path: `rust/config/sem_os_seeds/constellation_maps/kyc_extended.yaml`
- constellation name value: `kyc.extended`
- jurisdiction value: `ALL`
- Number of top-level slots: `3`
- Slot names:
  - `entity` | type=`entity` | cardinality=`root` | entity_kinds=`person, company` | state_machine=`-` | depends_on=`-` | verb count=`1`
  - `board` | type=`entity` | cardinality=`optional` | entity_kinds=`person` | state_machine=`-` | depends_on=`entity` | verb count=`9`
  - `bods` | type=`entity` | cardinality=`optional` | entity_kinds=`person, company` | state_machine=`-` | depends_on=`entity` | verb count=`9`

- File path: `rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml`
- constellation name value: `kyc.onboarding`
- jurisdiction value: `ALL`
- Number of top-level slots: `7`
- Slot names:
  - `cbu` | type=`cbu` | cardinality=`root` | entity_kinds=`-` | state_machine=`-` | depends_on=`-` | verb count=`1`
  - `kyc_case` | type=`case` | cardinality=`mandatory` | entity_kinds=`-` | state_machine=`kyc_case_lifecycle` | depends_on=`cbu` | verb count=`11`
  - `entity_workstream` | type=`entity` | cardinality=`optional` | entity_kinds=`person, company` | state_machine=`-` | depends_on=`kyc_case` | verb count=`34`
  - `screening` | type=`entity` | cardinality=`optional` | entity_kinds=`person, company` | state_machine=`screening_lifecycle` | depends_on=`entity_workstream` | verb count=`13`
  - `kyc_agreement` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`-` | depends_on=`kyc_case` | verb count=`6`
  - `identifier` | type=`entity` | cardinality=`optional` | entity_kinds=`entity` | state_machine=`-` | depends_on=`entity_workstream` | verb count=`11`
  - `request` | type=`entity` | cardinality=`optional` | entity_kinds=`entity` | state_machine=`-` | depends_on=`kyc_case` | verb count=`9`

- File path: `rust/config/sem_os_seeds/constellation_maps/struct_hedge_cross_border.yaml`
- constellation name value: `struct.hedge.cross-border`
- jurisdiction value: `XB`
- Number of top-level slots: `11`
- Slot names:
  - `cbu` | type=`cbu` | cardinality=`root` | entity_kinds=`-` | state_machine=`-` | depends_on=`-` | verb count=`3`
  - `aifm` | type=`entity` | cardinality=`mandatory` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `depositary` | type=`entity` | cardinality=`mandatory` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `prime_broker` | type=`entity` | cardinality=`mandatory` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `investment_manager` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `administrator` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `auditor` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `secondary_prime_broker` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `ownership_chain` | type=`entity_graph` | cardinality=`recursive` | entity_kinds=`person, company` | state_machine=`ubo_epistemic_lifecycle` | depends_on=`aifm` | verb count=`6`
  - `case` | type=`case` | cardinality=`optional` | entity_kinds=`-` | state_machine=`kyc_case_lifecycle` | depends_on=`aifm` | verb count=`5`
  - `mandate` | type=`mandate` | cardinality=`optional` | entity_kinds=`-` | state_machine=`-` | depends_on=`cbu (min_state=filled), case (min_state=intake)` | verb count=`1`

- File path: `rust/config/sem_os_seeds/constellation_maps/struct_ie_aif_icav.yaml`
- constellation name value: `struct.ie.aif.icav`
- jurisdiction value: `IE`
- Number of top-level slots: `11`
- Slot names:
  - `cbu` | type=`cbu` | cardinality=`root` | entity_kinds=`-` | state_machine=`-` | depends_on=`-` | verb count=`3`
  - `aifm` | type=`entity` | cardinality=`mandatory` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `depositary` | type=`entity` | cardinality=`mandatory` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `investment_manager` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `administrator` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `auditor` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `prime_broker` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `company_secretary` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `ownership_chain` | type=`entity_graph` | cardinality=`recursive` | entity_kinds=`person, company` | state_machine=`ubo_epistemic_lifecycle` | depends_on=`aifm` | verb count=`6`
  - `case` | type=`case` | cardinality=`optional` | entity_kinds=`-` | state_machine=`kyc_case_lifecycle` | depends_on=`aifm` | verb count=`5`
  - `mandate` | type=`mandate` | cardinality=`optional` | entity_kinds=`-` | state_machine=`-` | depends_on=`cbu (min_state=filled), case (min_state=intake)` | verb count=`1`

- File path: `rust/config/sem_os_seeds/constellation_maps/struct_ie_hedge_icav.yaml`
- constellation name value: `struct.ie.hedge.icav`
- jurisdiction value: `IE`
- Number of top-level slots: `13`
- Slot names:
  - `cbu` | type=`cbu` | cardinality=`root` | entity_kinds=`-` | state_machine=`-` | depends_on=`-` | verb count=`3`
  - `aifm` | type=`entity` | cardinality=`mandatory` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `depositary` | type=`entity` | cardinality=`mandatory` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `investment_manager` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `administrator` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `auditor` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `prime_broker` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `secondary_prime_broker` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `executing_broker` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `company_secretary` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `ownership_chain` | type=`entity_graph` | cardinality=`recursive` | entity_kinds=`person, company` | state_machine=`ubo_epistemic_lifecycle` | depends_on=`aifm` | verb count=`6`
  - `case` | type=`case` | cardinality=`optional` | entity_kinds=`-` | state_machine=`kyc_case_lifecycle` | depends_on=`aifm` | verb count=`5`
  - `mandate` | type=`mandate` | cardinality=`optional` | entity_kinds=`-` | state_machine=`-` | depends_on=`cbu (min_state=filled), case (min_state=intake)` | verb count=`1`

- File path: `rust/config/sem_os_seeds/constellation_maps/struct_ie_ucits_icav.yaml`
- constellation name value: `struct.ie.ucits.icav`
- jurisdiction value: `IE`
- Number of top-level slots: `11`
- Slot names:
  - `cbu` | type=`cbu` | cardinality=`root` | entity_kinds=`-` | state_machine=`-` | depends_on=`-` | verb count=`3`
  - `management_company` | type=`entity` | cardinality=`mandatory` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `depositary` | type=`entity` | cardinality=`mandatory` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `investment_manager` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `administrator` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `auditor` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `company_secretary` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `legal_counsel` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `ownership_chain` | type=`entity_graph` | cardinality=`recursive` | entity_kinds=`person, company` | state_machine=`ubo_epistemic_lifecycle` | depends_on=`management_company` | verb count=`6`
  - `case` | type=`case` | cardinality=`optional` | entity_kinds=`-` | state_machine=`kyc_case_lifecycle` | depends_on=`management_company` | verb count=`5`
  - `mandate` | type=`mandate` | cardinality=`optional` | entity_kinds=`-` | state_machine=`-` | depends_on=`cbu (min_state=filled), case (min_state=intake)` | verb count=`1`

- File path: `rust/config/sem_os_seeds/constellation_maps/struct_lux_aif_raif.yaml`
- constellation name value: `struct.lux.aif.raif`
- jurisdiction value: `LU`
- Number of top-level slots: `10`
- Slot names:
  - `cbu` | type=`cbu` | cardinality=`root` | entity_kinds=`-` | state_machine=`-` | depends_on=`-` | verb count=`3`
  - `aifm` | type=`entity` | cardinality=`mandatory` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `depositary` | type=`entity` | cardinality=`mandatory` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `investment_manager` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `administrator` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `auditor` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `prime_broker` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `ownership_chain` | type=`entity_graph` | cardinality=`recursive` | entity_kinds=`person, company` | state_machine=`ubo_epistemic_lifecycle` | depends_on=`aifm` | verb count=`6`
  - `case` | type=`case` | cardinality=`optional` | entity_kinds=`-` | state_machine=`kyc_case_lifecycle` | depends_on=`aifm` | verb count=`5`
  - `mandate` | type=`mandate` | cardinality=`optional` | entity_kinds=`-` | state_machine=`-` | depends_on=`cbu (min_state=filled), case (min_state=intake)` | verb count=`1`

- File path: `rust/config/sem_os_seeds/constellation_maps/struct_lux_pe_scsp.yaml`
- constellation name value: `struct.lux.pe.scsp`
- jurisdiction value: `LU`
- Number of top-level slots: `10`
- Slot names:
  - `cbu` | type=`cbu` | cardinality=`root` | entity_kinds=`-` | state_machine=`-` | depends_on=`-` | verb count=`3`
  - `general_partner` | type=`entity` | cardinality=`mandatory` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `aifm` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `depositary` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `administrator` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `auditor` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `legal_counsel` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `ownership_chain` | type=`entity_graph` | cardinality=`recursive` | entity_kinds=`person, company` | state_machine=`ubo_epistemic_lifecycle` | depends_on=`general_partner` | verb count=`6`
  - `case` | type=`case` | cardinality=`optional` | entity_kinds=`-` | state_machine=`kyc_case_lifecycle` | depends_on=`general_partner` | verb count=`5`
  - `mandate` | type=`mandate` | cardinality=`optional` | entity_kinds=`-` | state_machine=`-` | depends_on=`cbu (min_state=filled), case (min_state=intake)` | verb count=`1`

- File path: `rust/config/sem_os_seeds/constellation_maps/struct_lux_ucits_sicav.yaml`
- constellation name value: `struct.lux.ucits.sicav`
- jurisdiction value: `LU`
- Number of top-level slots: `7`
- Slot names:
  - `cbu` | type=`cbu` | cardinality=`root` | entity_kinds=`-` | state_machine=`-` | depends_on=`-` | verb count=`3`
  - `management_company` | type=`entity` | cardinality=`mandatory` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `depositary` | type=`entity` | cardinality=`mandatory` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`4`
  - `investment_manager` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`2`
  - `ownership_chain` | type=`entity_graph` | cardinality=`recursive` | entity_kinds=`person, company` | state_machine=`ubo_epistemic_lifecycle` | depends_on=`management_company` | verb count=`6`
  - `case` | type=`case` | cardinality=`optional` | entity_kinds=`-` | state_machine=`kyc_case_lifecycle` | depends_on=`management_company` | verb count=`5`
  - `mandate` | type=`mandate` | cardinality=`optional` | entity_kinds=`-` | state_machine=`-` | depends_on=`cbu (min_state=filled), case (min_state=intake)` | verb count=`1`

- File path: `rust/config/sem_os_seeds/constellation_maps/struct_pe_cross_border.yaml`
- constellation name value: `struct.pe.cross-border`
- jurisdiction value: `XB`
- Number of top-level slots: `10`
- Slot names:
  - `cbu` | type=`cbu` | cardinality=`root` | entity_kinds=`-` | state_machine=`-` | depends_on=`-` | verb count=`3`
  - `general_partner` | type=`entity` | cardinality=`mandatory` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `aifm` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `depositary` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `administrator` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `auditor` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `legal_counsel` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `ownership_chain` | type=`entity_graph` | cardinality=`recursive` | entity_kinds=`person, company` | state_machine=`ubo_epistemic_lifecycle` | depends_on=`general_partner` | verb count=`6`
  - `case` | type=`case` | cardinality=`optional` | entity_kinds=`-` | state_machine=`kyc_case_lifecycle` | depends_on=`general_partner` | verb count=`5`
  - `mandate` | type=`mandate` | cardinality=`optional` | entity_kinds=`-` | state_machine=`-` | depends_on=`cbu (min_state=filled), case (min_state=intake)` | verb count=`1`

- File path: `rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_acs.yaml`
- constellation name value: `struct.uk.authorised.acs`
- jurisdiction value: `UK`
- Number of top-level slots: `9`
- Slot names:
  - `cbu` | type=`cbu` | cardinality=`root` | entity_kinds=`-` | state_machine=`-` | depends_on=`-` | verb count=`3`
  - `acs_operator` | type=`entity` | cardinality=`mandatory` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `depositary` | type=`entity` | cardinality=`mandatory` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `investment_manager` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `administrator` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `auditor` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `ownership_chain` | type=`entity_graph` | cardinality=`recursive` | entity_kinds=`person, company` | state_machine=`ubo_epistemic_lifecycle` | depends_on=`acs_operator` | verb count=`6`
  - `case` | type=`case` | cardinality=`optional` | entity_kinds=`-` | state_machine=`kyc_case_lifecycle` | depends_on=`acs_operator` | verb count=`5`
  - `mandate` | type=`mandate` | cardinality=`optional` | entity_kinds=`-` | state_machine=`-` | depends_on=`cbu (min_state=filled), case (min_state=intake)` | verb count=`1`

- File path: `rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_aut.yaml`
- constellation name value: `struct.uk.authorised.aut`
- jurisdiction value: `UK`
- Number of top-level slots: `9`
- Slot names:
  - `cbu` | type=`cbu` | cardinality=`root` | entity_kinds=`-` | state_machine=`-` | depends_on=`-` | verb count=`3`
  - `authorised_fund_manager` | type=`entity` | cardinality=`mandatory` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `trustee` | type=`entity` | cardinality=`mandatory` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `investment_manager` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `administrator` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `auditor` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `ownership_chain` | type=`entity_graph` | cardinality=`recursive` | entity_kinds=`person, company` | state_machine=`ubo_epistemic_lifecycle` | depends_on=`authorised_fund_manager` | verb count=`6`
  - `case` | type=`case` | cardinality=`optional` | entity_kinds=`-` | state_machine=`kyc_case_lifecycle` | depends_on=`authorised_fund_manager` | verb count=`5`
  - `mandate` | type=`mandate` | cardinality=`optional` | entity_kinds=`-` | state_machine=`-` | depends_on=`cbu (min_state=filled), case (min_state=intake)` | verb count=`1`

- File path: `rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_ltaf.yaml`
- constellation name value: `struct.uk.authorised.ltaf`
- jurisdiction value: `UK`
- Number of top-level slots: `11`
- Slot names:
  - `cbu` | type=`cbu` | cardinality=`root` | entity_kinds=`-` | state_machine=`-` | depends_on=`-` | verb count=`3`
  - `authorised_corporate_director` | type=`entity` | cardinality=`mandatory` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `depositary` | type=`entity` | cardinality=`mandatory` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `investment_manager` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `administrator` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `auditor` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `registrar` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `valuation_agent` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `ownership_chain` | type=`entity_graph` | cardinality=`recursive` | entity_kinds=`person, company` | state_machine=`ubo_epistemic_lifecycle` | depends_on=`authorised_corporate_director` | verb count=`6`
  - `case` | type=`case` | cardinality=`optional` | entity_kinds=`-` | state_machine=`kyc_case_lifecycle` | depends_on=`authorised_corporate_director` | verb count=`5`
  - `mandate` | type=`mandate` | cardinality=`optional` | entity_kinds=`-` | state_machine=`-` | depends_on=`cbu (min_state=filled), case (min_state=intake)` | verb count=`1`

- File path: `rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_oeic.yaml`
- constellation name value: `struct.uk.authorised.oeic`
- jurisdiction value: `UK`
- Number of top-level slots: `10`
- Slot names:
  - `cbu` | type=`cbu` | cardinality=`root` | entity_kinds=`-` | state_machine=`-` | depends_on=`-` | verb count=`3`
  - `authorised_corporate_director` | type=`entity` | cardinality=`mandatory` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `depositary` | type=`entity` | cardinality=`mandatory` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `investment_manager` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `administrator` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `auditor` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `registrar` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `ownership_chain` | type=`entity_graph` | cardinality=`recursive` | entity_kinds=`person, company` | state_machine=`ubo_epistemic_lifecycle` | depends_on=`authorised_corporate_director` | verb count=`6`
  - `case` | type=`case` | cardinality=`optional` | entity_kinds=`-` | state_machine=`kyc_case_lifecycle` | depends_on=`authorised_corporate_director` | verb count=`5`
  - `mandate` | type=`mandate` | cardinality=`optional` | entity_kinds=`-` | state_machine=`-` | depends_on=`cbu (min_state=filled), case (min_state=intake)` | verb count=`1`

- File path: `rust/config/sem_os_seeds/constellation_maps/struct_uk_manager_llp.yaml`
- constellation name value: `struct.uk.manager.llp`
- jurisdiction value: `UK`
- Number of top-level slots: `8`
- Slot names:
  - `cbu` | type=`cbu` | cardinality=`root` | entity_kinds=`-` | state_machine=`-` | depends_on=`-` | verb count=`3`
  - `designated_member_1` | type=`entity` | cardinality=`mandatory` | entity_kinds=`company, person` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `designated_member_2` | type=`entity` | cardinality=`mandatory` | entity_kinds=`company, person` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `compliance_officer` | type=`entity` | cardinality=`optional` | entity_kinds=`person` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `mlro` | type=`entity` | cardinality=`optional` | entity_kinds=`person` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `auditor` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `ownership_chain` | type=`entity_graph` | cardinality=`recursive` | entity_kinds=`person, company` | state_machine=`ubo_epistemic_lifecycle` | depends_on=`designated_member_1, designated_member_2` | verb count=`6`
  - `case` | type=`case` | cardinality=`optional` | entity_kinds=`-` | state_machine=`kyc_case_lifecycle` | depends_on=`designated_member_1` | verb count=`5`

- File path: `rust/config/sem_os_seeds/constellation_maps/struct_uk_pe_lp.yaml`
- constellation name value: `struct.uk.private-equity.lp`
- jurisdiction value: `UK`
- Number of top-level slots: `10`
- Slot names:
  - `cbu` | type=`cbu` | cardinality=`root` | entity_kinds=`-` | state_machine=`-` | depends_on=`-` | verb count=`3`
  - `general_partner` | type=`entity` | cardinality=`mandatory` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `aifm` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `depositary` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `administrator` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `auditor` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `legal_counsel` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `ownership_chain` | type=`entity_graph` | cardinality=`recursive` | entity_kinds=`person, company` | state_machine=`ubo_epistemic_lifecycle` | depends_on=`general_partner` | verb count=`6`
  - `case` | type=`case` | cardinality=`optional` | entity_kinds=`-` | state_machine=`kyc_case_lifecycle` | depends_on=`general_partner` | verb count=`5`
  - `mandate` | type=`mandate` | cardinality=`optional` | entity_kinds=`-` | state_machine=`-` | depends_on=`cbu (min_state=filled), case (min_state=intake)` | verb count=`1`

- File path: `rust/config/sem_os_seeds/constellation_maps/struct_us_40act_closed_end.yaml`
- constellation name value: `struct.us.40act.closed-end`
- jurisdiction value: `US`
- Number of top-level slots: `11`
- Slot names:
  - `cbu` | type=`cbu` | cardinality=`root` | entity_kinds=`-` | state_machine=`-` | depends_on=`-` | verb count=`3`
  - `investment_adviser` | type=`entity` | cardinality=`mandatory` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `custodian` | type=`entity` | cardinality=`mandatory` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `sub_adviser` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `administrator` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `transfer_agent` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `auditor` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `legal_counsel` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `ownership_chain` | type=`entity_graph` | cardinality=`recursive` | entity_kinds=`person, company` | state_machine=`ubo_epistemic_lifecycle` | depends_on=`investment_adviser` | verb count=`6`
  - `case` | type=`case` | cardinality=`optional` | entity_kinds=`-` | state_machine=`kyc_case_lifecycle` | depends_on=`investment_adviser` | verb count=`5`
  - `mandate` | type=`mandate` | cardinality=`optional` | entity_kinds=`-` | state_machine=`-` | depends_on=`cbu (min_state=filled), case (min_state=intake)` | verb count=`1`

- File path: `rust/config/sem_os_seeds/constellation_maps/struct_us_40act_open_end.yaml`
- constellation name value: `struct.us.40act.open-end`
- jurisdiction value: `US`
- Number of top-level slots: `12`
- Slot names:
  - `cbu` | type=`cbu` | cardinality=`root` | entity_kinds=`-` | state_machine=`-` | depends_on=`-` | verb count=`3`
  - `investment_adviser` | type=`entity` | cardinality=`mandatory` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `custodian` | type=`entity` | cardinality=`mandatory` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `sub_adviser` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `administrator` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `transfer_agent` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `distributor` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `auditor` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `legal_counsel` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `ownership_chain` | type=`entity_graph` | cardinality=`recursive` | entity_kinds=`person, company` | state_machine=`ubo_epistemic_lifecycle` | depends_on=`investment_adviser` | verb count=`6`
  - `case` | type=`case` | cardinality=`optional` | entity_kinds=`-` | state_machine=`kyc_case_lifecycle` | depends_on=`investment_adviser` | verb count=`5`
  - `mandate` | type=`mandate` | cardinality=`optional` | entity_kinds=`-` | state_machine=`-` | depends_on=`cbu (min_state=filled), case (min_state=intake)` | verb count=`1`

- File path: `rust/config/sem_os_seeds/constellation_maps/struct_us_etf_40act.yaml`
- constellation name value: `struct.us.etf.40act`
- jurisdiction value: `US`
- Number of top-level slots: `13`
- Slot names:
  - `cbu` | type=`cbu` | cardinality=`root` | entity_kinds=`-` | state_machine=`-` | depends_on=`-` | verb count=`3`
  - `investment_adviser` | type=`entity` | cardinality=`mandatory` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `custodian` | type=`entity` | cardinality=`mandatory` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `authorized_participant` | type=`entity` | cardinality=`mandatory` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `sub_adviser` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `administrator` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `transfer_agent` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `distributor` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `auditor` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `market_maker` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `ownership_chain` | type=`entity_graph` | cardinality=`recursive` | entity_kinds=`person, company` | state_machine=`ubo_epistemic_lifecycle` | depends_on=`investment_adviser` | verb count=`6`
  - `case` | type=`case` | cardinality=`optional` | entity_kinds=`-` | state_machine=`kyc_case_lifecycle` | depends_on=`investment_adviser` | verb count=`5`
  - `mandate` | type=`mandate` | cardinality=`optional` | entity_kinds=`-` | state_machine=`-` | depends_on=`cbu (min_state=filled), case (min_state=intake)` | verb count=`1`

- File path: `rust/config/sem_os_seeds/constellation_maps/struct_us_private_fund_delaware_lp.yaml`
- constellation name value: `struct.us.private-fund.delaware-lp`
- jurisdiction value: `US`
- Number of top-level slots: `12`
- Slot names:
  - `cbu` | type=`cbu` | cardinality=`root` | entity_kinds=`-` | state_machine=`-` | depends_on=`-` | verb count=`3`
  - `general_partner` | type=`entity` | cardinality=`mandatory` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `investment_manager` | type=`entity` | cardinality=`mandatory` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `custodian` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `administrator` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `prime_broker` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `auditor` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `legal_counsel` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `tax_advisor` | type=`entity` | cardinality=`optional` | entity_kinds=`company, person` | state_machine=`entity_kyc_lifecycle` | depends_on=`cbu` | verb count=`5`
  - `ownership_chain` | type=`entity_graph` | cardinality=`recursive` | entity_kinds=`person, company` | state_machine=`ubo_epistemic_lifecycle` | depends_on=`general_partner` | verb count=`6`
  - `case` | type=`case` | cardinality=`optional` | entity_kinds=`-` | state_machine=`kyc_case_lifecycle` | depends_on=`general_partner` | verb count=`5`
  - `mandate` | type=`mandate` | cardinality=`optional` | entity_kinds=`-` | state_machine=`-` | depends_on=`cbu (min_state=filled), case (min_state=intake)` | verb count=`1`

- File path: `rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml`
- constellation name value: `trading.streetside`
- jurisdiction value: `ALL`
- Number of top-level slots: `11`
- Slot names:
  - `cbu` | type=`cbu` | cardinality=`root` | entity_kinds=`-` | state_machine=`-` | depends_on=`-` | verb count=`1`
  - `trading_profile` | type=`mandate` | cardinality=`optional` | entity_kinds=`-` | state_machine=`trading_profile_lifecycle` | depends_on=`cbu` | verb count=`38`
  - `custody` | type=`entity` | cardinality=`optional` | entity_kinds=`cbu` | state_machine=`-` | depends_on=`trading_profile` | verb count=`8`
  - `booking_principal` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`-` | depends_on=`cbu` | verb count=`9`
  - `cash_sweep` | type=`entity` | cardinality=`optional` | entity_kinds=`entity` | state_machine=`-` | depends_on=`custody` | verb count=`9`
  - `service_resource` | type=`entity` | cardinality=`optional` | entity_kinds=`entity` | state_machine=`-` | depends_on=`cbu` | verb count=`8`
  - `service_intent` | type=`entity` | cardinality=`optional` | entity_kinds=`entity` | state_machine=`-` | depends_on=`cbu` | verb count=`12`
  - `booking_location` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`-` | depends_on=`booking_principal` | verb count=`3`
  - `legal_entity` | type=`entity` | cardinality=`optional` | entity_kinds=`company` | state_machine=`-` | depends_on=`booking_principal` | verb count=`3`
  - `product` | type=`entity` | cardinality=`optional` | entity_kinds=`entity` | state_machine=`-` | depends_on=`cbu` | verb count=`2`
  - `delivery` | type=`entity` | cardinality=`optional` | entity_kinds=`entity` | state_machine=`-` | depends_on=`cbu` | verb count=`3`

## 2. State Machine Definitions
- File path: `rust/config/sem_os_seeds/state_machines/client_group_lifecycle.yaml`
- State machine name: `client_group_lifecycle`
- States:
  - `prospect`
  - `researching`
  - `ubo_mapped`
  - `control_mapped`
  - `cbus_identified`
  - `onboarding`
  - `active`
  - `dormant`
  - `offboarded`
- Transitions:
  - `prospect -> researching` trigger=`gleif.import-tree, client-group.research`
  - `researching -> ubo_mapped` trigger=`ubo.discover, ubo.allege`
  - `ubo_mapped -> control_mapped` trigger=`control.build-graph, ownership.trace-chain`
  - `control_mapped -> cbus_identified` trigger=`cbu.create, cbu.create-from-client-group`
  - `cbus_identified -> onboarding` trigger=`kyc-case.create, kyc.open-case`
  - `onboarding -> active` trigger=`kyc-case.update-status`
  - `active -> dormant` trigger=`client-group.suspend`
  - `dormant -> active` trigger=`client-group.reactivate`
  - `active -> offboarded` trigger=`client-group.offboard`
  - `ubo_mapped -> researching` trigger=`ubo.reset`
  - `control_mapped -> ubo_mapped` trigger=`control.reset`

- File path: `rust/config/sem_os_seeds/state_machines/deal_lifecycle.yaml`
- State machine name: `deal_lifecycle`
- States:
  - `prospect`
  - `qualifying`
  - `negotiating`
  - `contracted`
  - `onboarding`
  - `active`
  - `winding_down`
  - `offboarded`
  - `cancelled`
- Transitions:
  - `prospect -> qualifying` trigger=`deal.create, deal.update-status`
  - `qualifying -> negotiating` trigger=`deal.update-status, deal.create-rate-card`
  - `negotiating -> contracted` trigger=`deal.update-status, deal.agree-rate-card`
  - `contracted -> onboarding` trigger=`deal.update-status, deal.request-onboarding`
  - `onboarding -> active` trigger=`deal.update-status`
  - `active -> winding_down` trigger=`deal.update-status`
  - `winding_down -> offboarded` trigger=`deal.update-status`
  - `prospect -> cancelled` trigger=`deal.cancel`
  - `qualifying -> cancelled` trigger=`deal.cancel`
  - `negotiating -> cancelled` trigger=`deal.cancel`
  - `contracted -> cancelled` trigger=`deal.cancel`
  - `onboarding -> cancelled` trigger=`deal.cancel`

- File path: `rust/config/sem_os_seeds/state_machines/document_lifecycle.yaml`
- State machine name: `document_lifecycle`
- States:
  - `missing`
  - `requested`
  - `received`
  - `in_qa`
  - `verified`
  - `rejected`
  - `waived`
  - `expired`
- Transitions:
  - `missing -> requested` trigger=`document.solicit, document.solicit-set`
  - `missing -> waived` trigger=`requirement.waive`
  - `requested -> received` trigger=`document.upload`
  - `received -> in_qa` trigger=`document.review`
  - `in_qa -> verified` trigger=`document.verify`
  - `in_qa -> rejected` trigger=`document.reject`
  - `rejected -> requested` trigger=`document.solicit`
  - `verified -> expired` trigger=`document.expire`
  - `expired -> requested` trigger=`document.solicit`
  - `waived -> missing` trigger=`requirement.reinstate`
  - `requested -> missing` trigger=`document.cancel-request`

- File path: `rust/config/sem_os_seeds/state_machines/entity_kyc_lifecycle.yaml`
- State machine name: `entity_kyc_lifecycle`
- States:
  - `approved`
  - `verified`
  - `evidence_collected`
  - `screening_complete`
  - `workstream_open`
  - `filled`
  - `placeholder`
  - `empty`
- Transitions:
  - `empty -> placeholder` trigger=`entity.ensure-or-placeholder`
  - `empty -> filled` trigger=`party.add, cbu.assign-role`
  - `placeholder -> filled` trigger=`party.search, entity.identify, cbu.assign-role`
  - `filled -> workstream_open` trigger=`kyc-workstream.add`
  - `workstream_open -> screening_complete` trigger=`screening.run`
  - `workstream_open -> evidence_collected` trigger=`evidence.verify`
  - `screening_complete -> verified` trigger=`kyc-workstream.close`
  - `evidence_collected -> verified` trigger=`kyc-workstream.close`
  - `verified -> approved` trigger=`case.approve`

- File path: `rust/config/sem_os_seeds/state_machines/fund_lifecycle.yaml`
- State machine name: `fund_lifecycle`
- States:
  - `draft`
  - `registered`
  - `authorized`
  - `active`
  - `soft_closed`
  - `hard_closed`
  - `winding_down`
  - `terminated`
- Transitions:
  - `draft -> registered` trigger=`fund.create, fund.ensure`
  - `registered -> authorized` trigger=`fund.upsert-vehicle`
  - `authorized -> active` trigger=`fund.upsert-vehicle`
  - `active -> soft_closed` trigger=`fund.upsert-vehicle`
  - `soft_closed -> active` trigger=`fund.upsert-vehicle`
  - `soft_closed -> hard_closed` trigger=`fund.upsert-vehicle`
  - `hard_closed -> winding_down` trigger=`fund.upsert-vehicle`
  - `winding_down -> terminated` trigger=`fund.delete-vehicle`

- File path: `rust/config/sem_os_seeds/state_machines/kyc_case_lifecycle.yaml`
- State machine name: `kyc_case_lifecycle`
- States:
  - `intake`
  - `discovery`
  - `assessment`
  - `review`
  - `blocked`
  - `approved`
  - `rejected`
  - `withdrawn`
  - `expired`
  - `refer_to_regulator`
  - `do_not_onboard`
- Transitions:
  - `intake -> discovery` trigger=`kyc-case.update-status`
  - `discovery -> assessment` trigger=`kyc-case.update-status`
  - `assessment -> review` trigger=`kyc-case.update-status, kyc-case.set-risk-rating`
  - `review -> approved` trigger=`kyc-case.update-status`
  - `review -> rejected` trigger=`kyc-case.close`
  - `review -> refer_to_regulator` trigger=`kyc-case.escalate`
  - `blocked -> discovery` trigger=`kyc-case.reopen`
  - `blocked -> withdrawn` trigger=`kyc-case.close`
  - `approved -> review` trigger=`kyc-case.reopen`

- File path: `rust/config/sem_os_seeds/state_machines/screening_lifecycle.yaml`
- State machine name: `screening_lifecycle`
- States:
  - `not_started`
  - `sanctions_pending`
  - `sanctions_clear`
  - `sanctions_hit`
  - `pep_pending`
  - `pep_clear`
  - `pep_hit`
  - `media_pending`
  - `media_clear`
  - `media_hit`
  - `all_clear`
  - `escalated`
  - `resolved`
- Transitions:
  - `not_started -> sanctions_pending` trigger=`screening.run, screening.sanctions`
  - `not_started -> pep_pending` trigger=`screening.pep`
  - `not_started -> media_pending` trigger=`screening.adverse-media`
  - `sanctions_pending -> sanctions_clear` trigger=`screening.update-status`
  - `sanctions_pending -> sanctions_hit` trigger=`screening.update-status`
  - `sanctions_clear -> pep_pending` trigger=`screening.pep`
  - `pep_pending -> pep_clear` trigger=`screening.update-status`
  - `pep_pending -> pep_hit` trigger=`screening.update-status`
  - `pep_clear -> media_pending` trigger=`screening.adverse-media`
  - `media_pending -> media_clear` trigger=`screening.update-status`
  - `media_pending -> media_hit` trigger=`screening.update-status`
  - `media_clear -> all_clear` trigger=`screening.complete`
  - `sanctions_hit -> escalated` trigger=`screening.escalate`
  - `pep_hit -> escalated` trigger=`screening.escalate`
  - `media_hit -> escalated` trigger=`screening.escalate`
  - `escalated -> resolved` trigger=`screening.resolve`
  - `all_clear -> not_started` trigger=`screening.run`

- File path: `rust/config/sem_os_seeds/state_machines/trading_profile_lifecycle.yaml`
- State machine name: `trading_profile_lifecycle`
- States:
  - `draft`
  - `submitted`
  - `approved`
  - `active`
  - `suspended`
  - `archived`
  - `rejected`
- Transitions:
  - `draft -> submitted` trigger=`trading-profile.submit`
  - `submitted -> approved` trigger=`trading-profile.approve`
  - `submitted -> rejected` trigger=`trading-profile.reject`
  - `approved -> active` trigger=`trading-profile.activate`
  - `active -> suspended` trigger=`trading-profile.archive`
  - `suspended -> active` trigger=`trading-profile.activate`
  - `active -> archived` trigger=`trading-profile.archive`
  - `rejected -> draft` trigger=`trading-profile.create-draft`
  - `submitted -> draft` trigger=`trading-profile.create-draft`

- File path: `rust/config/sem_os_seeds/state_machines/ubo_epistemic_lifecycle.yaml`
- State machine name: `ubo_epistemic_lifecycle`
- States:
  - `approved`
  - `proved`
  - `provable`
  - `alleged`
  - `undiscovered`
- Transitions:
  - `undiscovered -> alleged` trigger=`ubo.allege`
  - `alleged -> provable` trigger=`ubo.collect-evidence`
  - `provable -> proved` trigger=`ubo.verify`
  - `proved -> approved` trigger=`case.approve`

- File path: `rust/config/workflows/cbu_creation.yaml`
- State machine name: `cbu_creation`
- States:
  - `DRAFT`
  - `DATA_CAPTURE`
  - `STRUCTURE_SETUP`
  - `READY_FOR_KYC`
  - `CANCELLED`
- Transitions:
  - `DRAFT -> DATA_CAPTURE` trigger=`-`
  - `DATA_CAPTURE -> STRUCTURE_SETUP` trigger=`-`
  - `STRUCTURE_SETUP -> READY_FOR_KYC` trigger=`-`
  - `DRAFT -> CANCELLED` trigger=`-`
  - `DATA_CAPTURE -> CANCELLED` trigger=`-`
  - `STRUCTURE_SETUP -> CANCELLED` trigger=`-`

- File path: `rust/config/workflows/enhanced_due_diligence.yaml`
- State machine name: `enhanced_due_diligence`
- States:
  - `EDD_INITIATED`
  - `EXTENDED_ALLEGATION`
  - `SOURCE_OF_WEALTH`
  - `SOURCE_OF_FUNDS`
  - `EXTENDED_PROOF_COLLECTION`
  - `CORROBORATION`
  - `EXTENDED_VERIFICATION`
  - `CONVERGENCE_CHECK`
  - `SENIOR_ASSERTION`
  - `EXTENDED_EVALUATION`
  - `COMMITTEE_REVIEW`
  - `EDD_DECISION`
  - `EDD_REJECTED`
  - `ONGOING_MONITORING`
- Transitions:
  - `EDD_INITIATED -> EXTENDED_ALLEGATION` trigger=`-`
  - `EXTENDED_ALLEGATION -> SOURCE_OF_WEALTH` trigger=`-`
  - `SOURCE_OF_WEALTH -> SOURCE_OF_FUNDS` trigger=`-`
  - `SOURCE_OF_FUNDS -> EXTENDED_PROOF_COLLECTION` trigger=`-`
  - `EXTENDED_PROOF_COLLECTION -> CORROBORATION` trigger=`-`
  - `CORROBORATION -> EXTENDED_VERIFICATION` trigger=`-`
  - `EXTENDED_VERIFICATION -> CONVERGENCE_CHECK` trigger=`-`
  - `CONVERGENCE_CHECK -> SENIOR_ASSERTION` trigger=`-`
  - `CONVERGENCE_CHECK -> EXTENDED_ALLEGATION` trigger=`-`
  - `SENIOR_ASSERTION -> EXTENDED_EVALUATION` trigger=`-`
  - `SENIOR_ASSERTION -> EDD_REJECTED` trigger=`-`
  - `EXTENDED_EVALUATION -> COMMITTEE_REVIEW` trigger=`-`
  - `EXTENDED_EVALUATION -> EDD_DECISION` trigger=`-`
  - `COMMITTEE_REVIEW -> EDD_DECISION` trigger=`-`
  - `COMMITTEE_REVIEW -> EDD_REJECTED` trigger=`-`
  - `EDD_DECISION -> ONGOING_MONITORING` trigger=`-`

- File path: `rust/config/workflows/kyc_case.yaml`
- State machine name: `kyc_case`
- States:
  - `OPENED`
  - `ASSIGNED`
  - `DATA_GATHERING`
  - `SCREENING`
  - `ANALYSIS`
  - `ESCALATED`
  - `PENDING_APPROVAL`
  - `APPROVED`
  - `REJECTED`
  - `ON_HOLD`
  - `CLOSED_NO_ACTION`
- Transitions:
  - `OPENED -> ASSIGNED` trigger=`-`
  - `ASSIGNED -> DATA_GATHERING` trigger=`-`
  - `DATA_GATHERING -> SCREENING` trigger=`-`
  - `SCREENING -> ANALYSIS` trigger=`-`
  - `ANALYSIS -> PENDING_APPROVAL` trigger=`-`
  - `PENDING_APPROVAL -> APPROVED` trigger=`-`
  - `PENDING_APPROVAL -> REJECTED` trigger=`-`
  - `ANALYSIS -> ESCALATED` trigger=`-`
  - `ESCALATED -> ANALYSIS` trigger=`-`
  - `ESCALATED -> PENDING_APPROVAL` trigger=`-`
  - `ESCALATED -> REJECTED` trigger=`-`
  - `DATA_GATHERING -> ON_HOLD` trigger=`-`
  - `SCREENING -> ON_HOLD` trigger=`-`
  - `ANALYSIS -> ON_HOLD` trigger=`-`
  - `ON_HOLD -> DATA_GATHERING` trigger=`-`
  - `ON_HOLD -> SCREENING` trigger=`-`
  - `ON_HOLD -> ANALYSIS` trigger=`-`
  - `OPENED -> CLOSED_NO_ACTION` trigger=`-`
  - `ASSIGNED -> CLOSED_NO_ACTION` trigger=`-`
  - `ON_HOLD -> CLOSED_NO_ACTION` trigger=`-`

- File path: `rust/config/workflows/kyc_convergence.yaml`
- State machine name: `kyc_convergence`
- States:
  - `INTAKE`
  - `ALLEGATION_COMPLETE`
  - `PROOF_COLLECTION`
  - `OBSERVATION_EXTRACTION`
  - `VERIFICATION`
  - `CONVERGENCE_CHECK`
  - `ASSERTION_GATE`
  - `EVALUATION`
  - `DECISION`
  - `REMEDIATION`
  - `BLOCKED`
- Transitions:
  - `INTAKE -> ALLEGATION_COMPLETE` trigger=`-`
  - `ALLEGATION_COMPLETE -> PROOF_COLLECTION` trigger=`-`
  - `PROOF_COLLECTION -> OBSERVATION_EXTRACTION` trigger=`-`
  - `OBSERVATION_EXTRACTION -> VERIFICATION` trigger=`-`
  - `VERIFICATION -> CONVERGENCE_CHECK` trigger=`-`
  - `CONVERGENCE_CHECK -> ASSERTION_GATE` trigger=`-`
  - `CONVERGENCE_CHECK -> REMEDIATION` trigger=`-`
  - `REMEDIATION -> PROOF_COLLECTION` trigger=`-`
  - `ASSERTION_GATE -> EVALUATION` trigger=`-`
  - `ASSERTION_GATE -> BLOCKED` trigger=`-`
  - `EVALUATION -> DECISION` trigger=`-`
  - `VERIFICATION -> BLOCKED` trigger=`-`
  - `REMEDIATION -> BLOCKED` trigger=`-`

- File path: `rust/config/workflows/kyc_onboarding.yaml`
- State machine name: `kyc_onboarding`
- States:
  - `INTAKE`
  - `ENTITY_COLLECTION`
  - `SCREENING`
  - `DOCUMENT_COLLECTION`
  - `UBO_DETERMINATION`
  - `REVIEW`
  - `APPROVED`
  - `REJECTED`
  - `REMEDIATION`
- Transitions:
  - `INTAKE -> ENTITY_COLLECTION` trigger=`-`
  - `ENTITY_COLLECTION -> SCREENING` trigger=`-`
  - `SCREENING -> DOCUMENT_COLLECTION` trigger=`-`
  - `DOCUMENT_COLLECTION -> UBO_DETERMINATION` trigger=`-`
  - `UBO_DETERMINATION -> REVIEW` trigger=`-`
  - `REVIEW -> APPROVED` trigger=`-`
  - `REVIEW -> REJECTED` trigger=`-`
  - `REVIEW -> REMEDIATION` trigger=`-`
  - `REMEDIATION -> ENTITY_COLLECTION` trigger=`-`

- File path: `rust/config/workflows/periodic_review.yaml`
- State machine name: `periodic_review`
- States:
  - `SCHEDULED`
  - `INITIATED`
  - `DATA_REFRESH`
  - `SCREENING_REFRESH`
  - `CHANGE_ANALYSIS`
  - `REVIEW_COMPLETE`
  - `ESCALATED_TO_FULL_REVIEW`
  - `DEFERRED`
- Transitions:
  - `SCHEDULED -> INITIATED` trigger=`-`
  - `INITIATED -> DATA_REFRESH` trigger=`-`
  - `DATA_REFRESH -> SCREENING_REFRESH` trigger=`-`
  - `SCREENING_REFRESH -> CHANGE_ANALYSIS` trigger=`-`
  - `CHANGE_ANALYSIS -> REVIEW_COMPLETE` trigger=`-`
  - `CHANGE_ANALYSIS -> ESCALATED_TO_FULL_REVIEW` trigger=`-`
  - `SCHEDULED -> DEFERRED` trigger=`-`
  - `DEFERRED -> SCHEDULED` trigger=`-`

- File path: `rust/config/workflows/ubo_determination.yaml`
- State machine name: `ubo_determination`
- States:
  - `INITIATED`
  - `OWNERSHIP_MAPPING`
  - `CHAIN_TRACING`
  - `UBO_IDENTIFICATION`
  - `VERIFICATION`
  - `DOCUMENTATION`
  - `COMPLETE`
  - `BLOCKED`
- Transitions:
  - `INITIATED -> OWNERSHIP_MAPPING` trigger=`-`
  - `OWNERSHIP_MAPPING -> CHAIN_TRACING` trigger=`-`
  - `CHAIN_TRACING -> UBO_IDENTIFICATION` trigger=`-`
  - `UBO_IDENTIFICATION -> VERIFICATION` trigger=`-`
  - `VERIFICATION -> DOCUMENTATION` trigger=`-`
  - `DOCUMENTATION -> COMPLETE` trigger=`-`
  - `OWNERSHIP_MAPPING -> BLOCKED` trigger=`-`
  - `CHAIN_TRACING -> BLOCKED` trigger=`-`
  - `VERIFICATION -> BLOCKED` trigger=`-`

- File path: `sem_os_core/src/seeds.rs`
- State machine name: `StateMachineSeed`
- States:
  - `N/A (type definition only)`
- Transitions:
  - `N/A (type definition only)`

- File path: `sem_os_core/src/state_machine_def.rs`
- State machine name: `StateMachineDefBody`
- States:
  - `N/A (type definition only)`
- Transitions:
  - `N/A (type definition only)`

- File path: `sem_os_runtime/constellation_runtime.rs`
- State machine name: `RuntimeStateMachine`
- States:
  - `N/A (type definition only)`
- Transitions:
  - `N/A (type definition only)`

## 3. Entity Types
- Variant name: `cbu`
- Database table it maps to: `UNRESOLVED`
- Has state machine?: `N`
- Appears in constellation?: `Y`

- Variant name: `company`
- Database table it maps to: `UNRESOLVED`
- Has state machine?: `Y`
- Appears in constellation?: `Y`

- Variant name: `contract`
- Database table it maps to: `UNRESOLVED`
- Has state machine?: `N`
- Appears in constellation?: `Y`

- Variant name: `entity`
- Database table it maps to: `UNRESOLVED`
- Has state machine?: `N`
- Appears in constellation?: `Y`

- Variant name: `fund`
- Database table it maps to: `UNRESOLVED`
- Has state machine?: `N`
- Appears in constellation?: `Y`

- Variant name: `person`
- Database table it maps to: `UNRESOLVED`
- Has state machine?: `Y`
- Appears in constellation?: `Y`

## 4. Slot Detail Table
### `deal.lifecycle`
| Slot | Type | Cardinality | Entity Kinds | Depends On | State Machine | Verb Count | Verb FQNs |
| --- | --- | --- | --- | --- | --- | ---: | --- |
| `deal` | `cbu` | `root` | `-` | `-` | `deal_lifecycle` | `20` | `deal.create, deal.read-record, deal.list, deal.search-records, deal.read-summary, deal.read-timeline, deal.list-documents, deal.list-slas, deal.list-active-rate-cards, deal.list-rate-card-lines, deal.list-rate-card-history, deal.update-record, deal.update-status, deal.add-document, deal.update-document-status, deal.add-sla, deal.remove-sla, deal.add-ubo-assessment, deal.update-ubo-assessment, deal.cancel` |
| `participant` | `entity` | `optional` | `person` | `deal` | `-` | `3` | `deal.add-participant, deal.remove-participant, deal.list-participants` |
| `deal_contract` | `entity` | `optional` | `contract` | `deal` | `-` | `3` | `deal.add-contract, deal.remove-contract, deal.list-contracts` |
| `contract` | `entity` | `optional` | `contract` | `deal` | `-` | `14` | `contract.create, contract.get, contract.list, contract.list-products, contract.list-rate-cards, contract.list-subscriptions, contract.for-client, contract.update, contract.add-product, contract.remove-product, contract.create-rate-card, contract.subscribe, contract.unsubscribe, contract.terminate` |
| `deal_product` | `entity` | `optional` | `entity` | `deal` | `-` | `4` | `deal.add-product, deal.update-product-status, deal.remove-product, deal.list-products` |
| `rate_card` | `entity` | `optional` | `entity` | `deal_product` | `-` | `11` | `deal.create-rate-card, deal.add-rate-card-line, deal.update-rate-card-line, deal.remove-rate-card-line, deal.list-rate-cards, deal.list-rate-card-lines, deal.list-rate-card-history, deal.list-active-rate-cards, deal.propose-rate-card, deal.counter-rate-card, deal.agree-rate-card` |
| `onboarding_request` | `entity` | `optional` | `entity` | `deal (min_state=contracted)` | `-` | `4` | `deal.request-onboarding, deal.request-onboarding-batch, deal.update-onboarding-status, deal.list-onboarding-requests` |
| `billing_profile` | `entity` | `optional` | `entity` | `rate_card` | `-` | `17` | `billing.create-profile, billing.activate-profile, billing.suspend-profile, billing.close-profile, billing.get-profile, billing.list-profiles, billing.add-account-target, billing.remove-account-target, billing.list-account-targets, billing.create-period, billing.calculate-period, billing.review-period, billing.approve-period, billing.generate-invoice, billing.dispute-period, billing.period-summary, billing.revenue-summary` |
| `pricing` | `entity` | `optional` | `entity` | `rate_card` | `-` | `12` | `pricing-config.set-valuation-schedule, pricing-config.set-nav-threshold, pricing-config.set-settlement-calendar, pricing-config.set-holiday-schedule, pricing-config.set-reporting, pricing-config.set-tax-status, pricing-config.set-reclaim-config, pricing-config.find-for-instrument, pricing-config.list-jurisdictions, pricing-config.list-treaty-rates, pricing-config.list-tax-status, pricing-config.list-reclaim-configs` |
| `contract_template` | `entity` | `optional` | `contract` | `contract` | `-` | `2` | `contract-pack.create, contract-pack.read` |

### `fund.administration`
| Slot | Type | Cardinality | Entity Kinds | Depends On | State Machine | Verb Count | Verb FQNs |
| --- | --- | --- | --- | --- | --- | ---: | --- |
| `fund` | `cbu` | `root` | `-` | `-` | `fund_lifecycle` | `7` | `fund.create, fund.ensure, fund.read-vehicle, fund.list-by-manager, fund.list-by-vehicle-type, fund.upsert-vehicle, fund.delete-vehicle` |
| `umbrella` | `entity` | `optional` | `fund` | `fund` | `-` | `6` | `fund.add-to-umbrella, fund.list-subfunds, fund.upsert-compartment, fund.read-compartment, fund.list-compartments-by-umbrella, fund.delete-compartment` |
| `share_class` | `entity` | `optional` | `fund` | `fund` | `-` | `2` | `fund.add-share-class, fund.list-share-classes` |
| `feeder` | `entity` | `optional` | `fund` | `fund` | `-` | `2` | `fund.link-feeder, fund.list-feeders` |
| `investment` | `entity` | `optional` | `entity` | `fund` | `-` | `5` | `fund.add-investment, fund.update-investment, fund.end-investment, fund.list-investments, fund.list-investors` |
| `capital` | `entity` | `optional` | `fund` | `fund` | `-` | `30` | `capital.allocate, capital.issue.initial, capital.issue.new, capital.issue-shares, capital.cancel-shares, capital.transfer, capital.split, capital.buyback, capital.cancel, capital.reconcile, capital.cap-table, capital.holders, capital.list-by-issuer, capital.list-shareholders, capital.get-ownership-chain, capital.define-share-class, capital.share-class.create, capital.share-class.list, capital.share-class.get-supply, capital.share-class.add-identifier, capital.control-config.get, capital.control-config.set, capital.dilution.grant-options, capital.dilution.issue-warrant, capital.dilution.create-safe, capital.dilution.create-convertible-note, capital.dilution.exercise, capital.dilution.forfeit, capital.dilution.list, capital.dilution.get-summary` |
| `investment_manager` | `entity` | `optional` | `company` | `fund` | `-` | `7` | `investment-manager.assign, investment-manager.set-scope, investment-manager.link-connectivity, investment-manager.list, investment-manager.suspend, investment-manager.terminate, investment-manager.find-for-trade` |
| `manco_group` | `entity` | `optional` | `company` | `fund` | `-` | `16` | `manco.create, manco.read, manco.list, manco.derive-groups, manco.bridge-roles, manco.list-members, manco.list-roles, manco.assign-role, manco.remove-role, manco.link-entity, manco.unlink-entity, manco.set-regulatory-status, manco.list-managed-funds, manco.verify, manco.compute-control-chain, manco.refresh` |
| `trust` | `entity` | `optional` | `entity` | `fund` | `-` | `8` | `trust.create, trust.read, trust.list, trust.add-trustee, trust.remove-trustee, trust.add-beneficiary, trust.add-settlor, trust.identify-ubos` |
| `partnership` | `entity` | `optional` | `entity` | `fund` | `-` | `7` | `partnership.create, partnership.read, partnership.list, partnership.add-partner, partnership.remove-partner, partnership.set-general-partner, partnership.list-partners` |

### `governance.compliance`
| Slot | Type | Cardinality | Entity Kinds | Depends On | State Machine | Verb Count | Verb FQNs |
| --- | --- | --- | --- | --- | --- | ---: | --- |
| `group` | `cbu` | `root` | `-` | `-` | `-` | `0` | `-` |
| `sla` | `entity` | `optional` | `contract` | `group` | `-` | `22` | `sla.create, sla.read, sla.read-template, sla.list, sla.list-templates, sla.list-commitments, sla.list-measurements, sla.list-breaches, sla.list-open-breaches, sla.update, sla.bind, sla.commit, sla.record-measurement, sla.activate, sla.suspend, sla.suspend-commitment, sla.renew, sla.record-breach, sla.report-breach, sla.escalate-breach, sla.resolve-breach, sla.update-remediation` |
| `access_review` | `entity` | `optional` | `entity` | `group` | `-` | `21` | `access-review.create, access-review.create-campaign, access-review.read, access-review.list, access-review.list-items, access-review.list-flagged, access-review.my-pending, access-review.campaign-status, access-review.audit-report, access-review.populate-campaign, access-review.launch-campaign, access-review.send-reminders, access-review.process-deadline, access-review.attest, access-review.extend-access, access-review.revoke-access, access-review.escalate-item, access-review.start, access-review.complete, access-review.approve, access-review.reject` |
| `regulatory` | `entity` | `optional` | `entity` | `group` | `-` | `10` | `regulatory.create, regulatory.registration.add, regulatory.read, regulatory.list, regulatory.registration.list, regulatory.registration.check, regulatory.registration.verify, regulatory.update, regulatory.submit, regulatory.registration.remove` |
| `ruleset` | `entity` | `optional` | `entity` | `group` | `-` | `4` | `ruleset.create, ruleset.read, ruleset.publish, ruleset.retire` |
| `delegation` | `entity` | `optional` | `entity` | `group` | `-` | `8` | `delegation.create, delegation.add, delegation.read, delegation.list, delegation.list-delegates, delegation.list-delegations-received, delegation.end, delegation.revoke` |
| `team` | `entity` | `optional` | `person` | `group` | `-` | `20` | `team.add-member, team.remove-member, team.list-members, team.list, team.create, team.read, team.update, team.assign-role, team.remove-role, team.transfer-member, team.list-by-role, team.set-lead, team.add-governance-member, team.remove-governance-member, team.list-governance-members, team.add-ops-member, team.remove-ops-member, team.list-ops-members, team.assign-capacity, team.list-capacity` |
| `rule` | `entity` | `optional` | `entity` | `ruleset` | `-` | `3` | `rule.create, rule.read, rule.update` |
| `rule_field` | `entity` | `optional` | `entity` | `ruleset` | `-` | `2` | `rule-field.list, rule-field.read` |

### `group.ownership`
| Slot | Type | Cardinality | Entity Kinds | Depends On | State Machine | Verb Count | Verb FQNs |
| --- | --- | --- | --- | --- | --- | ---: | --- |
| `client_group` | `cbu` | `root` | `-` | `-` | `client_group_lifecycle` | `24` | `client-group.create, client-group.read, client-group.research, client-group.update, client-group.set-canonical, client-group.start-discovery, client-group.discover-entities, client-group.complete-discovery, client-group.entity-add, client-group.entity-remove, client-group.list-entities, client-group.search-entities, client-group.list-parties, client-group.list-unverified, client-group.list-discrepancies, client-group.verify-ownership, client-group.reject-entity, client-group.assign-role, client-group.remove-role, client-group.list-roles, client-group.add-relationship, client-group.list-relationships, client-group.tag-add, client-group.tag-remove` |
| `gleif_import` | `entity` | `optional` | `company` | `client_group` | `-` | `16` | `gleif.import-tree, gleif.import-to-client-group, gleif.import-managed-funds, gleif.search, gleif.refresh, gleif.enrich, gleif.get-record, gleif.get-parent, gleif.get-children, gleif.get-manager, gleif.get-managed-funds, gleif.get-master-fund, gleif.get-umbrella, gleif.lookup-by-isin, gleif.resolve-successor, gleif.trace-ownership` |
| `ubo_discovery` | `entity_graph` | `recursive` | `person, company` | `gleif_import` | `ubo_epistemic_lifecycle` | `32` | `ubo.discover, ubo.allege, ubo.calculate, ubo.compute-chains, ubo.trace-chains, ubo.verify, ubo.promote, ubo.approve, ubo.reject, ubo.list, ubo.list-ubos, ubo.list-owned, ubo.list-owners, ubo.add-ownership, ubo.update-ownership, ubo.add-control, ubo.transfer-control, ubo.add-trust-role, ubo.delete-relationship, ubo.end-relationship, ubo.waive-verification, ubo.mark-deceased, ubo.mark-terminus, ubo.convergence-supersede, ubo.registry.create, ubo.registry.advance, ubo.registry.promote, ubo.registry.reject, ubo.registry.expire, ubo.registry.waive, ubo.snapshot.capture, ubo.snapshot.diff` |
| `control_chain` | `entity_graph` | `recursive` | `company` | `ubo_discovery` | `-` | `35` | `ownership.trace-chain, control.build-graph, ownership.refresh, control.read, control.list-links, control.add, control.end, control.analyze, control.list-controllers, control.list-controlled, control.trace-chain, control.compute-controllers, control.identify-ubos, control.reconcile-ownership, control.set-board-controller, control.show-board-controller, control.recompute-board-controller, control.clear-board-controller-override, control.import-gleif-control, control.import-psc-register, ownership.compute, ownership.control-positions, ownership.who-controls, ownership.analyze-gaps, ownership.reconcile, ownership.reconcile.findings, ownership.reconcile.list-runs, ownership.reconcile.resolve-finding, ownership.right.add-to-class, ownership.right.add-to-holder, ownership.right.end, ownership.right.list-for-holder, ownership.right.list-for-issuer, ownership.snapshot.get, ownership.snapshot.list` |
| `cbu_identification` | `cbu` | `optional` | `-` | `control_chain` | `-` | `34` | `cbu.create, cbu.create-from-client-group, cbu.ensure, cbu.read, cbu.list, cbu.list-subscriptions, cbu.list-evidence, cbu.list-structure-links, cbu.parties, cbu.update, cbu.rename, cbu.set-jurisdiction, cbu.set-client-type, cbu.set-commercial-client, cbu.add-product, cbu.remove-product, cbu.assign-control, cbu.assign-ownership, cbu.assign-fund-role, cbu.assign-trust-role, cbu.assign-service-provider, cbu.assign-signatory, cbu.remove-role, cbu.validate-roles, cbu.attach-evidence, cbu.verify-evidence, cbu.request-proof-update, cbu.link-structure, cbu.unlink-structure, cbu.submit-for-validation, cbu.reopen-validation, cbu.decide, cbu.delete, cbu.delete-cascade` |

### `kyc.extended`
| Slot | Type | Cardinality | Entity Kinds | Depends On | State Machine | Verb Count | Verb FQNs |
| --- | --- | --- | --- | --- | --- | ---: | --- |
| `entity` | `entity` | `root` | `person, company` | `-` | `-` | `1` | `entity.read` |
| `board` | `entity` | `optional` | `person` | `entity` | `-` | `9` | `board.appoint, board.resign, board.list-by-entity, board.list-by-person, board.grant-appointment-right, board.revoke-appointment-right, board.list-appointment-rights, board.list-rights-held, board.analyze-control` |
| `bods` | `entity` | `optional` | `person, company` | `entity` | `-` | `9` | `bods.discover-ubos, bods.import, bods.link-entity, bods.get-statement, bods.list-by-entity, bods.find-by-lei, bods.list-persons, bods.list-ownership, bods.sync-from-gleif` |

### `kyc.onboarding`
| Slot | Type | Cardinality | Entity Kinds | Depends On | State Machine | Verb Count | Verb FQNs |
| --- | --- | --- | --- | --- | --- | ---: | --- |
| `cbu` | `cbu` | `root` | `-` | `-` | `-` | `1` | `cbu.show` |
| `kyc_case` | `case` | `mandatory` | `-` | `cbu` | `kyc_case_lifecycle` | `11` | `kyc-case.create, kyc.open-case, kyc-case.read, kyc-case.list-by-cbu, kyc-case.state, kyc-case.assign, kyc-case.update-status, kyc-case.set-risk-rating, kyc-case.close, kyc-case.reopen, kyc-case.escalate` |
| `kyc_case.tollgate` | `tollgate` | `optional` | `-` | `kyc_case (min_state=review)` | `-` | `11` | `tollgate.evaluate, tollgate.evaluate-gate, tollgate.read, tollgate.get-decision-readiness, tollgate.get-metrics, tollgate.list-evaluations, tollgate.list-thresholds, tollgate.set-threshold, tollgate.override, tollgate.list-overrides, tollgate.expire-override` |
| `entity_workstream` | `entity` | `optional` | `person, company` | `kyc_case` | `-` | `34` | `entity-workstream.create, entity-workstream.read, entity-workstream.list-by-case, entity-workstream.state, entity-workstream.update-status, entity-workstream.set-enhanced-dd, entity-workstream.set-ubo, entity-workstream.complete, entity-workstream.block, red-flag.raise, red-flag.read, red-flag.list, red-flag.resolve, red-flag.escalate, red-flag.update, red-flag.list-by-severity, red-flag.close, requirement.create, requirement.create-set, requirement.check, requirement.list, requirement.for-entity, requirement.unsatisfied, requirement.waive, requirement.reinstate, document.solicit, document.solicit-set, document.upload, document.verify, document.reject, document.read, document.list, document.compute-requirements, document.missing-for-entity` |
| `screening` | `entity` | `optional` | `person, company` | `entity_workstream` | `screening_lifecycle` | `13` | `screening.run, screening.sanctions, screening.pep, screening.adverse-media, screening.bulk-refresh, screening.read, screening.list, screening.list-by-workstream, screening.review-hit, screening.update-status, screening.escalate, screening.resolve, screening.complete` |
| `kyc_agreement` | `entity` | `optional` | `company` | `kyc_case` | `-` | `6` | `kyc-agreement.create, kyc-agreement.read, kyc-agreement.list, kyc-agreement.update, kyc-agreement.update-status, kyc-agreement.sign` |
| `identifier` | `entity` | `optional` | `entity` | `entity_workstream` | `-` | `11` | `identifier.add, identifier.read, identifier.list, identifier.verify, identifier.expire, identifier.update, identifier.search, identifier.resolve, identifier.list-by-type, identifier.set-primary, identifier.remove` |
| `request` | `entity` | `optional` | `entity` | `kyc_case` | `-` | `9` | `request.create, request.read, request.list, request.update, request.complete, request.cancel, request.assign, request.reopen, request.escalate` |

### `struct.hedge.cross-border`
| Slot | Type | Cardinality | Entity Kinds | Depends On | State Machine | Verb Count | Verb FQNs |
| --- | --- | --- | --- | --- | --- | ---: | --- |
| `cbu` | `cbu` | `root` | `-` | `-` | `-` | `3` | `cbu.create, cbu.read, cbu.show` |
| `cbu.us_feeder` | `cbu` | `optional` | `-` | `cbu` | `-` | `1` | `cbu.read` |
| `cbu.ie_feeder` | `cbu` | `optional` | `-` | `cbu` | `-` | `1` | `cbu.read` |
| `aifm` | `entity` | `mandatory` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `depositary` | `entity` | `mandatory` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `prime_broker` | `entity` | `mandatory` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `investment_manager` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `administrator` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `auditor` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `secondary_prime_broker` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `ownership_chain` | `entity_graph` | `recursive` | `person, company` | `aifm` | `ubo_epistemic_lifecycle` | `6` | `ubo.discover, ubo.allege, ubo.verify, ubo.promote, ubo.approve, ubo.reject` |
| `case` | `case` | `optional` | `-` | `aifm` | `kyc_case_lifecycle` | `5` | `case.open, case.submit, case.approve, case.reject, case.request-info` |
| `case.tollgate` | `tollgate` | `optional` | `-` | `case (min_state=intake)` | `-` | `1` | `tollgate.evaluate` |
| `mandate` | `mandate` | `optional` | `-` | `cbu (min_state=filled), case (min_state=intake)` | `-` | `1` | `mandate.create` |

### `struct.ie.aif.icav`
| Slot | Type | Cardinality | Entity Kinds | Depends On | State Machine | Verb Count | Verb FQNs |
| --- | --- | --- | --- | --- | --- | ---: | --- |
| `cbu` | `cbu` | `root` | `-` | `-` | `-` | `3` | `cbu.create, cbu.read, cbu.show` |
| `aifm` | `entity` | `mandatory` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `depositary` | `entity` | `mandatory` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `investment_manager` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `administrator` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `auditor` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `prime_broker` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `company_secretary` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `ownership_chain` | `entity_graph` | `recursive` | `person, company` | `aifm` | `ubo_epistemic_lifecycle` | `6` | `ubo.discover, ubo.allege, ubo.verify, ubo.promote, ubo.approve, ubo.reject` |
| `case` | `case` | `optional` | `-` | `aifm` | `kyc_case_lifecycle` | `5` | `case.open, case.submit, case.approve, case.reject, case.request-info` |
| `case.tollgate` | `tollgate` | `optional` | `-` | `case (min_state=intake)` | `-` | `1` | `tollgate.evaluate` |
| `mandate` | `mandate` | `optional` | `-` | `cbu (min_state=filled), case (min_state=intake)` | `-` | `1` | `mandate.create` |

### `struct.ie.hedge.icav`
| Slot | Type | Cardinality | Entity Kinds | Depends On | State Machine | Verb Count | Verb FQNs |
| --- | --- | --- | --- | --- | --- | ---: | --- |
| `cbu` | `cbu` | `root` | `-` | `-` | `-` | `3` | `cbu.create, cbu.read, cbu.show` |
| `aifm` | `entity` | `mandatory` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `depositary` | `entity` | `mandatory` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `investment_manager` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `administrator` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `auditor` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `prime_broker` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `secondary_prime_broker` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `executing_broker` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `company_secretary` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `ownership_chain` | `entity_graph` | `recursive` | `person, company` | `aifm` | `ubo_epistemic_lifecycle` | `6` | `ubo.discover, ubo.allege, ubo.verify, ubo.promote, ubo.approve, ubo.reject` |
| `case` | `case` | `optional` | `-` | `aifm` | `kyc_case_lifecycle` | `5` | `case.open, case.submit, case.approve, case.reject, case.request-info` |
| `case.tollgate` | `tollgate` | `optional` | `-` | `case (min_state=intake)` | `-` | `1` | `tollgate.evaluate` |
| `mandate` | `mandate` | `optional` | `-` | `cbu (min_state=filled), case (min_state=intake)` | `-` | `1` | `mandate.create` |

### `struct.ie.ucits.icav`
| Slot | Type | Cardinality | Entity Kinds | Depends On | State Machine | Verb Count | Verb FQNs |
| --- | --- | --- | --- | --- | --- | ---: | --- |
| `cbu` | `cbu` | `root` | `-` | `-` | `-` | `3` | `cbu.create, cbu.read, cbu.show` |
| `management_company` | `entity` | `mandatory` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `depositary` | `entity` | `mandatory` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `investment_manager` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `administrator` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `auditor` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `company_secretary` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `legal_counsel` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `ownership_chain` | `entity_graph` | `recursive` | `person, company` | `management_company` | `ubo_epistemic_lifecycle` | `6` | `ubo.discover, ubo.allege, ubo.verify, ubo.promote, ubo.approve, ubo.reject` |
| `case` | `case` | `optional` | `-` | `management_company` | `kyc_case_lifecycle` | `5` | `case.open, case.submit, case.approve, case.reject, case.request-info` |
| `case.tollgate` | `tollgate` | `optional` | `-` | `case (min_state=intake)` | `-` | `1` | `tollgate.evaluate` |
| `mandate` | `mandate` | `optional` | `-` | `cbu (min_state=filled), case (min_state=intake)` | `-` | `1` | `mandate.create` |

### `struct.lux.aif.raif`
| Slot | Type | Cardinality | Entity Kinds | Depends On | State Machine | Verb Count | Verb FQNs |
| --- | --- | --- | --- | --- | --- | ---: | --- |
| `cbu` | `cbu` | `root` | `-` | `-` | `-` | `3` | `cbu.create, cbu.read, cbu.show` |
| `aifm` | `entity` | `mandatory` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `depositary` | `entity` | `mandatory` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `investment_manager` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `administrator` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `auditor` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `prime_broker` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `ownership_chain` | `entity_graph` | `recursive` | `person, company` | `aifm` | `ubo_epistemic_lifecycle` | `6` | `ubo.discover, ubo.allege, ubo.verify, ubo.promote, ubo.approve, ubo.reject` |
| `case` | `case` | `optional` | `-` | `aifm` | `kyc_case_lifecycle` | `5` | `case.open, case.submit, case.approve, case.reject, case.request-info` |
| `case.tollgate` | `tollgate` | `optional` | `-` | `case (min_state=intake)` | `-` | `1` | `tollgate.evaluate` |
| `mandate` | `mandate` | `optional` | `-` | `cbu (min_state=filled), case (min_state=intake)` | `-` | `1` | `mandate.create` |

### `struct.lux.pe.scsp`
| Slot | Type | Cardinality | Entity Kinds | Depends On | State Machine | Verb Count | Verb FQNs |
| --- | --- | --- | --- | --- | --- | ---: | --- |
| `cbu` | `cbu` | `root` | `-` | `-` | `-` | `3` | `cbu.create, cbu.read, cbu.show` |
| `general_partner` | `entity` | `mandatory` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `aifm` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `depositary` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `administrator` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `auditor` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `legal_counsel` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `ownership_chain` | `entity_graph` | `recursive` | `person, company` | `general_partner` | `ubo_epistemic_lifecycle` | `6` | `ubo.discover, ubo.allege, ubo.verify, ubo.promote, ubo.approve, ubo.reject` |
| `case` | `case` | `optional` | `-` | `general_partner` | `kyc_case_lifecycle` | `5` | `case.open, case.submit, case.approve, case.reject, case.request-info` |
| `case.tollgate` | `tollgate` | `optional` | `-` | `case (min_state=intake)` | `-` | `1` | `tollgate.evaluate` |
| `mandate` | `mandate` | `optional` | `-` | `cbu (min_state=filled), case (min_state=intake)` | `-` | `1` | `mandate.create` |

### `struct.lux.ucits.sicav`
| Slot | Type | Cardinality | Entity Kinds | Depends On | State Machine | Verb Count | Verb FQNs |
| --- | --- | --- | --- | --- | --- | ---: | --- |
| `cbu` | `cbu` | `root` | `-` | `-` | `-` | `3` | `cbu.create, cbu.read, cbu.show` |
| `management_company` | `entity` | `mandatory` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `depositary` | `entity` | `mandatory` | `company` | `cbu` | `entity_kyc_lifecycle` | `4` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add` |
| `investment_manager` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `2` | `entity.ensure-or-placeholder, cbu.assign-role` |
| `ownership_chain` | `entity_graph` | `recursive` | `person, company` | `management_company` | `ubo_epistemic_lifecycle` | `6` | `ubo.discover, ubo.allege, ubo.verify, ubo.promote, ubo.approve, ubo.reject` |
| `case` | `case` | `optional` | `-` | `management_company` | `kyc_case_lifecycle` | `5` | `case.open, case.submit, case.approve, case.reject, case.request-info` |
| `case.tollgate` | `tollgate` | `optional` | `-` | `case (min_state=intake)` | `-` | `1` | `tollgate.evaluate` |
| `mandate` | `mandate` | `optional` | `-` | `cbu (min_state=filled), case (min_state=intake)` | `-` | `1` | `mandate.create` |

### `struct.pe.cross-border`
| Slot | Type | Cardinality | Entity Kinds | Depends On | State Machine | Verb Count | Verb FQNs |
| --- | --- | --- | --- | --- | --- | ---: | --- |
| `cbu` | `cbu` | `root` | `-` | `-` | `-` | `3` | `cbu.create, cbu.read, cbu.show` |
| `cbu.us_parallel` | `cbu` | `optional` | `-` | `cbu` | `-` | `1` | `cbu.read` |
| `cbu.aggregator` | `cbu` | `optional` | `-` | `cbu` | `-` | `1` | `cbu.read` |
| `general_partner` | `entity` | `mandatory` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `aifm` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `depositary` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `administrator` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `auditor` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `legal_counsel` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `ownership_chain` | `entity_graph` | `recursive` | `person, company` | `general_partner` | `ubo_epistemic_lifecycle` | `6` | `ubo.discover, ubo.allege, ubo.verify, ubo.promote, ubo.approve, ubo.reject` |
| `case` | `case` | `optional` | `-` | `general_partner` | `kyc_case_lifecycle` | `5` | `case.open, case.submit, case.approve, case.reject, case.request-info` |
| `case.tollgate` | `tollgate` | `optional` | `-` | `case (min_state=intake)` | `-` | `1` | `tollgate.evaluate` |
| `mandate` | `mandate` | `optional` | `-` | `cbu (min_state=filled), case (min_state=intake)` | `-` | `1` | `mandate.create` |

### `struct.uk.authorised.acs`
| Slot | Type | Cardinality | Entity Kinds | Depends On | State Machine | Verb Count | Verb FQNs |
| --- | --- | --- | --- | --- | --- | ---: | --- |
| `cbu` | `cbu` | `root` | `-` | `-` | `-` | `3` | `cbu.create, cbu.read, cbu.show` |
| `acs_operator` | `entity` | `mandatory` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `depositary` | `entity` | `mandatory` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `investment_manager` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `administrator` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `auditor` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `ownership_chain` | `entity_graph` | `recursive` | `person, company` | `acs_operator` | `ubo_epistemic_lifecycle` | `6` | `ubo.discover, ubo.allege, ubo.verify, ubo.promote, ubo.approve, ubo.reject` |
| `case` | `case` | `optional` | `-` | `acs_operator` | `kyc_case_lifecycle` | `5` | `case.open, case.submit, case.approve, case.reject, case.request-info` |
| `case.tollgate` | `tollgate` | `optional` | `-` | `case (min_state=intake)` | `-` | `1` | `tollgate.evaluate` |
| `mandate` | `mandate` | `optional` | `-` | `cbu (min_state=filled), case (min_state=intake)` | `-` | `1` | `mandate.create` |

### `struct.uk.authorised.aut`
| Slot | Type | Cardinality | Entity Kinds | Depends On | State Machine | Verb Count | Verb FQNs |
| --- | --- | --- | --- | --- | --- | ---: | --- |
| `cbu` | `cbu` | `root` | `-` | `-` | `-` | `3` | `cbu.create, cbu.read, cbu.show` |
| `authorised_fund_manager` | `entity` | `mandatory` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `trustee` | `entity` | `mandatory` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `investment_manager` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `administrator` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `auditor` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `ownership_chain` | `entity_graph` | `recursive` | `person, company` | `authorised_fund_manager` | `ubo_epistemic_lifecycle` | `6` | `ubo.discover, ubo.allege, ubo.verify, ubo.promote, ubo.approve, ubo.reject` |
| `case` | `case` | `optional` | `-` | `authorised_fund_manager` | `kyc_case_lifecycle` | `5` | `case.open, case.submit, case.approve, case.reject, case.request-info` |
| `case.tollgate` | `tollgate` | `optional` | `-` | `case (min_state=intake)` | `-` | `1` | `tollgate.evaluate` |
| `mandate` | `mandate` | `optional` | `-` | `cbu (min_state=filled), case (min_state=intake)` | `-` | `1` | `mandate.create` |

### `struct.uk.authorised.ltaf`
| Slot | Type | Cardinality | Entity Kinds | Depends On | State Machine | Verb Count | Verb FQNs |
| --- | --- | --- | --- | --- | --- | ---: | --- |
| `cbu` | `cbu` | `root` | `-` | `-` | `-` | `3` | `cbu.create, cbu.read, cbu.show` |
| `authorised_corporate_director` | `entity` | `mandatory` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `depositary` | `entity` | `mandatory` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `investment_manager` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `administrator` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `auditor` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `registrar` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `valuation_agent` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `ownership_chain` | `entity_graph` | `recursive` | `person, company` | `authorised_corporate_director` | `ubo_epistemic_lifecycle` | `6` | `ubo.discover, ubo.allege, ubo.verify, ubo.promote, ubo.approve, ubo.reject` |
| `case` | `case` | `optional` | `-` | `authorised_corporate_director` | `kyc_case_lifecycle` | `5` | `case.open, case.submit, case.approve, case.reject, case.request-info` |
| `case.tollgate` | `tollgate` | `optional` | `-` | `case (min_state=intake)` | `-` | `1` | `tollgate.evaluate` |
| `mandate` | `mandate` | `optional` | `-` | `cbu (min_state=filled), case (min_state=intake)` | `-` | `1` | `mandate.create` |

### `struct.uk.authorised.oeic`
| Slot | Type | Cardinality | Entity Kinds | Depends On | State Machine | Verb Count | Verb FQNs |
| --- | --- | --- | --- | --- | --- | ---: | --- |
| `cbu` | `cbu` | `root` | `-` | `-` | `-` | `3` | `cbu.create, cbu.read, cbu.show` |
| `authorised_corporate_director` | `entity` | `mandatory` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `depositary` | `entity` | `mandatory` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `investment_manager` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `administrator` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `auditor` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `registrar` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `ownership_chain` | `entity_graph` | `recursive` | `person, company` | `authorised_corporate_director` | `ubo_epistemic_lifecycle` | `6` | `ubo.discover, ubo.allege, ubo.verify, ubo.promote, ubo.approve, ubo.reject` |
| `case` | `case` | `optional` | `-` | `authorised_corporate_director` | `kyc_case_lifecycle` | `5` | `case.open, case.submit, case.approve, case.reject, case.request-info` |
| `case.tollgate` | `tollgate` | `optional` | `-` | `case (min_state=intake)` | `-` | `1` | `tollgate.evaluate` |
| `mandate` | `mandate` | `optional` | `-` | `cbu (min_state=filled), case (min_state=intake)` | `-` | `1` | `mandate.create` |

### `struct.uk.manager.llp`
| Slot | Type | Cardinality | Entity Kinds | Depends On | State Machine | Verb Count | Verb FQNs |
| --- | --- | --- | --- | --- | --- | ---: | --- |
| `cbu` | `cbu` | `root` | `-` | `-` | `-` | `3` | `cbu.create, cbu.read, cbu.show` |
| `designated_member_1` | `entity` | `mandatory` | `company, person` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `designated_member_2` | `entity` | `mandatory` | `company, person` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `compliance_officer` | `entity` | `optional` | `person` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `mlro` | `entity` | `optional` | `person` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `auditor` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `ownership_chain` | `entity_graph` | `recursive` | `person, company` | `designated_member_1, designated_member_2` | `ubo_epistemic_lifecycle` | `6` | `ubo.discover, ubo.allege, ubo.verify, ubo.promote, ubo.approve, ubo.reject` |
| `case` | `case` | `optional` | `-` | `designated_member_1` | `kyc_case_lifecycle` | `5` | `case.open, case.submit, case.approve, case.reject, case.request-info` |
| `case.tollgate` | `tollgate` | `optional` | `-` | `case (min_state=intake)` | `-` | `1` | `tollgate.evaluate` |

### `struct.uk.private-equity.lp`
| Slot | Type | Cardinality | Entity Kinds | Depends On | State Machine | Verb Count | Verb FQNs |
| --- | --- | --- | --- | --- | --- | ---: | --- |
| `cbu` | `cbu` | `root` | `-` | `-` | `-` | `3` | `cbu.create, cbu.read, cbu.show` |
| `general_partner` | `entity` | `mandatory` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `aifm` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `depositary` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `administrator` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `auditor` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `legal_counsel` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `ownership_chain` | `entity_graph` | `recursive` | `person, company` | `general_partner` | `ubo_epistemic_lifecycle` | `6` | `ubo.discover, ubo.allege, ubo.verify, ubo.promote, ubo.approve, ubo.reject` |
| `case` | `case` | `optional` | `-` | `general_partner` | `kyc_case_lifecycle` | `5` | `case.open, case.submit, case.approve, case.reject, case.request-info` |
| `case.tollgate` | `tollgate` | `optional` | `-` | `case (min_state=intake)` | `-` | `1` | `tollgate.evaluate` |
| `mandate` | `mandate` | `optional` | `-` | `cbu (min_state=filled), case (min_state=intake)` | `-` | `1` | `mandate.create` |

### `struct.us.40act.closed-end`
| Slot | Type | Cardinality | Entity Kinds | Depends On | State Machine | Verb Count | Verb FQNs |
| --- | --- | --- | --- | --- | --- | ---: | --- |
| `cbu` | `cbu` | `root` | `-` | `-` | `-` | `3` | `cbu.create, cbu.read, cbu.show` |
| `investment_adviser` | `entity` | `mandatory` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `custodian` | `entity` | `mandatory` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `sub_adviser` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `administrator` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `transfer_agent` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `auditor` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `legal_counsel` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `ownership_chain` | `entity_graph` | `recursive` | `person, company` | `investment_adviser` | `ubo_epistemic_lifecycle` | `6` | `ubo.discover, ubo.allege, ubo.verify, ubo.promote, ubo.approve, ubo.reject` |
| `case` | `case` | `optional` | `-` | `investment_adviser` | `kyc_case_lifecycle` | `5` | `case.open, case.submit, case.approve, case.reject, case.request-info` |
| `case.tollgate` | `tollgate` | `optional` | `-` | `case (min_state=intake)` | `-` | `1` | `tollgate.evaluate` |
| `mandate` | `mandate` | `optional` | `-` | `cbu (min_state=filled), case (min_state=intake)` | `-` | `1` | `mandate.create` |

### `struct.us.40act.open-end`
| Slot | Type | Cardinality | Entity Kinds | Depends On | State Machine | Verb Count | Verb FQNs |
| --- | --- | --- | --- | --- | --- | ---: | --- |
| `cbu` | `cbu` | `root` | `-` | `-` | `-` | `3` | `cbu.create, cbu.read, cbu.show` |
| `investment_adviser` | `entity` | `mandatory` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `custodian` | `entity` | `mandatory` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `sub_adviser` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `administrator` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `transfer_agent` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `distributor` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `auditor` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `legal_counsel` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `ownership_chain` | `entity_graph` | `recursive` | `person, company` | `investment_adviser` | `ubo_epistemic_lifecycle` | `6` | `ubo.discover, ubo.allege, ubo.verify, ubo.promote, ubo.approve, ubo.reject` |
| `case` | `case` | `optional` | `-` | `investment_adviser` | `kyc_case_lifecycle` | `5` | `case.open, case.submit, case.approve, case.reject, case.request-info` |
| `case.tollgate` | `tollgate` | `optional` | `-` | `case (min_state=intake)` | `-` | `1` | `tollgate.evaluate` |
| `mandate` | `mandate` | `optional` | `-` | `cbu (min_state=filled), case (min_state=intake)` | `-` | `1` | `mandate.create` |

### `struct.us.etf.40act`
| Slot | Type | Cardinality | Entity Kinds | Depends On | State Machine | Verb Count | Verb FQNs |
| --- | --- | --- | --- | --- | --- | ---: | --- |
| `cbu` | `cbu` | `root` | `-` | `-` | `-` | `3` | `cbu.create, cbu.read, cbu.show` |
| `investment_adviser` | `entity` | `mandatory` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `custodian` | `entity` | `mandatory` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `authorized_participant` | `entity` | `mandatory` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `sub_adviser` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `administrator` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `transfer_agent` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `distributor` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `auditor` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `market_maker` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `ownership_chain` | `entity_graph` | `recursive` | `person, company` | `investment_adviser` | `ubo_epistemic_lifecycle` | `6` | `ubo.discover, ubo.allege, ubo.verify, ubo.promote, ubo.approve, ubo.reject` |
| `case` | `case` | `optional` | `-` | `investment_adviser` | `kyc_case_lifecycle` | `5` | `case.open, case.submit, case.approve, case.reject, case.request-info` |
| `case.tollgate` | `tollgate` | `optional` | `-` | `case (min_state=intake)` | `-` | `1` | `tollgate.evaluate` |
| `mandate` | `mandate` | `optional` | `-` | `cbu (min_state=filled), case (min_state=intake)` | `-` | `1` | `mandate.create` |

### `struct.us.private-fund.delaware-lp`
| Slot | Type | Cardinality | Entity Kinds | Depends On | State Machine | Verb Count | Verb FQNs |
| --- | --- | --- | --- | --- | --- | ---: | --- |
| `cbu` | `cbu` | `root` | `-` | `-` | `-` | `3` | `cbu.create, cbu.read, cbu.show` |
| `general_partner` | `entity` | `mandatory` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `investment_manager` | `entity` | `mandatory` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `custodian` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `administrator` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `prime_broker` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `auditor` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `legal_counsel` | `entity` | `optional` | `company` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `tax_advisor` | `entity` | `optional` | `company, person` | `cbu` | `entity_kyc_lifecycle` | `5` | `entity.ensure-or-placeholder, cbu.assign-role, party.search, party.add, entity.read` |
| `ownership_chain` | `entity_graph` | `recursive` | `person, company` | `general_partner` | `ubo_epistemic_lifecycle` | `6` | `ubo.discover, ubo.allege, ubo.verify, ubo.promote, ubo.approve, ubo.reject` |
| `case` | `case` | `optional` | `-` | `general_partner` | `kyc_case_lifecycle` | `5` | `case.open, case.submit, case.approve, case.reject, case.request-info` |
| `case.tollgate` | `tollgate` | `optional` | `-` | `case (min_state=intake)` | `-` | `1` | `tollgate.evaluate` |
| `mandate` | `mandate` | `optional` | `-` | `cbu (min_state=filled), case (min_state=intake)` | `-` | `1` | `mandate.create` |

### `trading.streetside`
| Slot | Type | Cardinality | Entity Kinds | Depends On | State Machine | Verb Count | Verb FQNs |
| --- | --- | --- | --- | --- | --- | ---: | --- |
| `cbu` | `cbu` | `root` | `-` | `-` | `-` | `1` | `cbu.read` |
| `trading_profile` | `mandate` | `optional` | `-` | `cbu` | `trading_profile_lifecycle` | `38` | `trading-profile.import, trading-profile.create-draft, trading-profile.read, trading-profile.get-active, trading-profile.list-versions, trading-profile.materialize, trading-profile.activate, trading-profile.diff, trading-profile.clone-to, trading-profile.create-new-version, trading-profile.add-component, trading-profile.remove-component, trading-profile.set-base-currency, trading-profile.link-csa-ssi, trading-profile.update-im-scope, trading-profile.ca.add-cutoff-rule, trading-profile.ca.remove-cutoff-rule, trading-profile.ca.enable-event-types, trading-profile.ca.disable-event-types, trading-profile.ca.set-default-option, trading-profile.ca.remove-default-option, trading-profile.ca.link-proceeds-ssi, trading-profile.ca.remove-proceeds-ssi, trading-profile.validate-go-live-ready, trading-profile.validate-universe-coverage, trading-profile.submit, trading-profile.approve, trading-profile.reject, trading-profile.archive, matrix-overlay.create, matrix-overlay.read, matrix-overlay.list, matrix-overlay.update, matrix-overlay.apply, matrix-overlay.remove, matrix-overlay.diff, matrix-overlay.preview, matrix-overlay.list-active` |
| `custody` | `entity` | `optional` | `cbu` | `trading_profile` | `-` | `8` | `custody.list-universe, custody.list-ssis, custody.list-booking-rules, custody.list-agent-overrides, custody.derive-required-coverage, custody.validate-booking-coverage, custody.lookup-ssi, custody.setup-ssi` |
| `booking_principal` | `entity` | `optional` | `company` | `cbu` | `-` | `9` | `booking-principal.create, booking-principal.update, booking-principal.retire, booking-principal.evaluate, booking-principal.select, booking-principal.explain, booking-principal.coverage-matrix, booking-principal.gap-report, booking-principal.impact-analysis` |
| `cash_sweep` | `entity` | `optional` | `entity` | `custody` | `-` | `9` | `cash-sweep.configure, cash-sweep.link-resource, cash-sweep.list, cash-sweep.update-threshold, cash-sweep.update-timing, cash-sweep.change-vehicle, cash-sweep.suspend, cash-sweep.reactivate, cash-sweep.remove` |
| `service_resource` | `entity` | `optional` | `entity` | `cbu` | `-` | `8` | `service-resource.read, service-resource.list, service-resource.provision, service-resource.set-attr, service-resource.activate, service-resource.suspend, service-resource.decommission, service-resource.validate-attrs` |
| `service_intent` | `entity` | `optional` | `entity` | `cbu` | `-` | `12` | `service-intent.create, service-intent.read, service-intent.list, service-intent.update, service-intent.approve, service-intent.reject, service-intent.cancel, service-intent.list-available, service-intent.list-by-status, service-intent.activate, service-intent.deactivate, service-intent.clone` |
| `booking_location` | `entity` | `optional` | `company` | `booking_principal` | `-` | `3` | `booking-location.create, booking-location.read, booking-location.list` |
| `legal_entity` | `entity` | `optional` | `company` | `booking_principal` | `-` | `3` | `legal-entity.create, legal-entity.read, legal-entity.list` |
| `product` | `entity` | `optional` | `entity` | `cbu` | `-` | `2` | `product.create, product.list` |
| `delivery` | `entity` | `optional` | `entity` | `cbu` | `-` | `3` | `delivery.create, delivery.read, delivery.list` |

## 5. Verb Surface per Constellation
### `deal.lifecycle`
| Slot | Verb Key | Verb FQN | Gated? | When Condition |
| --- | --- | --- | --- | --- |
| `deal` | `create` | `deal.create` | `Y` | `empty` |
| `deal` | `read` | `deal.read-record` | `Y` | `filled` |
| `deal` | `list` | `deal.list` | `Y` | `empty, filled` |
| `deal` | `search` | `deal.search-records` | `Y` | `empty, filled` |
| `deal` | `summary` | `deal.read-summary` | `Y` | `filled` |
| `deal` | `timeline` | `deal.read-timeline` | `Y` | `filled` |
| `deal` | `list_documents` | `deal.list-documents` | `Y` | `filled` |
| `deal` | `list_slas` | `deal.list-slas` | `Y` | `filled` |
| `deal` | `list_active_rate_cards` | `deal.list-active-rate-cards` | `Y` | `filled` |
| `deal` | `list_rate_card_lines` | `deal.list-rate-card-lines` | `Y` | `filled` |
| `deal` | `list_rate_card_history` | `deal.list-rate-card-history` | `Y` | `filled` |
| `deal` | `update` | `deal.update-record` | `Y` | `filled` |
| `deal` | `update_status` | `deal.update-status` | `Y` | `filled` |
| `deal` | `add_document` | `deal.add-document` | `Y` | `filled` |
| `deal` | `update_document_status` | `deal.update-document-status` | `Y` | `filled` |
| `deal` | `add_sla` | `deal.add-sla` | `Y` | `filled` |
| `deal` | `remove_sla` | `deal.remove-sla` | `Y` | `filled` |
| `deal` | `add_ubo_assessment` | `deal.add-ubo-assessment` | `Y` | `filled` |
| `deal` | `update_ubo_assessment` | `deal.update-ubo-assessment` | `Y` | `filled` |
| `deal` | `cancel` | `deal.cancel` | `Y` | `filled` |
| `participant` | `add` | `deal.add-participant` | `Y` | `empty, filled` |
| `participant` | `remove` | `deal.remove-participant` | `Y` | `filled` |
| `participant` | `list` | `deal.list-participants` | `Y` | `filled` |
| `deal_contract` | `add` | `deal.add-contract` | `Y` | `empty, filled` |
| `deal_contract` | `remove` | `deal.remove-contract` | `Y` | `filled` |
| `deal_contract` | `list` | `deal.list-contracts` | `Y` | `filled` |
| `contract` | `create` | `contract.create` | `Y` | `empty` |
| `contract` | `read` | `contract.get` | `Y` | `filled` |
| `contract` | `list` | `contract.list` | `Y` | `empty, filled` |
| `contract` | `list_products` | `contract.list-products` | `Y` | `filled` |
| `contract` | `list_rate_cards` | `contract.list-rate-cards` | `Y` | `filled` |
| `contract` | `list_subscriptions` | `contract.list-subscriptions` | `Y` | `filled` |
| `contract` | `for_client` | `contract.for-client` | `Y` | `empty, filled` |
| `contract` | `update` | `contract.update` | `Y` | `filled` |
| `contract` | `add_product` | `contract.add-product` | `Y` | `filled` |
| `contract` | `remove_product` | `contract.remove-product` | `Y` | `filled` |
| `contract` | `create_rate_card` | `contract.create-rate-card` | `Y` | `filled` |
| `contract` | `subscribe` | `contract.subscribe` | `Y` | `filled` |
| `contract` | `unsubscribe` | `contract.unsubscribe` | `Y` | `filled` |
| `contract` | `terminate` | `contract.terminate` | `Y` | `filled` |
| `deal_product` | `add` | `deal.add-product` | `Y` | `empty, filled` |
| `deal_product` | `update` | `deal.update-product-status` | `Y` | `filled` |
| `deal_product` | `remove` | `deal.remove-product` | `Y` | `filled` |
| `deal_product` | `list` | `deal.list-products` | `Y` | `filled` |
| `rate_card` | `create` | `deal.create-rate-card` | `Y` | `empty` |
| `rate_card` | `add_line` | `deal.add-rate-card-line` | `Y` | `filled` |
| `rate_card` | `update_line` | `deal.update-rate-card-line` | `Y` | `filled` |
| `rate_card` | `remove_line` | `deal.remove-rate-card-line` | `Y` | `filled` |
| `rate_card` | `list` | `deal.list-rate-cards` | `Y` | `filled` |
| `rate_card` | `list_lines` | `deal.list-rate-card-lines` | `Y` | `filled` |
| `rate_card` | `list_history` | `deal.list-rate-card-history` | `Y` | `filled` |
| `rate_card` | `list_active` | `deal.list-active-rate-cards` | `Y` | `filled` |
| `rate_card` | `propose` | `deal.propose-rate-card` | `Y` | `filled` |
| `rate_card` | `counter` | `deal.counter-rate-card` | `Y` | `filled` |
| `rate_card` | `agree` | `deal.agree-rate-card` | `Y` | `filled` |
| `onboarding_request` | `request` | `deal.request-onboarding` | `Y` | `empty` |
| `onboarding_request` | `request_batch` | `deal.request-onboarding-batch` | `Y` | `empty, filled` |
| `onboarding_request` | `update` | `deal.update-onboarding-status` | `Y` | `filled` |
| `onboarding_request` | `list` | `deal.list-onboarding-requests` | `Y` | `filled` |
| `billing_profile` | `create` | `billing.create-profile` | `Y` | `empty` |
| `billing_profile` | `activate` | `billing.activate-profile` | `Y` | `filled` |
| `billing_profile` | `suspend` | `billing.suspend-profile` | `Y` | `filled` |
| `billing_profile` | `close` | `billing.close-profile` | `Y` | `filled` |
| `billing_profile` | `read` | `billing.get-profile` | `Y` | `filled` |
| `billing_profile` | `list` | `billing.list-profiles` | `Y` | `empty, filled` |
| `billing_profile` | `add_target` | `billing.add-account-target` | `Y` | `filled` |
| `billing_profile` | `remove_target` | `billing.remove-account-target` | `Y` | `filled` |
| `billing_profile` | `list_targets` | `billing.list-account-targets` | `Y` | `filled` |
| `billing_profile` | `create_period` | `billing.create-period` | `Y` | `filled` |
| `billing_profile` | `calculate` | `billing.calculate-period` | `Y` | `filled` |
| `billing_profile` | `review` | `billing.review-period` | `Y` | `filled` |
| `billing_profile` | `approve` | `billing.approve-period` | `Y` | `filled` |
| `billing_profile` | `invoice` | `billing.generate-invoice` | `Y` | `filled` |
| `billing_profile` | `dispute` | `billing.dispute-period` | `Y` | `filled` |
| `billing_profile` | `period_summary` | `billing.period-summary` | `Y` | `filled` |
| `billing_profile` | `revenue` | `billing.revenue-summary` | `Y` | `filled` |
| `pricing` | `set_valuation` | `pricing-config.set-valuation-schedule` | `Y` | `empty, filled` |
| `pricing` | `set_nav` | `pricing-config.set-nav-threshold` | `Y` | `empty, filled` |
| `pricing` | `set_settlement` | `pricing-config.set-settlement-calendar` | `Y` | `empty, filled` |
| `pricing` | `set_holiday` | `pricing-config.set-holiday-schedule` | `Y` | `empty, filled` |
| `pricing` | `set_reporting` | `pricing-config.set-reporting` | `Y` | `empty, filled` |
| `pricing` | `set_tax` | `pricing-config.set-tax-status` | `Y` | `empty, filled` |
| `pricing` | `set_reclaim` | `pricing-config.set-reclaim-config` | `Y` | `empty, filled` |
| `pricing` | `find` | `pricing-config.find-for-instrument` | `Y` | `filled` |
| `pricing` | `list_jurisdictions` | `pricing-config.list-jurisdictions` | `Y` | `filled` |
| `pricing` | `list_treaty_rates` | `pricing-config.list-treaty-rates` | `Y` | `filled` |
| `pricing` | `list_tax_status` | `pricing-config.list-tax-status` | `Y` | `filled` |
| `pricing` | `list_reclaims` | `pricing-config.list-reclaim-configs` | `Y` | `filled` |
| `contract_template` | `create` | `contract-pack.create` | `Y` | `empty` |
| `contract_template` | `read` | `contract-pack.read` | `Y` | `filled` |

### `fund.administration`
| Slot | Verb Key | Verb FQN | Gated? | When Condition |
| --- | --- | --- | --- | --- |
| `fund` | `create` | `fund.create` | `Y` | `empty` |
| `fund` | `ensure` | `fund.ensure` | `Y` | `empty, filled` |
| `fund` | `read` | `fund.read-vehicle` | `Y` | `filled` |
| `fund` | `list` | `fund.list-by-manager` | `Y` | `empty, filled` |
| `fund` | `list_by_type` | `fund.list-by-vehicle-type` | `Y` | `empty, filled` |
| `fund` | `upsert` | `fund.upsert-vehicle` | `Y` | `empty, filled` |
| `fund` | `delete` | `fund.delete-vehicle` | `Y` | `filled` |
| `umbrella` | `add_subfund` | `fund.add-to-umbrella` | `Y` | `filled` |
| `umbrella` | `list_subfunds` | `fund.list-subfunds` | `Y` | `filled` |
| `umbrella` | `upsert_compartment` | `fund.upsert-compartment` | `Y` | `empty, filled` |
| `umbrella` | `read_compartment` | `fund.read-compartment` | `Y` | `filled` |
| `umbrella` | `list_compartments` | `fund.list-compartments-by-umbrella` | `Y` | `filled` |
| `umbrella` | `delete_compartment` | `fund.delete-compartment` | `Y` | `filled` |
| `share_class` | `add` | `fund.add-share-class` | `Y` | `empty, filled` |
| `share_class` | `list` | `fund.list-share-classes` | `Y` | `filled` |
| `feeder` | `link` | `fund.link-feeder` | `Y` | `empty, filled` |
| `feeder` | `list` | `fund.list-feeders` | `Y` | `filled` |
| `investment` | `add` | `fund.add-investment` | `Y` | `empty, filled` |
| `investment` | `update` | `fund.update-investment` | `Y` | `filled` |
| `investment` | `end` | `fund.end-investment` | `Y` | `filled` |
| `investment` | `list` | `fund.list-investments` | `Y` | `filled` |
| `investment` | `list_investors` | `fund.list-investors` | `Y` | `filled` |
| `capital` | `allocate` | `capital.allocate` | `Y` | `filled` |
| `capital` | `issue_initial` | `capital.issue.initial` | `Y` | `empty, filled` |
| `capital` | `issue_new` | `capital.issue.new` | `Y` | `filled` |
| `capital` | `issue_shares` | `capital.issue-shares` | `Y` | `filled` |
| `capital` | `cancel_shares` | `capital.cancel-shares` | `Y` | `filled` |
| `capital` | `transfer` | `capital.transfer` | `Y` | `filled` |
| `capital` | `split` | `capital.split` | `Y` | `filled` |
| `capital` | `buyback` | `capital.buyback` | `Y` | `filled` |
| `capital` | `cancel` | `capital.cancel` | `Y` | `filled` |
| `capital` | `reconcile` | `capital.reconcile` | `Y` | `filled` |
| `capital` | `cap_table` | `capital.cap-table` | `Y` | `filled` |
| `capital` | `holders` | `capital.holders` | `Y` | `filled` |
| `capital` | `list_by_issuer` | `capital.list-by-issuer` | `Y` | `filled` |
| `capital` | `list_shareholders` | `capital.list-shareholders` | `Y` | `filled` |
| `capital` | `get_ownership_chain` | `capital.get-ownership-chain` | `Y` | `filled` |
| `capital` | `define_share_class` | `capital.define-share-class` | `Y` | `empty, filled` |
| `capital` | `share_class_create` | `capital.share-class.create` | `Y` | `empty, filled` |
| `capital` | `share_class_list` | `capital.share-class.list` | `Y` | `filled` |
| `capital` | `share_class_get_supply` | `capital.share-class.get-supply` | `Y` | `filled` |
| `capital` | `share_class_add_identifier` | `capital.share-class.add-identifier` | `Y` | `filled` |
| `capital` | `control_config_get` | `capital.control-config.get` | `Y` | `filled` |
| `capital` | `control_config_set` | `capital.control-config.set` | `Y` | `filled` |
| `capital` | `dilution_grant_options` | `capital.dilution.grant-options` | `Y` | `filled` |
| `capital` | `dilution_issue_warrant` | `capital.dilution.issue-warrant` | `Y` | `filled` |
| `capital` | `dilution_create_safe` | `capital.dilution.create-safe` | `Y` | `filled` |
| `capital` | `dilution_create_note` | `capital.dilution.create-convertible-note` | `Y` | `filled` |
| `capital` | `dilution_exercise` | `capital.dilution.exercise` | `Y` | `filled` |
| `capital` | `dilution_forfeit` | `capital.dilution.forfeit` | `Y` | `filled` |
| `capital` | `dilution_list` | `capital.dilution.list` | `Y` | `filled` |
| `capital` | `dilution_get_summary` | `capital.dilution.get-summary` | `Y` | `filled` |
| `investment_manager` | `assign` | `investment-manager.assign` | `Y` | `empty, filled` |
| `investment_manager` | `set_scope` | `investment-manager.set-scope` | `Y` | `filled` |
| `investment_manager` | `link_connectivity` | `investment-manager.link-connectivity` | `Y` | `filled` |
| `investment_manager` | `list` | `investment-manager.list` | `Y` | `filled` |
| `investment_manager` | `suspend` | `investment-manager.suspend` | `Y` | `filled` |
| `investment_manager` | `terminate` | `investment-manager.terminate` | `Y` | `filled` |
| `investment_manager` | `find` | `investment-manager.find-for-trade` | `Y` | `filled` |
| `manco_group` | `create` | `manco.create` | `Y` | `empty` |
| `manco_group` | `read` | `manco.read` | `Y` | `filled` |
| `manco_group` | `list` | `manco.list` | `Y` | `empty, filled` |
| `manco_group` | `derive` | `manco.derive-groups` | `Y` | `filled` |
| `manco_group` | `bridge` | `manco.bridge-roles` | `Y` | `filled` |
| `manco_group` | `list_members` | `manco.list-members` | `Y` | `filled` |
| `manco_group` | `list_roles` | `manco.list-roles` | `Y` | `filled` |
| `manco_group` | `assign_role` | `manco.assign-role` | `Y` | `filled` |
| `manco_group` | `remove_role` | `manco.remove-role` | `Y` | `filled` |
| `manco_group` | `link` | `manco.link-entity` | `Y` | `filled` |
| `manco_group` | `unlink` | `manco.unlink-entity` | `Y` | `filled` |
| `manco_group` | `set_regulatory` | `manco.set-regulatory-status` | `Y` | `filled` |
| `manco_group` | `list_funds` | `manco.list-managed-funds` | `Y` | `filled` |
| `manco_group` | `verify` | `manco.verify` | `Y` | `filled` |
| `manco_group` | `compute_control` | `manco.compute-control-chain` | `Y` | `filled` |
| `manco_group` | `refresh` | `manco.refresh` | `Y` | `filled` |
| `trust` | `create` | `trust.create` | `Y` | `empty` |
| `trust` | `read` | `trust.read` | `Y` | `filled` |
| `trust` | `list` | `trust.list` | `Y` | `empty, filled` |
| `trust` | `add_trustee` | `trust.add-trustee` | `Y` | `filled` |
| `trust` | `remove_trustee` | `trust.remove-trustee` | `Y` | `filled` |
| `trust` | `add_beneficiary` | `trust.add-beneficiary` | `Y` | `filled` |
| `trust` | `add_settlor` | `trust.add-settlor` | `Y` | `filled` |
| `trust` | `identify_ubos` | `trust.identify-ubos` | `Y` | `filled` |
| `partnership` | `create` | `partnership.create` | `Y` | `empty` |
| `partnership` | `read` | `partnership.read` | `Y` | `filled` |
| `partnership` | `list` | `partnership.list` | `Y` | `empty, filled` |
| `partnership` | `add_partner` | `partnership.add-partner` | `Y` | `filled` |
| `partnership` | `remove_partner` | `partnership.remove-partner` | `Y` | `filled` |
| `partnership` | `set_gp` | `partnership.set-general-partner` | `Y` | `filled` |
| `partnership` | `list_partners` | `partnership.list-partners` | `Y` | `filled` |

### `governance.compliance`
| Slot | Verb Key | Verb FQN | Gated? | When Condition |
| --- | --- | --- | --- | --- |
| `sla` | `create` | `sla.create` | `Y` | `empty` |
| `sla` | `read` | `sla.read` | `Y` | `filled` |
| `sla` | `read_template` | `sla.read-template` | `Y` | `filled` |
| `sla` | `list` | `sla.list` | `Y` | `empty, filled` |
| `sla` | `list_templates` | `sla.list-templates` | `Y` | `empty, filled` |
| `sla` | `list_commitments` | `sla.list-commitments` | `Y` | `filled` |
| `sla` | `list_measurements` | `sla.list-measurements` | `Y` | `filled` |
| `sla` | `list_breaches` | `sla.list-breaches` | `Y` | `filled` |
| `sla` | `list_open_breaches` | `sla.list-open-breaches` | `Y` | `filled` |
| `sla` | `update` | `sla.update` | `Y` | `filled` |
| `sla` | `bind` | `sla.bind` | `Y` | `filled` |
| `sla` | `commit` | `sla.commit` | `Y` | `filled` |
| `sla` | `record_measurement` | `sla.record-measurement` | `Y` | `filled` |
| `sla` | `activate` | `sla.activate` | `Y` | `filled` |
| `sla` | `suspend` | `sla.suspend` | `Y` | `filled` |
| `sla` | `suspend_commitment` | `sla.suspend-commitment` | `Y` | `filled` |
| `sla` | `renew` | `sla.renew` | `Y` | `filled` |
| `sla` | `breach` | `sla.record-breach` | `Y` | `filled` |
| `sla` | `report_breach` | `sla.report-breach` | `Y` | `filled` |
| `sla` | `escalate_breach` | `sla.escalate-breach` | `Y` | `filled` |
| `sla` | `resolve_breach` | `sla.resolve-breach` | `Y` | `filled` |
| `sla` | `update_remediation` | `sla.update-remediation` | `Y` | `filled` |
| `access_review` | `create` | `access-review.create` | `Y` | `empty` |
| `access_review` | `create_campaign` | `access-review.create-campaign` | `Y` | `empty, filled` |
| `access_review` | `read` | `access-review.read` | `Y` | `filled` |
| `access_review` | `list` | `access-review.list` | `Y` | `empty, filled` |
| `access_review` | `list_items` | `access-review.list-items` | `Y` | `filled` |
| `access_review` | `list_flagged` | `access-review.list-flagged` | `Y` | `filled` |
| `access_review` | `my_pending` | `access-review.my-pending` | `Y` | `filled` |
| `access_review` | `campaign_status` | `access-review.campaign-status` | `Y` | `filled` |
| `access_review` | `audit_report` | `access-review.audit-report` | `Y` | `filled` |
| `access_review` | `populate_campaign` | `access-review.populate-campaign` | `Y` | `filled` |
| `access_review` | `launch_campaign` | `access-review.launch-campaign` | `Y` | `filled` |
| `access_review` | `send_reminders` | `access-review.send-reminders` | `Y` | `filled` |
| `access_review` | `process_deadline` | `access-review.process-deadline` | `Y` | `filled` |
| `access_review` | `attest` | `access-review.attest` | `Y` | `filled` |
| `access_review` | `extend_access` | `access-review.extend-access` | `Y` | `filled` |
| `access_review` | `revoke_access` | `access-review.revoke-access` | `Y` | `filled` |
| `access_review` | `escalate_item` | `access-review.escalate-item` | `Y` | `filled` |
| `access_review` | `start` | `access-review.start` | `Y` | `filled` |
| `access_review` | `complete` | `access-review.complete` | `Y` | `filled` |
| `access_review` | `approve` | `access-review.approve` | `Y` | `filled` |
| `access_review` | `reject` | `access-review.reject` | `Y` | `filled` |
| `regulatory` | `create` | `regulatory.create` | `Y` | `empty` |
| `regulatory` | `registration_add` | `regulatory.registration.add` | `Y` | `empty, filled` |
| `regulatory` | `read` | `regulatory.read` | `Y` | `filled` |
| `regulatory` | `list` | `regulatory.list` | `Y` | `empty, filled` |
| `regulatory` | `registration_list` | `regulatory.registration.list` | `Y` | `filled` |
| `regulatory` | `registration_check` | `regulatory.registration.check` | `Y` | `filled` |
| `regulatory` | `registration_verify` | `regulatory.registration.verify` | `Y` | `filled` |
| `regulatory` | `update` | `regulatory.update` | `Y` | `filled` |
| `regulatory` | `submit` | `regulatory.submit` | `Y` | `filled` |
| `regulatory` | `registration_remove` | `regulatory.registration.remove` | `Y` | `filled` |
| `ruleset` | `create` | `ruleset.create` | `Y` | `empty` |
| `ruleset` | `read` | `ruleset.read` | `Y` | `filled` |
| `ruleset` | `publish` | `ruleset.publish` | `Y` | `filled` |
| `ruleset` | `retire` | `ruleset.retire` | `Y` | `filled` |
| `delegation` | `create` | `delegation.create` | `Y` | `empty` |
| `delegation` | `add` | `delegation.add` | `Y` | `empty, filled` |
| `delegation` | `read` | `delegation.read` | `Y` | `filled` |
| `delegation` | `list` | `delegation.list` | `Y` | `empty, filled` |
| `delegation` | `list_delegates` | `delegation.list-delegates` | `Y` | `filled` |
| `delegation` | `list_received` | `delegation.list-delegations-received` | `Y` | `filled` |
| `delegation` | `end` | `delegation.end` | `Y` | `filled` |
| `delegation` | `revoke` | `delegation.revoke` | `Y` | `filled` |
| `team` | `add` | `team.add-member` | `Y` | `empty, filled` |
| `team` | `remove` | `team.remove-member` | `Y` | `filled` |
| `team` | `list` | `team.list-members` | `Y` | `filled` |
| `team` | `list_teams` | `team.list` | `Y` | `empty, filled` |
| `team` | `create` | `team.create` | `Y` | `empty` |
| `team` | `read` | `team.read` | `Y` | `filled` |
| `team` | `update` | `team.update` | `Y` | `filled` |
| `team` | `assign_role` | `team.assign-role` | `Y` | `filled` |
| `team` | `remove_role` | `team.remove-role` | `Y` | `filled` |
| `team` | `transfer` | `team.transfer-member` | `Y` | `filled` |
| `team` | `list_by_role` | `team.list-by-role` | `Y` | `filled` |
| `team` | `set_lead` | `team.set-lead` | `Y` | `filled` |
| `team` | `add_governance` | `team.add-governance-member` | `Y` | `filled` |
| `team` | `remove_governance` | `team.remove-governance-member` | `Y` | `filled` |
| `team` | `list_governance` | `team.list-governance-members` | `Y` | `filled` |
| `team` | `add_ops` | `team.add-ops-member` | `Y` | `filled` |
| `team` | `remove_ops` | `team.remove-ops-member` | `Y` | `filled` |
| `team` | `list_ops` | `team.list-ops-members` | `Y` | `filled` |
| `team` | `assign_capacity` | `team.assign-capacity` | `Y` | `filled` |
| `team` | `list_capacity` | `team.list-capacity` | `Y` | `filled` |
| `rule` | `create` | `rule.create` | `Y` | `empty` |
| `rule` | `read` | `rule.read` | `Y` | `filled` |
| `rule` | `update` | `rule.update` | `Y` | `filled` |
| `rule_field` | `list` | `rule-field.list` | `Y` | `empty, filled` |
| `rule_field` | `read` | `rule-field.read` | `Y` | `filled` |

### `group.ownership`
| Slot | Verb Key | Verb FQN | Gated? | When Condition |
| --- | --- | --- | --- | --- |
| `client_group` | `create` | `client-group.create` | `Y` | `empty` |
| `client_group` | `read` | `client-group.read` | `Y` | `filled` |
| `client_group` | `research` | `client-group.research` | `Y` | `empty, filled` |
| `client_group` | `update` | `client-group.update` | `Y` | `filled` |
| `client_group` | `set_canonical` | `client-group.set-canonical` | `Y` | `filled` |
| `client_group` | `start_discovery` | `client-group.start-discovery` | `Y` | `empty, filled` |
| `client_group` | `discover_entities` | `client-group.discover-entities` | `Y` | `empty, filled` |
| `client_group` | `complete_discovery` | `client-group.complete-discovery` | `Y` | `filled` |
| `client_group` | `entity_add` | `client-group.entity-add` | `Y` | `empty, filled` |
| `client_group` | `entity_remove` | `client-group.entity-remove` | `Y` | `filled` |
| `client_group` | `list_entities` | `client-group.list-entities` | `Y` | `filled` |
| `client_group` | `search_entities` | `client-group.search-entities` | `Y` | `empty, filled` |
| `client_group` | `list_parties` | `client-group.list-parties` | `Y` | `filled` |
| `client_group` | `list_unverified` | `client-group.list-unverified` | `Y` | `filled` |
| `client_group` | `list_discrepancies` | `client-group.list-discrepancies` | `Y` | `filled` |
| `client_group` | `verify_ownership` | `client-group.verify-ownership` | `Y` | `filled` |
| `client_group` | `reject_entity` | `client-group.reject-entity` | `Y` | `filled` |
| `client_group` | `assign_role` | `client-group.assign-role` | `Y` | `filled` |
| `client_group` | `remove_role` | `client-group.remove-role` | `Y` | `filled` |
| `client_group` | `list_roles` | `client-group.list-roles` | `Y` | `filled` |
| `client_group` | `add_relationship` | `client-group.add-relationship` | `Y` | `filled` |
| `client_group` | `list_relationships` | `client-group.list-relationships` | `Y` | `filled` |
| `client_group` | `tag_add` | `client-group.tag-add` | `Y` | `filled` |
| `client_group` | `tag_remove` | `client-group.tag-remove` | `Y` | `filled` |
| `gleif_import` | `import` | `gleif.import-tree` | `Y` | `empty` |
| `gleif_import` | `import_to_group` | `gleif.import-to-client-group` | `Y` | `empty, filled` |
| `gleif_import` | `import_managed_funds` | `gleif.import-managed-funds` | `Y` | `empty, filled` |
| `gleif_import` | `search` | `gleif.search` | `Y` | `empty, filled` |
| `gleif_import` | `refresh` | `gleif.refresh` | `Y` | `filled` |
| `gleif_import` | `enrich` | `gleif.enrich` | `Y` | `filled` |
| `gleif_import` | `get_record` | `gleif.get-record` | `Y` | `filled` |
| `gleif_import` | `get_parent` | `gleif.get-parent` | `Y` | `filled` |
| `gleif_import` | `get_children` | `gleif.get-children` | `Y` | `filled` |
| `gleif_import` | `get_manager` | `gleif.get-manager` | `Y` | `filled` |
| `gleif_import` | `get_managed_funds` | `gleif.get-managed-funds` | `Y` | `filled` |
| `gleif_import` | `get_master_fund` | `gleif.get-master-fund` | `Y` | `filled` |
| `gleif_import` | `get_umbrella` | `gleif.get-umbrella` | `Y` | `filled` |
| `gleif_import` | `lookup_by_isin` | `gleif.lookup-by-isin` | `Y` | `empty, filled` |
| `gleif_import` | `resolve_successor` | `gleif.resolve-successor` | `Y` | `filled` |
| `gleif_import` | `trace_ownership` | `gleif.trace-ownership` | `Y` | `filled` |
| `ubo_discovery` | `discover` | `ubo.discover` | `Y` | `empty` |
| `ubo_discovery` | `allege` | `ubo.allege` | `Y` | `empty, filled` |
| `ubo_discovery` | `calculate` | `ubo.calculate` | `Y` | `empty, filled` |
| `ubo_discovery` | `compute_chains` | `ubo.compute-chains` | `Y` | `empty, filled` |
| `ubo_discovery` | `trace_chains` | `ubo.trace-chains` | `Y` | `filled` |
| `ubo_discovery` | `verify` | `ubo.verify` | `Y` | `filled` |
| `ubo_discovery` | `promote` | `ubo.promote` | `Y` | `filled` |
| `ubo_discovery` | `approve` | `ubo.approve` | `Y` | `filled` |
| `ubo_discovery` | `reject` | `ubo.reject` | `Y` | `filled` |
| `ubo_discovery` | `list` | `ubo.list` | `Y` | `filled` |
| `ubo_discovery` | `list_ubos` | `ubo.list-ubos` | `Y` | `filled` |
| `ubo_discovery` | `list_owned` | `ubo.list-owned` | `Y` | `filled` |
| `ubo_discovery` | `list_owners` | `ubo.list-owners` | `Y` | `filled` |
| `ubo_discovery` | `add_ownership` | `ubo.add-ownership` | `Y` | `filled` |
| `ubo_discovery` | `update_ownership` | `ubo.update-ownership` | `Y` | `filled` |
| `ubo_discovery` | `add_control` | `ubo.add-control` | `Y` | `filled` |
| `ubo_discovery` | `transfer_control` | `ubo.transfer-control` | `Y` | `filled` |
| `ubo_discovery` | `add_trust_role` | `ubo.add-trust-role` | `Y` | `filled` |
| `ubo_discovery` | `delete_relationship` | `ubo.delete-relationship` | `Y` | `filled` |
| `ubo_discovery` | `end_relationship` | `ubo.end-relationship` | `Y` | `filled` |
| `ubo_discovery` | `waive_verification` | `ubo.waive-verification` | `Y` | `filled` |
| `ubo_discovery` | `mark_deceased` | `ubo.mark-deceased` | `Y` | `filled` |
| `ubo_discovery` | `mark_terminus` | `ubo.mark-terminus` | `Y` | `filled` |
| `ubo_discovery` | `convergence_supersede` | `ubo.convergence-supersede` | `Y` | `filled` |
| `ubo_discovery` | `registry_create` | `ubo.registry.create` | `Y` | `empty, filled` |
| `ubo_discovery` | `registry_advance` | `ubo.registry.advance` | `Y` | `filled` |
| `ubo_discovery` | `registry_promote` | `ubo.registry.promote` | `Y` | `filled` |
| `ubo_discovery` | `registry_reject` | `ubo.registry.reject` | `Y` | `filled` |
| `ubo_discovery` | `registry_expire` | `ubo.registry.expire` | `Y` | `filled` |
| `ubo_discovery` | `registry_waive` | `ubo.registry.waive` | `Y` | `filled` |
| `ubo_discovery` | `snapshot_capture` | `ubo.snapshot.capture` | `Y` | `filled` |
| `ubo_discovery` | `snapshot_diff` | `ubo.snapshot.diff` | `Y` | `filled` |
| `control_chain` | `trace` | `ownership.trace-chain` | `Y` | `empty` |
| `control_chain` | `build` | `control.build-graph` | `Y` | `empty, filled` |
| `control_chain` | `refresh` | `ownership.refresh` | `Y` | `filled` |
| `control_chain` | `read` | `control.read` | `Y` | `filled` |
| `control_chain` | `list_links` | `control.list-links` | `Y` | `filled` |
| `control_chain` | `add` | `control.add` | `Y` | `empty, filled` |
| `control_chain` | `end` | `control.end` | `Y` | `filled` |
| `control_chain` | `analyze` | `control.analyze` | `Y` | `filled` |
| `control_chain` | `list_controllers` | `control.list-controllers` | `Y` | `filled` |
| `control_chain` | `list_controlled` | `control.list-controlled` | `Y` | `filled` |
| `control_chain` | `trace_chain` | `control.trace-chain` | `Y` | `filled` |
| `control_chain` | `compute_controllers` | `control.compute-controllers` | `Y` | `filled` |
| `control_chain` | `identify_ubos` | `control.identify-ubos` | `Y` | `filled` |
| `control_chain` | `reconcile_ownership` | `control.reconcile-ownership` | `Y` | `filled` |
| `control_chain` | `set_board_controller` | `control.set-board-controller` | `Y` | `filled` |
| `control_chain` | `show_board_controller` | `control.show-board-controller` | `Y` | `filled` |
| `control_chain` | `recompute_board_controller` | `control.recompute-board-controller` | `Y` | `filled` |
| `control_chain` | `clear_board_controller_override` | `control.clear-board-controller-override` | `Y` | `filled` |
| `control_chain` | `import_gleif_control` | `control.import-gleif-control` | `Y` | `empty, filled` |
| `control_chain` | `import_psc_register` | `control.import-psc-register` | `Y` | `empty, filled` |
| `control_chain` | `ownership_compute` | `ownership.compute` | `Y` | `filled` |
| `control_chain` | `ownership_control_positions` | `ownership.control-positions` | `Y` | `filled` |
| `control_chain` | `ownership_who_controls` | `ownership.who-controls` | `Y` | `filled` |
| `control_chain` | `ownership_analyze_gaps` | `ownership.analyze-gaps` | `Y` | `filled` |
| `control_chain` | `ownership_reconcile` | `ownership.reconcile` | `Y` | `filled` |
| `control_chain` | `ownership_reconcile_findings` | `ownership.reconcile.findings` | `Y` | `filled` |
| `control_chain` | `ownership_reconcile_list_runs` | `ownership.reconcile.list-runs` | `Y` | `filled` |
| `control_chain` | `ownership_reconcile_resolve` | `ownership.reconcile.resolve-finding` | `Y` | `filled` |
| `control_chain` | `ownership_right_add_class` | `ownership.right.add-to-class` | `Y` | `filled` |
| `control_chain` | `ownership_right_add_holder` | `ownership.right.add-to-holder` | `Y` | `filled` |
| `control_chain` | `ownership_right_end` | `ownership.right.end` | `Y` | `filled` |
| `control_chain` | `ownership_right_list_holder` | `ownership.right.list-for-holder` | `Y` | `filled` |
| `control_chain` | `ownership_right_list_issuer` | `ownership.right.list-for-issuer` | `Y` | `filled` |
| `control_chain` | `ownership_snapshot_get` | `ownership.snapshot.get` | `Y` | `filled` |
| `control_chain` | `ownership_snapshot_list` | `ownership.snapshot.list` | `Y` | `filled` |
| `cbu_identification` | `create` | `cbu.create` | `Y` | `empty` |
| `cbu_identification` | `create_from_group` | `cbu.create-from-client-group` | `Y` | `empty` |
| `cbu_identification` | `ensure` | `cbu.ensure` | `Y` | `empty, filled` |
| `cbu_identification` | `read` | `cbu.read` | `Y` | `filled` |
| `cbu_identification` | `list` | `cbu.list` | `Y` | `filled` |
| `cbu_identification` | `list_subscriptions` | `cbu.list-subscriptions` | `Y` | `filled` |
| `cbu_identification` | `list_evidence` | `cbu.list-evidence` | `Y` | `filled` |
| `cbu_identification` | `list_structure_links` | `cbu.list-structure-links` | `Y` | `filled` |
| `cbu_identification` | `parties` | `cbu.parties` | `Y` | `filled` |
| `cbu_identification` | `update` | `cbu.update` | `Y` | `filled` |
| `cbu_identification` | `rename` | `cbu.rename` | `Y` | `filled` |
| `cbu_identification` | `set_jurisdiction` | `cbu.set-jurisdiction` | `Y` | `filled` |
| `cbu_identification` | `set_client_type` | `cbu.set-client-type` | `Y` | `filled` |
| `cbu_identification` | `set_commercial_client` | `cbu.set-commercial-client` | `Y` | `filled` |
| `cbu_identification` | `add_product` | `cbu.add-product` | `Y` | `filled` |
| `cbu_identification` | `remove_product` | `cbu.remove-product` | `Y` | `filled` |
| `cbu_identification` | `assign_control` | `cbu.assign-control` | `Y` | `filled` |
| `cbu_identification` | `assign_ownership` | `cbu.assign-ownership` | `Y` | `filled` |
| `cbu_identification` | `assign_fund_role` | `cbu.assign-fund-role` | `Y` | `filled` |
| `cbu_identification` | `assign_trust_role` | `cbu.assign-trust-role` | `Y` | `filled` |
| `cbu_identification` | `assign_service_provider` | `cbu.assign-service-provider` | `Y` | `filled` |
| `cbu_identification` | `assign_signatory` | `cbu.assign-signatory` | `Y` | `filled` |
| `cbu_identification` | `remove_role` | `cbu.remove-role` | `Y` | `filled` |
| `cbu_identification` | `validate_roles` | `cbu.validate-roles` | `Y` | `filled` |
| `cbu_identification` | `attach_evidence` | `cbu.attach-evidence` | `Y` | `filled` |
| `cbu_identification` | `verify_evidence` | `cbu.verify-evidence` | `Y` | `filled` |
| `cbu_identification` | `request_proof_update` | `cbu.request-proof-update` | `Y` | `filled` |
| `cbu_identification` | `link_structure` | `cbu.link-structure` | `Y` | `filled` |
| `cbu_identification` | `unlink_structure` | `cbu.unlink-structure` | `Y` | `filled` |
| `cbu_identification` | `submit_for_validation` | `cbu.submit-for-validation` | `Y` | `filled` |
| `cbu_identification` | `reopen_validation` | `cbu.reopen-validation` | `Y` | `filled` |
| `cbu_identification` | `decide` | `cbu.decide` | `Y` | `filled` |
| `cbu_identification` | `delete` | `cbu.delete` | `Y` | `filled` |
| `cbu_identification` | `delete_cascade` | `cbu.delete-cascade` | `Y` | `filled` |

### `kyc.extended`
| Slot | Verb Key | Verb FQN | Gated? | When Condition |
| --- | --- | --- | --- | --- |
| `entity` | `read` | `entity.read` | `Y` | `filled` |
| `board` | `appoint` | `board.appoint` | `Y` | `empty, filled` |
| `board` | `resign` | `board.resign` | `Y` | `filled` |
| `board` | `list_by_entity` | `board.list-by-entity` | `Y` | `filled` |
| `board` | `list_by_person` | `board.list-by-person` | `Y` | `filled` |
| `board` | `grant_right` | `board.grant-appointment-right` | `Y` | `filled` |
| `board` | `revoke_right` | `board.revoke-appointment-right` | `Y` | `filled` |
| `board` | `list_rights` | `board.list-appointment-rights` | `Y` | `filled` |
| `board` | `list_held` | `board.list-rights-held` | `Y` | `filled` |
| `board` | `analyze` | `board.analyze-control` | `Y` | `filled` |
| `bods` | `discover` | `bods.discover-ubos` | `Y` | `empty, filled` |
| `bods` | `import` | `bods.import` | `Y` | `empty, filled` |
| `bods` | `link` | `bods.link-entity` | `Y` | `filled` |
| `bods` | `get_statement` | `bods.get-statement` | `Y` | `filled` |
| `bods` | `list_by_entity` | `bods.list-by-entity` | `Y` | `filled` |
| `bods` | `find_by_lei` | `bods.find-by-lei` | `Y` | `empty, filled` |
| `bods` | `list_persons` | `bods.list-persons` | `Y` | `filled` |
| `bods` | `list_ownership` | `bods.list-ownership` | `Y` | `filled` |
| `bods` | `sync` | `bods.sync-from-gleif` | `Y` | `filled` |

### `kyc.onboarding`
| Slot | Verb Key | Verb FQN | Gated? | When Condition |
| --- | --- | --- | --- | --- |
| `cbu` | `show` | `cbu.show` | `Y` | `filled` |
| `kyc_case` | `create` | `kyc-case.create` | `Y` | `empty` |
| `kyc_case` | `open` | `kyc.open-case` | `Y` | `empty` |
| `kyc_case` | `read` | `kyc-case.read` | `Y` | `filled` |
| `kyc_case` | `list_by_cbu` | `kyc-case.list-by-cbu` | `Y` | `filled` |
| `kyc_case` | `state` | `kyc-case.state` | `Y` | `filled` |
| `kyc_case` | `assign` | `kyc-case.assign` | `Y` | `filled` |
| `kyc_case` | `update_status` | `kyc-case.update-status` | `Y` | `filled` |
| `kyc_case` | `set_risk` | `kyc-case.set-risk-rating` | `Y` | `filled` |
| `kyc_case` | `close` | `kyc-case.close` | `Y` | `filled` |
| `kyc_case` | `reopen` | `kyc-case.reopen` | `Y` | `filled` |
| `kyc_case` | `escalate` | `kyc-case.escalate` | `Y` | `filled` |
| `kyc_case.tollgate` | `evaluate` | `tollgate.evaluate` | `Y` | `empty` |
| `kyc_case.tollgate` | `evaluate_gate` | `tollgate.evaluate-gate` | `Y` | `empty, filled` |
| `kyc_case.tollgate` | `read` | `tollgate.read` | `Y` | `filled` |
| `kyc_case.tollgate` | `get_decision_readiness` | `tollgate.get-decision-readiness` | `Y` | `filled` |
| `kyc_case.tollgate` | `get_metrics` | `tollgate.get-metrics` | `Y` | `filled` |
| `kyc_case.tollgate` | `list_evaluations` | `tollgate.list-evaluations` | `Y` | `filled` |
| `kyc_case.tollgate` | `list_thresholds` | `tollgate.list-thresholds` | `Y` | `filled` |
| `kyc_case.tollgate` | `set_threshold` | `tollgate.set-threshold` | `Y` | `filled` |
| `kyc_case.tollgate` | `override` | `tollgate.override` | `Y` | `filled` |
| `kyc_case.tollgate` | `list_overrides` | `tollgate.list-overrides` | `Y` | `filled` |
| `kyc_case.tollgate` | `expire_override` | `tollgate.expire-override` | `Y` | `filled` |
| `entity_workstream` | `create` | `entity-workstream.create` | `Y` | `empty` |
| `entity_workstream` | `read` | `entity-workstream.read` | `Y` | `filled` |
| `entity_workstream` | `list_by_case` | `entity-workstream.list-by-case` | `Y` | `filled` |
| `entity_workstream` | `state` | `entity-workstream.state` | `Y` | `filled` |
| `entity_workstream` | `update` | `entity-workstream.update-status` | `Y` | `filled` |
| `entity_workstream` | `set_enhanced_dd` | `entity-workstream.set-enhanced-dd` | `Y` | `filled` |
| `entity_workstream` | `set_ubo` | `entity-workstream.set-ubo` | `Y` | `filled` |
| `entity_workstream` | `complete` | `entity-workstream.complete` | `Y` | `filled` |
| `entity_workstream` | `block` | `entity-workstream.block` | `Y` | `filled` |
| `entity_workstream` | `flag_raise` | `red-flag.raise` | `Y` | `empty, filled` |
| `entity_workstream` | `flag_read` | `red-flag.read` | `Y` | `filled` |
| `entity_workstream` | `flag_list` | `red-flag.list` | `Y` | `filled` |
| `entity_workstream` | `flag_resolve` | `red-flag.resolve` | `Y` | `filled` |
| `entity_workstream` | `flag_escalate` | `red-flag.escalate` | `Y` | `filled` |
| `entity_workstream` | `flag_update` | `red-flag.update` | `Y` | `filled` |
| `entity_workstream` | `flag_list_severity` | `red-flag.list-by-severity` | `Y` | `filled` |
| `entity_workstream` | `flag_close` | `red-flag.close` | `Y` | `filled` |
| `entity_workstream` | `req_create` | `requirement.create` | `Y` | `empty, filled` |
| `entity_workstream` | `req_create_set` | `requirement.create-set` | `Y` | `empty, filled` |
| `entity_workstream` | `req_check` | `requirement.check` | `Y` | `filled` |
| `entity_workstream` | `req_list` | `requirement.list` | `Y` | `filled` |
| `entity_workstream` | `req_for_entity` | `requirement.for-entity` | `Y` | `filled` |
| `entity_workstream` | `req_unsatisfied` | `requirement.unsatisfied` | `Y` | `filled` |
| `entity_workstream` | `req_waive` | `requirement.waive` | `Y` | `filled` |
| `entity_workstream` | `req_reinstate` | `requirement.reinstate` | `Y` | `filled` |
| `entity_workstream` | `doc_solicit` | `document.solicit` | `Y` | `empty, filled` |
| `entity_workstream` | `doc_solicit_set` | `document.solicit-set` | `Y` | `empty, filled` |
| `entity_workstream` | `doc_upload` | `document.upload` | `Y` | `filled` |
| `entity_workstream` | `doc_verify` | `document.verify` | `Y` | `filled` |
| `entity_workstream` | `doc_reject` | `document.reject` | `Y` | `filled` |
| `entity_workstream` | `doc_read` | `document.read` | `Y` | `filled` |
| `entity_workstream` | `doc_list` | `document.list` | `Y` | `filled` |
| `entity_workstream` | `doc_compute_reqs` | `document.compute-requirements` | `Y` | `filled` |
| `entity_workstream` | `doc_missing` | `document.missing-for-entity` | `Y` | `filled` |
| `screening` | `run` | `screening.run` | `Y` | `empty` |
| `screening` | `sanctions` | `screening.sanctions` | `Y` | `empty, filled` |
| `screening` | `pep` | `screening.pep` | `Y` | `empty, filled` |
| `screening` | `adverse_media` | `screening.adverse-media` | `Y` | `empty, filled` |
| `screening` | `bulk_refresh` | `screening.bulk-refresh` | `Y` | `filled` |
| `screening` | `read` | `screening.read` | `Y` | `filled` |
| `screening` | `list` | `screening.list` | `Y` | `filled` |
| `screening` | `list_by_workstream` | `screening.list-by-workstream` | `Y` | `filled` |
| `screening` | `review_hit` | `screening.review-hit` | `Y` | `filled` |
| `screening` | `update` | `screening.update-status` | `Y` | `filled` |
| `screening` | `escalate` | `screening.escalate` | `Y` | `filled` |
| `screening` | `resolve` | `screening.resolve` | `Y` | `filled` |
| `screening` | `complete` | `screening.complete` | `Y` | `filled` |
| `kyc_agreement` | `create` | `kyc-agreement.create` | `Y` | `empty` |
| `kyc_agreement` | `read` | `kyc-agreement.read` | `Y` | `filled` |
| `kyc_agreement` | `list` | `kyc-agreement.list` | `Y` | `filled` |
| `kyc_agreement` | `update` | `kyc-agreement.update` | `Y` | `filled` |
| `kyc_agreement` | `update_status` | `kyc-agreement.update-status` | `Y` | `filled` |
| `kyc_agreement` | `sign` | `kyc-agreement.sign` | `Y` | `filled` |
| `identifier` | `add` | `identifier.add` | `Y` | `empty, filled` |
| `identifier` | `read` | `identifier.read` | `Y` | `filled` |
| `identifier` | `list` | `identifier.list` | `Y` | `filled` |
| `identifier` | `verify` | `identifier.verify` | `Y` | `filled` |
| `identifier` | `expire` | `identifier.expire` | `Y` | `filled` |
| `identifier` | `update` | `identifier.update` | `Y` | `filled` |
| `identifier` | `search` | `identifier.search` | `Y` | `empty, filled` |
| `identifier` | `resolve` | `identifier.resolve` | `Y` | `filled` |
| `identifier` | `list_by_type` | `identifier.list-by-type` | `Y` | `filled` |
| `identifier` | `set_primary` | `identifier.set-primary` | `Y` | `filled` |
| `identifier` | `remove` | `identifier.remove` | `Y` | `filled` |
| `request` | `create` | `request.create` | `Y` | `empty` |
| `request` | `read` | `request.read` | `Y` | `filled` |
| `request` | `list` | `request.list` | `Y` | `empty, filled` |
| `request` | `update` | `request.update` | `Y` | `filled` |
| `request` | `complete` | `request.complete` | `Y` | `filled` |
| `request` | `cancel` | `request.cancel` | `Y` | `filled` |
| `request` | `assign` | `request.assign` | `Y` | `filled` |
| `request` | `reopen` | `request.reopen` | `Y` | `filled` |
| `request` | `escalate` | `request.escalate` | `Y` | `filled` |

### `struct.hedge.cross-border`
| Slot | Verb Key | Verb FQN | Gated? | When Condition |
| --- | --- | --- | --- | --- |
| `cbu` | `create` | `cbu.create` | `N` | `-` |
| `cbu` | `read` | `cbu.read` | `N` | `-` |
| `cbu` | `show` | `cbu.show` | `N` | `-` |
| `cbu.us_feeder` | `show` | `cbu.read` | `N` | `-` |
| `cbu.ie_feeder` | `show` | `cbu.read` | `N` | `-` |
| `aifm` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `aifm` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `aifm` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `aifm` | `add` | `party.add` | `Y` | `empty` |
| `aifm` | `show` | `entity.read` | `Y` | `filled` |
| `depositary` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `depositary` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `depositary` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `depositary` | `add` | `party.add` | `Y` | `empty` |
| `depositary` | `show` | `entity.read` | `Y` | `filled` |
| `prime_broker` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `prime_broker` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `prime_broker` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `prime_broker` | `add` | `party.add` | `Y` | `empty` |
| `prime_broker` | `show` | `entity.read` | `Y` | `filled` |
| `investment_manager` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `investment_manager` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `investment_manager` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `investment_manager` | `add` | `party.add` | `Y` | `empty` |
| `investment_manager` | `show` | `entity.read` | `Y` | `filled` |
| `administrator` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `administrator` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `administrator` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `administrator` | `add` | `party.add` | `Y` | `empty` |
| `administrator` | `show` | `entity.read` | `Y` | `filled` |
| `auditor` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `auditor` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `auditor` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `auditor` | `add` | `party.add` | `Y` | `empty` |
| `auditor` | `show` | `entity.read` | `Y` | `filled` |
| `secondary_prime_broker` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `secondary_prime_broker` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `secondary_prime_broker` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `secondary_prime_broker` | `add` | `party.add` | `Y` | `empty` |
| `secondary_prime_broker` | `show` | `entity.read` | `Y` | `filled` |
| `ownership_chain` | `discover` | `ubo.discover` | `N` | `-` |
| `ownership_chain` | `allege` | `ubo.allege` | `N` | `-` |
| `ownership_chain` | `verify` | `ubo.verify` | `N` | `-` |
| `ownership_chain` | `promote` | `ubo.promote` | `N` | `-` |
| `ownership_chain` | `approve` | `ubo.approve` | `N` | `-` |
| `ownership_chain` | `reject` | `ubo.reject` | `N` | `-` |
| `case` | `open` | `case.open` | `N` | `-` |
| `case` | `submit` | `case.submit` | `N` | `-` |
| `case` | `approve` | `case.approve` | `N` | `-` |
| `case` | `reject` | `case.reject` | `N` | `-` |
| `case` | `request_info` | `case.request-info` | `N` | `-` |
| `case.tollgate` | `evaluate` | `tollgate.evaluate` | `N` | `-` |
| `mandate` | `create` | `mandate.create` | `N` | `-` |

### `struct.ie.aif.icav`
| Slot | Verb Key | Verb FQN | Gated? | When Condition |
| --- | --- | --- | --- | --- |
| `cbu` | `create` | `cbu.create` | `N` | `-` |
| `cbu` | `read` | `cbu.read` | `N` | `-` |
| `cbu` | `show` | `cbu.show` | `N` | `-` |
| `aifm` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `aifm` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `aifm` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `aifm` | `add` | `party.add` | `Y` | `empty` |
| `aifm` | `show` | `entity.read` | `Y` | `filled` |
| `depositary` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `depositary` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `depositary` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `depositary` | `add` | `party.add` | `Y` | `empty` |
| `depositary` | `show` | `entity.read` | `Y` | `filled` |
| `investment_manager` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `investment_manager` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `investment_manager` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `investment_manager` | `add` | `party.add` | `Y` | `empty` |
| `investment_manager` | `show` | `entity.read` | `Y` | `filled` |
| `administrator` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `administrator` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `administrator` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `administrator` | `add` | `party.add` | `Y` | `empty` |
| `administrator` | `show` | `entity.read` | `Y` | `filled` |
| `auditor` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `auditor` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `auditor` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `auditor` | `add` | `party.add` | `Y` | `empty` |
| `auditor` | `show` | `entity.read` | `Y` | `filled` |
| `prime_broker` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `prime_broker` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `prime_broker` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `prime_broker` | `add` | `party.add` | `Y` | `empty` |
| `prime_broker` | `show` | `entity.read` | `Y` | `filled` |
| `company_secretary` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `company_secretary` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `company_secretary` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `company_secretary` | `add` | `party.add` | `Y` | `empty` |
| `company_secretary` | `show` | `entity.read` | `Y` | `filled` |
| `ownership_chain` | `discover` | `ubo.discover` | `N` | `-` |
| `ownership_chain` | `allege` | `ubo.allege` | `N` | `-` |
| `ownership_chain` | `verify` | `ubo.verify` | `N` | `-` |
| `ownership_chain` | `promote` | `ubo.promote` | `N` | `-` |
| `ownership_chain` | `approve` | `ubo.approve` | `N` | `-` |
| `ownership_chain` | `reject` | `ubo.reject` | `N` | `-` |
| `case` | `open` | `case.open` | `N` | `-` |
| `case` | `submit` | `case.submit` | `N` | `-` |
| `case` | `approve` | `case.approve` | `N` | `-` |
| `case` | `reject` | `case.reject` | `N` | `-` |
| `case` | `request_info` | `case.request-info` | `N` | `-` |
| `case.tollgate` | `evaluate` | `tollgate.evaluate` | `N` | `-` |
| `mandate` | `create` | `mandate.create` | `N` | `-` |

### `struct.ie.hedge.icav`
| Slot | Verb Key | Verb FQN | Gated? | When Condition |
| --- | --- | --- | --- | --- |
| `cbu` | `create` | `cbu.create` | `N` | `-` |
| `cbu` | `read` | `cbu.read` | `N` | `-` |
| `cbu` | `show` | `cbu.show` | `N` | `-` |
| `aifm` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `aifm` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `aifm` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `aifm` | `add` | `party.add` | `Y` | `empty` |
| `aifm` | `show` | `entity.read` | `Y` | `filled` |
| `depositary` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `depositary` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `depositary` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `depositary` | `add` | `party.add` | `Y` | `empty` |
| `depositary` | `show` | `entity.read` | `Y` | `filled` |
| `investment_manager` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `investment_manager` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `investment_manager` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `investment_manager` | `add` | `party.add` | `Y` | `empty` |
| `investment_manager` | `show` | `entity.read` | `Y` | `filled` |
| `administrator` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `administrator` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `administrator` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `administrator` | `add` | `party.add` | `Y` | `empty` |
| `administrator` | `show` | `entity.read` | `Y` | `filled` |
| `auditor` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `auditor` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `auditor` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `auditor` | `add` | `party.add` | `Y` | `empty` |
| `auditor` | `show` | `entity.read` | `Y` | `filled` |
| `prime_broker` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `prime_broker` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `prime_broker` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `prime_broker` | `add` | `party.add` | `Y` | `empty` |
| `prime_broker` | `show` | `entity.read` | `Y` | `filled` |
| `secondary_prime_broker` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `secondary_prime_broker` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `secondary_prime_broker` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `secondary_prime_broker` | `add` | `party.add` | `Y` | `empty` |
| `secondary_prime_broker` | `show` | `entity.read` | `Y` | `filled` |
| `executing_broker` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `executing_broker` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `executing_broker` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `executing_broker` | `add` | `party.add` | `Y` | `empty` |
| `executing_broker` | `show` | `entity.read` | `Y` | `filled` |
| `company_secretary` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `company_secretary` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `company_secretary` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `company_secretary` | `add` | `party.add` | `Y` | `empty` |
| `company_secretary` | `show` | `entity.read` | `Y` | `filled` |
| `ownership_chain` | `discover` | `ubo.discover` | `N` | `-` |
| `ownership_chain` | `allege` | `ubo.allege` | `N` | `-` |
| `ownership_chain` | `verify` | `ubo.verify` | `N` | `-` |
| `ownership_chain` | `promote` | `ubo.promote` | `N` | `-` |
| `ownership_chain` | `approve` | `ubo.approve` | `N` | `-` |
| `ownership_chain` | `reject` | `ubo.reject` | `N` | `-` |
| `case` | `open` | `case.open` | `N` | `-` |
| `case` | `submit` | `case.submit` | `N` | `-` |
| `case` | `approve` | `case.approve` | `N` | `-` |
| `case` | `reject` | `case.reject` | `N` | `-` |
| `case` | `request_info` | `case.request-info` | `N` | `-` |
| `case.tollgate` | `evaluate` | `tollgate.evaluate` | `N` | `-` |
| `mandate` | `create` | `mandate.create` | `N` | `-` |

### `struct.ie.ucits.icav`
| Slot | Verb Key | Verb FQN | Gated? | When Condition |
| --- | --- | --- | --- | --- |
| `cbu` | `create` | `cbu.create` | `N` | `-` |
| `cbu` | `read` | `cbu.read` | `N` | `-` |
| `cbu` | `show` | `cbu.show` | `N` | `-` |
| `management_company` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `management_company` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `management_company` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `management_company` | `add` | `party.add` | `Y` | `empty` |
| `management_company` | `show` | `entity.read` | `Y` | `filled` |
| `depositary` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `depositary` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `depositary` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `depositary` | `add` | `party.add` | `Y` | `empty` |
| `depositary` | `show` | `entity.read` | `Y` | `filled` |
| `investment_manager` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `investment_manager` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `investment_manager` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `investment_manager` | `add` | `party.add` | `Y` | `empty` |
| `investment_manager` | `show` | `entity.read` | `Y` | `filled` |
| `administrator` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `administrator` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `administrator` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `administrator` | `add` | `party.add` | `Y` | `empty` |
| `administrator` | `show` | `entity.read` | `Y` | `filled` |
| `auditor` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `auditor` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `auditor` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `auditor` | `add` | `party.add` | `Y` | `empty` |
| `auditor` | `show` | `entity.read` | `Y` | `filled` |
| `company_secretary` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `company_secretary` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `company_secretary` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `company_secretary` | `add` | `party.add` | `Y` | `empty` |
| `company_secretary` | `show` | `entity.read` | `Y` | `filled` |
| `legal_counsel` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `legal_counsel` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `legal_counsel` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `legal_counsel` | `add` | `party.add` | `Y` | `empty` |
| `legal_counsel` | `show` | `entity.read` | `Y` | `filled` |
| `ownership_chain` | `discover` | `ubo.discover` | `N` | `-` |
| `ownership_chain` | `allege` | `ubo.allege` | `N` | `-` |
| `ownership_chain` | `verify` | `ubo.verify` | `N` | `-` |
| `ownership_chain` | `promote` | `ubo.promote` | `N` | `-` |
| `ownership_chain` | `approve` | `ubo.approve` | `N` | `-` |
| `ownership_chain` | `reject` | `ubo.reject` | `N` | `-` |
| `case` | `open` | `case.open` | `N` | `-` |
| `case` | `submit` | `case.submit` | `N` | `-` |
| `case` | `approve` | `case.approve` | `N` | `-` |
| `case` | `reject` | `case.reject` | `N` | `-` |
| `case` | `request_info` | `case.request-info` | `N` | `-` |
| `case.tollgate` | `evaluate` | `tollgate.evaluate` | `N` | `-` |
| `mandate` | `create` | `mandate.create` | `N` | `-` |

### `struct.lux.aif.raif`
| Slot | Verb Key | Verb FQN | Gated? | When Condition |
| --- | --- | --- | --- | --- |
| `cbu` | `create` | `cbu.create` | `N` | `-` |
| `cbu` | `read` | `cbu.read` | `N` | `-` |
| `cbu` | `show` | `cbu.show` | `N` | `-` |
| `aifm` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `aifm` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `aifm` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `aifm` | `add` | `party.add` | `Y` | `empty` |
| `aifm` | `show` | `entity.read` | `Y` | `filled` |
| `depositary` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `depositary` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `depositary` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `depositary` | `add` | `party.add` | `Y` | `empty` |
| `depositary` | `show` | `entity.read` | `Y` | `filled` |
| `investment_manager` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `investment_manager` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `investment_manager` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `investment_manager` | `add` | `party.add` | `Y` | `empty` |
| `investment_manager` | `show` | `entity.read` | `Y` | `filled` |
| `administrator` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `administrator` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `administrator` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `administrator` | `add` | `party.add` | `Y` | `empty` |
| `administrator` | `show` | `entity.read` | `Y` | `filled` |
| `auditor` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `auditor` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `auditor` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `auditor` | `add` | `party.add` | `Y` | `empty` |
| `auditor` | `show` | `entity.read` | `Y` | `filled` |
| `prime_broker` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `prime_broker` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `prime_broker` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `prime_broker` | `add` | `party.add` | `Y` | `empty` |
| `prime_broker` | `show` | `entity.read` | `Y` | `filled` |
| `ownership_chain` | `discover` | `ubo.discover` | `N` | `-` |
| `ownership_chain` | `allege` | `ubo.allege` | `N` | `-` |
| `ownership_chain` | `verify` | `ubo.verify` | `N` | `-` |
| `ownership_chain` | `promote` | `ubo.promote` | `N` | `-` |
| `ownership_chain` | `approve` | `ubo.approve` | `N` | `-` |
| `ownership_chain` | `reject` | `ubo.reject` | `N` | `-` |
| `case` | `open` | `case.open` | `N` | `-` |
| `case` | `submit` | `case.submit` | `N` | `-` |
| `case` | `approve` | `case.approve` | `N` | `-` |
| `case` | `reject` | `case.reject` | `N` | `-` |
| `case` | `request_info` | `case.request-info` | `N` | `-` |
| `case.tollgate` | `evaluate` | `tollgate.evaluate` | `N` | `-` |
| `mandate` | `create` | `mandate.create` | `N` | `-` |

### `struct.lux.pe.scsp`
| Slot | Verb Key | Verb FQN | Gated? | When Condition |
| --- | --- | --- | --- | --- |
| `cbu` | `create` | `cbu.create` | `N` | `-` |
| `cbu` | `read` | `cbu.read` | `N` | `-` |
| `cbu` | `show` | `cbu.show` | `N` | `-` |
| `general_partner` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `general_partner` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `general_partner` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `general_partner` | `add` | `party.add` | `Y` | `empty` |
| `general_partner` | `show` | `entity.read` | `Y` | `filled` |
| `aifm` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `aifm` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `aifm` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `aifm` | `add` | `party.add` | `Y` | `empty` |
| `aifm` | `show` | `entity.read` | `Y` | `filled` |
| `depositary` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `depositary` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `depositary` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `depositary` | `add` | `party.add` | `Y` | `empty` |
| `depositary` | `show` | `entity.read` | `Y` | `filled` |
| `administrator` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `administrator` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `administrator` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `administrator` | `add` | `party.add` | `Y` | `empty` |
| `administrator` | `show` | `entity.read` | `Y` | `filled` |
| `auditor` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `auditor` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `auditor` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `auditor` | `add` | `party.add` | `Y` | `empty` |
| `auditor` | `show` | `entity.read` | `Y` | `filled` |
| `legal_counsel` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `legal_counsel` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `legal_counsel` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `legal_counsel` | `add` | `party.add` | `Y` | `empty` |
| `legal_counsel` | `show` | `entity.read` | `Y` | `filled` |
| `ownership_chain` | `discover` | `ubo.discover` | `N` | `-` |
| `ownership_chain` | `allege` | `ubo.allege` | `N` | `-` |
| `ownership_chain` | `verify` | `ubo.verify` | `N` | `-` |
| `ownership_chain` | `promote` | `ubo.promote` | `N` | `-` |
| `ownership_chain` | `approve` | `ubo.approve` | `N` | `-` |
| `ownership_chain` | `reject` | `ubo.reject` | `N` | `-` |
| `case` | `open` | `case.open` | `N` | `-` |
| `case` | `submit` | `case.submit` | `N` | `-` |
| `case` | `approve` | `case.approve` | `N` | `-` |
| `case` | `reject` | `case.reject` | `N` | `-` |
| `case` | `request_info` | `case.request-info` | `N` | `-` |
| `case.tollgate` | `evaluate` | `tollgate.evaluate` | `N` | `-` |
| `mandate` | `create` | `mandate.create` | `N` | `-` |

### `struct.lux.ucits.sicav`
| Slot | Verb Key | Verb FQN | Gated? | When Condition |
| --- | --- | --- | --- | --- |
| `cbu` | `create` | `cbu.create` | `N` | `-` |
| `cbu` | `read` | `cbu.read` | `N` | `-` |
| `cbu` | `show` | `cbu.show` | `N` | `-` |
| `management_company` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `management_company` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `management_company` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `management_company` | `add` | `party.add` | `Y` | `empty` |
| `management_company` | `show` | `entity.read` | `Y` | `filled` |
| `depositary` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `depositary` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `depositary` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `depositary` | `add` | `party.add` | `Y` | `empty` |
| `investment_manager` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `investment_manager` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `ownership_chain` | `discover` | `ubo.discover` | `N` | `-` |
| `ownership_chain` | `allege` | `ubo.allege` | `N` | `-` |
| `ownership_chain` | `verify` | `ubo.verify` | `N` | `-` |
| `ownership_chain` | `promote` | `ubo.promote` | `N` | `-` |
| `ownership_chain` | `approve` | `ubo.approve` | `N` | `-` |
| `ownership_chain` | `reject` | `ubo.reject` | `N` | `-` |
| `case` | `open` | `case.open` | `N` | `-` |
| `case` | `submit` | `case.submit` | `N` | `-` |
| `case` | `approve` | `case.approve` | `N` | `-` |
| `case` | `reject` | `case.reject` | `N` | `-` |
| `case` | `request_info` | `case.request-info` | `N` | `-` |
| `case.tollgate` | `evaluate` | `tollgate.evaluate` | `N` | `-` |
| `mandate` | `create` | `mandate.create` | `N` | `-` |

### `struct.pe.cross-border`
| Slot | Verb Key | Verb FQN | Gated? | When Condition |
| --- | --- | --- | --- | --- |
| `cbu` | `create` | `cbu.create` | `N` | `-` |
| `cbu` | `read` | `cbu.read` | `N` | `-` |
| `cbu` | `show` | `cbu.show` | `N` | `-` |
| `cbu.us_parallel` | `show` | `cbu.read` | `N` | `-` |
| `cbu.aggregator` | `show` | `cbu.read` | `N` | `-` |
| `general_partner` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `general_partner` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `general_partner` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `general_partner` | `add` | `party.add` | `Y` | `empty` |
| `general_partner` | `show` | `entity.read` | `Y` | `filled` |
| `aifm` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `aifm` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `aifm` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `aifm` | `add` | `party.add` | `Y` | `empty` |
| `aifm` | `show` | `entity.read` | `Y` | `filled` |
| `depositary` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `depositary` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `depositary` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `depositary` | `add` | `party.add` | `Y` | `empty` |
| `depositary` | `show` | `entity.read` | `Y` | `filled` |
| `administrator` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `administrator` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `administrator` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `administrator` | `add` | `party.add` | `Y` | `empty` |
| `administrator` | `show` | `entity.read` | `Y` | `filled` |
| `auditor` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `auditor` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `auditor` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `auditor` | `add` | `party.add` | `Y` | `empty` |
| `auditor` | `show` | `entity.read` | `Y` | `filled` |
| `legal_counsel` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `legal_counsel` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `legal_counsel` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `legal_counsel` | `add` | `party.add` | `Y` | `empty` |
| `legal_counsel` | `show` | `entity.read` | `Y` | `filled` |
| `ownership_chain` | `discover` | `ubo.discover` | `N` | `-` |
| `ownership_chain` | `allege` | `ubo.allege` | `N` | `-` |
| `ownership_chain` | `verify` | `ubo.verify` | `N` | `-` |
| `ownership_chain` | `promote` | `ubo.promote` | `N` | `-` |
| `ownership_chain` | `approve` | `ubo.approve` | `N` | `-` |
| `ownership_chain` | `reject` | `ubo.reject` | `N` | `-` |
| `case` | `open` | `case.open` | `N` | `-` |
| `case` | `submit` | `case.submit` | `N` | `-` |
| `case` | `approve` | `case.approve` | `N` | `-` |
| `case` | `reject` | `case.reject` | `N` | `-` |
| `case` | `request_info` | `case.request-info` | `N` | `-` |
| `case.tollgate` | `evaluate` | `tollgate.evaluate` | `N` | `-` |
| `mandate` | `create` | `mandate.create` | `N` | `-` |

### `struct.uk.authorised.acs`
| Slot | Verb Key | Verb FQN | Gated? | When Condition |
| --- | --- | --- | --- | --- |
| `cbu` | `create` | `cbu.create` | `N` | `-` |
| `cbu` | `read` | `cbu.read` | `N` | `-` |
| `cbu` | `show` | `cbu.show` | `N` | `-` |
| `acs_operator` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `acs_operator` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `acs_operator` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `acs_operator` | `add` | `party.add` | `Y` | `empty` |
| `acs_operator` | `show` | `entity.read` | `Y` | `filled` |
| `depositary` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `depositary` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `depositary` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `depositary` | `add` | `party.add` | `Y` | `empty` |
| `depositary` | `show` | `entity.read` | `Y` | `filled` |
| `investment_manager` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `investment_manager` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `investment_manager` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `investment_manager` | `add` | `party.add` | `Y` | `empty` |
| `investment_manager` | `show` | `entity.read` | `Y` | `filled` |
| `administrator` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `administrator` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `administrator` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `administrator` | `add` | `party.add` | `Y` | `empty` |
| `administrator` | `show` | `entity.read` | `Y` | `filled` |
| `auditor` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `auditor` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `auditor` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `auditor` | `add` | `party.add` | `Y` | `empty` |
| `auditor` | `show` | `entity.read` | `Y` | `filled` |
| `ownership_chain` | `discover` | `ubo.discover` | `N` | `-` |
| `ownership_chain` | `allege` | `ubo.allege` | `N` | `-` |
| `ownership_chain` | `verify` | `ubo.verify` | `N` | `-` |
| `ownership_chain` | `promote` | `ubo.promote` | `N` | `-` |
| `ownership_chain` | `approve` | `ubo.approve` | `N` | `-` |
| `ownership_chain` | `reject` | `ubo.reject` | `N` | `-` |
| `case` | `open` | `case.open` | `N` | `-` |
| `case` | `submit` | `case.submit` | `N` | `-` |
| `case` | `approve` | `case.approve` | `N` | `-` |
| `case` | `reject` | `case.reject` | `N` | `-` |
| `case` | `request_info` | `case.request-info` | `N` | `-` |
| `case.tollgate` | `evaluate` | `tollgate.evaluate` | `N` | `-` |
| `mandate` | `create` | `mandate.create` | `N` | `-` |

### `struct.uk.authorised.aut`
| Slot | Verb Key | Verb FQN | Gated? | When Condition |
| --- | --- | --- | --- | --- |
| `cbu` | `create` | `cbu.create` | `N` | `-` |
| `cbu` | `read` | `cbu.read` | `N` | `-` |
| `cbu` | `show` | `cbu.show` | `N` | `-` |
| `authorised_fund_manager` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `authorised_fund_manager` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `authorised_fund_manager` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `authorised_fund_manager` | `add` | `party.add` | `Y` | `empty` |
| `authorised_fund_manager` | `show` | `entity.read` | `Y` | `filled` |
| `trustee` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `trustee` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `trustee` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `trustee` | `add` | `party.add` | `Y` | `empty` |
| `trustee` | `show` | `entity.read` | `Y` | `filled` |
| `investment_manager` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `investment_manager` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `investment_manager` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `investment_manager` | `add` | `party.add` | `Y` | `empty` |
| `investment_manager` | `show` | `entity.read` | `Y` | `filled` |
| `administrator` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `administrator` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `administrator` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `administrator` | `add` | `party.add` | `Y` | `empty` |
| `administrator` | `show` | `entity.read` | `Y` | `filled` |
| `auditor` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `auditor` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `auditor` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `auditor` | `add` | `party.add` | `Y` | `empty` |
| `auditor` | `show` | `entity.read` | `Y` | `filled` |
| `ownership_chain` | `discover` | `ubo.discover` | `N` | `-` |
| `ownership_chain` | `allege` | `ubo.allege` | `N` | `-` |
| `ownership_chain` | `verify` | `ubo.verify` | `N` | `-` |
| `ownership_chain` | `promote` | `ubo.promote` | `N` | `-` |
| `ownership_chain` | `approve` | `ubo.approve` | `N` | `-` |
| `ownership_chain` | `reject` | `ubo.reject` | `N` | `-` |
| `case` | `open` | `case.open` | `N` | `-` |
| `case` | `submit` | `case.submit` | `N` | `-` |
| `case` | `approve` | `case.approve` | `N` | `-` |
| `case` | `reject` | `case.reject` | `N` | `-` |
| `case` | `request_info` | `case.request-info` | `N` | `-` |
| `case.tollgate` | `evaluate` | `tollgate.evaluate` | `N` | `-` |
| `mandate` | `create` | `mandate.create` | `N` | `-` |

### `struct.uk.authorised.ltaf`
| Slot | Verb Key | Verb FQN | Gated? | When Condition |
| --- | --- | --- | --- | --- |
| `cbu` | `create` | `cbu.create` | `N` | `-` |
| `cbu` | `read` | `cbu.read` | `N` | `-` |
| `cbu` | `show` | `cbu.show` | `N` | `-` |
| `authorised_corporate_director` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `authorised_corporate_director` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `authorised_corporate_director` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `authorised_corporate_director` | `add` | `party.add` | `Y` | `empty` |
| `authorised_corporate_director` | `show` | `entity.read` | `Y` | `filled` |
| `depositary` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `depositary` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `depositary` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `depositary` | `add` | `party.add` | `Y` | `empty` |
| `depositary` | `show` | `entity.read` | `Y` | `filled` |
| `investment_manager` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `investment_manager` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `investment_manager` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `investment_manager` | `add` | `party.add` | `Y` | `empty` |
| `investment_manager` | `show` | `entity.read` | `Y` | `filled` |
| `administrator` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `administrator` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `administrator` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `administrator` | `add` | `party.add` | `Y` | `empty` |
| `administrator` | `show` | `entity.read` | `Y` | `filled` |
| `auditor` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `auditor` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `auditor` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `auditor` | `add` | `party.add` | `Y` | `empty` |
| `auditor` | `show` | `entity.read` | `Y` | `filled` |
| `registrar` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `registrar` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `registrar` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `registrar` | `add` | `party.add` | `Y` | `empty` |
| `registrar` | `show` | `entity.read` | `Y` | `filled` |
| `valuation_agent` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `valuation_agent` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `valuation_agent` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `valuation_agent` | `add` | `party.add` | `Y` | `empty` |
| `valuation_agent` | `show` | `entity.read` | `Y` | `filled` |
| `ownership_chain` | `discover` | `ubo.discover` | `N` | `-` |
| `ownership_chain` | `allege` | `ubo.allege` | `N` | `-` |
| `ownership_chain` | `verify` | `ubo.verify` | `N` | `-` |
| `ownership_chain` | `promote` | `ubo.promote` | `N` | `-` |
| `ownership_chain` | `approve` | `ubo.approve` | `N` | `-` |
| `ownership_chain` | `reject` | `ubo.reject` | `N` | `-` |
| `case` | `open` | `case.open` | `N` | `-` |
| `case` | `submit` | `case.submit` | `N` | `-` |
| `case` | `approve` | `case.approve` | `N` | `-` |
| `case` | `reject` | `case.reject` | `N` | `-` |
| `case` | `request_info` | `case.request-info` | `N` | `-` |
| `case.tollgate` | `evaluate` | `tollgate.evaluate` | `N` | `-` |
| `mandate` | `create` | `mandate.create` | `N` | `-` |

### `struct.uk.authorised.oeic`
| Slot | Verb Key | Verb FQN | Gated? | When Condition |
| --- | --- | --- | --- | --- |
| `cbu` | `create` | `cbu.create` | `N` | `-` |
| `cbu` | `read` | `cbu.read` | `N` | `-` |
| `cbu` | `show` | `cbu.show` | `N` | `-` |
| `authorised_corporate_director` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `authorised_corporate_director` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `authorised_corporate_director` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `authorised_corporate_director` | `add` | `party.add` | `Y` | `empty` |
| `authorised_corporate_director` | `show` | `entity.read` | `Y` | `filled` |
| `depositary` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `depositary` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `depositary` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `depositary` | `add` | `party.add` | `Y` | `empty` |
| `depositary` | `show` | `entity.read` | `Y` | `filled` |
| `investment_manager` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `investment_manager` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `investment_manager` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `investment_manager` | `add` | `party.add` | `Y` | `empty` |
| `investment_manager` | `show` | `entity.read` | `Y` | `filled` |
| `administrator` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `administrator` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `administrator` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `administrator` | `add` | `party.add` | `Y` | `empty` |
| `administrator` | `show` | `entity.read` | `Y` | `filled` |
| `auditor` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `auditor` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `auditor` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `auditor` | `add` | `party.add` | `Y` | `empty` |
| `auditor` | `show` | `entity.read` | `Y` | `filled` |
| `registrar` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `registrar` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `registrar` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `registrar` | `add` | `party.add` | `Y` | `empty` |
| `registrar` | `show` | `entity.read` | `Y` | `filled` |
| `ownership_chain` | `discover` | `ubo.discover` | `N` | `-` |
| `ownership_chain` | `allege` | `ubo.allege` | `N` | `-` |
| `ownership_chain` | `verify` | `ubo.verify` | `N` | `-` |
| `ownership_chain` | `promote` | `ubo.promote` | `N` | `-` |
| `ownership_chain` | `approve` | `ubo.approve` | `N` | `-` |
| `ownership_chain` | `reject` | `ubo.reject` | `N` | `-` |
| `case` | `open` | `case.open` | `N` | `-` |
| `case` | `submit` | `case.submit` | `N` | `-` |
| `case` | `approve` | `case.approve` | `N` | `-` |
| `case` | `reject` | `case.reject` | `N` | `-` |
| `case` | `request_info` | `case.request-info` | `N` | `-` |
| `case.tollgate` | `evaluate` | `tollgate.evaluate` | `N` | `-` |
| `mandate` | `create` | `mandate.create` | `N` | `-` |

### `struct.uk.manager.llp`
| Slot | Verb Key | Verb FQN | Gated? | When Condition |
| --- | --- | --- | --- | --- |
| `cbu` | `create` | `cbu.create` | `N` | `-` |
| `cbu` | `read` | `cbu.read` | `N` | `-` |
| `cbu` | `show` | `cbu.show` | `N` | `-` |
| `designated_member_1` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `designated_member_1` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `designated_member_1` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `designated_member_1` | `add` | `party.add` | `Y` | `empty` |
| `designated_member_1` | `show` | `entity.read` | `Y` | `filled` |
| `designated_member_2` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `designated_member_2` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `designated_member_2` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `designated_member_2` | `add` | `party.add` | `Y` | `empty` |
| `designated_member_2` | `show` | `entity.read` | `Y` | `filled` |
| `compliance_officer` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `compliance_officer` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `compliance_officer` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `compliance_officer` | `add` | `party.add` | `Y` | `empty` |
| `compliance_officer` | `show` | `entity.read` | `Y` | `filled` |
| `mlro` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `mlro` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `mlro` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `mlro` | `add` | `party.add` | `Y` | `empty` |
| `mlro` | `show` | `entity.read` | `Y` | `filled` |
| `auditor` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `auditor` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `auditor` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `auditor` | `add` | `party.add` | `Y` | `empty` |
| `auditor` | `show` | `entity.read` | `Y` | `filled` |
| `ownership_chain` | `discover` | `ubo.discover` | `N` | `-` |
| `ownership_chain` | `allege` | `ubo.allege` | `N` | `-` |
| `ownership_chain` | `verify` | `ubo.verify` | `N` | `-` |
| `ownership_chain` | `promote` | `ubo.promote` | `N` | `-` |
| `ownership_chain` | `approve` | `ubo.approve` | `N` | `-` |
| `ownership_chain` | `reject` | `ubo.reject` | `N` | `-` |
| `case` | `open` | `case.open` | `N` | `-` |
| `case` | `submit` | `case.submit` | `N` | `-` |
| `case` | `approve` | `case.approve` | `N` | `-` |
| `case` | `reject` | `case.reject` | `N` | `-` |
| `case` | `request_info` | `case.request-info` | `N` | `-` |
| `case.tollgate` | `evaluate` | `tollgate.evaluate` | `N` | `-` |

### `struct.uk.private-equity.lp`
| Slot | Verb Key | Verb FQN | Gated? | When Condition |
| --- | --- | --- | --- | --- |
| `cbu` | `create` | `cbu.create` | `N` | `-` |
| `cbu` | `read` | `cbu.read` | `N` | `-` |
| `cbu` | `show` | `cbu.show` | `N` | `-` |
| `general_partner` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `general_partner` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `general_partner` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `general_partner` | `add` | `party.add` | `Y` | `empty` |
| `general_partner` | `show` | `entity.read` | `Y` | `filled` |
| `aifm` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `aifm` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `aifm` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `aifm` | `add` | `party.add` | `Y` | `empty` |
| `aifm` | `show` | `entity.read` | `Y` | `filled` |
| `depositary` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `depositary` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `depositary` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `depositary` | `add` | `party.add` | `Y` | `empty` |
| `depositary` | `show` | `entity.read` | `Y` | `filled` |
| `administrator` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `administrator` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `administrator` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `administrator` | `add` | `party.add` | `Y` | `empty` |
| `administrator` | `show` | `entity.read` | `Y` | `filled` |
| `auditor` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `auditor` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `auditor` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `auditor` | `add` | `party.add` | `Y` | `empty` |
| `auditor` | `show` | `entity.read` | `Y` | `filled` |
| `legal_counsel` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `legal_counsel` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `legal_counsel` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `legal_counsel` | `add` | `party.add` | `Y` | `empty` |
| `legal_counsel` | `show` | `entity.read` | `Y` | `filled` |
| `ownership_chain` | `discover` | `ubo.discover` | `N` | `-` |
| `ownership_chain` | `allege` | `ubo.allege` | `N` | `-` |
| `ownership_chain` | `verify` | `ubo.verify` | `N` | `-` |
| `ownership_chain` | `promote` | `ubo.promote` | `N` | `-` |
| `ownership_chain` | `approve` | `ubo.approve` | `N` | `-` |
| `ownership_chain` | `reject` | `ubo.reject` | `N` | `-` |
| `case` | `open` | `case.open` | `N` | `-` |
| `case` | `submit` | `case.submit` | `N` | `-` |
| `case` | `approve` | `case.approve` | `N` | `-` |
| `case` | `reject` | `case.reject` | `N` | `-` |
| `case` | `request_info` | `case.request-info` | `N` | `-` |
| `case.tollgate` | `evaluate` | `tollgate.evaluate` | `N` | `-` |
| `mandate` | `create` | `mandate.create` | `N` | `-` |

### `struct.us.40act.closed-end`
| Slot | Verb Key | Verb FQN | Gated? | When Condition |
| --- | --- | --- | --- | --- |
| `cbu` | `create` | `cbu.create` | `N` | `-` |
| `cbu` | `read` | `cbu.read` | `N` | `-` |
| `cbu` | `show` | `cbu.show` | `N` | `-` |
| `investment_adviser` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `investment_adviser` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `investment_adviser` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `investment_adviser` | `add` | `party.add` | `Y` | `empty` |
| `investment_adviser` | `show` | `entity.read` | `Y` | `filled` |
| `custodian` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `custodian` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `custodian` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `custodian` | `add` | `party.add` | `Y` | `empty` |
| `custodian` | `show` | `entity.read` | `Y` | `filled` |
| `sub_adviser` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `sub_adviser` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `sub_adviser` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `sub_adviser` | `add` | `party.add` | `Y` | `empty` |
| `sub_adviser` | `show` | `entity.read` | `Y` | `filled` |
| `administrator` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `administrator` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `administrator` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `administrator` | `add` | `party.add` | `Y` | `empty` |
| `administrator` | `show` | `entity.read` | `Y` | `filled` |
| `transfer_agent` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `transfer_agent` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `transfer_agent` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `transfer_agent` | `add` | `party.add` | `Y` | `empty` |
| `transfer_agent` | `show` | `entity.read` | `Y` | `filled` |
| `auditor` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `auditor` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `auditor` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `auditor` | `add` | `party.add` | `Y` | `empty` |
| `auditor` | `show` | `entity.read` | `Y` | `filled` |
| `legal_counsel` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `legal_counsel` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `legal_counsel` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `legal_counsel` | `add` | `party.add` | `Y` | `empty` |
| `legal_counsel` | `show` | `entity.read` | `Y` | `filled` |
| `ownership_chain` | `discover` | `ubo.discover` | `N` | `-` |
| `ownership_chain` | `allege` | `ubo.allege` | `N` | `-` |
| `ownership_chain` | `verify` | `ubo.verify` | `N` | `-` |
| `ownership_chain` | `promote` | `ubo.promote` | `N` | `-` |
| `ownership_chain` | `approve` | `ubo.approve` | `N` | `-` |
| `ownership_chain` | `reject` | `ubo.reject` | `N` | `-` |
| `case` | `open` | `case.open` | `N` | `-` |
| `case` | `submit` | `case.submit` | `N` | `-` |
| `case` | `approve` | `case.approve` | `N` | `-` |
| `case` | `reject` | `case.reject` | `N` | `-` |
| `case` | `request_info` | `case.request-info` | `N` | `-` |
| `case.tollgate` | `evaluate` | `tollgate.evaluate` | `N` | `-` |
| `mandate` | `create` | `mandate.create` | `N` | `-` |

### `struct.us.40act.open-end`
| Slot | Verb Key | Verb FQN | Gated? | When Condition |
| --- | --- | --- | --- | --- |
| `cbu` | `create` | `cbu.create` | `N` | `-` |
| `cbu` | `read` | `cbu.read` | `N` | `-` |
| `cbu` | `show` | `cbu.show` | `N` | `-` |
| `investment_adviser` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `investment_adviser` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `investment_adviser` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `investment_adviser` | `add` | `party.add` | `Y` | `empty` |
| `investment_adviser` | `show` | `entity.read` | `Y` | `filled` |
| `custodian` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `custodian` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `custodian` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `custodian` | `add` | `party.add` | `Y` | `empty` |
| `custodian` | `show` | `entity.read` | `Y` | `filled` |
| `sub_adviser` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `sub_adviser` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `sub_adviser` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `sub_adviser` | `add` | `party.add` | `Y` | `empty` |
| `sub_adviser` | `show` | `entity.read` | `Y` | `filled` |
| `administrator` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `administrator` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `administrator` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `administrator` | `add` | `party.add` | `Y` | `empty` |
| `administrator` | `show` | `entity.read` | `Y` | `filled` |
| `transfer_agent` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `transfer_agent` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `transfer_agent` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `transfer_agent` | `add` | `party.add` | `Y` | `empty` |
| `transfer_agent` | `show` | `entity.read` | `Y` | `filled` |
| `distributor` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `distributor` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `distributor` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `distributor` | `add` | `party.add` | `Y` | `empty` |
| `distributor` | `show` | `entity.read` | `Y` | `filled` |
| `auditor` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `auditor` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `auditor` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `auditor` | `add` | `party.add` | `Y` | `empty` |
| `auditor` | `show` | `entity.read` | `Y` | `filled` |
| `legal_counsel` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `legal_counsel` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `legal_counsel` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `legal_counsel` | `add` | `party.add` | `Y` | `empty` |
| `legal_counsel` | `show` | `entity.read` | `Y` | `filled` |
| `ownership_chain` | `discover` | `ubo.discover` | `N` | `-` |
| `ownership_chain` | `allege` | `ubo.allege` | `N` | `-` |
| `ownership_chain` | `verify` | `ubo.verify` | `N` | `-` |
| `ownership_chain` | `promote` | `ubo.promote` | `N` | `-` |
| `ownership_chain` | `approve` | `ubo.approve` | `N` | `-` |
| `ownership_chain` | `reject` | `ubo.reject` | `N` | `-` |
| `case` | `open` | `case.open` | `N` | `-` |
| `case` | `submit` | `case.submit` | `N` | `-` |
| `case` | `approve` | `case.approve` | `N` | `-` |
| `case` | `reject` | `case.reject` | `N` | `-` |
| `case` | `request_info` | `case.request-info` | `N` | `-` |
| `case.tollgate` | `evaluate` | `tollgate.evaluate` | `N` | `-` |
| `mandate` | `create` | `mandate.create` | `N` | `-` |

### `struct.us.etf.40act`
| Slot | Verb Key | Verb FQN | Gated? | When Condition |
| --- | --- | --- | --- | --- |
| `cbu` | `create` | `cbu.create` | `N` | `-` |
| `cbu` | `read` | `cbu.read` | `N` | `-` |
| `cbu` | `show` | `cbu.show` | `N` | `-` |
| `investment_adviser` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `investment_adviser` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `investment_adviser` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `investment_adviser` | `add` | `party.add` | `Y` | `empty` |
| `investment_adviser` | `show` | `entity.read` | `Y` | `filled` |
| `custodian` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `custodian` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `custodian` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `custodian` | `add` | `party.add` | `Y` | `empty` |
| `custodian` | `show` | `entity.read` | `Y` | `filled` |
| `authorized_participant` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `authorized_participant` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `authorized_participant` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `authorized_participant` | `add` | `party.add` | `Y` | `empty` |
| `authorized_participant` | `show` | `entity.read` | `Y` | `filled` |
| `sub_adviser` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `sub_adviser` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `sub_adviser` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `sub_adviser` | `add` | `party.add` | `Y` | `empty` |
| `sub_adviser` | `show` | `entity.read` | `Y` | `filled` |
| `administrator` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `administrator` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `administrator` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `administrator` | `add` | `party.add` | `Y` | `empty` |
| `administrator` | `show` | `entity.read` | `Y` | `filled` |
| `transfer_agent` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `transfer_agent` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `transfer_agent` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `transfer_agent` | `add` | `party.add` | `Y` | `empty` |
| `transfer_agent` | `show` | `entity.read` | `Y` | `filled` |
| `distributor` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `distributor` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `distributor` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `distributor` | `add` | `party.add` | `Y` | `empty` |
| `distributor` | `show` | `entity.read` | `Y` | `filled` |
| `auditor` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `auditor` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `auditor` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `auditor` | `add` | `party.add` | `Y` | `empty` |
| `auditor` | `show` | `entity.read` | `Y` | `filled` |
| `market_maker` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `market_maker` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `market_maker` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `market_maker` | `add` | `party.add` | `Y` | `empty` |
| `market_maker` | `show` | `entity.read` | `Y` | `filled` |
| `ownership_chain` | `discover` | `ubo.discover` | `N` | `-` |
| `ownership_chain` | `allege` | `ubo.allege` | `N` | `-` |
| `ownership_chain` | `verify` | `ubo.verify` | `N` | `-` |
| `ownership_chain` | `promote` | `ubo.promote` | `N` | `-` |
| `ownership_chain` | `approve` | `ubo.approve` | `N` | `-` |
| `ownership_chain` | `reject` | `ubo.reject` | `N` | `-` |
| `case` | `open` | `case.open` | `N` | `-` |
| `case` | `submit` | `case.submit` | `N` | `-` |
| `case` | `approve` | `case.approve` | `N` | `-` |
| `case` | `reject` | `case.reject` | `N` | `-` |
| `case` | `request_info` | `case.request-info` | `N` | `-` |
| `case.tollgate` | `evaluate` | `tollgate.evaluate` | `N` | `-` |
| `mandate` | `create` | `mandate.create` | `N` | `-` |

### `struct.us.private-fund.delaware-lp`
| Slot | Verb Key | Verb FQN | Gated? | When Condition |
| --- | --- | --- | --- | --- |
| `cbu` | `create` | `cbu.create` | `N` | `-` |
| `cbu` | `read` | `cbu.read` | `N` | `-` |
| `cbu` | `show` | `cbu.show` | `N` | `-` |
| `general_partner` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `general_partner` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `general_partner` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `general_partner` | `add` | `party.add` | `Y` | `empty` |
| `general_partner` | `show` | `entity.read` | `Y` | `filled` |
| `investment_manager` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `investment_manager` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `investment_manager` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `investment_manager` | `add` | `party.add` | `Y` | `empty` |
| `investment_manager` | `show` | `entity.read` | `Y` | `filled` |
| `custodian` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `custodian` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `custodian` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `custodian` | `add` | `party.add` | `Y` | `empty` |
| `custodian` | `show` | `entity.read` | `Y` | `filled` |
| `administrator` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `administrator` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `administrator` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `administrator` | `add` | `party.add` | `Y` | `empty` |
| `administrator` | `show` | `entity.read` | `Y` | `filled` |
| `prime_broker` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `prime_broker` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `prime_broker` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `prime_broker` | `add` | `party.add` | `Y` | `empty` |
| `prime_broker` | `show` | `entity.read` | `Y` | `filled` |
| `auditor` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `auditor` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `auditor` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `auditor` | `add` | `party.add` | `Y` | `empty` |
| `auditor` | `show` | `entity.read` | `Y` | `filled` |
| `legal_counsel` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `legal_counsel` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `legal_counsel` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `legal_counsel` | `add` | `party.add` | `Y` | `empty` |
| `legal_counsel` | `show` | `entity.read` | `Y` | `filled` |
| `tax_advisor` | `ensure` | `entity.ensure-or-placeholder` | `Y` | `empty` |
| `tax_advisor` | `assign` | `cbu.assign-role` | `Y` | `placeholder` |
| `tax_advisor` | `search` | `party.search` | `Y` | `placeholder, filled` |
| `tax_advisor` | `add` | `party.add` | `Y` | `empty` |
| `tax_advisor` | `show` | `entity.read` | `Y` | `filled` |
| `ownership_chain` | `discover` | `ubo.discover` | `N` | `-` |
| `ownership_chain` | `allege` | `ubo.allege` | `N` | `-` |
| `ownership_chain` | `verify` | `ubo.verify` | `N` | `-` |
| `ownership_chain` | `promote` | `ubo.promote` | `N` | `-` |
| `ownership_chain` | `approve` | `ubo.approve` | `N` | `-` |
| `ownership_chain` | `reject` | `ubo.reject` | `N` | `-` |
| `case` | `open` | `case.open` | `N` | `-` |
| `case` | `submit` | `case.submit` | `N` | `-` |
| `case` | `approve` | `case.approve` | `N` | `-` |
| `case` | `reject` | `case.reject` | `N` | `-` |
| `case` | `request_info` | `case.request-info` | `N` | `-` |
| `case.tollgate` | `evaluate` | `tollgate.evaluate` | `N` | `-` |
| `mandate` | `create` | `mandate.create` | `N` | `-` |

### `trading.streetside`
| Slot | Verb Key | Verb FQN | Gated? | When Condition |
| --- | --- | --- | --- | --- |
| `cbu` | `read` | `cbu.read` | `Y` | `filled` |
| `trading_profile` | `import` | `trading-profile.import` | `Y` | `empty` |
| `trading_profile` | `create_draft` | `trading-profile.create-draft` | `Y` | `empty, filled` |
| `trading_profile` | `read` | `trading-profile.read` | `Y` | `filled` |
| `trading_profile` | `get_active` | `trading-profile.get-active` | `Y` | `filled` |
| `trading_profile` | `list_versions` | `trading-profile.list-versions` | `Y` | `filled` |
| `trading_profile` | `materialize` | `trading-profile.materialize` | `Y` | `filled` |
| `trading_profile` | `activate` | `trading-profile.activate` | `Y` | `filled` |
| `trading_profile` | `diff` | `trading-profile.diff` | `Y` | `filled` |
| `trading_profile` | `clone` | `trading-profile.clone-to` | `Y` | `filled` |
| `trading_profile` | `new_version` | `trading-profile.create-new-version` | `Y` | `filled` |
| `trading_profile` | `add_component` | `trading-profile.add-component` | `Y` | `filled` |
| `trading_profile` | `remove_component` | `trading-profile.remove-component` | `Y` | `filled` |
| `trading_profile` | `set_base_currency` | `trading-profile.set-base-currency` | `Y` | `filled` |
| `trading_profile` | `link_csa_ssi` | `trading-profile.link-csa-ssi` | `Y` | `filled` |
| `trading_profile` | `update_im_scope` | `trading-profile.update-im-scope` | `Y` | `filled` |
| `trading_profile` | `ca_add_cutoff_rule` | `trading-profile.ca.add-cutoff-rule` | `Y` | `filled` |
| `trading_profile` | `ca_remove_cutoff_rule` | `trading-profile.ca.remove-cutoff-rule` | `Y` | `filled` |
| `trading_profile` | `ca_enable_event_types` | `trading-profile.ca.enable-event-types` | `Y` | `filled` |
| `trading_profile` | `ca_disable_event_types` | `trading-profile.ca.disable-event-types` | `Y` | `filled` |
| `trading_profile` | `ca_set_default_option` | `trading-profile.ca.set-default-option` | `Y` | `filled` |
| `trading_profile` | `ca_remove_default_option` | `trading-profile.ca.remove-default-option` | `Y` | `filled` |
| `trading_profile` | `ca_link_proceeds_ssi` | `trading-profile.ca.link-proceeds-ssi` | `Y` | `filled` |
| `trading_profile` | `ca_remove_proceeds_ssi` | `trading-profile.ca.remove-proceeds-ssi` | `Y` | `filled` |
| `trading_profile` | `validate_golive` | `trading-profile.validate-go-live-ready` | `Y` | `filled` |
| `trading_profile` | `validate_coverage` | `trading-profile.validate-universe-coverage` | `Y` | `filled` |
| `trading_profile` | `submit` | `trading-profile.submit` | `Y` | `filled` |
| `trading_profile` | `approve` | `trading-profile.approve` | `Y` | `filled` |
| `trading_profile` | `reject` | `trading-profile.reject` | `Y` | `filled` |
| `trading_profile` | `archive` | `trading-profile.archive` | `Y` | `filled` |
| `trading_profile` | `overlay_create` | `matrix-overlay.create` | `Y` | `empty, filled` |
| `trading_profile` | `overlay_read` | `matrix-overlay.read` | `Y` | `filled` |
| `trading_profile` | `overlay_list` | `matrix-overlay.list` | `Y` | `filled` |
| `trading_profile` | `overlay_update` | `matrix-overlay.update` | `Y` | `filled` |
| `trading_profile` | `overlay_apply` | `matrix-overlay.apply` | `Y` | `filled` |
| `trading_profile` | `overlay_remove` | `matrix-overlay.remove` | `Y` | `filled` |
| `trading_profile` | `overlay_diff` | `matrix-overlay.diff` | `Y` | `filled` |
| `trading_profile` | `overlay_preview` | `matrix-overlay.preview` | `Y` | `filled` |
| `trading_profile` | `overlay_list_active` | `matrix-overlay.list-active` | `Y` | `filled` |
| `custody` | `list_universe` | `custody.list-universe` | `Y` | `empty, filled` |
| `custody` | `list_ssis` | `custody.list-ssis` | `Y` | `filled` |
| `custody` | `list_booking_rules` | `custody.list-booking-rules` | `Y` | `filled` |
| `custody` | `list_overrides` | `custody.list-agent-overrides` | `Y` | `filled` |
| `custody` | `derive_coverage` | `custody.derive-required-coverage` | `Y` | `filled` |
| `custody` | `validate` | `custody.validate-booking-coverage` | `Y` | `filled` |
| `custody` | `lookup_ssi` | `custody.lookup-ssi` | `Y` | `filled` |
| `custody` | `setup_ssi` | `custody.setup-ssi` | `Y` | `empty, filled` |
| `booking_principal` | `create` | `booking-principal.create` | `Y` | `empty` |
| `booking_principal` | `update` | `booking-principal.update` | `Y` | `filled` |
| `booking_principal` | `retire` | `booking-principal.retire` | `Y` | `filled` |
| `booking_principal` | `evaluate` | `booking-principal.evaluate` | `Y` | `filled` |
| `booking_principal` | `select` | `booking-principal.select` | `Y` | `filled` |
| `booking_principal` | `explain` | `booking-principal.explain` | `Y` | `filled` |
| `booking_principal` | `coverage` | `booking-principal.coverage-matrix` | `Y` | `filled` |
| `booking_principal` | `gaps` | `booking-principal.gap-report` | `Y` | `filled` |
| `booking_principal` | `impact` | `booking-principal.impact-analysis` | `Y` | `filled` |
| `cash_sweep` | `configure` | `cash-sweep.configure` | `Y` | `empty` |
| `cash_sweep` | `link` | `cash-sweep.link-resource` | `Y` | `filled` |
| `cash_sweep` | `list` | `cash-sweep.list` | `Y` | `empty, filled` |
| `cash_sweep` | `update_threshold` | `cash-sweep.update-threshold` | `Y` | `filled` |
| `cash_sweep` | `update_timing` | `cash-sweep.update-timing` | `Y` | `filled` |
| `cash_sweep` | `change_vehicle` | `cash-sweep.change-vehicle` | `Y` | `filled` |
| `cash_sweep` | `suspend` | `cash-sweep.suspend` | `Y` | `filled` |
| `cash_sweep` | `reactivate` | `cash-sweep.reactivate` | `Y` | `filled` |
| `cash_sweep` | `remove` | `cash-sweep.remove` | `Y` | `filled` |
| `service_resource` | `read` | `service-resource.read` | `Y` | `filled` |
| `service_resource` | `list` | `service-resource.list` | `Y` | `empty, filled` |
| `service_resource` | `provision` | `service-resource.provision` | `Y` | `empty, filled` |
| `service_resource` | `set_attr` | `service-resource.set-attr` | `Y` | `filled` |
| `service_resource` | `activate` | `service-resource.activate` | `Y` | `filled` |
| `service_resource` | `suspend` | `service-resource.suspend` | `Y` | `filled` |
| `service_resource` | `decommission` | `service-resource.decommission` | `Y` | `filled` |
| `service_resource` | `validate` | `service-resource.validate-attrs` | `Y` | `filled` |
| `service_intent` | `create` | `service-intent.create` | `Y` | `empty` |
| `service_intent` | `read` | `service-intent.read` | `Y` | `filled` |
| `service_intent` | `list` | `service-intent.list` | `Y` | `empty, filled` |
| `service_intent` | `update` | `service-intent.update` | `Y` | `filled` |
| `service_intent` | `approve` | `service-intent.approve` | `Y` | `filled` |
| `service_intent` | `reject` | `service-intent.reject` | `Y` | `filled` |
| `service_intent` | `cancel` | `service-intent.cancel` | `Y` | `filled` |
| `service_intent` | `list_available` | `service-intent.list-available` | `Y` | `empty, filled` |
| `service_intent` | `list_by_status` | `service-intent.list-by-status` | `Y` | `filled` |
| `service_intent` | `activate` | `service-intent.activate` | `Y` | `filled` |
| `service_intent` | `deactivate` | `service-intent.deactivate` | `Y` | `filled` |
| `service_intent` | `clone` | `service-intent.clone` | `Y` | `filled` |
| `booking_location` | `create` | `booking-location.create` | `Y` | `empty` |
| `booking_location` | `read` | `booking-location.read` | `Y` | `filled` |
| `booking_location` | `list` | `booking-location.list` | `Y` | `empty, filled` |
| `legal_entity` | `create` | `legal-entity.create` | `Y` | `empty` |
| `legal_entity` | `read` | `legal-entity.read` | `Y` | `filled` |
| `legal_entity` | `list` | `legal-entity.list` | `Y` | `empty, filled` |
| `product` | `create` | `product.create` | `Y` | `empty` |
| `product` | `list` | `product.list` | `Y` | `empty, filled` |
| `delivery` | `create` | `delivery.create` | `Y` | `empty` |
| `delivery` | `read` | `delivery.read` | `Y` | `filled` |
| `delivery` | `list` | `delivery.list` | `Y` | `empty, filled` |

## 6. Schema Tables for Constellation Entities
### Entity Kind: `company`
- Table name: `"ob-poc".entities`
  - Columns: `TABLE NOT FOUND IN schema_export.sql`
  - Foreign keys: `TABLE NOT FOUND IN schema_export.sql`

### Entity Kind: `person`
- Table name: `"ob-poc".entities`
  - Columns: `TABLE NOT FOUND IN schema_export.sql`
  - Foreign keys: `TABLE NOT FOUND IN schema_export.sql`

## 7. Gaps
- Slots referencing tables not in schema SQL:
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`deal` table=`"ob-poc".deals`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`fund` table=`"ob-poc".entity_funds`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`group` table=`"ob-poc".client_group`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`client_group` table=`"ob-poc".client_group`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`cbu_identification` table=`"ob-poc".cbus`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_extended.yaml` slot=`entity` table=`"ob-poc".entities`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`cbu` table=`"ob-poc".cbus`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`kyc_case` table=`"ob-poc".cases`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`kyc_case.tollgate` table=`"ob-poc".tollgate_evaluations`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_hedge_cross_border.yaml` slot=`cbu` table=`"ob-poc".cbus`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_hedge_cross_border.yaml` slot=`case` table=`"ob-poc".cases`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_hedge_cross_border.yaml` slot=`case.tollgate` table=`"ob-poc".tollgate_evaluations`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_hedge_cross_border.yaml` slot=`mandate` table=`"ob-poc".cbu_trading_profiles`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_aif_icav.yaml` slot=`cbu` table=`"ob-poc".cbus`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_aif_icav.yaml` slot=`case` table=`"ob-poc".cases`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_aif_icav.yaml` slot=`case.tollgate` table=`"ob-poc".tollgate_evaluations`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_aif_icav.yaml` slot=`mandate` table=`"ob-poc".cbu_trading_profiles`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_hedge_icav.yaml` slot=`cbu` table=`"ob-poc".cbus`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_hedge_icav.yaml` slot=`case` table=`"ob-poc".cases`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_hedge_icav.yaml` slot=`case.tollgate` table=`"ob-poc".tollgate_evaluations`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_hedge_icav.yaml` slot=`mandate` table=`"ob-poc".cbu_trading_profiles`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_ucits_icav.yaml` slot=`cbu` table=`"ob-poc".cbus`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_ucits_icav.yaml` slot=`case` table=`"ob-poc".cases`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_ucits_icav.yaml` slot=`case.tollgate` table=`"ob-poc".tollgate_evaluations`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_ucits_icav.yaml` slot=`mandate` table=`"ob-poc".cbu_trading_profiles`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_aif_raif.yaml` slot=`cbu` table=`"ob-poc".cbus`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_aif_raif.yaml` slot=`case` table=`"ob-poc".cases`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_aif_raif.yaml` slot=`case.tollgate` table=`"ob-poc".tollgate_evaluations`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_aif_raif.yaml` slot=`mandate` table=`"ob-poc".cbu_trading_profiles`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_pe_scsp.yaml` slot=`cbu` table=`"ob-poc".cbus`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_pe_scsp.yaml` slot=`case` table=`"ob-poc".cases`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_pe_scsp.yaml` slot=`case.tollgate` table=`"ob-poc".tollgate_evaluations`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_pe_scsp.yaml` slot=`mandate` table=`"ob-poc".cbu_trading_profiles`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_ucits_sicav.yaml` slot=`cbu` table=`"ob-poc".cbus`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_ucits_sicav.yaml` slot=`case` table=`"ob-poc".cases`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_ucits_sicav.yaml` slot=`case.tollgate` table=`"ob-poc".tollgate_evaluations`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_ucits_sicav.yaml` slot=`mandate` table=`"ob-poc".cbu_trading_profiles`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_pe_cross_border.yaml` slot=`cbu` table=`"ob-poc".cbus`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_pe_cross_border.yaml` slot=`case` table=`"ob-poc".cases`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_pe_cross_border.yaml` slot=`case.tollgate` table=`"ob-poc".tollgate_evaluations`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_pe_cross_border.yaml` slot=`mandate` table=`"ob-poc".cbu_trading_profiles`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_acs.yaml` slot=`cbu` table=`"ob-poc".cbus`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_acs.yaml` slot=`case` table=`"ob-poc".cases`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_acs.yaml` slot=`case.tollgate` table=`"ob-poc".tollgate_evaluations`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_acs.yaml` slot=`mandate` table=`"ob-poc".cbu_trading_profiles`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_aut.yaml` slot=`cbu` table=`"ob-poc".cbus`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_aut.yaml` slot=`case` table=`"ob-poc".cases`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_aut.yaml` slot=`case.tollgate` table=`"ob-poc".tollgate_evaluations`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_aut.yaml` slot=`mandate` table=`"ob-poc".cbu_trading_profiles`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_ltaf.yaml` slot=`cbu` table=`"ob-poc".cbus`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_ltaf.yaml` slot=`case` table=`"ob-poc".cases`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_ltaf.yaml` slot=`case.tollgate` table=`"ob-poc".tollgate_evaluations`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_ltaf.yaml` slot=`mandate` table=`"ob-poc".cbu_trading_profiles`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_oeic.yaml` slot=`cbu` table=`"ob-poc".cbus`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_oeic.yaml` slot=`case` table=`"ob-poc".cases`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_oeic.yaml` slot=`case.tollgate` table=`"ob-poc".tollgate_evaluations`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_oeic.yaml` slot=`mandate` table=`"ob-poc".cbu_trading_profiles`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_manager_llp.yaml` slot=`cbu` table=`"ob-poc".cbus`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_manager_llp.yaml` slot=`case` table=`"ob-poc".cases`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_manager_llp.yaml` slot=`case.tollgate` table=`"ob-poc".tollgate_evaluations`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_pe_lp.yaml` slot=`cbu` table=`"ob-poc".cbus`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_pe_lp.yaml` slot=`case` table=`"ob-poc".cases`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_pe_lp.yaml` slot=`case.tollgate` table=`"ob-poc".tollgate_evaluations`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_pe_lp.yaml` slot=`mandate` table=`"ob-poc".cbu_trading_profiles`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_closed_end.yaml` slot=`cbu` table=`"ob-poc".cbus`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_closed_end.yaml` slot=`case` table=`"ob-poc".cases`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_closed_end.yaml` slot=`case.tollgate` table=`"ob-poc".tollgate_evaluations`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_closed_end.yaml` slot=`mandate` table=`"ob-poc".cbu_trading_profiles`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_open_end.yaml` slot=`cbu` table=`"ob-poc".cbus`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_open_end.yaml` slot=`case` table=`"ob-poc".cases`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_open_end.yaml` slot=`case.tollgate` table=`"ob-poc".tollgate_evaluations`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_open_end.yaml` slot=`mandate` table=`"ob-poc".cbu_trading_profiles`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_etf_40act.yaml` slot=`cbu` table=`"ob-poc".cbus`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_etf_40act.yaml` slot=`case` table=`"ob-poc".cases`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_etf_40act.yaml` slot=`case.tollgate` table=`"ob-poc".tollgate_evaluations`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_etf_40act.yaml` slot=`mandate` table=`"ob-poc".cbu_trading_profiles`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_private_fund_delaware_lp.yaml` slot=`cbu` table=`"ob-poc".cbus`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_private_fund_delaware_lp.yaml` slot=`case` table=`"ob-poc".cases`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_private_fund_delaware_lp.yaml` slot=`case.tollgate` table=`"ob-poc".tollgate_evaluations`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_private_fund_delaware_lp.yaml` slot=`mandate` table=`"ob-poc".cbu_trading_profiles`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`cbu` table=`"ob-poc".cbus`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`trading_profile` table=`"ob-poc".cbu_trading_profiles`
- State machines referenced but not defined:
  - `None`
- entity_kinds in slots with no matching schema table:
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`participant` entity_kind=`person`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`deal_contract` entity_kind=`contract`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`contract` entity_kind=`contract`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`deal_product` entity_kind=`entity`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`rate_card` entity_kind=`entity`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`onboarding_request` entity_kind=`entity`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`billing_profile` entity_kind=`entity`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`pricing` entity_kind=`entity`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`contract_template` entity_kind=`contract`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`umbrella` entity_kind=`fund`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`share_class` entity_kind=`fund`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`feeder` entity_kind=`fund`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`investment` entity_kind=`entity`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`capital` entity_kind=`fund`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`investment_manager` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`manco_group` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`trust` entity_kind=`entity`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`partnership` entity_kind=`entity`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`sla` entity_kind=`contract`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`access_review` entity_kind=`entity`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`regulatory` entity_kind=`entity`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`ruleset` entity_kind=`entity`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`delegation` entity_kind=`entity`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`team` entity_kind=`person`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`rule` entity_kind=`entity`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`rule_field` entity_kind=`entity`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`gleif_import` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`ubo_discovery` entity_kind=`person`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`ubo_discovery` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`control_chain` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_extended.yaml` slot=`entity` entity_kind=`person`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_extended.yaml` slot=`entity` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_extended.yaml` slot=`board` entity_kind=`person`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_extended.yaml` slot=`bods` entity_kind=`person`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_extended.yaml` slot=`bods` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`entity_workstream` entity_kind=`person`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`entity_workstream` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`screening` entity_kind=`person`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`screening` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`kyc_agreement` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`identifier` entity_kind=`entity`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`request` entity_kind=`entity`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_hedge_cross_border.yaml` slot=`aifm` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_hedge_cross_border.yaml` slot=`depositary` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_hedge_cross_border.yaml` slot=`prime_broker` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_hedge_cross_border.yaml` slot=`investment_manager` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_hedge_cross_border.yaml` slot=`administrator` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_hedge_cross_border.yaml` slot=`auditor` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_hedge_cross_border.yaml` slot=`secondary_prime_broker` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_hedge_cross_border.yaml` slot=`ownership_chain` entity_kind=`person`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_hedge_cross_border.yaml` slot=`ownership_chain` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_aif_icav.yaml` slot=`aifm` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_aif_icav.yaml` slot=`depositary` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_aif_icav.yaml` slot=`investment_manager` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_aif_icav.yaml` slot=`administrator` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_aif_icav.yaml` slot=`auditor` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_aif_icav.yaml` slot=`prime_broker` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_aif_icav.yaml` slot=`company_secretary` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_aif_icav.yaml` slot=`ownership_chain` entity_kind=`person`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_aif_icav.yaml` slot=`ownership_chain` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_hedge_icav.yaml` slot=`aifm` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_hedge_icav.yaml` slot=`depositary` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_hedge_icav.yaml` slot=`investment_manager` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_hedge_icav.yaml` slot=`administrator` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_hedge_icav.yaml` slot=`auditor` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_hedge_icav.yaml` slot=`prime_broker` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_hedge_icav.yaml` slot=`secondary_prime_broker` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_hedge_icav.yaml` slot=`executing_broker` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_hedge_icav.yaml` slot=`company_secretary` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_hedge_icav.yaml` slot=`ownership_chain` entity_kind=`person`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_hedge_icav.yaml` slot=`ownership_chain` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_ucits_icav.yaml` slot=`management_company` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_ucits_icav.yaml` slot=`depositary` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_ucits_icav.yaml` slot=`investment_manager` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_ucits_icav.yaml` slot=`administrator` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_ucits_icav.yaml` slot=`auditor` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_ucits_icav.yaml` slot=`company_secretary` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_ucits_icav.yaml` slot=`legal_counsel` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_ucits_icav.yaml` slot=`ownership_chain` entity_kind=`person`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_ucits_icav.yaml` slot=`ownership_chain` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_aif_raif.yaml` slot=`aifm` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_aif_raif.yaml` slot=`depositary` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_aif_raif.yaml` slot=`investment_manager` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_aif_raif.yaml` slot=`administrator` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_aif_raif.yaml` slot=`auditor` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_aif_raif.yaml` slot=`prime_broker` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_aif_raif.yaml` slot=`ownership_chain` entity_kind=`person`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_aif_raif.yaml` slot=`ownership_chain` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_pe_scsp.yaml` slot=`general_partner` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_pe_scsp.yaml` slot=`aifm` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_pe_scsp.yaml` slot=`depositary` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_pe_scsp.yaml` slot=`administrator` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_pe_scsp.yaml` slot=`auditor` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_pe_scsp.yaml` slot=`legal_counsel` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_pe_scsp.yaml` slot=`ownership_chain` entity_kind=`person`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_pe_scsp.yaml` slot=`ownership_chain` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_ucits_sicav.yaml` slot=`management_company` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_ucits_sicav.yaml` slot=`depositary` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_ucits_sicav.yaml` slot=`investment_manager` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_ucits_sicav.yaml` slot=`ownership_chain` entity_kind=`person`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_ucits_sicav.yaml` slot=`ownership_chain` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_pe_cross_border.yaml` slot=`general_partner` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_pe_cross_border.yaml` slot=`aifm` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_pe_cross_border.yaml` slot=`depositary` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_pe_cross_border.yaml` slot=`administrator` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_pe_cross_border.yaml` slot=`auditor` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_pe_cross_border.yaml` slot=`legal_counsel` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_pe_cross_border.yaml` slot=`ownership_chain` entity_kind=`person`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_pe_cross_border.yaml` slot=`ownership_chain` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_acs.yaml` slot=`acs_operator` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_acs.yaml` slot=`depositary` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_acs.yaml` slot=`investment_manager` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_acs.yaml` slot=`administrator` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_acs.yaml` slot=`auditor` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_acs.yaml` slot=`ownership_chain` entity_kind=`person`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_acs.yaml` slot=`ownership_chain` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_aut.yaml` slot=`authorised_fund_manager` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_aut.yaml` slot=`trustee` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_aut.yaml` slot=`investment_manager` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_aut.yaml` slot=`administrator` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_aut.yaml` slot=`auditor` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_aut.yaml` slot=`ownership_chain` entity_kind=`person`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_aut.yaml` slot=`ownership_chain` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_ltaf.yaml` slot=`authorised_corporate_director` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_ltaf.yaml` slot=`depositary` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_ltaf.yaml` slot=`investment_manager` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_ltaf.yaml` slot=`administrator` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_ltaf.yaml` slot=`auditor` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_ltaf.yaml` slot=`registrar` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_ltaf.yaml` slot=`valuation_agent` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_ltaf.yaml` slot=`ownership_chain` entity_kind=`person`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_ltaf.yaml` slot=`ownership_chain` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_oeic.yaml` slot=`authorised_corporate_director` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_oeic.yaml` slot=`depositary` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_oeic.yaml` slot=`investment_manager` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_oeic.yaml` slot=`administrator` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_oeic.yaml` slot=`auditor` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_oeic.yaml` slot=`registrar` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_oeic.yaml` slot=`ownership_chain` entity_kind=`person`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_oeic.yaml` slot=`ownership_chain` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_manager_llp.yaml` slot=`designated_member_1` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_manager_llp.yaml` slot=`designated_member_1` entity_kind=`person`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_manager_llp.yaml` slot=`designated_member_2` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_manager_llp.yaml` slot=`designated_member_2` entity_kind=`person`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_manager_llp.yaml` slot=`compliance_officer` entity_kind=`person`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_manager_llp.yaml` slot=`mlro` entity_kind=`person`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_manager_llp.yaml` slot=`auditor` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_manager_llp.yaml` slot=`ownership_chain` entity_kind=`person`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_manager_llp.yaml` slot=`ownership_chain` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_pe_lp.yaml` slot=`general_partner` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_pe_lp.yaml` slot=`aifm` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_pe_lp.yaml` slot=`depositary` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_pe_lp.yaml` slot=`administrator` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_pe_lp.yaml` slot=`auditor` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_pe_lp.yaml` slot=`legal_counsel` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_pe_lp.yaml` slot=`ownership_chain` entity_kind=`person`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_pe_lp.yaml` slot=`ownership_chain` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_closed_end.yaml` slot=`investment_adviser` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_closed_end.yaml` slot=`custodian` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_closed_end.yaml` slot=`sub_adviser` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_closed_end.yaml` slot=`administrator` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_closed_end.yaml` slot=`transfer_agent` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_closed_end.yaml` slot=`auditor` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_closed_end.yaml` slot=`legal_counsel` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_closed_end.yaml` slot=`ownership_chain` entity_kind=`person`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_closed_end.yaml` slot=`ownership_chain` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_open_end.yaml` slot=`investment_adviser` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_open_end.yaml` slot=`custodian` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_open_end.yaml` slot=`sub_adviser` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_open_end.yaml` slot=`administrator` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_open_end.yaml` slot=`transfer_agent` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_open_end.yaml` slot=`distributor` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_open_end.yaml` slot=`auditor` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_open_end.yaml` slot=`legal_counsel` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_open_end.yaml` slot=`ownership_chain` entity_kind=`person`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_open_end.yaml` slot=`ownership_chain` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_etf_40act.yaml` slot=`investment_adviser` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_etf_40act.yaml` slot=`custodian` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_etf_40act.yaml` slot=`authorized_participant` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_etf_40act.yaml` slot=`sub_adviser` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_etf_40act.yaml` slot=`administrator` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_etf_40act.yaml` slot=`transfer_agent` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_etf_40act.yaml` slot=`distributor` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_etf_40act.yaml` slot=`auditor` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_etf_40act.yaml` slot=`market_maker` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_etf_40act.yaml` slot=`ownership_chain` entity_kind=`person`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_etf_40act.yaml` slot=`ownership_chain` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_private_fund_delaware_lp.yaml` slot=`general_partner` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_private_fund_delaware_lp.yaml` slot=`investment_manager` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_private_fund_delaware_lp.yaml` slot=`custodian` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_private_fund_delaware_lp.yaml` slot=`administrator` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_private_fund_delaware_lp.yaml` slot=`prime_broker` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_private_fund_delaware_lp.yaml` slot=`auditor` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_private_fund_delaware_lp.yaml` slot=`legal_counsel` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_private_fund_delaware_lp.yaml` slot=`tax_advisor` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_private_fund_delaware_lp.yaml` slot=`tax_advisor` entity_kind=`person`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_private_fund_delaware_lp.yaml` slot=`ownership_chain` entity_kind=`person`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_private_fund_delaware_lp.yaml` slot=`ownership_chain` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`custody` entity_kind=`cbu`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`booking_principal` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`cash_sweep` entity_kind=`entity`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`service_resource` entity_kind=`entity`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`service_intent` entity_kind=`entity`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`booking_location` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`legal_entity` entity_kind=`company`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`product` entity_kind=`entity`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`delivery` entity_kind=`entity`
- Verbs in palettes not found in rust/config/verbs/:
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`deal` verb=`deal.read-record`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`deal` verb=`deal.list`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`deal` verb=`deal.search-records`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`deal` verb=`deal.read-summary`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`deal` verb=`deal.read-timeline`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`deal` verb=`deal.list-documents`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`deal` verb=`deal.list-slas`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`deal` verb=`deal.list-active-rate-cards`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`deal` verb=`deal.list-rate-card-lines`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`deal` verb=`deal.list-rate-card-history`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`deal` verb=`deal.update-record`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`deal` verb=`deal.update-status`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`deal` verb=`deal.add-document`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`deal` verb=`deal.update-document-status`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`deal` verb=`deal.add-sla`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`deal` verb=`deal.remove-sla`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`deal` verb=`deal.add-ubo-assessment`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`deal` verb=`deal.update-ubo-assessment`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`deal` verb=`deal.cancel`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`participant` verb=`deal.add-participant`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`participant` verb=`deal.remove-participant`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`participant` verb=`deal.list-participants`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`deal_contract` verb=`deal.remove-contract`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`deal_contract` verb=`deal.list-contracts`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`contract` verb=`contract.create`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`contract` verb=`contract.get`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`contract` verb=`contract.list`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`contract` verb=`contract.list-products`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`contract` verb=`contract.list-rate-cards`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`contract` verb=`contract.list-subscriptions`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`contract` verb=`contract.for-client`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`contract` verb=`contract.update`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`contract` verb=`contract.add-product`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`contract` verb=`contract.remove-product`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`contract` verb=`contract.create-rate-card`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`contract` verb=`contract.subscribe`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`contract` verb=`contract.unsubscribe`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`contract` verb=`contract.terminate`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`deal_product` verb=`deal.update-product-status`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`deal_product` verb=`deal.remove-product`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`deal_product` verb=`deal.list-products`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`rate_card` verb=`deal.update-rate-card-line`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`rate_card` verb=`deal.remove-rate-card-line`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`rate_card` verb=`deal.list-rate-cards`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`rate_card` verb=`deal.list-rate-card-lines`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`rate_card` verb=`deal.list-rate-card-history`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`rate_card` verb=`deal.list-active-rate-cards`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`rate_card` verb=`deal.counter-rate-card`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`onboarding_request` verb=`deal.request-onboarding`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`onboarding_request` verb=`deal.request-onboarding-batch`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`onboarding_request` verb=`deal.update-onboarding-status`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`onboarding_request` verb=`deal.list-onboarding-requests`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`billing_profile` verb=`billing.suspend-profile`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`billing_profile` verb=`billing.close-profile`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`billing_profile` verb=`billing.get-profile`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`billing_profile` verb=`billing.list-profiles`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`billing_profile` verb=`billing.add-account-target`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`billing_profile` verb=`billing.remove-account-target`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`billing_profile` verb=`billing.list-account-targets`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`billing_profile` verb=`billing.generate-invoice`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`billing_profile` verb=`billing.dispute-period`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`billing_profile` verb=`billing.period-summary`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`billing_profile` verb=`billing.revenue-summary`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`pricing` verb=`pricing-config.set-valuation-schedule`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`pricing` verb=`pricing-config.set-nav-threshold`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`pricing` verb=`pricing-config.set-settlement-calendar`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`pricing` verb=`pricing-config.set-holiday-schedule`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`pricing` verb=`pricing-config.set-reporting`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`pricing` verb=`pricing-config.set-tax-status`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`pricing` verb=`pricing-config.set-reclaim-config`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`pricing` verb=`pricing-config.find-for-instrument`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`pricing` verb=`pricing-config.list-jurisdictions`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`pricing` verb=`pricing-config.list-treaty-rates`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`pricing` verb=`pricing-config.list-tax-status`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`pricing` verb=`pricing-config.list-reclaim-configs`
  - file=`rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml` slot=`contract_template` verb=`contract-pack.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`fund` verb=`fund.create`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`fund` verb=`fund.ensure`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`fund` verb=`fund.read-vehicle`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`fund` verb=`fund.list-by-manager`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`fund` verb=`fund.list-by-vehicle-type`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`fund` verb=`fund.upsert-vehicle`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`fund` verb=`fund.delete-vehicle`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`umbrella` verb=`fund.add-to-umbrella`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`umbrella` verb=`fund.list-subfunds`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`umbrella` verb=`fund.upsert-compartment`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`umbrella` verb=`fund.read-compartment`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`umbrella` verb=`fund.list-compartments-by-umbrella`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`umbrella` verb=`fund.delete-compartment`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`share_class` verb=`fund.add-share-class`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`share_class` verb=`fund.list-share-classes`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`feeder` verb=`fund.link-feeder`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`feeder` verb=`fund.list-feeders`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`investment` verb=`fund.add-investment`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`investment` verb=`fund.update-investment`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`investment` verb=`fund.end-investment`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`investment` verb=`fund.list-investments`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`investment` verb=`fund.list-investors`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`capital` verb=`capital.allocate`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`capital` verb=`capital.issue.initial`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`capital` verb=`capital.issue.new`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`capital` verb=`capital.issue-shares`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`capital` verb=`capital.cancel-shares`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`capital` verb=`capital.transfer`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`capital` verb=`capital.split`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`capital` verb=`capital.buyback`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`capital` verb=`capital.cancel`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`capital` verb=`capital.reconcile`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`capital` verb=`capital.cap-table`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`capital` verb=`capital.holders`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`capital` verb=`capital.list-by-issuer`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`capital` verb=`capital.list-shareholders`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`capital` verb=`capital.get-ownership-chain`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`capital` verb=`capital.define-share-class`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`capital` verb=`capital.share-class.create`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`capital` verb=`capital.share-class.list`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`capital` verb=`capital.share-class.get-supply`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`capital` verb=`capital.share-class.add-identifier`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`capital` verb=`capital.control-config.get`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`capital` verb=`capital.control-config.set`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`capital` verb=`capital.dilution.grant-options`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`capital` verb=`capital.dilution.issue-warrant`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`capital` verb=`capital.dilution.create-safe`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`capital` verb=`capital.dilution.create-convertible-note`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`capital` verb=`capital.dilution.exercise`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`capital` verb=`capital.dilution.forfeit`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`capital` verb=`capital.dilution.list`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`capital` verb=`capital.dilution.get-summary`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`investment_manager` verb=`investment-manager.assign`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`investment_manager` verb=`investment-manager.set-scope`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`investment_manager` verb=`investment-manager.link-connectivity`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`investment_manager` verb=`investment-manager.list`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`investment_manager` verb=`investment-manager.suspend`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`investment_manager` verb=`investment-manager.terminate`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`investment_manager` verb=`investment-manager.find-for-trade`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`manco_group` verb=`manco.create`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`manco_group` verb=`manco.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`manco_group` verb=`manco.list`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`manco_group` verb=`manco.derive-groups`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`manco_group` verb=`manco.bridge-roles`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`manco_group` verb=`manco.list-members`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`manco_group` verb=`manco.list-roles`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`manco_group` verb=`manco.assign-role`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`manco_group` verb=`manco.remove-role`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`manco_group` verb=`manco.link-entity`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`manco_group` verb=`manco.unlink-entity`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`manco_group` verb=`manco.set-regulatory-status`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`manco_group` verb=`manco.list-managed-funds`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`manco_group` verb=`manco.verify`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`manco_group` verb=`manco.compute-control-chain`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`trust` verb=`trust.create`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`trust` verb=`trust.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`trust` verb=`trust.list`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`trust` verb=`trust.add-trustee`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`trust` verb=`trust.remove-trustee`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`trust` verb=`trust.add-beneficiary`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`trust` verb=`trust.add-settlor`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`trust` verb=`trust.identify-ubos`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`partnership` verb=`partnership.create`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`partnership` verb=`partnership.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`partnership` verb=`partnership.list`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`partnership` verb=`partnership.add-partner`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`partnership` verb=`partnership.remove-partner`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`partnership` verb=`partnership.set-general-partner`
  - file=`rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml` slot=`partnership` verb=`partnership.list-partners`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`sla` verb=`sla.create`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`sla` verb=`sla.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`sla` verb=`sla.read-template`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`sla` verb=`sla.list`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`sla` verb=`sla.list-templates`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`sla` verb=`sla.list-commitments`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`sla` verb=`sla.list-measurements`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`sla` verb=`sla.list-breaches`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`sla` verb=`sla.list-open-breaches`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`sla` verb=`sla.update`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`sla` verb=`sla.bind`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`sla` verb=`sla.commit`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`sla` verb=`sla.record-measurement`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`sla` verb=`sla.activate`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`sla` verb=`sla.suspend`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`sla` verb=`sla.suspend-commitment`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`sla` verb=`sla.renew`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`sla` verb=`sla.record-breach`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`sla` verb=`sla.report-breach`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`sla` verb=`sla.escalate-breach`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`sla` verb=`sla.resolve-breach`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`sla` verb=`sla.update-remediation`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`access_review` verb=`access-review.create`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`access_review` verb=`access-review.create-campaign`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`access_review` verb=`access-review.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`access_review` verb=`access-review.list`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`access_review` verb=`access-review.list-items`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`access_review` verb=`access-review.list-flagged`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`access_review` verb=`access-review.my-pending`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`access_review` verb=`access-review.campaign-status`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`access_review` verb=`access-review.audit-report`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`access_review` verb=`access-review.populate-campaign`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`access_review` verb=`access-review.launch-campaign`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`access_review` verb=`access-review.send-reminders`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`access_review` verb=`access-review.process-deadline`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`access_review` verb=`access-review.attest`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`access_review` verb=`access-review.extend-access`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`access_review` verb=`access-review.revoke-access`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`access_review` verb=`access-review.escalate-item`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`access_review` verb=`access-review.start`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`access_review` verb=`access-review.complete`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`access_review` verb=`access-review.approve`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`access_review` verb=`access-review.reject`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`regulatory` verb=`regulatory.create`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`regulatory` verb=`regulatory.registration.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`regulatory` verb=`regulatory.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`regulatory` verb=`regulatory.list`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`regulatory` verb=`regulatory.registration.list`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`regulatory` verb=`regulatory.registration.check`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`regulatory` verb=`regulatory.registration.verify`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`regulatory` verb=`regulatory.update`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`regulatory` verb=`regulatory.submit`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`regulatory` verb=`regulatory.registration.remove`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`ruleset` verb=`ruleset.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`ruleset` verb=`ruleset.publish`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`ruleset` verb=`ruleset.retire`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`delegation` verb=`delegation.create`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`delegation` verb=`delegation.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`delegation` verb=`delegation.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`delegation` verb=`delegation.list`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`delegation` verb=`delegation.list-delegates`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`delegation` verb=`delegation.list-delegations-received`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`delegation` verb=`delegation.end`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`delegation` verb=`delegation.revoke`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`team` verb=`team.add-member`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`team` verb=`team.remove-member`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`team` verb=`team.list-members`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`team` verb=`team.list`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`team` verb=`team.create`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`team` verb=`team.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`team` verb=`team.update`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`team` verb=`team.assign-role`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`team` verb=`team.remove-role`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`team` verb=`team.transfer-member`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`team` verb=`team.list-by-role`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`team` verb=`team.set-lead`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`team` verb=`team.add-governance-member`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`team` verb=`team.remove-governance-member`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`team` verb=`team.list-governance-members`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`team` verb=`team.add-ops-member`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`team` verb=`team.remove-ops-member`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`team` verb=`team.list-ops-members`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`team` verb=`team.assign-capacity`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`team` verb=`team.list-capacity`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`rule` verb=`rule.create`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`rule` verb=`rule.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`rule` verb=`rule.update`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`rule_field` verb=`rule-field.list`
  - file=`rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml` slot=`rule_field` verb=`rule-field.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`client_group` verb=`client-group.create`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`client_group` verb=`client-group.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`client_group` verb=`client-group.research`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`client_group` verb=`client-group.update`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`client_group` verb=`client-group.set-canonical`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`client_group` verb=`client-group.start-discovery`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`client_group` verb=`client-group.discover-entities`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`client_group` verb=`client-group.complete-discovery`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`client_group` verb=`client-group.entity-add`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`client_group` verb=`client-group.entity-remove`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`client_group` verb=`client-group.list-entities`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`client_group` verb=`client-group.search-entities`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`client_group` verb=`client-group.list-parties`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`client_group` verb=`client-group.list-unverified`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`client_group` verb=`client-group.list-discrepancies`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`client_group` verb=`client-group.verify-ownership`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`client_group` verb=`client-group.reject-entity`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`client_group` verb=`client-group.assign-role`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`client_group` verb=`client-group.remove-role`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`client_group` verb=`client-group.list-roles`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`client_group` verb=`client-group.add-relationship`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`client_group` verb=`client-group.list-relationships`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`client_group` verb=`client-group.tag-add`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`client_group` verb=`client-group.tag-remove`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`gleif_import` verb=`gleif.import-tree`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`gleif_import` verb=`gleif.import-to-client-group`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`gleif_import` verb=`gleif.import-managed-funds`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`gleif_import` verb=`gleif.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`gleif_import` verb=`gleif.refresh`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`gleif_import` verb=`gleif.enrich`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`gleif_import` verb=`gleif.get-record`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`gleif_import` verb=`gleif.get-parent`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`gleif_import` verb=`gleif.get-children`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`gleif_import` verb=`gleif.get-manager`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`gleif_import` verb=`gleif.get-managed-funds`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`gleif_import` verb=`gleif.get-master-fund`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`gleif_import` verb=`gleif.get-umbrella`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`gleif_import` verb=`gleif.lookup-by-isin`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`gleif_import` verb=`gleif.resolve-successor`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`gleif_import` verb=`gleif.trace-ownership`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`ubo_discovery` verb=`ubo.discover`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`ubo_discovery` verb=`ubo.allege`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`ubo_discovery` verb=`ubo.calculate`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`ubo_discovery` verb=`ubo.compute-chains`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`ubo_discovery` verb=`ubo.verify`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`ubo_discovery` verb=`ubo.promote`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`ubo_discovery` verb=`ubo.approve`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`ubo_discovery` verb=`ubo.reject`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`ubo_discovery` verb=`ubo.list`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`ubo_discovery` verb=`ubo.list-ubos`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`ubo_discovery` verb=`ubo.list-owned`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`ubo_discovery` verb=`ubo.list-owners`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`ubo_discovery` verb=`ubo.add-control`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`ubo_discovery` verb=`ubo.transfer-control`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`ubo_discovery` verb=`ubo.add-trust-role`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`ubo_discovery` verb=`ubo.delete-relationship`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`ubo_discovery` verb=`ubo.end-relationship`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`ubo_discovery` verb=`ubo.waive-verification`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`ubo_discovery` verb=`ubo.mark-deceased`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`ubo_discovery` verb=`ubo.mark-terminus`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`ubo_discovery` verb=`ubo.convergence-supersede`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`ubo_discovery` verb=`ubo.registry.create`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`ubo_discovery` verb=`ubo.registry.advance`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`ubo_discovery` verb=`ubo.registry.promote`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`ubo_discovery` verb=`ubo.registry.reject`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`ubo_discovery` verb=`ubo.registry.expire`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`ubo_discovery` verb=`ubo.registry.waive`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`ubo_discovery` verb=`ubo.snapshot.capture`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`ubo_discovery` verb=`ubo.snapshot.diff`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`control_chain` verb=`ownership.trace-chain`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`control_chain` verb=`control.build-graph`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`control_chain` verb=`ownership.refresh`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`control_chain` verb=`control.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`control_chain` verb=`control.list-links`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`control_chain` verb=`control.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`control_chain` verb=`control.end`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`control_chain` verb=`control.analyze`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`control_chain` verb=`control.list-controllers`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`control_chain` verb=`control.list-controlled`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`control_chain` verb=`control.trace-chain`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`control_chain` verb=`control.compute-controllers`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`control_chain` verb=`control.identify-ubos`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`control_chain` verb=`control.reconcile-ownership`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`control_chain` verb=`control.set-board-controller`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`control_chain` verb=`control.show-board-controller`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`control_chain` verb=`control.recompute-board-controller`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`control_chain` verb=`control.clear-board-controller-override`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`control_chain` verb=`control.import-gleif-control`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`control_chain` verb=`control.import-psc-register`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`control_chain` verb=`ownership.compute`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`control_chain` verb=`ownership.control-positions`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`control_chain` verb=`ownership.who-controls`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`control_chain` verb=`ownership.analyze-gaps`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`control_chain` verb=`ownership.reconcile`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`control_chain` verb=`ownership.reconcile.findings`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`control_chain` verb=`ownership.reconcile.list-runs`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`control_chain` verb=`ownership.reconcile.resolve-finding`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`control_chain` verb=`ownership.right.add-to-class`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`control_chain` verb=`ownership.right.add-to-holder`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`control_chain` verb=`ownership.right.end`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`control_chain` verb=`ownership.right.list-for-holder`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`control_chain` verb=`ownership.right.list-for-issuer`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`control_chain` verb=`ownership.snapshot.get`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`control_chain` verb=`ownership.snapshot.list`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`cbu_identification` verb=`cbu.create-from-client-group`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`cbu_identification` verb=`cbu.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`cbu_identification` verb=`cbu.list`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`cbu_identification` verb=`cbu.list-subscriptions`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`cbu_identification` verb=`cbu.list-evidence`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`cbu_identification` verb=`cbu.list-structure-links`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`cbu_identification` verb=`cbu.parties`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`cbu_identification` verb=`cbu.update`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`cbu_identification` verb=`cbu.rename`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`cbu_identification` verb=`cbu.set-jurisdiction`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`cbu_identification` verb=`cbu.set-client-type`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`cbu_identification` verb=`cbu.set-commercial-client`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`cbu_identification` verb=`cbu.add-product`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`cbu_identification` verb=`cbu.remove-product`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`cbu_identification` verb=`cbu.assign-control`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`cbu_identification` verb=`cbu.assign-ownership`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`cbu_identification` verb=`cbu.assign-fund-role`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`cbu_identification` verb=`cbu.assign-trust-role`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`cbu_identification` verb=`cbu.assign-service-provider`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`cbu_identification` verb=`cbu.assign-signatory`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`cbu_identification` verb=`cbu.remove-role`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`cbu_identification` verb=`cbu.validate-roles`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`cbu_identification` verb=`cbu.attach-evidence`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`cbu_identification` verb=`cbu.verify-evidence`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`cbu_identification` verb=`cbu.request-proof-update`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`cbu_identification` verb=`cbu.link-structure`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`cbu_identification` verb=`cbu.unlink-structure`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`cbu_identification` verb=`cbu.submit-for-validation`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`cbu_identification` verb=`cbu.reopen-validation`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`cbu_identification` verb=`cbu.decide`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`cbu_identification` verb=`cbu.delete`
  - file=`rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml` slot=`cbu_identification` verb=`cbu.delete-cascade`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_extended.yaml` slot=`entity` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_extended.yaml` slot=`board` verb=`board.appoint`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_extended.yaml` slot=`board` verb=`board.resign`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_extended.yaml` slot=`board` verb=`board.list-by-entity`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_extended.yaml` slot=`board` verb=`board.list-by-person`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_extended.yaml` slot=`board` verb=`board.grant-appointment-right`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_extended.yaml` slot=`board` verb=`board.revoke-appointment-right`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_extended.yaml` slot=`board` verb=`board.list-appointment-rights`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_extended.yaml` slot=`board` verb=`board.list-rights-held`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_extended.yaml` slot=`board` verb=`board.analyze-control`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_extended.yaml` slot=`bods` verb=`bods.discover-ubos`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_extended.yaml` slot=`bods` verb=`bods.import`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_extended.yaml` slot=`bods` verb=`bods.link-entity`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_extended.yaml` slot=`bods` verb=`bods.get-statement`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_extended.yaml` slot=`bods` verb=`bods.list-by-entity`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_extended.yaml` slot=`bods` verb=`bods.find-by-lei`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_extended.yaml` slot=`bods` verb=`bods.list-persons`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_extended.yaml` slot=`bods` verb=`bods.list-ownership`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_extended.yaml` slot=`bods` verb=`bods.sync-from-gleif`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`cbu` verb=`cbu.show`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`kyc_case` verb=`kyc.open-case`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`kyc_case` verb=`kyc-case.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`kyc_case` verb=`kyc-case.list-by-cbu`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`kyc_case` verb=`kyc-case.state`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`kyc_case` verb=`kyc-case.assign`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`kyc_case` verb=`kyc-case.update-status`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`kyc_case` verb=`kyc-case.reopen`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`kyc_case.tollgate` verb=`tollgate.evaluate`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`kyc_case.tollgate` verb=`tollgate.evaluate-gate`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`kyc_case.tollgate` verb=`tollgate.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`kyc_case.tollgate` verb=`tollgate.get-decision-readiness`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`kyc_case.tollgate` verb=`tollgate.get-metrics`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`kyc_case.tollgate` verb=`tollgate.list-evaluations`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`kyc_case.tollgate` verb=`tollgate.list-thresholds`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`kyc_case.tollgate` verb=`tollgate.set-threshold`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`kyc_case.tollgate` verb=`tollgate.override`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`kyc_case.tollgate` verb=`tollgate.list-overrides`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`kyc_case.tollgate` verb=`tollgate.expire-override`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`entity_workstream` verb=`entity-workstream.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`entity_workstream` verb=`entity-workstream.list-by-case`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`entity_workstream` verb=`entity-workstream.state`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`entity_workstream` verb=`entity-workstream.update-status`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`entity_workstream` verb=`entity-workstream.set-enhanced-dd`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`entity_workstream` verb=`entity-workstream.set-ubo`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`entity_workstream` verb=`entity-workstream.complete`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`entity_workstream` verb=`entity-workstream.block`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`entity_workstream` verb=`red-flag.raise`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`entity_workstream` verb=`red-flag.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`entity_workstream` verb=`red-flag.list`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`entity_workstream` verb=`red-flag.resolve`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`entity_workstream` verb=`red-flag.escalate`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`entity_workstream` verb=`red-flag.update`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`entity_workstream` verb=`red-flag.list-by-severity`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`entity_workstream` verb=`red-flag.close`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`entity_workstream` verb=`requirement.check`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`entity_workstream` verb=`requirement.list`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`entity_workstream` verb=`requirement.for-entity`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`entity_workstream` verb=`requirement.unsatisfied`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`entity_workstream` verb=`requirement.waive`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`entity_workstream` verb=`requirement.reinstate`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`entity_workstream` verb=`document.solicit`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`entity_workstream` verb=`document.upload`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`entity_workstream` verb=`document.reject`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`entity_workstream` verb=`document.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`entity_workstream` verb=`document.list`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`entity_workstream` verb=`document.compute-requirements`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`entity_workstream` verb=`document.missing-for-entity`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`screening` verb=`screening.sanctions`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`screening` verb=`screening.pep`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`screening` verb=`screening.adverse-media`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`screening` verb=`screening.bulk-refresh`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`screening` verb=`screening.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`screening` verb=`screening.list`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`screening` verb=`screening.list-by-workstream`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`screening` verb=`screening.update-status`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`screening` verb=`screening.escalate`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`screening` verb=`screening.resolve`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`screening` verb=`screening.complete`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`kyc_agreement` verb=`kyc-agreement.create`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`kyc_agreement` verb=`kyc-agreement.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`kyc_agreement` verb=`kyc-agreement.list`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`kyc_agreement` verb=`kyc-agreement.update`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`kyc_agreement` verb=`kyc-agreement.update-status`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`kyc_agreement` verb=`kyc-agreement.sign`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`identifier` verb=`identifier.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`identifier` verb=`identifier.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`identifier` verb=`identifier.list`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`identifier` verb=`identifier.verify`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`identifier` verb=`identifier.expire`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`identifier` verb=`identifier.update`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`identifier` verb=`identifier.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`identifier` verb=`identifier.resolve`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`identifier` verb=`identifier.list-by-type`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`identifier` verb=`identifier.set-primary`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`identifier` verb=`identifier.remove`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`request` verb=`request.create`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`request` verb=`request.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`request` verb=`request.list`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`request` verb=`request.update`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`request` verb=`request.complete`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`request` verb=`request.cancel`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`request` verb=`request.assign`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`request` verb=`request.reopen`
  - file=`rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml` slot=`request` verb=`request.escalate`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_hedge_cross_border.yaml` slot=`cbu` verb=`cbu.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_hedge_cross_border.yaml` slot=`cbu` verb=`cbu.show`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_hedge_cross_border.yaml` slot=`cbu.us_feeder` verb=`cbu.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_hedge_cross_border.yaml` slot=`cbu.ie_feeder` verb=`cbu.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_hedge_cross_border.yaml` slot=`aifm` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_hedge_cross_border.yaml` slot=`aifm` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_hedge_cross_border.yaml` slot=`aifm` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_hedge_cross_border.yaml` slot=`aifm` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_hedge_cross_border.yaml` slot=`depositary` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_hedge_cross_border.yaml` slot=`depositary` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_hedge_cross_border.yaml` slot=`depositary` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_hedge_cross_border.yaml` slot=`depositary` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_hedge_cross_border.yaml` slot=`prime_broker` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_hedge_cross_border.yaml` slot=`prime_broker` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_hedge_cross_border.yaml` slot=`prime_broker` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_hedge_cross_border.yaml` slot=`prime_broker` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_hedge_cross_border.yaml` slot=`investment_manager` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_hedge_cross_border.yaml` slot=`investment_manager` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_hedge_cross_border.yaml` slot=`investment_manager` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_hedge_cross_border.yaml` slot=`investment_manager` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_hedge_cross_border.yaml` slot=`administrator` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_hedge_cross_border.yaml` slot=`administrator` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_hedge_cross_border.yaml` slot=`administrator` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_hedge_cross_border.yaml` slot=`administrator` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_hedge_cross_border.yaml` slot=`auditor` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_hedge_cross_border.yaml` slot=`auditor` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_hedge_cross_border.yaml` slot=`auditor` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_hedge_cross_border.yaml` slot=`auditor` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_hedge_cross_border.yaml` slot=`secondary_prime_broker` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_hedge_cross_border.yaml` slot=`secondary_prime_broker` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_hedge_cross_border.yaml` slot=`secondary_prime_broker` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_hedge_cross_border.yaml` slot=`secondary_prime_broker` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_hedge_cross_border.yaml` slot=`ownership_chain` verb=`ubo.discover`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_hedge_cross_border.yaml` slot=`ownership_chain` verb=`ubo.allege`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_hedge_cross_border.yaml` slot=`ownership_chain` verb=`ubo.verify`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_hedge_cross_border.yaml` slot=`ownership_chain` verb=`ubo.promote`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_hedge_cross_border.yaml` slot=`ownership_chain` verb=`ubo.approve`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_hedge_cross_border.yaml` slot=`ownership_chain` verb=`ubo.reject`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_hedge_cross_border.yaml` slot=`case` verb=`case.open`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_hedge_cross_border.yaml` slot=`case` verb=`case.submit`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_hedge_cross_border.yaml` slot=`case` verb=`case.approve`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_hedge_cross_border.yaml` slot=`case` verb=`case.reject`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_hedge_cross_border.yaml` slot=`case` verb=`case.request-info`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_hedge_cross_border.yaml` slot=`case.tollgate` verb=`tollgate.evaluate`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_hedge_cross_border.yaml` slot=`mandate` verb=`mandate.create`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_aif_icav.yaml` slot=`cbu` verb=`cbu.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_aif_icav.yaml` slot=`cbu` verb=`cbu.show`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_aif_icav.yaml` slot=`aifm` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_aif_icav.yaml` slot=`aifm` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_aif_icav.yaml` slot=`aifm` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_aif_icav.yaml` slot=`aifm` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_aif_icav.yaml` slot=`depositary` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_aif_icav.yaml` slot=`depositary` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_aif_icav.yaml` slot=`depositary` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_aif_icav.yaml` slot=`depositary` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_aif_icav.yaml` slot=`investment_manager` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_aif_icav.yaml` slot=`investment_manager` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_aif_icav.yaml` slot=`investment_manager` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_aif_icav.yaml` slot=`investment_manager` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_aif_icav.yaml` slot=`administrator` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_aif_icav.yaml` slot=`administrator` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_aif_icav.yaml` slot=`administrator` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_aif_icav.yaml` slot=`administrator` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_aif_icav.yaml` slot=`auditor` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_aif_icav.yaml` slot=`auditor` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_aif_icav.yaml` slot=`auditor` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_aif_icav.yaml` slot=`auditor` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_aif_icav.yaml` slot=`prime_broker` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_aif_icav.yaml` slot=`prime_broker` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_aif_icav.yaml` slot=`prime_broker` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_aif_icav.yaml` slot=`prime_broker` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_aif_icav.yaml` slot=`company_secretary` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_aif_icav.yaml` slot=`company_secretary` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_aif_icav.yaml` slot=`company_secretary` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_aif_icav.yaml` slot=`company_secretary` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_aif_icav.yaml` slot=`ownership_chain` verb=`ubo.discover`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_aif_icav.yaml` slot=`ownership_chain` verb=`ubo.allege`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_aif_icav.yaml` slot=`ownership_chain` verb=`ubo.verify`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_aif_icav.yaml` slot=`ownership_chain` verb=`ubo.promote`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_aif_icav.yaml` slot=`ownership_chain` verb=`ubo.approve`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_aif_icav.yaml` slot=`ownership_chain` verb=`ubo.reject`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_aif_icav.yaml` slot=`case` verb=`case.open`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_aif_icav.yaml` slot=`case` verb=`case.submit`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_aif_icav.yaml` slot=`case` verb=`case.approve`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_aif_icav.yaml` slot=`case` verb=`case.reject`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_aif_icav.yaml` slot=`case` verb=`case.request-info`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_aif_icav.yaml` slot=`case.tollgate` verb=`tollgate.evaluate`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_aif_icav.yaml` slot=`mandate` verb=`mandate.create`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_hedge_icav.yaml` slot=`cbu` verb=`cbu.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_hedge_icav.yaml` slot=`cbu` verb=`cbu.show`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_hedge_icav.yaml` slot=`aifm` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_hedge_icav.yaml` slot=`aifm` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_hedge_icav.yaml` slot=`aifm` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_hedge_icav.yaml` slot=`aifm` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_hedge_icav.yaml` slot=`depositary` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_hedge_icav.yaml` slot=`depositary` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_hedge_icav.yaml` slot=`depositary` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_hedge_icav.yaml` slot=`depositary` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_hedge_icav.yaml` slot=`investment_manager` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_hedge_icav.yaml` slot=`investment_manager` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_hedge_icav.yaml` slot=`investment_manager` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_hedge_icav.yaml` slot=`investment_manager` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_hedge_icav.yaml` slot=`administrator` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_hedge_icav.yaml` slot=`administrator` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_hedge_icav.yaml` slot=`administrator` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_hedge_icav.yaml` slot=`administrator` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_hedge_icav.yaml` slot=`auditor` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_hedge_icav.yaml` slot=`auditor` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_hedge_icav.yaml` slot=`auditor` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_hedge_icav.yaml` slot=`auditor` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_hedge_icav.yaml` slot=`prime_broker` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_hedge_icav.yaml` slot=`prime_broker` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_hedge_icav.yaml` slot=`prime_broker` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_hedge_icav.yaml` slot=`prime_broker` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_hedge_icav.yaml` slot=`secondary_prime_broker` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_hedge_icav.yaml` slot=`secondary_prime_broker` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_hedge_icav.yaml` slot=`secondary_prime_broker` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_hedge_icav.yaml` slot=`secondary_prime_broker` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_hedge_icav.yaml` slot=`executing_broker` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_hedge_icav.yaml` slot=`executing_broker` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_hedge_icav.yaml` slot=`executing_broker` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_hedge_icav.yaml` slot=`executing_broker` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_hedge_icav.yaml` slot=`company_secretary` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_hedge_icav.yaml` slot=`company_secretary` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_hedge_icav.yaml` slot=`company_secretary` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_hedge_icav.yaml` slot=`company_secretary` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_hedge_icav.yaml` slot=`ownership_chain` verb=`ubo.discover`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_hedge_icav.yaml` slot=`ownership_chain` verb=`ubo.allege`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_hedge_icav.yaml` slot=`ownership_chain` verb=`ubo.verify`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_hedge_icav.yaml` slot=`ownership_chain` verb=`ubo.promote`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_hedge_icav.yaml` slot=`ownership_chain` verb=`ubo.approve`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_hedge_icav.yaml` slot=`ownership_chain` verb=`ubo.reject`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_hedge_icav.yaml` slot=`case` verb=`case.open`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_hedge_icav.yaml` slot=`case` verb=`case.submit`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_hedge_icav.yaml` slot=`case` verb=`case.approve`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_hedge_icav.yaml` slot=`case` verb=`case.reject`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_hedge_icav.yaml` slot=`case` verb=`case.request-info`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_hedge_icav.yaml` slot=`case.tollgate` verb=`tollgate.evaluate`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_hedge_icav.yaml` slot=`mandate` verb=`mandate.create`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_ucits_icav.yaml` slot=`cbu` verb=`cbu.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_ucits_icav.yaml` slot=`cbu` verb=`cbu.show`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_ucits_icav.yaml` slot=`management_company` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_ucits_icav.yaml` slot=`management_company` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_ucits_icav.yaml` slot=`management_company` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_ucits_icav.yaml` slot=`management_company` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_ucits_icav.yaml` slot=`depositary` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_ucits_icav.yaml` slot=`depositary` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_ucits_icav.yaml` slot=`depositary` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_ucits_icav.yaml` slot=`depositary` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_ucits_icav.yaml` slot=`investment_manager` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_ucits_icav.yaml` slot=`investment_manager` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_ucits_icav.yaml` slot=`investment_manager` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_ucits_icav.yaml` slot=`investment_manager` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_ucits_icav.yaml` slot=`administrator` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_ucits_icav.yaml` slot=`administrator` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_ucits_icav.yaml` slot=`administrator` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_ucits_icav.yaml` slot=`administrator` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_ucits_icav.yaml` slot=`auditor` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_ucits_icav.yaml` slot=`auditor` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_ucits_icav.yaml` slot=`auditor` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_ucits_icav.yaml` slot=`auditor` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_ucits_icav.yaml` slot=`company_secretary` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_ucits_icav.yaml` slot=`company_secretary` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_ucits_icav.yaml` slot=`company_secretary` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_ucits_icav.yaml` slot=`company_secretary` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_ucits_icav.yaml` slot=`legal_counsel` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_ucits_icav.yaml` slot=`legal_counsel` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_ucits_icav.yaml` slot=`legal_counsel` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_ucits_icav.yaml` slot=`legal_counsel` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_ucits_icav.yaml` slot=`ownership_chain` verb=`ubo.discover`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_ucits_icav.yaml` slot=`ownership_chain` verb=`ubo.allege`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_ucits_icav.yaml` slot=`ownership_chain` verb=`ubo.verify`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_ucits_icav.yaml` slot=`ownership_chain` verb=`ubo.promote`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_ucits_icav.yaml` slot=`ownership_chain` verb=`ubo.approve`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_ucits_icav.yaml` slot=`ownership_chain` verb=`ubo.reject`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_ucits_icav.yaml` slot=`case` verb=`case.open`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_ucits_icav.yaml` slot=`case` verb=`case.submit`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_ucits_icav.yaml` slot=`case` verb=`case.approve`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_ucits_icav.yaml` slot=`case` verb=`case.reject`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_ucits_icav.yaml` slot=`case` verb=`case.request-info`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_ucits_icav.yaml` slot=`case.tollgate` verb=`tollgate.evaluate`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_ie_ucits_icav.yaml` slot=`mandate` verb=`mandate.create`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_aif_raif.yaml` slot=`cbu` verb=`cbu.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_aif_raif.yaml` slot=`cbu` verb=`cbu.show`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_aif_raif.yaml` slot=`aifm` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_aif_raif.yaml` slot=`aifm` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_aif_raif.yaml` slot=`aifm` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_aif_raif.yaml` slot=`aifm` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_aif_raif.yaml` slot=`depositary` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_aif_raif.yaml` slot=`depositary` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_aif_raif.yaml` slot=`depositary` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_aif_raif.yaml` slot=`depositary` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_aif_raif.yaml` slot=`investment_manager` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_aif_raif.yaml` slot=`investment_manager` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_aif_raif.yaml` slot=`investment_manager` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_aif_raif.yaml` slot=`investment_manager` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_aif_raif.yaml` slot=`administrator` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_aif_raif.yaml` slot=`administrator` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_aif_raif.yaml` slot=`administrator` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_aif_raif.yaml` slot=`administrator` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_aif_raif.yaml` slot=`auditor` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_aif_raif.yaml` slot=`auditor` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_aif_raif.yaml` slot=`auditor` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_aif_raif.yaml` slot=`auditor` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_aif_raif.yaml` slot=`prime_broker` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_aif_raif.yaml` slot=`prime_broker` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_aif_raif.yaml` slot=`prime_broker` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_aif_raif.yaml` slot=`prime_broker` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_aif_raif.yaml` slot=`ownership_chain` verb=`ubo.discover`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_aif_raif.yaml` slot=`ownership_chain` verb=`ubo.allege`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_aif_raif.yaml` slot=`ownership_chain` verb=`ubo.verify`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_aif_raif.yaml` slot=`ownership_chain` verb=`ubo.promote`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_aif_raif.yaml` slot=`ownership_chain` verb=`ubo.approve`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_aif_raif.yaml` slot=`ownership_chain` verb=`ubo.reject`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_aif_raif.yaml` slot=`case` verb=`case.open`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_aif_raif.yaml` slot=`case` verb=`case.submit`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_aif_raif.yaml` slot=`case` verb=`case.approve`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_aif_raif.yaml` slot=`case` verb=`case.reject`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_aif_raif.yaml` slot=`case` verb=`case.request-info`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_aif_raif.yaml` slot=`case.tollgate` verb=`tollgate.evaluate`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_aif_raif.yaml` slot=`mandate` verb=`mandate.create`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_pe_scsp.yaml` slot=`cbu` verb=`cbu.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_pe_scsp.yaml` slot=`cbu` verb=`cbu.show`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_pe_scsp.yaml` slot=`general_partner` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_pe_scsp.yaml` slot=`general_partner` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_pe_scsp.yaml` slot=`general_partner` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_pe_scsp.yaml` slot=`general_partner` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_pe_scsp.yaml` slot=`aifm` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_pe_scsp.yaml` slot=`aifm` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_pe_scsp.yaml` slot=`aifm` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_pe_scsp.yaml` slot=`aifm` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_pe_scsp.yaml` slot=`depositary` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_pe_scsp.yaml` slot=`depositary` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_pe_scsp.yaml` slot=`depositary` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_pe_scsp.yaml` slot=`depositary` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_pe_scsp.yaml` slot=`administrator` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_pe_scsp.yaml` slot=`administrator` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_pe_scsp.yaml` slot=`administrator` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_pe_scsp.yaml` slot=`administrator` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_pe_scsp.yaml` slot=`auditor` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_pe_scsp.yaml` slot=`auditor` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_pe_scsp.yaml` slot=`auditor` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_pe_scsp.yaml` slot=`auditor` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_pe_scsp.yaml` slot=`legal_counsel` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_pe_scsp.yaml` slot=`legal_counsel` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_pe_scsp.yaml` slot=`legal_counsel` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_pe_scsp.yaml` slot=`legal_counsel` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_pe_scsp.yaml` slot=`ownership_chain` verb=`ubo.discover`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_pe_scsp.yaml` slot=`ownership_chain` verb=`ubo.allege`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_pe_scsp.yaml` slot=`ownership_chain` verb=`ubo.verify`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_pe_scsp.yaml` slot=`ownership_chain` verb=`ubo.promote`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_pe_scsp.yaml` slot=`ownership_chain` verb=`ubo.approve`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_pe_scsp.yaml` slot=`ownership_chain` verb=`ubo.reject`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_pe_scsp.yaml` slot=`case` verb=`case.open`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_pe_scsp.yaml` slot=`case` verb=`case.submit`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_pe_scsp.yaml` slot=`case` verb=`case.approve`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_pe_scsp.yaml` slot=`case` verb=`case.reject`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_pe_scsp.yaml` slot=`case` verb=`case.request-info`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_pe_scsp.yaml` slot=`case.tollgate` verb=`tollgate.evaluate`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_pe_scsp.yaml` slot=`mandate` verb=`mandate.create`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_ucits_sicav.yaml` slot=`cbu` verb=`cbu.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_ucits_sicav.yaml` slot=`cbu` verb=`cbu.show`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_ucits_sicav.yaml` slot=`management_company` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_ucits_sicav.yaml` slot=`management_company` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_ucits_sicav.yaml` slot=`management_company` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_ucits_sicav.yaml` slot=`management_company` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_ucits_sicav.yaml` slot=`depositary` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_ucits_sicav.yaml` slot=`depositary` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_ucits_sicav.yaml` slot=`depositary` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_ucits_sicav.yaml` slot=`investment_manager` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_ucits_sicav.yaml` slot=`ownership_chain` verb=`ubo.discover`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_ucits_sicav.yaml` slot=`ownership_chain` verb=`ubo.allege`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_ucits_sicav.yaml` slot=`ownership_chain` verb=`ubo.verify`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_ucits_sicav.yaml` slot=`ownership_chain` verb=`ubo.promote`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_ucits_sicav.yaml` slot=`ownership_chain` verb=`ubo.approve`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_ucits_sicav.yaml` slot=`ownership_chain` verb=`ubo.reject`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_ucits_sicav.yaml` slot=`case` verb=`case.open`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_ucits_sicav.yaml` slot=`case` verb=`case.submit`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_ucits_sicav.yaml` slot=`case` verb=`case.approve`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_ucits_sicav.yaml` slot=`case` verb=`case.reject`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_ucits_sicav.yaml` slot=`case` verb=`case.request-info`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_ucits_sicav.yaml` slot=`case.tollgate` verb=`tollgate.evaluate`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_lux_ucits_sicav.yaml` slot=`mandate` verb=`mandate.create`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_pe_cross_border.yaml` slot=`cbu` verb=`cbu.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_pe_cross_border.yaml` slot=`cbu` verb=`cbu.show`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_pe_cross_border.yaml` slot=`cbu.us_parallel` verb=`cbu.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_pe_cross_border.yaml` slot=`cbu.aggregator` verb=`cbu.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_pe_cross_border.yaml` slot=`general_partner` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_pe_cross_border.yaml` slot=`general_partner` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_pe_cross_border.yaml` slot=`general_partner` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_pe_cross_border.yaml` slot=`general_partner` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_pe_cross_border.yaml` slot=`aifm` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_pe_cross_border.yaml` slot=`aifm` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_pe_cross_border.yaml` slot=`aifm` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_pe_cross_border.yaml` slot=`aifm` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_pe_cross_border.yaml` slot=`depositary` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_pe_cross_border.yaml` slot=`depositary` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_pe_cross_border.yaml` slot=`depositary` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_pe_cross_border.yaml` slot=`depositary` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_pe_cross_border.yaml` slot=`administrator` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_pe_cross_border.yaml` slot=`administrator` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_pe_cross_border.yaml` slot=`administrator` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_pe_cross_border.yaml` slot=`administrator` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_pe_cross_border.yaml` slot=`auditor` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_pe_cross_border.yaml` slot=`auditor` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_pe_cross_border.yaml` slot=`auditor` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_pe_cross_border.yaml` slot=`auditor` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_pe_cross_border.yaml` slot=`legal_counsel` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_pe_cross_border.yaml` slot=`legal_counsel` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_pe_cross_border.yaml` slot=`legal_counsel` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_pe_cross_border.yaml` slot=`legal_counsel` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_pe_cross_border.yaml` slot=`ownership_chain` verb=`ubo.discover`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_pe_cross_border.yaml` slot=`ownership_chain` verb=`ubo.allege`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_pe_cross_border.yaml` slot=`ownership_chain` verb=`ubo.verify`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_pe_cross_border.yaml` slot=`ownership_chain` verb=`ubo.promote`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_pe_cross_border.yaml` slot=`ownership_chain` verb=`ubo.approve`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_pe_cross_border.yaml` slot=`ownership_chain` verb=`ubo.reject`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_pe_cross_border.yaml` slot=`case` verb=`case.open`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_pe_cross_border.yaml` slot=`case` verb=`case.submit`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_pe_cross_border.yaml` slot=`case` verb=`case.approve`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_pe_cross_border.yaml` slot=`case` verb=`case.reject`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_pe_cross_border.yaml` slot=`case` verb=`case.request-info`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_pe_cross_border.yaml` slot=`case.tollgate` verb=`tollgate.evaluate`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_pe_cross_border.yaml` slot=`mandate` verb=`mandate.create`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_acs.yaml` slot=`cbu` verb=`cbu.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_acs.yaml` slot=`cbu` verb=`cbu.show`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_acs.yaml` slot=`acs_operator` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_acs.yaml` slot=`acs_operator` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_acs.yaml` slot=`acs_operator` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_acs.yaml` slot=`acs_operator` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_acs.yaml` slot=`depositary` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_acs.yaml` slot=`depositary` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_acs.yaml` slot=`depositary` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_acs.yaml` slot=`depositary` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_acs.yaml` slot=`investment_manager` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_acs.yaml` slot=`investment_manager` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_acs.yaml` slot=`investment_manager` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_acs.yaml` slot=`investment_manager` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_acs.yaml` slot=`administrator` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_acs.yaml` slot=`administrator` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_acs.yaml` slot=`administrator` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_acs.yaml` slot=`administrator` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_acs.yaml` slot=`auditor` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_acs.yaml` slot=`auditor` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_acs.yaml` slot=`auditor` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_acs.yaml` slot=`auditor` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_acs.yaml` slot=`ownership_chain` verb=`ubo.discover`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_acs.yaml` slot=`ownership_chain` verb=`ubo.allege`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_acs.yaml` slot=`ownership_chain` verb=`ubo.verify`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_acs.yaml` slot=`ownership_chain` verb=`ubo.promote`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_acs.yaml` slot=`ownership_chain` verb=`ubo.approve`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_acs.yaml` slot=`ownership_chain` verb=`ubo.reject`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_acs.yaml` slot=`case` verb=`case.open`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_acs.yaml` slot=`case` verb=`case.submit`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_acs.yaml` slot=`case` verb=`case.approve`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_acs.yaml` slot=`case` verb=`case.reject`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_acs.yaml` slot=`case` verb=`case.request-info`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_acs.yaml` slot=`case.tollgate` verb=`tollgate.evaluate`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_acs.yaml` slot=`mandate` verb=`mandate.create`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_aut.yaml` slot=`cbu` verb=`cbu.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_aut.yaml` slot=`cbu` verb=`cbu.show`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_aut.yaml` slot=`authorised_fund_manager` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_aut.yaml` slot=`authorised_fund_manager` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_aut.yaml` slot=`authorised_fund_manager` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_aut.yaml` slot=`authorised_fund_manager` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_aut.yaml` slot=`trustee` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_aut.yaml` slot=`trustee` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_aut.yaml` slot=`trustee` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_aut.yaml` slot=`trustee` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_aut.yaml` slot=`investment_manager` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_aut.yaml` slot=`investment_manager` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_aut.yaml` slot=`investment_manager` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_aut.yaml` slot=`investment_manager` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_aut.yaml` slot=`administrator` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_aut.yaml` slot=`administrator` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_aut.yaml` slot=`administrator` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_aut.yaml` slot=`administrator` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_aut.yaml` slot=`auditor` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_aut.yaml` slot=`auditor` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_aut.yaml` slot=`auditor` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_aut.yaml` slot=`auditor` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_aut.yaml` slot=`ownership_chain` verb=`ubo.discover`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_aut.yaml` slot=`ownership_chain` verb=`ubo.allege`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_aut.yaml` slot=`ownership_chain` verb=`ubo.verify`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_aut.yaml` slot=`ownership_chain` verb=`ubo.promote`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_aut.yaml` slot=`ownership_chain` verb=`ubo.approve`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_aut.yaml` slot=`ownership_chain` verb=`ubo.reject`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_aut.yaml` slot=`case` verb=`case.open`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_aut.yaml` slot=`case` verb=`case.submit`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_aut.yaml` slot=`case` verb=`case.approve`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_aut.yaml` slot=`case` verb=`case.reject`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_aut.yaml` slot=`case` verb=`case.request-info`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_aut.yaml` slot=`case.tollgate` verb=`tollgate.evaluate`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_aut.yaml` slot=`mandate` verb=`mandate.create`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_ltaf.yaml` slot=`cbu` verb=`cbu.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_ltaf.yaml` slot=`cbu` verb=`cbu.show`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_ltaf.yaml` slot=`authorised_corporate_director` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_ltaf.yaml` slot=`authorised_corporate_director` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_ltaf.yaml` slot=`authorised_corporate_director` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_ltaf.yaml` slot=`authorised_corporate_director` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_ltaf.yaml` slot=`depositary` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_ltaf.yaml` slot=`depositary` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_ltaf.yaml` slot=`depositary` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_ltaf.yaml` slot=`depositary` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_ltaf.yaml` slot=`investment_manager` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_ltaf.yaml` slot=`investment_manager` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_ltaf.yaml` slot=`investment_manager` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_ltaf.yaml` slot=`investment_manager` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_ltaf.yaml` slot=`administrator` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_ltaf.yaml` slot=`administrator` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_ltaf.yaml` slot=`administrator` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_ltaf.yaml` slot=`administrator` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_ltaf.yaml` slot=`auditor` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_ltaf.yaml` slot=`auditor` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_ltaf.yaml` slot=`auditor` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_ltaf.yaml` slot=`auditor` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_ltaf.yaml` slot=`registrar` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_ltaf.yaml` slot=`registrar` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_ltaf.yaml` slot=`registrar` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_ltaf.yaml` slot=`registrar` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_ltaf.yaml` slot=`valuation_agent` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_ltaf.yaml` slot=`valuation_agent` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_ltaf.yaml` slot=`valuation_agent` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_ltaf.yaml` slot=`valuation_agent` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_ltaf.yaml` slot=`ownership_chain` verb=`ubo.discover`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_ltaf.yaml` slot=`ownership_chain` verb=`ubo.allege`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_ltaf.yaml` slot=`ownership_chain` verb=`ubo.verify`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_ltaf.yaml` slot=`ownership_chain` verb=`ubo.promote`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_ltaf.yaml` slot=`ownership_chain` verb=`ubo.approve`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_ltaf.yaml` slot=`ownership_chain` verb=`ubo.reject`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_ltaf.yaml` slot=`case` verb=`case.open`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_ltaf.yaml` slot=`case` verb=`case.submit`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_ltaf.yaml` slot=`case` verb=`case.approve`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_ltaf.yaml` slot=`case` verb=`case.reject`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_ltaf.yaml` slot=`case` verb=`case.request-info`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_ltaf.yaml` slot=`case.tollgate` verb=`tollgate.evaluate`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_ltaf.yaml` slot=`mandate` verb=`mandate.create`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_oeic.yaml` slot=`cbu` verb=`cbu.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_oeic.yaml` slot=`cbu` verb=`cbu.show`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_oeic.yaml` slot=`authorised_corporate_director` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_oeic.yaml` slot=`authorised_corporate_director` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_oeic.yaml` slot=`authorised_corporate_director` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_oeic.yaml` slot=`authorised_corporate_director` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_oeic.yaml` slot=`depositary` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_oeic.yaml` slot=`depositary` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_oeic.yaml` slot=`depositary` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_oeic.yaml` slot=`depositary` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_oeic.yaml` slot=`investment_manager` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_oeic.yaml` slot=`investment_manager` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_oeic.yaml` slot=`investment_manager` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_oeic.yaml` slot=`investment_manager` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_oeic.yaml` slot=`administrator` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_oeic.yaml` slot=`administrator` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_oeic.yaml` slot=`administrator` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_oeic.yaml` slot=`administrator` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_oeic.yaml` slot=`auditor` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_oeic.yaml` slot=`auditor` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_oeic.yaml` slot=`auditor` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_oeic.yaml` slot=`auditor` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_oeic.yaml` slot=`registrar` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_oeic.yaml` slot=`registrar` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_oeic.yaml` slot=`registrar` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_oeic.yaml` slot=`registrar` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_oeic.yaml` slot=`ownership_chain` verb=`ubo.discover`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_oeic.yaml` slot=`ownership_chain` verb=`ubo.allege`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_oeic.yaml` slot=`ownership_chain` verb=`ubo.verify`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_oeic.yaml` slot=`ownership_chain` verb=`ubo.promote`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_oeic.yaml` slot=`ownership_chain` verb=`ubo.approve`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_oeic.yaml` slot=`ownership_chain` verb=`ubo.reject`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_oeic.yaml` slot=`case` verb=`case.open`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_oeic.yaml` slot=`case` verb=`case.submit`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_oeic.yaml` slot=`case` verb=`case.approve`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_oeic.yaml` slot=`case` verb=`case.reject`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_oeic.yaml` slot=`case` verb=`case.request-info`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_oeic.yaml` slot=`case.tollgate` verb=`tollgate.evaluate`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_oeic.yaml` slot=`mandate` verb=`mandate.create`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_manager_llp.yaml` slot=`cbu` verb=`cbu.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_manager_llp.yaml` slot=`cbu` verb=`cbu.show`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_manager_llp.yaml` slot=`designated_member_1` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_manager_llp.yaml` slot=`designated_member_1` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_manager_llp.yaml` slot=`designated_member_1` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_manager_llp.yaml` slot=`designated_member_1` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_manager_llp.yaml` slot=`designated_member_2` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_manager_llp.yaml` slot=`designated_member_2` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_manager_llp.yaml` slot=`designated_member_2` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_manager_llp.yaml` slot=`designated_member_2` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_manager_llp.yaml` slot=`compliance_officer` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_manager_llp.yaml` slot=`compliance_officer` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_manager_llp.yaml` slot=`compliance_officer` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_manager_llp.yaml` slot=`compliance_officer` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_manager_llp.yaml` slot=`mlro` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_manager_llp.yaml` slot=`mlro` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_manager_llp.yaml` slot=`mlro` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_manager_llp.yaml` slot=`mlro` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_manager_llp.yaml` slot=`auditor` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_manager_llp.yaml` slot=`auditor` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_manager_llp.yaml` slot=`auditor` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_manager_llp.yaml` slot=`auditor` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_manager_llp.yaml` slot=`ownership_chain` verb=`ubo.discover`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_manager_llp.yaml` slot=`ownership_chain` verb=`ubo.allege`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_manager_llp.yaml` slot=`ownership_chain` verb=`ubo.verify`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_manager_llp.yaml` slot=`ownership_chain` verb=`ubo.promote`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_manager_llp.yaml` slot=`ownership_chain` verb=`ubo.approve`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_manager_llp.yaml` slot=`ownership_chain` verb=`ubo.reject`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_manager_llp.yaml` slot=`case` verb=`case.open`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_manager_llp.yaml` slot=`case` verb=`case.submit`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_manager_llp.yaml` slot=`case` verb=`case.approve`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_manager_llp.yaml` slot=`case` verb=`case.reject`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_manager_llp.yaml` slot=`case` verb=`case.request-info`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_manager_llp.yaml` slot=`case.tollgate` verb=`tollgate.evaluate`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_pe_lp.yaml` slot=`cbu` verb=`cbu.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_pe_lp.yaml` slot=`cbu` verb=`cbu.show`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_pe_lp.yaml` slot=`general_partner` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_pe_lp.yaml` slot=`general_partner` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_pe_lp.yaml` slot=`general_partner` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_pe_lp.yaml` slot=`general_partner` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_pe_lp.yaml` slot=`aifm` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_pe_lp.yaml` slot=`aifm` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_pe_lp.yaml` slot=`aifm` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_pe_lp.yaml` slot=`aifm` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_pe_lp.yaml` slot=`depositary` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_pe_lp.yaml` slot=`depositary` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_pe_lp.yaml` slot=`depositary` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_pe_lp.yaml` slot=`depositary` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_pe_lp.yaml` slot=`administrator` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_pe_lp.yaml` slot=`administrator` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_pe_lp.yaml` slot=`administrator` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_pe_lp.yaml` slot=`administrator` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_pe_lp.yaml` slot=`auditor` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_pe_lp.yaml` slot=`auditor` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_pe_lp.yaml` slot=`auditor` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_pe_lp.yaml` slot=`auditor` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_pe_lp.yaml` slot=`legal_counsel` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_pe_lp.yaml` slot=`legal_counsel` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_pe_lp.yaml` slot=`legal_counsel` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_pe_lp.yaml` slot=`legal_counsel` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_pe_lp.yaml` slot=`ownership_chain` verb=`ubo.discover`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_pe_lp.yaml` slot=`ownership_chain` verb=`ubo.allege`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_pe_lp.yaml` slot=`ownership_chain` verb=`ubo.verify`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_pe_lp.yaml` slot=`ownership_chain` verb=`ubo.promote`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_pe_lp.yaml` slot=`ownership_chain` verb=`ubo.approve`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_pe_lp.yaml` slot=`ownership_chain` verb=`ubo.reject`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_pe_lp.yaml` slot=`case` verb=`case.open`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_pe_lp.yaml` slot=`case` verb=`case.submit`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_pe_lp.yaml` slot=`case` verb=`case.approve`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_pe_lp.yaml` slot=`case` verb=`case.reject`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_pe_lp.yaml` slot=`case` verb=`case.request-info`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_pe_lp.yaml` slot=`case.tollgate` verb=`tollgate.evaluate`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_uk_pe_lp.yaml` slot=`mandate` verb=`mandate.create`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_closed_end.yaml` slot=`cbu` verb=`cbu.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_closed_end.yaml` slot=`cbu` verb=`cbu.show`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_closed_end.yaml` slot=`investment_adviser` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_closed_end.yaml` slot=`investment_adviser` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_closed_end.yaml` slot=`investment_adviser` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_closed_end.yaml` slot=`investment_adviser` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_closed_end.yaml` slot=`custodian` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_closed_end.yaml` slot=`custodian` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_closed_end.yaml` slot=`custodian` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_closed_end.yaml` slot=`custodian` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_closed_end.yaml` slot=`sub_adviser` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_closed_end.yaml` slot=`sub_adviser` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_closed_end.yaml` slot=`sub_adviser` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_closed_end.yaml` slot=`sub_adviser` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_closed_end.yaml` slot=`administrator` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_closed_end.yaml` slot=`administrator` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_closed_end.yaml` slot=`administrator` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_closed_end.yaml` slot=`administrator` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_closed_end.yaml` slot=`transfer_agent` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_closed_end.yaml` slot=`transfer_agent` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_closed_end.yaml` slot=`transfer_agent` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_closed_end.yaml` slot=`transfer_agent` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_closed_end.yaml` slot=`auditor` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_closed_end.yaml` slot=`auditor` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_closed_end.yaml` slot=`auditor` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_closed_end.yaml` slot=`auditor` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_closed_end.yaml` slot=`legal_counsel` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_closed_end.yaml` slot=`legal_counsel` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_closed_end.yaml` slot=`legal_counsel` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_closed_end.yaml` slot=`legal_counsel` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_closed_end.yaml` slot=`ownership_chain` verb=`ubo.discover`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_closed_end.yaml` slot=`ownership_chain` verb=`ubo.allege`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_closed_end.yaml` slot=`ownership_chain` verb=`ubo.verify`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_closed_end.yaml` slot=`ownership_chain` verb=`ubo.promote`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_closed_end.yaml` slot=`ownership_chain` verb=`ubo.approve`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_closed_end.yaml` slot=`ownership_chain` verb=`ubo.reject`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_closed_end.yaml` slot=`case` verb=`case.open`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_closed_end.yaml` slot=`case` verb=`case.submit`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_closed_end.yaml` slot=`case` verb=`case.approve`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_closed_end.yaml` slot=`case` verb=`case.reject`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_closed_end.yaml` slot=`case` verb=`case.request-info`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_closed_end.yaml` slot=`case.tollgate` verb=`tollgate.evaluate`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_closed_end.yaml` slot=`mandate` verb=`mandate.create`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_open_end.yaml` slot=`cbu` verb=`cbu.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_open_end.yaml` slot=`cbu` verb=`cbu.show`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_open_end.yaml` slot=`investment_adviser` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_open_end.yaml` slot=`investment_adviser` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_open_end.yaml` slot=`investment_adviser` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_open_end.yaml` slot=`investment_adviser` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_open_end.yaml` slot=`custodian` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_open_end.yaml` slot=`custodian` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_open_end.yaml` slot=`custodian` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_open_end.yaml` slot=`custodian` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_open_end.yaml` slot=`sub_adviser` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_open_end.yaml` slot=`sub_adviser` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_open_end.yaml` slot=`sub_adviser` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_open_end.yaml` slot=`sub_adviser` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_open_end.yaml` slot=`administrator` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_open_end.yaml` slot=`administrator` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_open_end.yaml` slot=`administrator` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_open_end.yaml` slot=`administrator` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_open_end.yaml` slot=`transfer_agent` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_open_end.yaml` slot=`transfer_agent` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_open_end.yaml` slot=`transfer_agent` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_open_end.yaml` slot=`transfer_agent` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_open_end.yaml` slot=`distributor` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_open_end.yaml` slot=`distributor` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_open_end.yaml` slot=`distributor` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_open_end.yaml` slot=`distributor` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_open_end.yaml` slot=`auditor` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_open_end.yaml` slot=`auditor` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_open_end.yaml` slot=`auditor` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_open_end.yaml` slot=`auditor` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_open_end.yaml` slot=`legal_counsel` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_open_end.yaml` slot=`legal_counsel` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_open_end.yaml` slot=`legal_counsel` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_open_end.yaml` slot=`legal_counsel` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_open_end.yaml` slot=`ownership_chain` verb=`ubo.discover`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_open_end.yaml` slot=`ownership_chain` verb=`ubo.allege`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_open_end.yaml` slot=`ownership_chain` verb=`ubo.verify`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_open_end.yaml` slot=`ownership_chain` verb=`ubo.promote`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_open_end.yaml` slot=`ownership_chain` verb=`ubo.approve`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_open_end.yaml` slot=`ownership_chain` verb=`ubo.reject`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_open_end.yaml` slot=`case` verb=`case.open`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_open_end.yaml` slot=`case` verb=`case.submit`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_open_end.yaml` slot=`case` verb=`case.approve`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_open_end.yaml` slot=`case` verb=`case.reject`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_open_end.yaml` slot=`case` verb=`case.request-info`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_open_end.yaml` slot=`case.tollgate` verb=`tollgate.evaluate`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_40act_open_end.yaml` slot=`mandate` verb=`mandate.create`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_etf_40act.yaml` slot=`cbu` verb=`cbu.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_etf_40act.yaml` slot=`cbu` verb=`cbu.show`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_etf_40act.yaml` slot=`investment_adviser` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_etf_40act.yaml` slot=`investment_adviser` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_etf_40act.yaml` slot=`investment_adviser` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_etf_40act.yaml` slot=`investment_adviser` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_etf_40act.yaml` slot=`custodian` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_etf_40act.yaml` slot=`custodian` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_etf_40act.yaml` slot=`custodian` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_etf_40act.yaml` slot=`custodian` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_etf_40act.yaml` slot=`authorized_participant` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_etf_40act.yaml` slot=`authorized_participant` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_etf_40act.yaml` slot=`authorized_participant` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_etf_40act.yaml` slot=`authorized_participant` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_etf_40act.yaml` slot=`sub_adviser` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_etf_40act.yaml` slot=`sub_adviser` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_etf_40act.yaml` slot=`sub_adviser` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_etf_40act.yaml` slot=`sub_adviser` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_etf_40act.yaml` slot=`administrator` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_etf_40act.yaml` slot=`administrator` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_etf_40act.yaml` slot=`administrator` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_etf_40act.yaml` slot=`administrator` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_etf_40act.yaml` slot=`transfer_agent` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_etf_40act.yaml` slot=`transfer_agent` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_etf_40act.yaml` slot=`transfer_agent` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_etf_40act.yaml` slot=`transfer_agent` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_etf_40act.yaml` slot=`distributor` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_etf_40act.yaml` slot=`distributor` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_etf_40act.yaml` slot=`distributor` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_etf_40act.yaml` slot=`distributor` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_etf_40act.yaml` slot=`auditor` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_etf_40act.yaml` slot=`auditor` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_etf_40act.yaml` slot=`auditor` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_etf_40act.yaml` slot=`auditor` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_etf_40act.yaml` slot=`market_maker` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_etf_40act.yaml` slot=`market_maker` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_etf_40act.yaml` slot=`market_maker` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_etf_40act.yaml` slot=`market_maker` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_etf_40act.yaml` slot=`ownership_chain` verb=`ubo.discover`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_etf_40act.yaml` slot=`ownership_chain` verb=`ubo.allege`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_etf_40act.yaml` slot=`ownership_chain` verb=`ubo.verify`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_etf_40act.yaml` slot=`ownership_chain` verb=`ubo.promote`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_etf_40act.yaml` slot=`ownership_chain` verb=`ubo.approve`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_etf_40act.yaml` slot=`ownership_chain` verb=`ubo.reject`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_etf_40act.yaml` slot=`case` verb=`case.open`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_etf_40act.yaml` slot=`case` verb=`case.submit`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_etf_40act.yaml` slot=`case` verb=`case.approve`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_etf_40act.yaml` slot=`case` verb=`case.reject`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_etf_40act.yaml` slot=`case` verb=`case.request-info`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_etf_40act.yaml` slot=`case.tollgate` verb=`tollgate.evaluate`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_etf_40act.yaml` slot=`mandate` verb=`mandate.create`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_private_fund_delaware_lp.yaml` slot=`cbu` verb=`cbu.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_private_fund_delaware_lp.yaml` slot=`cbu` verb=`cbu.show`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_private_fund_delaware_lp.yaml` slot=`general_partner` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_private_fund_delaware_lp.yaml` slot=`general_partner` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_private_fund_delaware_lp.yaml` slot=`general_partner` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_private_fund_delaware_lp.yaml` slot=`general_partner` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_private_fund_delaware_lp.yaml` slot=`investment_manager` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_private_fund_delaware_lp.yaml` slot=`investment_manager` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_private_fund_delaware_lp.yaml` slot=`investment_manager` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_private_fund_delaware_lp.yaml` slot=`investment_manager` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_private_fund_delaware_lp.yaml` slot=`custodian` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_private_fund_delaware_lp.yaml` slot=`custodian` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_private_fund_delaware_lp.yaml` slot=`custodian` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_private_fund_delaware_lp.yaml` slot=`custodian` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_private_fund_delaware_lp.yaml` slot=`administrator` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_private_fund_delaware_lp.yaml` slot=`administrator` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_private_fund_delaware_lp.yaml` slot=`administrator` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_private_fund_delaware_lp.yaml` slot=`administrator` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_private_fund_delaware_lp.yaml` slot=`prime_broker` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_private_fund_delaware_lp.yaml` slot=`prime_broker` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_private_fund_delaware_lp.yaml` slot=`prime_broker` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_private_fund_delaware_lp.yaml` slot=`prime_broker` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_private_fund_delaware_lp.yaml` slot=`auditor` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_private_fund_delaware_lp.yaml` slot=`auditor` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_private_fund_delaware_lp.yaml` slot=`auditor` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_private_fund_delaware_lp.yaml` slot=`auditor` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_private_fund_delaware_lp.yaml` slot=`legal_counsel` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_private_fund_delaware_lp.yaml` slot=`legal_counsel` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_private_fund_delaware_lp.yaml` slot=`legal_counsel` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_private_fund_delaware_lp.yaml` slot=`legal_counsel` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_private_fund_delaware_lp.yaml` slot=`tax_advisor` verb=`entity.ensure-or-placeholder`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_private_fund_delaware_lp.yaml` slot=`tax_advisor` verb=`party.search`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_private_fund_delaware_lp.yaml` slot=`tax_advisor` verb=`party.add`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_private_fund_delaware_lp.yaml` slot=`tax_advisor` verb=`entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_private_fund_delaware_lp.yaml` slot=`ownership_chain` verb=`ubo.discover`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_private_fund_delaware_lp.yaml` slot=`ownership_chain` verb=`ubo.allege`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_private_fund_delaware_lp.yaml` slot=`ownership_chain` verb=`ubo.verify`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_private_fund_delaware_lp.yaml` slot=`ownership_chain` verb=`ubo.promote`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_private_fund_delaware_lp.yaml` slot=`ownership_chain` verb=`ubo.approve`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_private_fund_delaware_lp.yaml` slot=`ownership_chain` verb=`ubo.reject`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_private_fund_delaware_lp.yaml` slot=`case` verb=`case.open`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_private_fund_delaware_lp.yaml` slot=`case` verb=`case.submit`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_private_fund_delaware_lp.yaml` slot=`case` verb=`case.approve`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_private_fund_delaware_lp.yaml` slot=`case` verb=`case.reject`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_private_fund_delaware_lp.yaml` slot=`case` verb=`case.request-info`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_private_fund_delaware_lp.yaml` slot=`case.tollgate` verb=`tollgate.evaluate`
  - file=`rust/config/sem_os_seeds/constellation_maps/struct_us_private_fund_delaware_lp.yaml` slot=`mandate` verb=`mandate.create`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`cbu` verb=`cbu.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`trading_profile` verb=`trading-profile.import`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`trading_profile` verb=`trading-profile.create-draft`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`trading_profile` verb=`trading-profile.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`trading_profile` verb=`trading-profile.get-active`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`trading_profile` verb=`trading-profile.list-versions`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`trading_profile` verb=`trading-profile.materialize`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`trading_profile` verb=`trading-profile.activate`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`trading_profile` verb=`trading-profile.diff`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`trading_profile` verb=`trading-profile.clone-to`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`trading_profile` verb=`trading-profile.create-new-version`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`trading_profile` verb=`trading-profile.set-base-currency`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`trading_profile` verb=`trading-profile.link-csa-ssi`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`trading_profile` verb=`trading-profile.update-im-scope`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`trading_profile` verb=`trading-profile.ca.add-cutoff-rule`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`trading_profile` verb=`trading-profile.ca.remove-cutoff-rule`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`trading_profile` verb=`trading-profile.ca.enable-event-types`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`trading_profile` verb=`trading-profile.ca.disable-event-types`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`trading_profile` verb=`trading-profile.ca.set-default-option`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`trading_profile` verb=`trading-profile.ca.remove-default-option`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`trading_profile` verb=`trading-profile.ca.link-proceeds-ssi`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`trading_profile` verb=`trading-profile.ca.remove-proceeds-ssi`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`trading_profile` verb=`trading-profile.validate-go-live-ready`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`trading_profile` verb=`trading-profile.validate-universe-coverage`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`trading_profile` verb=`trading-profile.submit`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`trading_profile` verb=`trading-profile.approve`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`trading_profile` verb=`trading-profile.reject`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`trading_profile` verb=`trading-profile.archive`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`trading_profile` verb=`matrix-overlay.create`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`trading_profile` verb=`matrix-overlay.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`trading_profile` verb=`matrix-overlay.list`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`trading_profile` verb=`matrix-overlay.update`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`trading_profile` verb=`matrix-overlay.apply`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`trading_profile` verb=`matrix-overlay.remove`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`trading_profile` verb=`matrix-overlay.diff`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`trading_profile` verb=`matrix-overlay.preview`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`trading_profile` verb=`matrix-overlay.list-active`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`custody` verb=`custody.list-universe`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`custody` verb=`custody.list-ssis`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`custody` verb=`custody.list-booking-rules`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`custody` verb=`custody.list-agent-overrides`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`custody` verb=`custody.derive-required-coverage`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`custody` verb=`custody.validate-booking-coverage`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`custody` verb=`custody.lookup-ssi`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`custody` verb=`custody.setup-ssi`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`booking_principal` verb=`booking-principal.update`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`booking_principal` verb=`booking-principal.retire`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`booking_principal` verb=`booking-principal.select`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`booking_principal` verb=`booking-principal.explain`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`booking_principal` verb=`booking-principal.coverage-matrix`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`booking_principal` verb=`booking-principal.gap-report`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`booking_principal` verb=`booking-principal.impact-analysis`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`cash_sweep` verb=`cash-sweep.configure`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`cash_sweep` verb=`cash-sweep.link-resource`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`cash_sweep` verb=`cash-sweep.list`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`cash_sweep` verb=`cash-sweep.update-threshold`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`cash_sweep` verb=`cash-sweep.update-timing`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`cash_sweep` verb=`cash-sweep.change-vehicle`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`cash_sweep` verb=`cash-sweep.suspend`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`cash_sweep` verb=`cash-sweep.reactivate`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`cash_sweep` verb=`cash-sweep.remove`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`service_resource` verb=`service-resource.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`service_resource` verb=`service-resource.list`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`service_resource` verb=`service-resource.provision`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`service_resource` verb=`service-resource.set-attr`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`service_resource` verb=`service-resource.activate`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`service_resource` verb=`service-resource.suspend`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`service_resource` verb=`service-resource.decommission`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`service_resource` verb=`service-resource.validate-attrs`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`service_intent` verb=`service-intent.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`service_intent` verb=`service-intent.list`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`service_intent` verb=`service-intent.update`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`service_intent` verb=`service-intent.approve`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`service_intent` verb=`service-intent.reject`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`service_intent` verb=`service-intent.cancel`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`service_intent` verb=`service-intent.list-available`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`service_intent` verb=`service-intent.list-by-status`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`service_intent` verb=`service-intent.activate`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`service_intent` verb=`service-intent.deactivate`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`service_intent` verb=`service-intent.clone`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`booking_location` verb=`booking-location.create`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`booking_location` verb=`booking-location.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`booking_location` verb=`booking-location.list`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`legal_entity` verb=`legal-entity.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`legal_entity` verb=`legal-entity.list`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`product` verb=`product.create`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`product` verb=`product.list`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`delivery` verb=`delivery.create`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`delivery` verb=`delivery.read`
  - file=`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml` slot=`delivery` verb=`delivery.list`
- depends_on referencing non-existent slots:
  - `None`
