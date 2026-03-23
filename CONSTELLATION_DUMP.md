# Full Constellation Map Dump

---
## File: rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml
```yaml
constellation: deal.lifecycle
description: Commercial deal entity topology — the deal as a hub entity linking participants, contracts, products, and onboarding requests. Rate cards and billing attach to the deal but are workflow concerns, not structural positions.
jurisdiction: ALL
slots:
  deal:
    type: cbu
    table: deals
    pk: deal_id
    cardinality: root
    state_machine: deal_lifecycle
    verbs:
      create: { verb: deal.create, when: empty }
      read: { verb: deal.read-record, when: filled }
      list: { verb: deal.list, when: [empty, filled] }
      search: { verb: deal.search-records, when: [empty, filled] }
      summary: { verb: deal.read-summary, when: filled }
      timeline: { verb: deal.read-timeline, when: filled }
      list_documents: { verb: deal.list-documents, when: filled }
      list_slas: { verb: deal.list-slas, when: filled }
      list_active_rate_cards: { verb: deal.list-active-rate-cards, when: filled }
      list_rate_card_lines: { verb: deal.list-rate-card-lines, when: filled }
      list_rate_card_history: { verb: deal.list-rate-card-history, when: filled }
      update: { verb: deal.update-record, when: filled }
      update_status: { verb: deal.update-status, when: filled }
      add_document: { verb: deal.add-document, when: filled }
      update_document_status: { verb: deal.update-document-status, when: filled }
      add_sla: { verb: deal.add-sla, when: filled }
      remove_sla: { verb: deal.remove-sla, when: filled }
      add_ubo_assessment: { verb: deal.add-ubo-assessment, when: filled }
      update_ubo_assessment: { verb: deal.update-ubo-assessment, when: filled }
      cancel: { verb: deal.cancel, when: filled }
  participant:
    type: entity
    entity_kinds: [person]
    join: { via: deal_participants, parent_fk: deal_id, child_fk: participant_id }
    cardinality: optional
    depends_on: [deal]
    verbs:
      add: { verb: deal.add-participant, when: [empty, filled] }
      remove: { verb: deal.remove-participant, when: filled }
      list: { verb: deal.list-participants, when: filled }
  deal_contract:
    type: entity
    entity_kinds: [contract]
    join: { via: deal_contracts, parent_fk: deal_id, child_fk: contract_id }
    cardinality: optional
    depends_on: [deal]
    verbs:
      add: { verb: deal.add-contract, when: [empty, filled] }
      remove: { verb: deal.remove-contract, when: filled }
      list: { verb: deal.list-contracts, when: filled }
  contract:
    type: entity
    entity_kinds: [contract]
    join: { via: legal_contracts, parent_fk: contract_id, child_fk: contract_id }
    cardinality: optional
    depends_on: [deal]
    verbs:
      create: { verb: contract.create, when: empty }
      read: { verb: contract.get, when: filled }
      list: { verb: contract.list, when: [empty, filled] }
      list_products: { verb: contract.list-products, when: filled }
      list_rate_cards: { verb: contract.list-rate-cards, when: filled }
      list_subscriptions: { verb: contract.list-subscriptions, when: filled }
      for_client: { verb: contract.for-client, when: [empty, filled] }
      update: { verb: contract.update, when: filled }
      add_product: { verb: contract.add-product, when: filled }
      remove_product: { verb: contract.remove-product, when: filled }
      create_rate_card: { verb: contract.create-rate-card, when: filled }
      subscribe: { verb: contract.subscribe, when: filled }
      unsubscribe: { verb: contract.unsubscribe, when: filled }
      terminate: { verb: contract.terminate, when: filled }
  deal_product:
    type: entity
    entity_kinds: [entity]
    join: { via: deal_products, parent_fk: deal_id, child_fk: product_id }
    cardinality: optional
    depends_on: [deal]
    verbs:
      add: { verb: deal.add-product, when: [empty, filled] }
      update: { verb: deal.update-product-status, when: filled }
      remove: { verb: deal.remove-product, when: filled }
      list: { verb: deal.list-products, when: filled }
  rate_card:
    type: entity
    entity_kinds: [entity]
    join: { via: deal_rate_cards, parent_fk: deal_id, child_fk: rate_card_id }
    cardinality: optional
    depends_on: [deal_product]
    verbs:
      create: { verb: deal.create-rate-card, when: empty }
      add_line: { verb: deal.add-rate-card-line, when: filled }
      update_line: { verb: deal.update-rate-card-line, when: filled }
      remove_line: { verb: deal.remove-rate-card-line, when: filled }
      list: { verb: deal.list-rate-cards, when: filled }
      list_lines: { verb: deal.list-rate-card-lines, when: filled }
      list_history: { verb: deal.list-rate-card-history, when: filled }
      list_active: { verb: deal.list-active-rate-cards, when: filled }
      propose: { verb: deal.propose-rate-card, when: filled }
      counter: { verb: deal.counter-rate-card, when: filled }
      agree: { verb: deal.agree-rate-card, when: filled }
  onboarding_request:
    type: entity
    entity_kinds: [entity]
    join: { via: deal_onboarding_requests, parent_fk: deal_id, child_fk: request_id }
    cardinality: optional
    depends_on: [{ slot: deal, min_state: contracted }]
    verbs:
      request: { verb: deal.request-onboarding, when: empty }
      request_batch: { verb: deal.request-onboarding-batch, when: [empty, filled] }
      update: { verb: deal.update-onboarding-status, when: filled }
      list: { verb: deal.list-onboarding-requests, when: filled }
  billing_profile:
    type: entity
    entity_kinds: [entity]
    join: { via: fee_billing_profiles, parent_fk: deal_id, child_fk: profile_id }
    cardinality: optional
    depends_on: [rate_card]
    verbs:
      create: { verb: billing.create-profile, when: empty }
      activate: { verb: billing.activate-profile, when: filled }
      suspend: { verb: billing.suspend-profile, when: filled }
      close: { verb: billing.close-profile, when: filled }
      read: { verb: billing.get-profile, when: filled }
      list: { verb: billing.list-profiles, when: [empty, filled] }
      add_target: { verb: billing.add-account-target, when: filled }
      remove_target: { verb: billing.remove-account-target, when: filled }
      list_targets: { verb: billing.list-account-targets, when: filled }
      create_period: { verb: billing.create-period, when: filled }
      calculate: { verb: billing.calculate-period, when: filled }
      review: { verb: billing.review-period, when: filled }
      approve: { verb: billing.approve-period, when: filled }
      invoice: { verb: billing.generate-invoice, when: filled }
      dispute: { verb: billing.dispute-period, when: filled }
      period_summary: { verb: billing.period-summary, when: filled }
      revenue: { verb: billing.revenue-summary, when: filled }
  pricing:
    type: entity
    entity_kinds: [entity]
    join: { via: pricing_configs, parent_fk: deal_id, child_fk: config_id }
    cardinality: optional
    depends_on: [rate_card]
    verbs:
      set_valuation: { verb: pricing-config.set-valuation-schedule, when: [empty, filled] }
      set_nav: { verb: pricing-config.set-nav-threshold, when: [empty, filled] }
      set_settlement: { verb: pricing-config.set-settlement-calendar, when: [empty, filled] }
      set_holiday: { verb: pricing-config.set-holiday-schedule, when: [empty, filled] }
      set_reporting: { verb: pricing-config.set-reporting, when: [empty, filled] }
      set_tax: { verb: pricing-config.set-tax-status, when: [empty, filled] }
      set_reclaim: { verb: pricing-config.set-reclaim-config, when: [empty, filled] }
      find: { verb: pricing-config.find-for-instrument, when: filled }
      list_jurisdictions: { verb: pricing-config.list-jurisdictions, when: filled }
      list_treaty_rates: { verb: pricing-config.list-treaty-rates, when: filled }
      list_tax_status: { verb: pricing-config.list-tax-status, when: filled }
      list_reclaims: { verb: pricing-config.list-reclaim-configs, when: filled }
  contract_template:
    type: entity
    entity_kinds: [contract]
    join: { via: contract_packs, parent_fk: contract_id, child_fk: pack_id }
    cardinality: optional
    depends_on: [contract]
    verbs:
      create: { verb: contract-pack.create, when: empty }
      read: { verb: contract-pack.read, when: filled }
```

---
## File: rust/config/sem_os_seeds/constellation_maps/fund_administration.yaml
```yaml
constellation: fund.administration
description: Fund vehicle administration constellation — umbrella/sub-fund/share-class structures, capital events, investment management, feeder/master relationships.
jurisdiction: ALL
slots:
  fund:
    type: cbu
    table: entity_funds
    pk: fund_id
    cardinality: root
    state_machine: fund_lifecycle
    verbs:
      create: { verb: fund.create, when: empty }
      ensure: { verb: fund.ensure, when: [empty, filled] }
      read: { verb: fund.read-vehicle, when: filled }
      list: { verb: fund.list-by-manager, when: [empty, filled] }
      list_by_type: { verb: fund.list-by-vehicle-type, when: [empty, filled] }
      upsert: { verb: fund.upsert-vehicle, when: [empty, filled] }
      delete: { verb: fund.delete-vehicle, when: filled }
  umbrella:
    type: entity
    entity_kinds: [fund]
    join: { via: entity_funds, parent_fk: umbrella_id, child_fk: fund_id }
    cardinality: optional
    depends_on: [fund]
    verbs:
      add_subfund: { verb: fund.add-to-umbrella, when: filled }
      list_subfunds: { verb: fund.list-subfunds, when: filled }
      upsert_compartment: { verb: fund.upsert-compartment, when: [empty, filled] }
      read_compartment: { verb: fund.read-compartment, when: filled }
      list_compartments: { verb: fund.list-compartments-by-umbrella, when: filled }
      delete_compartment: { verb: fund.delete-compartment, when: filled }
  share_class:
    type: entity
    entity_kinds: [fund]
    join: { via: fund_share_classes, parent_fk: fund_id, child_fk: share_class_id }
    cardinality: optional
    depends_on: [fund]
    verbs:
      add: { verb: fund.add-share-class, when: [empty, filled] }
      list: { verb: fund.list-share-classes, when: filled }
  feeder:
    type: entity
    entity_kinds: [fund]
    join: { via: fund_feeder_links, parent_fk: master_fund_id, child_fk: feeder_fund_id }
    cardinality: optional
    depends_on: [fund]
    verbs:
      link: { verb: fund.link-feeder, when: [empty, filled] }
      list: { verb: fund.list-feeders, when: filled }
  investment:
    type: entity
    entity_kinds: [entity]
    join: { via: fund_investments, parent_fk: fund_id, child_fk: investment_id }
    cardinality: optional
    depends_on: [fund]
    verbs:
      add: { verb: fund.add-investment, when: [empty, filled] }
      update: { verb: fund.update-investment, when: filled }
      end: { verb: fund.end-investment, when: filled }
      list: { verb: fund.list-investments, when: filled }
      list_investors: { verb: fund.list-investors, when: filled }
  capital:
    type: entity
    entity_kinds: [fund]
    join: { via: capital_events, parent_fk: fund_id, child_fk: event_id }
    cardinality: optional
    depends_on: [fund]
    verbs:
      allocate: { verb: capital.allocate, when: filled }
      issue_initial: { verb: capital.issue.initial, when: [empty, filled] }
      issue_new: { verb: capital.issue.new, when: filled }
      issue_shares: { verb: capital.issue-shares, when: filled }
      cancel_shares: { verb: capital.cancel-shares, when: filled }
      transfer: { verb: capital.transfer, when: filled }
      split: { verb: capital.split, when: filled }
      buyback: { verb: capital.buyback, when: filled }
      cancel: { verb: capital.cancel, when: filled }
      reconcile: { verb: capital.reconcile, when: filled }
      cap_table: { verb: capital.cap-table, when: filled }
      holders: { verb: capital.holders, when: filled }
      list_by_issuer: { verb: capital.list-by-issuer, when: filled }
      list_shareholders: { verb: capital.list-shareholders, when: filled }
      get_ownership_chain: { verb: capital.get-ownership-chain, when: filled }
      define_share_class: { verb: capital.define-share-class, when: [empty, filled] }
      share_class_create: { verb: capital.share-class.create, when: [empty, filled] }
      share_class_list: { verb: capital.share-class.list, when: filled }
      share_class_get_supply: { verb: capital.share-class.get-supply, when: filled }
      share_class_add_identifier: { verb: capital.share-class.add-identifier, when: filled }
      control_config_get: { verb: capital.control-config.get, when: filled }
      control_config_set: { verb: capital.control-config.set, when: filled }
      dilution_grant_options: { verb: capital.dilution.grant-options, when: filled }
      dilution_issue_warrant: { verb: capital.dilution.issue-warrant, when: filled }
      dilution_create_safe: { verb: capital.dilution.create-safe, when: filled }
      dilution_create_note: { verb: capital.dilution.create-convertible-note, when: filled }
      dilution_exercise: { verb: capital.dilution.exercise, when: filled }
      dilution_forfeit: { verb: capital.dilution.forfeit, when: filled }
      dilution_list: { verb: capital.dilution.list, when: filled }
      dilution_get_summary: { verb: capital.dilution.get-summary, when: filled }
  investment_manager:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: investment-manager }
    cardinality: optional
    depends_on: [fund]
    verbs:
      assign: { verb: investment-manager.assign, when: [empty, filled] }
      set_scope: { verb: investment-manager.set-scope, when: filled }
      link_connectivity: { verb: investment-manager.link-connectivity, when: filled }
      list: { verb: investment-manager.list, when: filled }
      suspend: { verb: investment-manager.suspend, when: filled }
      terminate: { verb: investment-manager.terminate, when: filled }
      find: { verb: investment-manager.find-for-trade, when: filled }
  manco_group:
    type: entity
    entity_kinds: [company]
    join: { via: manco_groups, parent_fk: fund_id, child_fk: manco_id }
    cardinality: optional
    depends_on: [fund]
    verbs:
      create: { verb: manco.create, when: empty }
      read: { verb: manco.read, when: filled }
      list: { verb: manco.list, when: [empty, filled] }
      derive: { verb: manco.derive-groups, when: filled }
      bridge: { verb: manco.bridge-roles, when: filled }
      list_members: { verb: manco.list-members, when: filled }
      list_roles: { verb: manco.list-roles, when: filled }
      assign_role: { verb: manco.assign-role, when: filled }
      remove_role: { verb: manco.remove-role, when: filled }
      link: { verb: manco.link-entity, when: filled }
      unlink: { verb: manco.unlink-entity, when: filled }
      set_regulatory: { verb: manco.set-regulatory-status, when: filled }
      list_funds: { verb: manco.list-managed-funds, when: filled }
      verify: { verb: manco.verify, when: filled }
      compute_control: { verb: manco.compute-control-chain, when: filled }
      refresh: { verb: manco.refresh, when: filled }
  trust:
    type: entity
    entity_kinds: [entity]
    join: { via: trust_structures, parent_fk: fund_id, child_fk: trust_id }
    cardinality: optional
    depends_on: [fund]
    verbs:
      create: { verb: trust.create, when: empty }
      read: { verb: trust.read, when: filled }
      list: { verb: trust.list, when: [empty, filled] }
      add_trustee: { verb: trust.add-trustee, when: filled }
      remove_trustee: { verb: trust.remove-trustee, when: filled }
      add_beneficiary: { verb: trust.add-beneficiary, when: filled }
      add_settlor: { verb: trust.add-settlor, when: filled }
      identify_ubos: { verb: trust.identify-ubos, when: filled }
  partnership:
    type: entity
    entity_kinds: [entity]
    join: { via: partnership_structures, parent_fk: fund_id, child_fk: partnership_id }
    cardinality: optional
    depends_on: [fund]
    verbs:
      create: { verb: partnership.create, when: empty }
      read: { verb: partnership.read, when: filled }
      list: { verb: partnership.list, when: [empty, filled] }
      add_partner: { verb: partnership.add-partner, when: filled }
      remove_partner: { verb: partnership.remove-partner, when: filled }
      set_gp: { verb: partnership.set-general-partner, when: filled }
      list_partners: { verb: partnership.list-partners, when: filled }
```

---
## File: rust/config/sem_os_seeds/constellation_maps/governance_compliance.yaml
```yaml
constellation: governance.compliance
description: Governance and compliance constellation — SLA management, access reviews, regulatory compliance, rulesets, and delegation within a group context.
jurisdiction: ALL
slots:
  group:
    type: cbu
    table: client_group
    pk: group_id
    cardinality: root
    # group identity is owned by group.ownership constellation.
    # governance navigates to it but does not own read/create verbs.
    verbs: {}
  sla:
    type: entity
    entity_kinds: [contract]
    join: { via: deal_slas, parent_fk: deal_id, child_fk: sla_id }
    cardinality: optional
    depends_on: [group]
    verbs:
      create: { verb: sla.create, when: empty }
      read: { verb: sla.read, when: filled }
      read_template: { verb: sla.read-template, when: filled }
      list: { verb: sla.list, when: [empty, filled] }
      list_templates: { verb: sla.list-templates, when: [empty, filled] }
      list_commitments: { verb: sla.list-commitments, when: filled }
      list_measurements: { verb: sla.list-measurements, when: filled }
      list_breaches: { verb: sla.list-breaches, when: filled }
      list_open_breaches: { verb: sla.list-open-breaches, when: filled }
      update: { verb: sla.update, when: filled }
      bind: { verb: sla.bind, when: filled }
      commit: { verb: sla.commit, when: filled }
      record_measurement: { verb: sla.record-measurement, when: filled }
      activate: { verb: sla.activate, when: filled }
      suspend: { verb: sla.suspend, when: filled }
      suspend_commitment: { verb: sla.suspend-commitment, when: filled }
      renew: { verb: sla.renew, when: filled }
      breach: { verb: sla.record-breach, when: filled }
      report_breach: { verb: sla.report-breach, when: filled }
      escalate_breach: { verb: sla.escalate-breach, when: filled }
      resolve_breach: { verb: sla.resolve-breach, when: filled }
      update_remediation: { verb: sla.update-remediation, when: filled }
  access_review:
    type: entity
    entity_kinds: [entity]
    join: { via: access_reviews, parent_fk: group_id, child_fk: review_id }
    cardinality: optional
    depends_on: [group]
    verbs:
      create: { verb: access-review.create, when: empty }
      create_campaign: { verb: access-review.create-campaign, when: [empty, filled] }
      read: { verb: access-review.read, when: filled }
      list: { verb: access-review.list, when: [empty, filled] }
      list_items: { verb: access-review.list-items, when: filled }
      list_flagged: { verb: access-review.list-flagged, when: filled }
      my_pending: { verb: access-review.my-pending, when: filled }
      campaign_status: { verb: access-review.campaign-status, when: filled }
      audit_report: { verb: access-review.audit-report, when: filled }
      populate_campaign: { verb: access-review.populate-campaign, when: filled }
      launch_campaign: { verb: access-review.launch-campaign, when: filled }
      send_reminders: { verb: access-review.send-reminders, when: filled }
      process_deadline: { verb: access-review.process-deadline, when: filled }
      attest: { verb: access-review.attest, when: filled }
      extend_access: { verb: access-review.extend-access, when: filled }
      revoke_access: { verb: access-review.revoke-access, when: filled }
      escalate_item: { verb: access-review.escalate-item, when: filled }
      start: { verb: access-review.start, when: filled }
      complete: { verb: access-review.complete, when: filled }
      approve: { verb: access-review.approve, when: filled }
      reject: { verb: access-review.reject, when: filled }
  regulatory:
    type: entity
    entity_kinds: [entity]
    join: { via: regulatory_filings, parent_fk: group_id, child_fk: filing_id }
    cardinality: optional
    depends_on: [group]
    verbs:
      create: { verb: regulatory.create, when: empty }
      registration_add: { verb: regulatory.registration.add, when: [empty, filled] }
      read: { verb: regulatory.read, when: filled }
      list: { verb: regulatory.list, when: [empty, filled] }
      registration_list: { verb: regulatory.registration.list, when: filled }
      registration_check: { verb: regulatory.registration.check, when: filled }
      registration_verify: { verb: regulatory.registration.verify, when: filled }
      update: { verb: regulatory.update, when: filled }
      submit: { verb: regulatory.submit, when: filled }
      registration_remove: { verb: regulatory.registration.remove, when: filled }
  ruleset:
    type: entity
    entity_kinds: [entity]
    join: { via: rulesets, parent_fk: group_id, child_fk: ruleset_id }
    cardinality: optional
    depends_on: [group]
    verbs:
      create: { verb: ruleset.create, when: empty }
      read: { verb: ruleset.read, when: filled }
      publish: { verb: ruleset.publish, when: filled }
      retire: { verb: ruleset.retire, when: filled }
  delegation:
    type: entity
    entity_kinds: [entity]
    join: { via: delegations, parent_fk: group_id, child_fk: delegation_id }
    cardinality: optional
    depends_on: [group]
    verbs:
      create: { verb: delegation.create, when: empty }
      add: { verb: delegation.add, when: [empty, filled] }
      read: { verb: delegation.read, when: filled }
      list: { verb: delegation.list, when: [empty, filled] }
      list_delegates: { verb: delegation.list-delegates, when: filled }
      list_received: { verb: delegation.list-delegations-received, when: filled }
      end: { verb: delegation.end, when: filled }
      revoke: { verb: delegation.revoke, when: filled }
  team:
    type: entity
    entity_kinds: [person]
    join: { via: team_members, parent_fk: group_id, child_fk: member_id }
    cardinality: optional
    depends_on: [group]
    verbs:
      add: { verb: team.add-member, when: [empty, filled] }
      remove: { verb: team.remove-member, when: filled }
      list: { verb: team.list-members, when: filled }
      list_teams: { verb: team.list, when: [empty, filled] }
      create: { verb: team.create, when: empty }
      read: { verb: team.read, when: filled }
      update: { verb: team.update, when: filled }
      assign_role: { verb: team.assign-role, when: filled }
      remove_role: { verb: team.remove-role, when: filled }
      transfer: { verb: team.transfer-member, when: filled }
      list_by_role: { verb: team.list-by-role, when: filled }
      set_lead: { verb: team.set-lead, when: filled }
      add_governance: { verb: team.add-governance-member, when: filled }
      remove_governance: { verb: team.remove-governance-member, when: filled }
      list_governance: { verb: team.list-governance-members, when: filled }
      add_ops: { verb: team.add-ops-member, when: filled }
      remove_ops: { verb: team.remove-ops-member, when: filled }
      list_ops: { verb: team.list-ops-members, when: filled }
      assign_capacity: { verb: team.assign-capacity, when: filled }
      list_capacity: { verb: team.list-capacity, when: filled }
  rule:
    type: entity
    entity_kinds: [entity]
    join: { via: rules, parent_fk: ruleset_id, child_fk: rule_id }
    cardinality: optional
    depends_on: [ruleset]
    verbs:
      create: { verb: rule.create, when: empty }
      read: { verb: rule.read, when: filled }
      update: { verb: rule.update, when: filled }
  rule_field:
    type: entity
    entity_kinds: [entity]
    join: { via: rule_fields, parent_fk: field_id, child_fk: field_id }
    cardinality: optional
    depends_on: [ruleset]
    verbs:
      list: { verb: rule-field.list, when: [empty, filled] }
      read: { verb: rule-field.read, when: filled }
```

---
## File: rust/config/sem_os_seeds/constellation_maps/group_ownership.yaml
```yaml
constellation: group.ownership
description: Client group ownership and control constellation — the root context for all onboarding. Maps group identity, GLEIF hierarchy, UBO discovery, control chain, and CBU identification.
jurisdiction: ALL
slots:
  client_group:
    type: cbu
    table: client_group
    pk: group_id
    cardinality: root
    state_machine: client_group_lifecycle
    verbs:
      create: { verb: client-group.create, when: empty }
      read: { verb: client-group.read, when: filled }
      research: { verb: client-group.research, when: [empty, filled] }
      update: { verb: client-group.update, when: filled }
      set_canonical: { verb: client-group.set-canonical, when: filled }
      start_discovery: { verb: client-group.start-discovery, when: [empty, filled] }
      discover_entities: { verb: client-group.discover-entities, when: [empty, filled] }
      complete_discovery: { verb: client-group.complete-discovery, when: filled }
      entity_add: { verb: client-group.entity-add, when: [empty, filled] }
      entity_remove: { verb: client-group.entity-remove, when: filled }
      list_entities: { verb: client-group.list-entities, when: filled }
      search_entities: { verb: client-group.search-entities, when: [empty, filled] }
      list_parties: { verb: client-group.list-parties, when: filled }
      list_unverified: { verb: client-group.list-unverified, when: filled }
      list_discrepancies: { verb: client-group.list-discrepancies, when: filled }
      verify_ownership: { verb: client-group.verify-ownership, when: filled }
      reject_entity: { verb: client-group.reject-entity, when: filled }
      assign_role: { verb: client-group.assign-role, when: filled }
      remove_role: { verb: client-group.remove-role, when: filled }
      list_roles: { verb: client-group.list-roles, when: filled }
      add_relationship: { verb: client-group.add-relationship, when: filled }
      list_relationships: { verb: client-group.list-relationships, when: filled }
      tag_add: { verb: client-group.tag-add, when: filled }
      tag_remove: { verb: client-group.tag-remove, when: filled }
  gleif_import:
    type: entity
    entity_kinds: [company]
    join: { via: client_group_entity, parent_fk: group_id, child_fk: entity_id }
    cardinality: optional
    depends_on: [client_group]
    verbs:
      import: { verb: gleif.import-tree, when: empty }
      import_to_group: { verb: gleif.import-to-client-group, when: [empty, filled] }
      import_managed_funds: { verb: gleif.import-managed-funds, when: [empty, filled] }
      search: { verb: gleif.search, when: [empty, filled] }
      refresh: { verb: gleif.refresh, when: filled }
      enrich: { verb: gleif.enrich, when: filled }
      get_record: { verb: gleif.get-record, when: filled }
      get_parent: { verb: gleif.get-parent, when: filled }
      get_children: { verb: gleif.get-children, when: filled }
      get_manager: { verb: gleif.get-manager, when: filled }
      get_managed_funds: { verb: gleif.get-managed-funds, when: filled }
      get_master_fund: { verb: gleif.get-master-fund, when: filled }
      get_umbrella: { verb: gleif.get-umbrella, when: filled }
      lookup_by_isin: { verb: gleif.lookup-by-isin, when: [empty, filled] }
      resolve_successor: { verb: gleif.resolve-successor, when: filled }
      trace_ownership: { verb: gleif.trace-ownership, when: filled }
  ubo_discovery:
    type: entity_graph
    entity_kinds: [person, company]
    join: { via: ubo_registry, parent_fk: subject_entity_id, child_fk: ubo_entity_id }
    cardinality: recursive
    max_depth: 5
    depends_on: [gleif_import]
    state_machine: ubo_epistemic_lifecycle
    overlays: [registry, evidence, screenings]
    edge_overlays: [ownership]
    verbs:
      discover: { verb: ubo.discover, when: empty }
      allege: { verb: ubo.allege, when: [empty, filled] }
      calculate: { verb: ubo.calculate, when: [empty, filled] }
      compute_chains: { verb: ubo.compute-chains, when: [empty, filled] }
      trace_chains: { verb: ubo.trace-chains, when: filled }
      verify: { verb: ubo.verify, when: filled }
      promote: { verb: ubo.promote, when: filled }
      approve: { verb: ubo.approve, when: filled }
      reject: { verb: ubo.reject, when: filled }
      list: { verb: ubo.list, when: filled }
      list_ubos: { verb: ubo.list-ubos, when: filled }
      list_owned: { verb: ubo.list-owned, when: filled }
      list_owners: { verb: ubo.list-owners, when: filled }
      add_ownership: { verb: ubo.add-ownership, when: filled }
      update_ownership: { verb: ubo.update-ownership, when: filled }
      add_control: { verb: ubo.add-control, when: filled }
      transfer_control: { verb: ubo.transfer-control, when: filled }
      add_trust_role: { verb: ubo.add-trust-role, when: filled }
      delete_relationship: { verb: ubo.delete-relationship, when: filled }
      end_relationship: { verb: ubo.end-relationship, when: filled }
      waive_verification: { verb: ubo.waive-verification, when: filled }
      mark_deceased: { verb: ubo.mark-deceased, when: filled }
      mark_terminus: { verb: ubo.mark-terminus, when: filled }
      convergence_supersede: { verb: ubo.convergence-supersede, when: filled }
      registry_create: { verb: ubo.registry.create, when: [empty, filled] }
      registry_advance: { verb: ubo.registry.advance, when: filled }
      registry_promote: { verb: ubo.registry.promote, when: filled }
      registry_reject: { verb: ubo.registry.reject, when: filled }
      registry_expire: { verb: ubo.registry.expire, when: filled }
      registry_waive: { verb: ubo.registry.waive, when: filled }
      snapshot_capture: { verb: ubo.snapshot.capture, when: filled }
      snapshot_diff: { verb: ubo.snapshot.diff, when: filled }
  control_chain:
    type: entity_graph
    entity_kinds: [company]
    join: { via: ownership_snapshots, parent_fk: root_entity_id, child_fk: entity_id }
    cardinality: recursive
    max_depth: 10
    depends_on: [ubo_discovery]
    overlays: [ownership]
    edge_overlays: [control]
    verbs:
      trace: { verb: ownership.trace-chain, when: empty }
      build: { verb: control.build-graph, when: [empty, filled] }
      refresh: { verb: ownership.refresh, when: filled }
      read: { verb: control.read, when: filled }
      list_links: { verb: control.list-links, when: filled }
      add: { verb: control.add, when: [empty, filled] }
      end: { verb: control.end, when: filled }
      analyze: { verb: control.analyze, when: filled }
      list_controllers: { verb: control.list-controllers, when: filled }
      list_controlled: { verb: control.list-controlled, when: filled }
      trace_chain: { verb: control.trace-chain, when: filled }
      compute_controllers: { verb: control.compute-controllers, when: filled }
      identify_ubos: { verb: control.identify-ubos, when: filled }
      reconcile_ownership: { verb: control.reconcile-ownership, when: filled }
      set_board_controller: { verb: control.set-board-controller, when: filled }
      show_board_controller: { verb: control.show-board-controller, when: filled }
      recompute_board_controller: { verb: control.recompute-board-controller, when: filled }
      clear_board_controller_override: { verb: control.clear-board-controller-override, when: filled }
      import_gleif_control: { verb: control.import-gleif-control, when: [empty, filled] }
      import_psc_register: { verb: control.import-psc-register, when: [empty, filled] }
      ownership_compute: { verb: ownership.compute, when: filled }
      ownership_control_positions: { verb: ownership.control-positions, when: filled }
      ownership_who_controls: { verb: ownership.who-controls, when: filled }
      ownership_analyze_gaps: { verb: ownership.analyze-gaps, when: filled }
      ownership_reconcile: { verb: ownership.reconcile, when: filled }
      ownership_reconcile_findings: { verb: ownership.reconcile.findings, when: filled }
      ownership_reconcile_list_runs: { verb: ownership.reconcile.list-runs, when: filled }
      ownership_reconcile_resolve: { verb: ownership.reconcile.resolve-finding, when: filled }
      ownership_right_add_class: { verb: ownership.right.add-to-class, when: filled }
      ownership_right_add_holder: { verb: ownership.right.add-to-holder, when: filled }
      ownership_right_end: { verb: ownership.right.end, when: filled }
      ownership_right_list_holder: { verb: ownership.right.list-for-holder, when: filled }
      ownership_right_list_issuer: { verb: ownership.right.list-for-issuer, when: filled }
      ownership_snapshot_get: { verb: ownership.snapshot.get, when: filled }
      ownership_snapshot_list: { verb: ownership.snapshot.list, when: filled }
  cbu_identification:
    type: cbu
    table: cbus
    pk: cbu_id
    join: { via: cbu_entity_roles, parent_fk: entity_id, child_fk: cbu_id }
    cardinality: optional
    depends_on: [control_chain]
    verbs:
      create: { verb: cbu.create, when: empty }
      create_from_group: { verb: cbu.create-from-client-group, when: empty }
      ensure: { verb: cbu.ensure, when: [empty, filled] }
      read: { verb: cbu.read, when: filled }
      list: { verb: cbu.list, when: filled }
      list_subscriptions: { verb: cbu.list-subscriptions, when: filled }
      list_evidence: { verb: cbu.list-evidence, when: filled }
      list_structure_links: { verb: cbu.list-structure-links, when: filled }
      parties: { verb: cbu.parties, when: filled }
      update: { verb: cbu.update, when: filled }
      rename: { verb: cbu.rename, when: filled }
      set_jurisdiction: { verb: cbu.set-jurisdiction, when: filled }
      set_client_type: { verb: cbu.set-client-type, when: filled }
      set_commercial_client: { verb: cbu.set-commercial-client, when: filled }
      add_product: { verb: cbu.add-product, when: filled }
      remove_product: { verb: cbu.remove-product, when: filled }
      assign_control: { verb: cbu.assign-control, when: filled }
      assign_ownership: { verb: cbu.assign-ownership, when: filled }
      assign_fund_role: { verb: cbu.assign-fund-role, when: filled }
      assign_trust_role: { verb: cbu.assign-trust-role, when: filled }
      assign_service_provider: { verb: cbu.assign-service-provider, when: filled }
      assign_signatory: { verb: cbu.assign-signatory, when: filled }
      remove_role: { verb: cbu.remove-role, when: filled }
      validate_roles: { verb: cbu.validate-roles, when: filled }
      attach_evidence: { verb: cbu.attach-evidence, when: filled }
      verify_evidence: { verb: cbu.verify-evidence, when: filled }
      request_proof_update: { verb: cbu.request-proof-update, when: filled }
      link_structure: { verb: cbu.link-structure, when: filled }
      unlink_structure: { verb: cbu.unlink-structure, when: filled }
      submit_for_validation: { verb: cbu.submit-for-validation, when: filled }
      reopen_validation: { verb: cbu.reopen-validation, when: filled }
      decide: { verb: cbu.decide, when: filled }
      delete: { verb: cbu.delete, when: filled }
      delete_cascade: { verb: cbu.delete-cascade, when: filled }
      # cbu.assign-role is owned by struct_* constellations (role assignment
      # within a specific fund structure). Group ownership identifies CBUs,
      # not their role composition.
```

---
## File: rust/config/sem_os_seeds/constellation_maps/kyc_extended.yaml
```yaml
constellation: kyc.extended
description: Extended KYC investigation topology — board composition and BODS beneficial ownership data for deep compliance analysis. Entities positioned in the corporate governance structure of a subject entity.
jurisdiction: ALL
slots:
  entity:
    type: entity
    entity_kinds: [person, company]
    table: entities
    pk: entity_id
    cardinality: root
    verbs:
      read: { verb: entity.read, when: filled }
  board:
    type: entity
    entity_kinds: [person]
    join: { via: board_appointments, parent_fk: entity_id, child_fk: person_id }
    cardinality: optional
    depends_on: [entity]
    verbs:
      appoint: { verb: board.appoint, when: [empty, filled] }
      resign: { verb: board.resign, when: filled }
      list_by_entity: { verb: board.list-by-entity, when: filled }
      list_by_person: { verb: board.list-by-person, when: filled }
      grant_right: { verb: board.grant-appointment-right, when: filled }
      revoke_right: { verb: board.revoke-appointment-right, when: filled }
      list_rights: { verb: board.list-appointment-rights, when: filled }
      list_held: { verb: board.list-rights-held, when: filled }
      analyze: { verb: board.analyze-control, when: filled }
  bods:
    type: entity
    entity_kinds: [person, company]
    join: { via: bods_statements, parent_fk: entity_id, child_fk: statement_id }
    cardinality: optional
    depends_on: [entity]
    verbs:
      discover: { verb: bods.discover-ubos, when: [empty, filled] }
      import: { verb: bods.import, when: [empty, filled] }
      link: { verb: bods.link-entity, when: filled }
      get_statement: { verb: bods.get-statement, when: filled }
      list_by_entity: { verb: bods.list-by-entity, when: filled }
      find_by_lei: { verb: bods.find-by-lei, when: [empty, filled] }
      list_persons: { verb: bods.list-persons, when: filled }
      list_ownership: { verb: bods.list-ownership, when: filled }
      sync: { verb: bods.sync-from-gleif, when: filled }
  # red_flag verbs are annotations/events ON entities, not entities
  # positioned in a structural topology. They're available via the
  # entity's verb surface but don't create constellation slots.
```

---
## File: rust/config/sem_os_seeds/constellation_maps/kyc_onboarding.yaml
```yaml
constellation: kyc.onboarding
description: KYC onboarding lifecycle constellation — per-CBU case management, entity workstreams, screening, and tollgate approval. Evidence/documents are a separate constellation (evidence.collection) referenced by dependency.
jurisdiction: ALL
slots:
  cbu:
    type: cbu
    table: cbus
    pk: cbu_id
    cardinality: root
    # cbu.read/show are navigation — owned by group.ownership constellation.
    # KYC reads the CBU but does not create/modify it.
    verbs:
      show: { verb: cbu.show, when: filled }
  kyc_case:
    type: case
    table: cases
    pk: case_id
    join: { via: cases, parent_fk: cbu_id, child_fk: case_id }
    cardinality: mandatory
    depends_on: [cbu]
    state_machine: kyc_case_lifecycle
    verbs:
      create: { verb: kyc-case.create, when: empty }
      open: { verb: kyc.open-case, when: empty }
      read: { verb: kyc-case.read, when: filled }
      list_by_cbu: { verb: kyc-case.list-by-cbu, when: filled }
      state: { verb: kyc-case.state, when: filled }
      assign: { verb: kyc-case.assign, when: filled }
      update_status: { verb: kyc-case.update-status, when: filled }
      set_risk: { verb: kyc-case.set-risk-rating, when: filled }
      close: { verb: kyc-case.close, when: filled }
      reopen: { verb: kyc-case.reopen, when: filled }
      escalate: { verb: kyc-case.escalate, when: filled }
    children:
      tollgate:
        type: tollgate
        table: tollgate_evaluations
        pk: evaluation_id
        join: { via: tollgate_evaluations, parent_fk: case_id, child_fk: evaluation_id }
        cardinality: optional
        depends_on: [{ slot: kyc_case, min_state: review }]
        verbs:
          evaluate: { verb: tollgate.evaluate, when: empty }
          evaluate_gate: { verb: tollgate.evaluate-gate, when: [empty, filled] }
          read: { verb: tollgate.read, when: filled }
          get_decision_readiness: { verb: tollgate.get-decision-readiness, when: filled }
          get_metrics: { verb: tollgate.get-metrics, when: filled }
          list_evaluations: { verb: tollgate.list-evaluations, when: filled }
          list_thresholds: { verb: tollgate.list-thresholds, when: filled }
          set_threshold: { verb: tollgate.set-threshold, when: filled }
          override: { verb: tollgate.override, when: filled }
          list_overrides: { verb: tollgate.list-overrides, when: filled }
          expire_override: { verb: tollgate.expire-override, when: filled }
  entity_workstream:
    type: entity
    entity_kinds: [person, company]
    join: { via: entity_workstreams, parent_fk: case_id, child_fk: entity_id }
    cardinality: optional
    depends_on: [kyc_case]
    overlays: [red_flags, evidence]
    verbs:
      create: { verb: entity-workstream.create, when: empty }
      read: { verb: entity-workstream.read, when: filled }
      list_by_case: { verb: entity-workstream.list-by-case, when: filled }
      state: { verb: entity-workstream.state, when: filled }
      update: { verb: entity-workstream.update-status, when: filled }
      set_enhanced_dd: { verb: entity-workstream.set-enhanced-dd, when: filled }
      set_ubo: { verb: entity-workstream.set-ubo, when: filled }
      complete: { verb: entity-workstream.complete, when: filled }
      block: { verb: entity-workstream.block, when: filled }
      # Red flag state verbs — flags are a state of the entity, not a separate entity
      flag_raise: { verb: red-flag.raise, when: [empty, filled] }
      flag_read: { verb: red-flag.read, when: filled }
      flag_list: { verb: red-flag.list, when: filled }
      flag_resolve: { verb: red-flag.resolve, when: filled }
      flag_escalate: { verb: red-flag.escalate, when: filled }
      flag_update: { verb: red-flag.update, when: filled }
      flag_list_severity: { verb: red-flag.list-by-severity, when: filled }
      flag_close: { verb: red-flag.close, when: filled }
      # Evidence/document state verbs — documents are evidence FOR the entity,
      # not positioned entities. The requirement dictionary defines what's needed,
      # the state machine tracks each requirement instance.
      req_create: { verb: requirement.create, when: [empty, filled] }
      req_create_set: { verb: requirement.create-set, when: [empty, filled] }
      req_check: { verb: requirement.check, when: filled }
      req_list: { verb: requirement.list, when: filled }
      req_for_entity: { verb: requirement.for-entity, when: filled }
      req_unsatisfied: { verb: requirement.unsatisfied, when: filled }
      req_waive: { verb: requirement.waive, when: filled }
      req_reinstate: { verb: requirement.reinstate, when: filled }
      doc_solicit: { verb: document.solicit, when: [empty, filled] }
      doc_solicit_set: { verb: document.solicit-set, when: [empty, filled] }
      doc_upload: { verb: document.upload, when: filled }
      doc_verify: { verb: document.verify, when: filled }
      doc_reject: { verb: document.reject, when: filled }
      doc_read: { verb: document.read, when: filled }
      doc_list: { verb: document.list, when: filled }
      doc_compute_reqs: { verb: document.compute-requirements, when: filled }
      doc_missing: { verb: document.missing-for-entity, when: filled }
  screening:
    type: entity
    entity_kinds: [person, company]
    join: { via: screenings, parent_fk: workstream_id, child_fk: screening_id, filter_column: screening_type }
    cardinality: optional
    depends_on: [entity_workstream]
    state_machine: screening_lifecycle
    overlays: [screening_result]
    verbs:
      run: { verb: screening.run, when: empty }
      sanctions: { verb: screening.sanctions, when: [empty, filled] }
      pep: { verb: screening.pep, when: [empty, filled] }
      adverse_media: { verb: screening.adverse-media, when: [empty, filled] }
      bulk_refresh: { verb: screening.bulk-refresh, when: filled }
      read: { verb: screening.read, when: filled }
      list: { verb: screening.list, when: filled }
      list_by_workstream: { verb: screening.list-by-workstream, when: filled }
      review_hit: { verb: screening.review-hit, when: filled }
      update: { verb: screening.update-status, when: filled }
      escalate: { verb: screening.escalate, when: filled }
      resolve: { verb: screening.resolve, when: filled }
      complete: { verb: screening.complete, when: filled }
  # Evidence/documents are NOT embedded here — they are a separate concern
  # owned by evidence.collection constellation. KYC depends on evidence
  # being complete (tollgate checks coverage), but does not own the verbs.
  kyc_agreement:
    type: entity
    entity_kinds: [company]
    join: { via: kyc_agreements, parent_fk: case_id, child_fk: agreement_id }
    cardinality: optional
    depends_on: [kyc_case]
    verbs:
      create: { verb: kyc-agreement.create, when: empty }
      read: { verb: kyc-agreement.read, when: filled }
      list: { verb: kyc-agreement.list, when: filled }
      update: { verb: kyc-agreement.update, when: filled }
      update_status: { verb: kyc-agreement.update-status, when: filled }
      sign: { verb: kyc-agreement.sign, when: filled }
  identifier:
    type: entity
    entity_kinds: [entity]
    join: { via: entity_identifiers, parent_fk: entity_id, child_fk: identifier_id }
    cardinality: optional
    depends_on: [entity_workstream]
    verbs:
      add: { verb: identifier.add, when: [empty, filled] }
      read: { verb: identifier.read, when: filled }
      list: { verb: identifier.list, when: filled }
      verify: { verb: identifier.verify, when: filled }
      expire: { verb: identifier.expire, when: filled }
      update: { verb: identifier.update, when: filled }
      search: { verb: identifier.search, when: [empty, filled] }
      resolve: { verb: identifier.resolve, when: filled }
      list_by_type: { verb: identifier.list-by-type, when: filled }
      set_primary: { verb: identifier.set-primary, when: filled }
      remove: { verb: identifier.remove, when: filled }
  request:
    type: entity
    entity_kinds: [entity]
    join: { via: kyc_requests, parent_fk: case_id, child_fk: request_id }
    cardinality: optional
    depends_on: [kyc_case]
    verbs:
      create: { verb: request.create, when: empty }
      read: { verb: request.read, when: filled }
      list: { verb: request.list, when: [empty, filled] }
      update: { verb: request.update, when: filled }
      complete: { verb: request.complete, when: filled }
      cancel: { verb: request.cancel, when: filled }
      assign: { verb: request.assign, when: filled }
      reopen: { verb: request.reopen, when: filled }
      escalate: { verb: request.escalate, when: filled }
  # red_flag is a STATE of an entity workstream (like "screened" or
  # "document requested"), not a separate positioned entity.
  # Red flag verbs are on the entity_workstream slot above.
  # Evidence and case-event are similarly state/annotation, not entities.
```

---
## File: rust/config/sem_os_seeds/constellation_maps/struct_hedge_cross_border.yaml
```yaml
constellation: struct.hedge.cross-border
description: Cross-border hedge master-feeder onboarding constellation
jurisdiction: XB
slots:
  cbu:
    type: cbu
    table: cbus
    pk: cbu_id
    cardinality: root
    verbs: { create: cbu.create, read: cbu.read, show: cbu.show }
    children:
      us_feeder:
        type: cbu
        join:
          {
            via: cbu_structure_links,
            parent_fk: parent_cbu_id,
            child_fk: child_cbu_id,
            filter_column: relationship_selector,
            filter_value: feeder:us,
          }
        cardinality: optional
        depends_on: [cbu]
        verbs: { show: cbu.read }
      ie_feeder:
        type: cbu
        join:
          {
            via: cbu_structure_links,
            parent_fk: parent_cbu_id,
            child_fk: child_cbu_id,
            filter_column: relationship_selector,
            filter_value: feeder:ie,
          }
        cardinality: optional
        depends_on: [cbu]
        verbs: { show: cbu.read }
  aifm:
    type: entity
    entity_kinds: [company]
    join:
      {
        via: cbu_entity_roles,
        parent_fk: cbu_id,
        child_fk: entity_id,
        filter_column: role,
        filter_value: aifm,
      }
    cardinality: mandatory
    depends_on: [cbu]
    placeholder: AIFM TBD
    state_machine: entity_kyc_lifecycle
    overlays:
      [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs:
      {
        ensure: { verb: entity.ensure-or-placeholder, when: empty },
        assign: { verb: cbu.assign-role, when: placeholder },
        search: { verb: party.search, when: [placeholder, filled] },
        add: { verb: party.add, when: empty },
        show: { verb: entity.read, when: filled },
      }
  depositary:
    type: entity
    entity_kinds: [company]
    join:
      {
        via: cbu_entity_roles,
        parent_fk: cbu_id,
        child_fk: entity_id,
        filter_column: role,
        filter_value: depositary,
      }
    cardinality: mandatory
    depends_on: [cbu]
    placeholder: Depositary TBD
    state_machine: entity_kyc_lifecycle
    overlays:
      [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs:
      {
        ensure: { verb: entity.ensure-or-placeholder, when: empty },
        assign: { verb: cbu.assign-role, when: placeholder },
        search: { verb: party.search, when: [placeholder, filled] },
        add: { verb: party.add, when: empty },
        show: { verb: entity.read, when: filled },
      }
  prime_broker:
    type: entity
    entity_kinds: [company]
    join:
      {
        via: cbu_entity_roles,
        parent_fk: cbu_id,
        child_fk: entity_id,
        filter_column: role,
        filter_value: prime-broker,
      }
    occurrence: 1
    cardinality: mandatory
    depends_on: [cbu]
    placeholder: Prime Broker TBD
    state_machine: entity_kyc_lifecycle
    overlays:
      [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs:
      {
        ensure: { verb: entity.ensure-or-placeholder, when: empty },
        assign: { verb: cbu.assign-role, when: placeholder },
        search: { verb: party.search, when: [placeholder, filled] },
        add: { verb: party.add, when: empty },
        show: { verb: entity.read, when: filled },
      }
  investment_manager:
    type: entity
    entity_kinds: [company]
    join:
      {
        via: cbu_entity_roles,
        parent_fk: cbu_id,
        child_fk: entity_id,
        filter_column: role,
        filter_value: investment-manager,
      }
    cardinality: optional
    depends_on: [cbu]
    placeholder: Investment Manager TBD
    state_machine: entity_kyc_lifecycle
    overlays:
      [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs:
      {
        ensure: { verb: entity.ensure-or-placeholder, when: empty },
        assign: { verb: cbu.assign-role, when: placeholder },
        search: { verb: party.search, when: [placeholder, filled] },
        add: { verb: party.add, when: empty },
        show: { verb: entity.read, when: filled },
      }
  administrator:
    type: entity
    entity_kinds: [company]
    join:
      {
        via: cbu_entity_roles,
        parent_fk: cbu_id,
        child_fk: entity_id,
        filter_column: role,
        filter_value: administrator,
      }
    cardinality: optional
    depends_on: [cbu]
    placeholder: Administrator TBD
    state_machine: entity_kyc_lifecycle
    overlays:
      [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs:
      {
        ensure: { verb: entity.ensure-or-placeholder, when: empty },
        assign: { verb: cbu.assign-role, when: placeholder },
        search: { verb: party.search, when: [placeholder, filled] },
        add: { verb: party.add, when: empty },
        show: { verb: entity.read, when: filled },
      }
  auditor:
    type: entity
    entity_kinds: [company]
    join:
      {
        via: cbu_entity_roles,
        parent_fk: cbu_id,
        child_fk: entity_id,
        filter_column: role,
        filter_value: auditor,
      }
    cardinality: optional
    depends_on: [cbu]
    placeholder: Auditor TBD
    state_machine: entity_kyc_lifecycle
    overlays:
      [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs:
      {
        ensure: { verb: entity.ensure-or-placeholder, when: empty },
        assign: { verb: cbu.assign-role, when: placeholder },
        search: { verb: party.search, when: [placeholder, filled] },
        add: { verb: party.add, when: empty },
        show: { verb: entity.read, when: filled },
      }
  secondary_prime_broker:
    type: entity
    entity_kinds: [company]
    join:
      {
        via: cbu_entity_roles,
        parent_fk: cbu_id,
        child_fk: entity_id,
        filter_column: role,
        filter_value: prime-broker,
      }
    occurrence: 2
    cardinality: optional
    depends_on: [cbu]
    placeholder: Secondary Prime Broker TBD
    state_machine: entity_kyc_lifecycle
    overlays:
      [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs:
      {
        ensure: { verb: entity.ensure-or-placeholder, when: empty },
        assign: { verb: cbu.assign-role, when: placeholder },
        search: { verb: party.search, when: [placeholder, filled] },
        add: { verb: party.add, when: empty },
        show: { verb: entity.read, when: filled },
      }
  ownership_chain:
    type: entity_graph
    entity_kinds: [person, company]
    join:
      {
        via: entity_relationships,
        parent_fk: from_entity_id,
        child_fk: to_entity_id,
      }
    cardinality: recursive
    max_depth: 5
    depends_on: [aifm]
    state_machine: ubo_epistemic_lifecycle
    overlays: [registry, evidence, screenings]
    edge_overlays: [ownership]
    verbs:
      {
        discover: ubo.discover,
        allege: ubo.allege,
        verify: ubo.verify,
        promote: ubo.promote,
        approve: ubo.approve,
        reject: ubo.reject,
      }
  case:
    type: case
    table: cases
    pk: case_id
    join: { via: cases, parent_fk: cbu_id, child_fk: case_id }
    cardinality: optional
    depends_on: [aifm]
    state_machine: kyc_case_lifecycle
    verbs:
      {
        open: case.open,
        submit: case.submit,
        approve: case.approve,
        reject: case.reject,
        request_info: case.request-info,
      }
    children:
      tollgate:
        type: tollgate
        table: tollgate_evaluations
        pk: evaluation_id
        join:
          {
            via: tollgate_evaluations,
            parent_fk: case_id,
            child_fk: evaluation_id,
          }
        cardinality: optional
        depends_on: [{ slot: case, min_state: intake }]
        verbs: { evaluate: tollgate.evaluate }
  mandate:
    type: mandate
    table: cbu_trading_profiles
    pk: profile_id
    join: { via: cbu_trading_profiles, parent_fk: cbu_id, child_fk: profile_id }
    cardinality: optional
    depends_on:
      [{ slot: cbu, min_state: filled }, { slot: case, min_state: intake }]
    verbs: { create: mandate.create }
bulk_macros: [role_slots]
```

---
## File: rust/config/sem_os_seeds/constellation_maps/struct_ie_aif_icav.yaml
```yaml
constellation: struct.ie.aif.icav
description: Ireland AIF ICAV onboarding constellation
jurisdiction: IE
slots:
  cbu:
    type: cbu
    table: cbus
    pk: cbu_id
    cardinality: root
    verbs:
      create: cbu.create
      read: cbu.read
      show: cbu.show
  aifm:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: aifm }
    cardinality: mandatory
    depends_on: [cbu]
    placeholder: AIFM TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs:
      ensure: { verb: entity.ensure-or-placeholder, when: empty }
      assign: { verb: cbu.assign-role, when: placeholder }
      search: { verb: party.search, when: [placeholder, filled] }
      add: { verb: party.add, when: empty }
      show: { verb: entity.read, when: filled }
  depositary:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: depositary }
    cardinality: mandatory
    depends_on: [cbu]
    placeholder: Depositary TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs:
      ensure: { verb: entity.ensure-or-placeholder, when: empty }
      assign: { verb: cbu.assign-role, when: placeholder }
      search: { verb: party.search, when: [placeholder, filled] }
      add: { verb: party.add, when: empty }
      show: { verb: entity.read, when: filled }
  investment_manager:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: investment-manager }
    cardinality: optional
    depends_on: [cbu]
    placeholder: Investment Manager TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs:
      ensure: { verb: entity.ensure-or-placeholder, when: empty }
      assign: { verb: cbu.assign-role, when: placeholder }
      search: { verb: party.search, when: [placeholder, filled] }
      add: { verb: party.add, when: empty }
      show: { verb: entity.read, when: filled }
  administrator:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: administrator }
    cardinality: optional
    depends_on: [cbu]
    placeholder: Administrator TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs:
      ensure: { verb: entity.ensure-or-placeholder, when: empty }
      assign: { verb: cbu.assign-role, when: placeholder }
      search: { verb: party.search, when: [placeholder, filled] }
      add: { verb: party.add, when: empty }
      show: { verb: entity.read, when: filled }
  auditor:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: auditor }
    cardinality: optional
    depends_on: [cbu]
    placeholder: Auditor TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs:
      ensure: { verb: entity.ensure-or-placeholder, when: empty }
      assign: { verb: cbu.assign-role, when: placeholder }
      search: { verb: party.search, when: [placeholder, filled] }
      add: { verb: party.add, when: empty }
      show: { verb: entity.read, when: filled }
  prime_broker:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: prime-broker }
    cardinality: optional
    depends_on: [cbu]
    placeholder: Prime Broker TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs:
      ensure: { verb: entity.ensure-or-placeholder, when: empty }
      assign: { verb: cbu.assign-role, when: placeholder }
      search: { verb: party.search, when: [placeholder, filled] }
      add: { verb: party.add, when: empty }
      show: { verb: entity.read, when: filled }
  company_secretary:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: company-secretary }
    cardinality: optional
    depends_on: [cbu]
    placeholder: Company Secretary TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs:
      ensure: { verb: entity.ensure-or-placeholder, when: empty }
      assign: { verb: cbu.assign-role, when: placeholder }
      search: { verb: party.search, when: [placeholder, filled] }
      add: { verb: party.add, when: empty }
      show: { verb: entity.read, when: filled }
  ownership_chain:
    type: entity_graph
    entity_kinds: [person, company]
    join: { via: entity_relationships, parent_fk: from_entity_id, child_fk: to_entity_id }
    cardinality: recursive
    max_depth: 5
    depends_on: [aifm]
    state_machine: ubo_epistemic_lifecycle
    overlays: [registry, evidence, screenings]
    edge_overlays: [ownership]
    verbs:
      discover: ubo.discover
      allege: ubo.allege
      verify: ubo.verify
      promote: ubo.promote
      approve: ubo.approve
      reject: ubo.reject
  case:
    type: case
    table: cases
    pk: case_id
    join: { via: cases, parent_fk: cbu_id, child_fk: case_id }
    cardinality: optional
    depends_on: [aifm]
    state_machine: kyc_case_lifecycle
    verbs:
      open: case.open
      submit: case.submit
      approve: case.approve
      reject: case.reject
      request_info: case.request-info
    children:
      tollgate:
        type: tollgate
        table: tollgate_evaluations
        pk: evaluation_id
        join: { via: tollgate_evaluations, parent_fk: case_id, child_fk: evaluation_id }
        cardinality: optional
        depends_on:
          - slot: case
            min_state: intake
        verbs:
          evaluate: tollgate.evaluate
  mandate:
    type: mandate
    table: cbu_trading_profiles
    pk: profile_id
    join: { via: cbu_trading_profiles, parent_fk: cbu_id, child_fk: profile_id }
    cardinality: optional
    depends_on:
      - slot: cbu
        min_state: filled
      - slot: case
        min_state: intake
    verbs:
      create: mandate.create
bulk_macros: [role_slots]
```

---
## File: rust/config/sem_os_seeds/constellation_maps/struct_ie_hedge_icav.yaml
```yaml
constellation: struct.ie.hedge.icav
description: Ireland hedge ICAV onboarding constellation
jurisdiction: IE
slots:
  cbu:
    type: cbu
    table: cbus
    pk: cbu_id
    cardinality: root
    verbs:
      create: cbu.create
      read: cbu.read
      show: cbu.show
  aifm:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: aifm }
    cardinality: mandatory
    depends_on: [cbu]
    placeholder: AIFM TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs:
      ensure: { verb: entity.ensure-or-placeholder, when: empty }
      assign: { verb: cbu.assign-role, when: placeholder }
      search: { verb: party.search, when: [placeholder, filled] }
      add: { verb: party.add, when: empty }
      show: { verb: entity.read, when: filled }
  depositary:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: depositary }
    cardinality: mandatory
    depends_on: [cbu]
    placeholder: Depositary TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs:
      ensure: { verb: entity.ensure-or-placeholder, when: empty }
      assign: { verb: cbu.assign-role, when: placeholder }
      search: { verb: party.search, when: [placeholder, filled] }
      add: { verb: party.add, when: empty }
      show: { verb: entity.read, when: filled }
  investment_manager:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: investment-manager }
    cardinality: optional
    depends_on: [cbu]
    placeholder: Investment Manager TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs:
      ensure: { verb: entity.ensure-or-placeholder, when: empty }
      assign: { verb: cbu.assign-role, when: placeholder }
      search: { verb: party.search, when: [placeholder, filled] }
      add: { verb: party.add, when: empty }
      show: { verb: entity.read, when: filled }
  administrator:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: administrator }
    cardinality: optional
    depends_on: [cbu]
    placeholder: Administrator TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs:
      ensure: { verb: entity.ensure-or-placeholder, when: empty }
      assign: { verb: cbu.assign-role, when: placeholder }
      search: { verb: party.search, when: [placeholder, filled] }
      add: { verb: party.add, when: empty }
      show: { verb: entity.read, when: filled }
  auditor:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: auditor }
    cardinality: optional
    depends_on: [cbu]
    placeholder: Auditor TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs:
      ensure: { verb: entity.ensure-or-placeholder, when: empty }
      assign: { verb: cbu.assign-role, when: placeholder }
      search: { verb: party.search, when: [placeholder, filled] }
      add: { verb: party.add, when: empty }
      show: { verb: entity.read, when: filled }
  prime_broker:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: prime-broker }
    occurrence: 1
    cardinality: optional
    depends_on: [cbu]
    placeholder: Prime Broker TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs:
      ensure: { verb: entity.ensure-or-placeholder, when: empty }
      assign: { verb: cbu.assign-role, when: placeholder }
      search: { verb: party.search, when: [placeholder, filled] }
      add: { verb: party.add, when: empty }
      show: { verb: entity.read, when: filled }
  secondary_prime_broker:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: prime-broker }
    occurrence: 2
    cardinality: optional
    depends_on: [cbu]
    placeholder: Secondary Prime Broker TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs:
      ensure: { verb: entity.ensure-or-placeholder, when: empty }
      assign: { verb: cbu.assign-role, when: placeholder }
      search: { verb: party.search, when: [placeholder, filled] }
      add: { verb: party.add, when: empty }
      show: { verb: entity.read, when: filled }
  executing_broker:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: executing-broker }
    cardinality: optional
    depends_on: [cbu]
    placeholder: Executing Broker TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs:
      ensure: { verb: entity.ensure-or-placeholder, when: empty }
      assign: { verb: cbu.assign-role, when: placeholder }
      search: { verb: party.search, when: [placeholder, filled] }
      add: { verb: party.add, when: empty }
      show: { verb: entity.read, when: filled }
  company_secretary:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: company-secretary }
    cardinality: optional
    depends_on: [cbu]
    placeholder: Company Secretary TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs:
      ensure: { verb: entity.ensure-or-placeholder, when: empty }
      assign: { verb: cbu.assign-role, when: placeholder }
      search: { verb: party.search, when: [placeholder, filled] }
      add: { verb: party.add, when: empty }
      show: { verb: entity.read, when: filled }
  ownership_chain:
    type: entity_graph
    entity_kinds: [person, company]
    join: { via: entity_relationships, parent_fk: from_entity_id, child_fk: to_entity_id }
    cardinality: recursive
    max_depth: 5
    depends_on: [aifm]
    state_machine: ubo_epistemic_lifecycle
    overlays: [registry, evidence, screenings]
    edge_overlays: [ownership]
    verbs:
      discover: ubo.discover
      allege: ubo.allege
      verify: ubo.verify
      promote: ubo.promote
      approve: ubo.approve
      reject: ubo.reject
  case:
    type: case
    table: cases
    pk: case_id
    join: { via: cases, parent_fk: cbu_id, child_fk: case_id }
    cardinality: optional
    depends_on: [aifm]
    state_machine: kyc_case_lifecycle
    verbs:
      open: case.open
      submit: case.submit
      approve: case.approve
      reject: case.reject
      request_info: case.request-info
    children:
      tollgate:
        type: tollgate
        table: tollgate_evaluations
        pk: evaluation_id
        join: { via: tollgate_evaluations, parent_fk: case_id, child_fk: evaluation_id }
        cardinality: optional
        depends_on:
          - slot: case
            min_state: intake
        verbs:
          evaluate: tollgate.evaluate
  mandate:
    type: mandate
    table: cbu_trading_profiles
    pk: profile_id
    join: { via: cbu_trading_profiles, parent_fk: cbu_id, child_fk: profile_id }
    cardinality: optional
    depends_on:
      - slot: cbu
        min_state: filled
      - slot: case
        min_state: intake
    verbs:
      create: mandate.create
bulk_macros: [role_slots]
```

---
## File: rust/config/sem_os_seeds/constellation_maps/struct_ie_ucits_icav.yaml
```yaml
constellation: struct.ie.ucits.icav
description: Ireland UCITS ICAV onboarding constellation
jurisdiction: IE
slots:
  cbu:
    type: cbu
    table: cbus
    pk: cbu_id
    cardinality: root
    verbs:
      create: cbu.create
      read: cbu.read
      show: cbu.show
  management_company:
    type: entity
    entity_kinds: [company]
    join:
      via: cbu_entity_roles
      parent_fk: cbu_id
      child_fk: entity_id
      filter_column: role
      filter_value: management-company
    cardinality: mandatory
    depends_on: [cbu]
    placeholder: Management Company TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs:
      ensure: { verb: entity.ensure-or-placeholder, when: empty }
      assign: { verb: cbu.assign-role, when: placeholder }
      search: { verb: party.search, when: [placeholder, filled] }
      add: { verb: party.add, when: empty }
      show: { verb: entity.read, when: filled }
  depositary:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: depositary }
    cardinality: mandatory
    depends_on: [cbu]
    placeholder: Depositary TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs:
      ensure: { verb: entity.ensure-or-placeholder, when: empty }
      assign: { verb: cbu.assign-role, when: placeholder }
      search: { verb: party.search, when: [placeholder, filled] }
      add: { verb: party.add, when: empty }
      show: { verb: entity.read, when: filled }
  investment_manager:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: investment-manager }
    cardinality: optional
    depends_on: [cbu]
    placeholder: Investment Manager TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs:
      ensure: { verb: entity.ensure-or-placeholder, when: empty }
      assign: { verb: cbu.assign-role, when: placeholder }
      search: { verb: party.search, when: [placeholder, filled] }
      add: { verb: party.add, when: empty }
      show: { verb: entity.read, when: filled }
  administrator:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: administrator }
    cardinality: optional
    depends_on: [cbu]
    placeholder: Administrator TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs:
      ensure: { verb: entity.ensure-or-placeholder, when: empty }
      assign: { verb: cbu.assign-role, when: placeholder }
      search: { verb: party.search, when: [placeholder, filled] }
      add: { verb: party.add, when: empty }
      show: { verb: entity.read, when: filled }
  auditor:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: auditor }
    cardinality: optional
    depends_on: [cbu]
    placeholder: Auditor TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs:
      ensure: { verb: entity.ensure-or-placeholder, when: empty }
      assign: { verb: cbu.assign-role, when: placeholder }
      search: { verb: party.search, when: [placeholder, filled] }
      add: { verb: party.add, when: empty }
      show: { verb: entity.read, when: filled }
  company_secretary:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: company-secretary }
    cardinality: optional
    depends_on: [cbu]
    placeholder: Company Secretary TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs:
      ensure: { verb: entity.ensure-or-placeholder, when: empty }
      assign: { verb: cbu.assign-role, when: placeholder }
      search: { verb: party.search, when: [placeholder, filled] }
      add: { verb: party.add, when: empty }
      show: { verb: entity.read, when: filled }
  legal_counsel:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: legal-counsel }
    cardinality: optional
    depends_on: [cbu]
    placeholder: Legal Counsel TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs:
      ensure: { verb: entity.ensure-or-placeholder, when: empty }
      assign: { verb: cbu.assign-role, when: placeholder }
      search: { verb: party.search, when: [placeholder, filled] }
      add: { verb: party.add, when: empty }
      show: { verb: entity.read, when: filled }
  ownership_chain:
    type: entity_graph
    entity_kinds: [person, company]
    join: { via: entity_relationships, parent_fk: from_entity_id, child_fk: to_entity_id }
    cardinality: recursive
    max_depth: 5
    depends_on: [management_company]
    state_machine: ubo_epistemic_lifecycle
    overlays: [registry, evidence, screenings]
    edge_overlays: [ownership]
    verbs:
      discover: ubo.discover
      allege: ubo.allege
      verify: ubo.verify
      promote: ubo.promote
      approve: ubo.approve
      reject: ubo.reject
  case:
    type: case
    table: cases
    pk: case_id
    join: { via: cases, parent_fk: cbu_id, child_fk: case_id }
    cardinality: optional
    depends_on: [management_company]
    state_machine: kyc_case_lifecycle
    verbs:
      open: case.open
      submit: case.submit
      approve: case.approve
      reject: case.reject
      request_info: case.request-info
    children:
      tollgate:
        type: tollgate
        table: tollgate_evaluations
        pk: evaluation_id
        join: { via: tollgate_evaluations, parent_fk: case_id, child_fk: evaluation_id }
        cardinality: optional
        depends_on:
          - slot: case
            min_state: intake
        verbs:
          evaluate: tollgate.evaluate
  mandate:
    type: mandate
    table: cbu_trading_profiles
    pk: profile_id
    join: { via: cbu_trading_profiles, parent_fk: cbu_id, child_fk: profile_id }
    cardinality: optional
    depends_on:
      - slot: cbu
        min_state: filled
      - slot: case
        min_state: intake
    verbs:
      create: mandate.create
bulk_macros: [role_slots]
```

---
## File: rust/config/sem_os_seeds/constellation_maps/struct_lux_aif_raif.yaml
```yaml
constellation: struct.lux.aif.raif
description: Luxembourg AIF RAIF onboarding constellation
jurisdiction: LU
slots:
  cbu:
    type: cbu
    table: cbus
    pk: cbu_id
    cardinality: root
    verbs:
      create: cbu.create
      read: cbu.read
      show: cbu.show
  aifm:
    type: entity
    entity_kinds: [company]
    join:
      via: cbu_entity_roles
      parent_fk: cbu_id
      child_fk: entity_id
      filter_column: role
      filter_value: aifm
    cardinality: mandatory
    depends_on: [cbu]
    placeholder: AIFM TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs:
      ensure:
        verb: entity.ensure-or-placeholder
        when: empty
      assign:
        verb: cbu.assign-role
        when: placeholder
      search:
        verb: party.search
        when: [placeholder, filled]
      add:
        verb: party.add
        when: empty
      show:
        verb: entity.read
        when: filled
  depositary:
    type: entity
    entity_kinds: [company]
    join:
      via: cbu_entity_roles
      parent_fk: cbu_id
      child_fk: entity_id
      filter_column: role
      filter_value: depositary
    cardinality: mandatory
    depends_on: [cbu]
    placeholder: Depositary TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs:
      ensure:
        verb: entity.ensure-or-placeholder
        when: empty
      assign:
        verb: cbu.assign-role
        when: placeholder
      search:
        verb: party.search
        when: [placeholder, filled]
      add:
        verb: party.add
        when: empty
      show:
        verb: entity.read
        when: filled
  investment_manager:
    type: entity
    entity_kinds: [company]
    join:
      via: cbu_entity_roles
      parent_fk: cbu_id
      child_fk: entity_id
      filter_column: role
      filter_value: investment-manager
    cardinality: optional
    depends_on: [cbu]
    placeholder: Investment Manager TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs:
      ensure:
        verb: entity.ensure-or-placeholder
        when: empty
      assign:
        verb: cbu.assign-role
        when: placeholder
      search:
        verb: party.search
        when: [placeholder, filled]
      add:
        verb: party.add
        when: empty
      show:
        verb: entity.read
        when: filled
  administrator:
    type: entity
    entity_kinds: [company]
    join:
      via: cbu_entity_roles
      parent_fk: cbu_id
      child_fk: entity_id
      filter_column: role
      filter_value: administrator
    cardinality: optional
    depends_on: [cbu]
    placeholder: Administrator TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs:
      ensure:
        verb: entity.ensure-or-placeholder
        when: empty
      assign:
        verb: cbu.assign-role
        when: placeholder
      search:
        verb: party.search
        when: [placeholder, filled]
      add:
        verb: party.add
        when: empty
      show:
        verb: entity.read
        when: filled
  auditor:
    type: entity
    entity_kinds: [company]
    join:
      via: cbu_entity_roles
      parent_fk: cbu_id
      child_fk: entity_id
      filter_column: role
      filter_value: auditor
    cardinality: optional
    depends_on: [cbu]
    placeholder: Auditor TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs:
      ensure:
        verb: entity.ensure-or-placeholder
        when: empty
      assign:
        verb: cbu.assign-role
        when: placeholder
      search:
        verb: party.search
        when: [placeholder, filled]
      add:
        verb: party.add
        when: empty
      show:
        verb: entity.read
        when: filled
  prime_broker:
    type: entity
    entity_kinds: [company]
    join:
      via: cbu_entity_roles
      parent_fk: cbu_id
      child_fk: entity_id
      filter_column: role
      filter_value: prime-broker
    cardinality: optional
    depends_on: [cbu]
    placeholder: Prime Broker TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs:
      ensure:
        verb: entity.ensure-or-placeholder
        when: empty
      assign:
        verb: cbu.assign-role
        when: placeholder
      search:
        verb: party.search
        when: [placeholder, filled]
      add:
        verb: party.add
        when: empty
      show:
        verb: entity.read
        when: filled
  ownership_chain:
    type: entity_graph
    entity_kinds: [person, company]
    join:
      via: entity_relationships
      parent_fk: from_entity_id
      child_fk: to_entity_id
    cardinality: recursive
    max_depth: 5
    depends_on: [aifm]
    state_machine: ubo_epistemic_lifecycle
    overlays: [registry, evidence, screenings]
    edge_overlays: [ownership]
    verbs:
      discover: ubo.discover
      allege: ubo.allege
      verify: ubo.verify
      promote: ubo.promote
      approve: ubo.approve
      reject: ubo.reject
  case:
    type: case
    table: cases
    pk: case_id
    join:
      via: cases
      parent_fk: cbu_id
      child_fk: case_id
    cardinality: optional
    depends_on: [aifm]
    state_machine: kyc_case_lifecycle
    verbs:
      open: case.open
      submit: case.submit
      approve: case.approve
      reject: case.reject
      request_info: case.request-info
    children:
      tollgate:
        type: tollgate
        table: tollgate_evaluations
        pk: evaluation_id
        join:
          via: tollgate_evaluations
          parent_fk: case_id
          child_fk: evaluation_id
        cardinality: optional
        depends_on:
          - slot: case
            min_state: intake
        verbs:
          evaluate: tollgate.evaluate
  mandate:
    type: mandate
    table: cbu_trading_profiles
    pk: profile_id
    join:
      via: cbu_trading_profiles
      parent_fk: cbu_id
      child_fk: profile_id
    cardinality: optional
    depends_on:
      - slot: cbu
        min_state: filled
      - slot: case
        min_state: intake
    verbs:
      create: mandate.create
bulk_macros:
  - role_slots
```

---
## File: rust/config/sem_os_seeds/constellation_maps/struct_lux_pe_scsp.yaml
```yaml
constellation: struct.lux.pe.scsp
description: Luxembourg private equity SCSp onboarding constellation
jurisdiction: LU
slots:
  cbu:
    type: cbu
    table: cbus
    pk: cbu_id
    cardinality: root
    verbs:
      create: cbu.create
      read: cbu.read
      show: cbu.show
  general_partner:
    type: entity
    entity_kinds: [company]
    join:
      via: cbu_entity_roles
      parent_fk: cbu_id
      child_fk: entity_id
      filter_column: role
      filter_value: general-partner
    cardinality: mandatory
    depends_on: [cbu]
    placeholder: GP TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs:
      ensure:
        verb: entity.ensure-or-placeholder
        when: empty
      assign:
        verb: cbu.assign-role
        when: placeholder
      search:
        verb: party.search
        when: [placeholder, filled]
      add:
        verb: party.add
        when: empty
      show:
        verb: entity.read
        when: filled
  aifm:
    type: entity
    entity_kinds: [company]
    join:
      via: cbu_entity_roles
      parent_fk: cbu_id
      child_fk: entity_id
      filter_column: role
      filter_value: aifm
    cardinality: optional
    depends_on: [cbu]
    placeholder: AIFM TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs:
      ensure:
        verb: entity.ensure-or-placeholder
        when: empty
      assign:
        verb: cbu.assign-role
        when: placeholder
      search:
        verb: party.search
        when: [placeholder, filled]
      add:
        verb: party.add
        when: empty
      show:
        verb: entity.read
        when: filled
  depositary:
    type: entity
    entity_kinds: [company]
    join:
      via: cbu_entity_roles
      parent_fk: cbu_id
      child_fk: entity_id
      filter_column: role
      filter_value: depositary
    cardinality: optional
    depends_on: [cbu]
    placeholder: Depositary TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs:
      ensure:
        verb: entity.ensure-or-placeholder
        when: empty
      assign:
        verb: cbu.assign-role
        when: placeholder
      search:
        verb: party.search
        when: [placeholder, filled]
      add:
        verb: party.add
        when: empty
      show:
        verb: entity.read
        when: filled
  administrator:
    type: entity
    entity_kinds: [company]
    join:
      via: cbu_entity_roles
      parent_fk: cbu_id
      child_fk: entity_id
      filter_column: role
      filter_value: administrator
    cardinality: optional
    depends_on: [cbu]
    placeholder: Administrator TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs:
      ensure:
        verb: entity.ensure-or-placeholder
        when: empty
      assign:
        verb: cbu.assign-role
        when: placeholder
      search:
        verb: party.search
        when: [placeholder, filled]
      add:
        verb: party.add
        when: empty
      show:
        verb: entity.read
        when: filled
  auditor:
    type: entity
    entity_kinds: [company]
    join:
      via: cbu_entity_roles
      parent_fk: cbu_id
      child_fk: entity_id
      filter_column: role
      filter_value: auditor
    cardinality: optional
    depends_on: [cbu]
    placeholder: Auditor TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs:
      ensure:
        verb: entity.ensure-or-placeholder
        when: empty
      assign:
        verb: cbu.assign-role
        when: placeholder
      search:
        verb: party.search
        when: [placeholder, filled]
      add:
        verb: party.add
        when: empty
      show:
        verb: entity.read
        when: filled
  legal_counsel:
    type: entity
    entity_kinds: [company]
    join:
      via: cbu_entity_roles
      parent_fk: cbu_id
      child_fk: entity_id
      filter_column: role
      filter_value: legal-counsel
    cardinality: optional
    depends_on: [cbu]
    placeholder: Legal Counsel TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs:
      ensure:
        verb: entity.ensure-or-placeholder
        when: empty
      assign:
        verb: cbu.assign-role
        when: placeholder
      search:
        verb: party.search
        when: [placeholder, filled]
      add:
        verb: party.add
        when: empty
      show:
        verb: entity.read
        when: filled
  ownership_chain:
    type: entity_graph
    entity_kinds: [person, company]
    join:
      via: entity_relationships
      parent_fk: from_entity_id
      child_fk: to_entity_id
    cardinality: recursive
    max_depth: 5
    depends_on: [general_partner]
    state_machine: ubo_epistemic_lifecycle
    overlays: [registry, evidence, screenings]
    edge_overlays: [ownership]
    verbs:
      discover: ubo.discover
      allege: ubo.allege
      verify: ubo.verify
      promote: ubo.promote
      approve: ubo.approve
      reject: ubo.reject
  case:
    type: case
    table: cases
    pk: case_id
    join:
      via: cases
      parent_fk: cbu_id
      child_fk: case_id
    cardinality: optional
    depends_on: [general_partner]
    state_machine: kyc_case_lifecycle
    verbs:
      open: case.open
      submit: case.submit
      approve: case.approve
      reject: case.reject
      request_info: case.request-info
    children:
      tollgate:
        type: tollgate
        table: tollgate_evaluations
        pk: evaluation_id
        join:
          via: tollgate_evaluations
          parent_fk: case_id
          child_fk: evaluation_id
        cardinality: optional
        depends_on:
          - slot: case
            min_state: intake
        verbs:
          evaluate: tollgate.evaluate
  mandate:
    type: mandate
    table: cbu_trading_profiles
    pk: profile_id
    join:
      via: cbu_trading_profiles
      parent_fk: cbu_id
      child_fk: profile_id
    cardinality: optional
    depends_on:
      - slot: cbu
        min_state: filled
      - slot: case
        min_state: intake
    verbs:
      create: mandate.create
bulk_macros:
  - role_slots
```

---
## File: rust/config/sem_os_seeds/constellation_maps/struct_lux_ucits_sicav.yaml
```yaml
constellation: struct.lux.ucits.sicav
description: Luxembourg UCITS SICAV onboarding constellation
jurisdiction: LU
slots:
  cbu:
    type: cbu
    table: cbus
    pk: cbu_id
    cardinality: root
    verbs:
      create: cbu.create
      read: cbu.read
      show: cbu.show
  management_company:
    type: entity
    entity_kinds: [company]
    join:
      via: cbu_entity_roles
      parent_fk: cbu_id
      child_fk: entity_id
      filter_column: role
      filter_value: management-company
    cardinality: mandatory
    depends_on: [cbu]
    placeholder: true
    placeholder_detection: name_match
    state_machine: entity_kyc_lifecycle
    overlays:
      [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs:
      ensure:
        verb: entity.ensure-or-placeholder
        when: empty
      assign:
        verb: cbu.assign-role
        when: placeholder
      search:
        verb: party.search
        when: [placeholder, filled]
      add:
        verb: party.add
        when: empty
      show:
        verb: entity.read
        when: filled
  depositary:
    type: entity
    entity_kinds: [company]
    join:
      via: cbu_entity_roles
      parent_fk: cbu_id
      child_fk: entity_id
      filter_column: role
      filter_value: depositary
    cardinality: mandatory
    depends_on: [cbu]
    placeholder: true
    state_machine: entity_kyc_lifecycle
    overlays:
      [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs:
      ensure:
        verb: entity.ensure-or-placeholder
        when: empty
      assign:
        verb: cbu.assign-role
        when: placeholder
      search:
        verb: party.search
        when: [placeholder, filled]
      add:
        verb: party.add
        when: empty
  investment_manager:
    type: entity
    entity_kinds: [company]
    join:
      via: cbu_entity_roles
      parent_fk: cbu_id
      child_fk: entity_id
      filter_column: role
      filter_value: investment-manager
    cardinality: optional
    depends_on: [cbu]
    placeholder: true
    state_machine: entity_kyc_lifecycle
    overlays:
      [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs:
      ensure:
        verb: entity.ensure-or-placeholder
        when: empty
      assign:
        verb: cbu.assign-role
        when: placeholder
  ownership_chain:
    type: entity_graph
    entity_kinds: [person, company]
    join:
      via: entity_relationships
      parent_fk: from_entity_id
      child_fk: to_entity_id
    cardinality: recursive
    max_depth: 5
    depends_on: [management_company]
    state_machine: ubo_epistemic_lifecycle
    overlays: [registry, evidence, screenings]
    edge_overlays: [ownership]
    verbs:
      discover: ubo.discover
      allege: ubo.allege
      verify: ubo.verify
      promote: ubo.promote
      approve: ubo.approve
      reject: ubo.reject
  case:
    type: case
    table: cases
    pk: case_id
    join:
      via: cases
      parent_fk: cbu_id
      child_fk: case_id
    cardinality: optional
    depends_on: [management_company]
    state_machine: kyc_case_lifecycle
    verbs:
      open: case.open
      submit: case.submit
      approve: case.approve
      reject: case.reject
      request_info: case.request-info
    children:
      tollgate:
        type: tollgate
        table: tollgate_evaluations
        pk: evaluation_id
        join:
          via: tollgate_evaluations
          parent_fk: case_id
          child_fk: evaluation_id
        cardinality: optional
        depends_on:
          - slot: case
            min_state: intake
        verbs:
          evaluate: tollgate.evaluate
  mandate:
    type: mandate
    table: cbu_trading_profiles
    pk: profile_id
    join:
      via: cbu_trading_profiles
      parent_fk: cbu_id
      child_fk: profile_id
    cardinality: optional
    depends_on:
      - slot: cbu
        min_state: filled
      - slot: case
        min_state: intake
    verbs:
      create: mandate.create
bulk_macros:
  - role_slots
```

---
## File: rust/config/sem_os_seeds/constellation_maps/struct_pe_cross_border.yaml
```yaml
constellation: struct.pe.cross-border
description: Cross-border private equity parallel-fund onboarding constellation
jurisdiction: XB
slots:
  cbu:
    type: cbu
    table: cbus
    pk: cbu_id
    cardinality: root
    verbs: { create: cbu.create, read: cbu.read, show: cbu.show }
    children:
      us_parallel:
        type: cbu
        join:
          {
            via: cbu_structure_links,
            parent_fk: parent_cbu_id,
            child_fk: child_cbu_id,
            filter_column: relationship_selector,
            filter_value: parallel:us,
          }
        cardinality: optional
        depends_on: [cbu]
        verbs: { show: cbu.read }
      aggregator:
        type: cbu
        join:
          {
            via: cbu_structure_links,
            parent_fk: parent_cbu_id,
            child_fk: child_cbu_id,
            filter_column: relationship_selector,
            filter_value: aggregator,
          }
        cardinality: optional
        depends_on: [cbu]
        verbs: { show: cbu.read }
  general_partner:
    type: entity
    entity_kinds: [company]
    join:
      {
        via: cbu_entity_roles,
        parent_fk: cbu_id,
        child_fk: entity_id,
        filter_column: role,
        filter_value: general-partner,
      }
    cardinality: mandatory
    depends_on: [cbu]
    placeholder: GP TBD
    state_machine: entity_kyc_lifecycle
    overlays:
      [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs:
      {
        ensure: { verb: entity.ensure-or-placeholder, when: empty },
        assign: { verb: cbu.assign-role, when: placeholder },
        search: { verb: party.search, when: [placeholder, filled] },
        add: { verb: party.add, when: empty },
        show: { verb: entity.read, when: filled },
      }
  aifm:
    type: entity
    entity_kinds: [company]
    join:
      {
        via: cbu_entity_roles,
        parent_fk: cbu_id,
        child_fk: entity_id,
        filter_column: role,
        filter_value: aifm,
      }
    cardinality: optional
    depends_on: [cbu]
    placeholder: AIFM TBD
    state_machine: entity_kyc_lifecycle
    overlays:
      [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs:
      {
        ensure: { verb: entity.ensure-or-placeholder, when: empty },
        assign: { verb: cbu.assign-role, when: placeholder },
        search: { verb: party.search, when: [placeholder, filled] },
        add: { verb: party.add, when: empty },
        show: { verb: entity.read, when: filled },
      }
  depositary:
    type: entity
    entity_kinds: [company]
    join:
      {
        via: cbu_entity_roles,
        parent_fk: cbu_id,
        child_fk: entity_id,
        filter_column: role,
        filter_value: depositary,
      }
    cardinality: optional
    depends_on: [cbu]
    placeholder: Depositary TBD
    state_machine: entity_kyc_lifecycle
    overlays:
      [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs:
      {
        ensure: { verb: entity.ensure-or-placeholder, when: empty },
        assign: { verb: cbu.assign-role, when: placeholder },
        search: { verb: party.search, when: [placeholder, filled] },
        add: { verb: party.add, when: empty },
        show: { verb: entity.read, when: filled },
      }
  administrator:
    type: entity
    entity_kinds: [company]
    join:
      {
        via: cbu_entity_roles,
        parent_fk: cbu_id,
        child_fk: entity_id,
        filter_column: role,
        filter_value: administrator,
      }
    cardinality: optional
    depends_on: [cbu]
    placeholder: Administrator TBD
    state_machine: entity_kyc_lifecycle
    overlays:
      [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs:
      {
        ensure: { verb: entity.ensure-or-placeholder, when: empty },
        assign: { verb: cbu.assign-role, when: placeholder },
        search: { verb: party.search, when: [placeholder, filled] },
        add: { verb: party.add, when: empty },
        show: { verb: entity.read, when: filled },
      }
  auditor:
    type: entity
    entity_kinds: [company]
    join:
      {
        via: cbu_entity_roles,
        parent_fk: cbu_id,
        child_fk: entity_id,
        filter_column: role,
        filter_value: auditor,
      }
    cardinality: optional
    depends_on: [cbu]
    placeholder: Auditor TBD
    state_machine: entity_kyc_lifecycle
    overlays:
      [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs:
      {
        ensure: { verb: entity.ensure-or-placeholder, when: empty },
        assign: { verb: cbu.assign-role, when: placeholder },
        search: { verb: party.search, when: [placeholder, filled] },
        add: { verb: party.add, when: empty },
        show: { verb: entity.read, when: filled },
      }
  legal_counsel:
    type: entity
    entity_kinds: [company]
    join:
      {
        via: cbu_entity_roles,
        parent_fk: cbu_id,
        child_fk: entity_id,
        filter_column: role,
        filter_value: legal-counsel,
      }
    cardinality: optional
    depends_on: [cbu]
    placeholder: Legal Counsel TBD
    state_machine: entity_kyc_lifecycle
    overlays:
      [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs:
      {
        ensure: { verb: entity.ensure-or-placeholder, when: empty },
        assign: { verb: cbu.assign-role, when: placeholder },
        search: { verb: party.search, when: [placeholder, filled] },
        add: { verb: party.add, when: empty },
        show: { verb: entity.read, when: filled },
      }
  ownership_chain:
    type: entity_graph
    entity_kinds: [person, company]
    join:
      {
        via: entity_relationships,
        parent_fk: from_entity_id,
        child_fk: to_entity_id,
      }
    cardinality: recursive
    max_depth: 5
    depends_on: [general_partner]
    state_machine: ubo_epistemic_lifecycle
    overlays: [registry, evidence, screenings]
    edge_overlays: [ownership]
    verbs:
      {
        discover: ubo.discover,
        allege: ubo.allege,
        verify: ubo.verify,
        promote: ubo.promote,
        approve: ubo.approve,
        reject: ubo.reject,
      }
  case:
    type: case
    table: cases
    pk: case_id
    join: { via: cases, parent_fk: cbu_id, child_fk: case_id }
    cardinality: optional
    depends_on: [general_partner]
    state_machine: kyc_case_lifecycle
    verbs:
      {
        open: case.open,
        submit: case.submit,
        approve: case.approve,
        reject: case.reject,
        request_info: case.request-info,
      }
    children:
      tollgate:
        type: tollgate
        table: tollgate_evaluations
        pk: evaluation_id
        join:
          {
            via: tollgate_evaluations,
            parent_fk: case_id,
            child_fk: evaluation_id,
          }
        cardinality: optional
        depends_on: [{ slot: case, min_state: intake }]
        verbs: { evaluate: tollgate.evaluate }
  mandate:
    type: mandate
    table: cbu_trading_profiles
    pk: profile_id
    join: { via: cbu_trading_profiles, parent_fk: cbu_id, child_fk: profile_id }
    cardinality: optional
    depends_on:
      [{ slot: cbu, min_state: filled }, { slot: case, min_state: intake }]
    verbs: { create: mandate.create }
bulk_macros: [role_slots]
```

---
## File: rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_acs.yaml
```yaml
constellation: struct.uk.authorised.acs
description: United Kingdom authorised ACS onboarding constellation
jurisdiction: UK
slots:
  cbu:
    type: cbu
    table: cbus
    pk: cbu_id
    cardinality: root
    verbs: { create: cbu.create, read: cbu.read, show: cbu.show }
  acs_operator:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: acs-operator }
    cardinality: mandatory
    depends_on: [cbu]
    placeholder: ACS Operator TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs: { ensure: { verb: entity.ensure-or-placeholder, when: empty }, assign: { verb: cbu.assign-role, when: placeholder }, search: { verb: party.search, when: [placeholder, filled] }, add: { verb: party.add, when: empty }, show: { verb: entity.read, when: filled } }
  depositary:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: depositary }
    cardinality: mandatory
    depends_on: [cbu]
    placeholder: Depositary TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs: { ensure: { verb: entity.ensure-or-placeholder, when: empty }, assign: { verb: cbu.assign-role, when: placeholder }, search: { verb: party.search, when: [placeholder, filled] }, add: { verb: party.add, when: empty }, show: { verb: entity.read, when: filled } }
  investment_manager:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: investment-manager }
    cardinality: optional
    depends_on: [cbu]
    placeholder: Investment Manager TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs: { ensure: { verb: entity.ensure-or-placeholder, when: empty }, assign: { verb: cbu.assign-role, when: placeholder }, search: { verb: party.search, when: [placeholder, filled] }, add: { verb: party.add, when: empty }, show: { verb: entity.read, when: filled } }
  administrator:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: administrator }
    cardinality: optional
    depends_on: [cbu]
    placeholder: Administrator TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs: { ensure: { verb: entity.ensure-or-placeholder, when: empty }, assign: { verb: cbu.assign-role, when: placeholder }, search: { verb: party.search, when: [placeholder, filled] }, add: { verb: party.add, when: empty }, show: { verb: entity.read, when: filled } }
  auditor:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: auditor }
    cardinality: optional
    depends_on: [cbu]
    placeholder: Auditor TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs: { ensure: { verb: entity.ensure-or-placeholder, when: empty }, assign: { verb: cbu.assign-role, when: placeholder }, search: { verb: party.search, when: [placeholder, filled] }, add: { verb: party.add, when: empty }, show: { verb: entity.read, when: filled } }
  ownership_chain:
    type: entity_graph
    entity_kinds: [person, company]
    join: { via: entity_relationships, parent_fk: from_entity_id, child_fk: to_entity_id }
    cardinality: recursive
    max_depth: 5
    depends_on: [acs_operator]
    state_machine: ubo_epistemic_lifecycle
    overlays: [registry, evidence, screenings]
    edge_overlays: [ownership]
    verbs: { discover: ubo.discover, allege: ubo.allege, verify: ubo.verify, promote: ubo.promote, approve: ubo.approve, reject: ubo.reject }
  case:
    type: case
    table: cases
    pk: case_id
    join: { via: cases, parent_fk: cbu_id, child_fk: case_id }
    cardinality: optional
    depends_on: [acs_operator]
    state_machine: kyc_case_lifecycle
    verbs: { open: case.open, submit: case.submit, approve: case.approve, reject: case.reject, request_info: case.request-info }
    children:
      tollgate:
        type: tollgate
        table: tollgate_evaluations
        pk: evaluation_id
        join: { via: tollgate_evaluations, parent_fk: case_id, child_fk: evaluation_id }
        cardinality: optional
        depends_on: [{ slot: case, min_state: intake }]
        verbs: { evaluate: tollgate.evaluate }
  mandate:
    type: mandate
    table: cbu_trading_profiles
    pk: profile_id
    join: { via: cbu_trading_profiles, parent_fk: cbu_id, child_fk: profile_id }
    cardinality: optional
    depends_on: [{ slot: cbu, min_state: filled }, { slot: case, min_state: intake }]
    verbs: { create: mandate.create }
bulk_macros: [role_slots]
```

---
## File: rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_aut.yaml
```yaml
constellation: struct.uk.authorised.aut
description: United Kingdom authorised unit trust onboarding constellation
jurisdiction: UK
slots:
  cbu:
    type: cbu
    table: cbus
    pk: cbu_id
    cardinality: root
    verbs: { create: cbu.create, read: cbu.read, show: cbu.show }
  authorised_fund_manager:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: authorised-fund-manager }
    cardinality: mandatory
    depends_on: [cbu]
    placeholder: Authorised Fund Manager TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs: { ensure: { verb: entity.ensure-or-placeholder, when: empty }, assign: { verb: cbu.assign-role, when: placeholder }, search: { verb: party.search, when: [placeholder, filled] }, add: { verb: party.add, when: empty }, show: { verb: entity.read, when: filled } }
  trustee:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: trustee }
    cardinality: mandatory
    depends_on: [cbu]
    placeholder: Trustee TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs: { ensure: { verb: entity.ensure-or-placeholder, when: empty }, assign: { verb: cbu.assign-role, when: placeholder }, search: { verb: party.search, when: [placeholder, filled] }, add: { verb: party.add, when: empty }, show: { verb: entity.read, when: filled } }
  investment_manager:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: investment-manager }
    cardinality: optional
    depends_on: [cbu]
    placeholder: Investment Manager TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs: { ensure: { verb: entity.ensure-or-placeholder, when: empty }, assign: { verb: cbu.assign-role, when: placeholder }, search: { verb: party.search, when: [placeholder, filled] }, add: { verb: party.add, when: empty }, show: { verb: entity.read, when: filled } }
  administrator:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: administrator }
    cardinality: optional
    depends_on: [cbu]
    placeholder: Administrator TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs: { ensure: { verb: entity.ensure-or-placeholder, when: empty }, assign: { verb: cbu.assign-role, when: placeholder }, search: { verb: party.search, when: [placeholder, filled] }, add: { verb: party.add, when: empty }, show: { verb: entity.read, when: filled } }
  auditor:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: auditor }
    cardinality: optional
    depends_on: [cbu]
    placeholder: Auditor TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs: { ensure: { verb: entity.ensure-or-placeholder, when: empty }, assign: { verb: cbu.assign-role, when: placeholder }, search: { verb: party.search, when: [placeholder, filled] }, add: { verb: party.add, when: empty }, show: { verb: entity.read, when: filled } }
  ownership_chain:
    type: entity_graph
    entity_kinds: [person, company]
    join: { via: entity_relationships, parent_fk: from_entity_id, child_fk: to_entity_id }
    cardinality: recursive
    max_depth: 5
    depends_on: [authorised_fund_manager]
    state_machine: ubo_epistemic_lifecycle
    overlays: [registry, evidence, screenings]
    edge_overlays: [ownership]
    verbs: { discover: ubo.discover, allege: ubo.allege, verify: ubo.verify, promote: ubo.promote, approve: ubo.approve, reject: ubo.reject }
  case:
    type: case
    table: cases
    pk: case_id
    join: { via: cases, parent_fk: cbu_id, child_fk: case_id }
    cardinality: optional
    depends_on: [authorised_fund_manager]
    state_machine: kyc_case_lifecycle
    verbs: { open: case.open, submit: case.submit, approve: case.approve, reject: case.reject, request_info: case.request-info }
    children:
      tollgate:
        type: tollgate
        table: tollgate_evaluations
        pk: evaluation_id
        join: { via: tollgate_evaluations, parent_fk: case_id, child_fk: evaluation_id }
        cardinality: optional
        depends_on: [{ slot: case, min_state: intake }]
        verbs: { evaluate: tollgate.evaluate }
  mandate:
    type: mandate
    table: cbu_trading_profiles
    pk: profile_id
    join: { via: cbu_trading_profiles, parent_fk: cbu_id, child_fk: profile_id }
    cardinality: optional
    depends_on: [{ slot: cbu, min_state: filled }, { slot: case, min_state: intake }]
    verbs: { create: mandate.create }
bulk_macros: [role_slots]
```

---
## File: rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_ltaf.yaml
```yaml
constellation: struct.uk.authorised.ltaf
description: United Kingdom authorised LTAF onboarding constellation
jurisdiction: UK
slots:
  cbu:
    type: cbu
    table: cbus
    pk: cbu_id
    cardinality: root
    verbs: { create: cbu.create, read: cbu.read, show: cbu.show }
  authorised_corporate_director:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: authorised-corporate-director }
    cardinality: mandatory
    depends_on: [cbu]
    placeholder: ACD TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs: { ensure: { verb: entity.ensure-or-placeholder, when: empty }, assign: { verb: cbu.assign-role, when: placeholder }, search: { verb: party.search, when: [placeholder, filled] }, add: { verb: party.add, when: empty }, show: { verb: entity.read, when: filled } }
  depositary:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: depositary }
    cardinality: mandatory
    depends_on: [cbu]
    placeholder: Depositary TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs: { ensure: { verb: entity.ensure-or-placeholder, when: empty }, assign: { verb: cbu.assign-role, when: placeholder }, search: { verb: party.search, when: [placeholder, filled] }, add: { verb: party.add, when: empty }, show: { verb: entity.read, when: filled } }
  investment_manager:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: investment-manager }
    cardinality: optional
    depends_on: [cbu]
    placeholder: Investment Manager TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs: { ensure: { verb: entity.ensure-or-placeholder, when: empty }, assign: { verb: cbu.assign-role, when: placeholder }, search: { verb: party.search, when: [placeholder, filled] }, add: { verb: party.add, when: empty }, show: { verb: entity.read, when: filled } }
  administrator:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: administrator }
    cardinality: optional
    depends_on: [cbu]
    placeholder: Administrator TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs: { ensure: { verb: entity.ensure-or-placeholder, when: empty }, assign: { verb: cbu.assign-role, when: placeholder }, search: { verb: party.search, when: [placeholder, filled] }, add: { verb: party.add, when: empty }, show: { verb: entity.read, when: filled } }
  auditor:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: auditor }
    cardinality: optional
    depends_on: [cbu]
    placeholder: Auditor TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs: { ensure: { verb: entity.ensure-or-placeholder, when: empty }, assign: { verb: cbu.assign-role, when: placeholder }, search: { verb: party.search, when: [placeholder, filled] }, add: { verb: party.add, when: empty }, show: { verb: entity.read, when: filled } }
  registrar:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: registrar }
    cardinality: optional
    depends_on: [cbu]
    placeholder: Registrar TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs: { ensure: { verb: entity.ensure-or-placeholder, when: empty }, assign: { verb: cbu.assign-role, when: placeholder }, search: { verb: party.search, when: [placeholder, filled] }, add: { verb: party.add, when: empty }, show: { verb: entity.read, when: filled } }
  valuation_agent:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: valuation-agent }
    cardinality: optional
    depends_on: [cbu]
    placeholder: Valuation Agent TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs: { ensure: { verb: entity.ensure-or-placeholder, when: empty }, assign: { verb: cbu.assign-role, when: placeholder }, search: { verb: party.search, when: [placeholder, filled] }, add: { verb: party.add, when: empty }, show: { verb: entity.read, when: filled } }
  ownership_chain:
    type: entity_graph
    entity_kinds: [person, company]
    join: { via: entity_relationships, parent_fk: from_entity_id, child_fk: to_entity_id }
    cardinality: recursive
    max_depth: 5
    depends_on: [authorised_corporate_director]
    state_machine: ubo_epistemic_lifecycle
    overlays: [registry, evidence, screenings]
    edge_overlays: [ownership]
    verbs: { discover: ubo.discover, allege: ubo.allege, verify: ubo.verify, promote: ubo.promote, approve: ubo.approve, reject: ubo.reject }
  case:
    type: case
    table: cases
    pk: case_id
    join: { via: cases, parent_fk: cbu_id, child_fk: case_id }
    cardinality: optional
    depends_on: [authorised_corporate_director]
    state_machine: kyc_case_lifecycle
    verbs: { open: case.open, submit: case.submit, approve: case.approve, reject: case.reject, request_info: case.request-info }
    children:
      tollgate:
        type: tollgate
        table: tollgate_evaluations
        pk: evaluation_id
        join: { via: tollgate_evaluations, parent_fk: case_id, child_fk: evaluation_id }
        cardinality: optional
        depends_on: [{ slot: case, min_state: intake }]
        verbs: { evaluate: tollgate.evaluate }
  mandate:
    type: mandate
    table: cbu_trading_profiles
    pk: profile_id
    join: { via: cbu_trading_profiles, parent_fk: cbu_id, child_fk: profile_id }
    cardinality: optional
    depends_on: [{ slot: cbu, min_state: filled }, { slot: case, min_state: intake }]
    verbs: { create: mandate.create }
bulk_macros: [role_slots]
```

---
## File: rust/config/sem_os_seeds/constellation_maps/struct_uk_authorised_oeic.yaml
```yaml
constellation: struct.uk.authorised.oeic
description: United Kingdom authorised OEIC onboarding constellation
jurisdiction: UK
slots:
  cbu:
    type: cbu
    table: cbus
    pk: cbu_id
    cardinality: root
    verbs: { create: cbu.create, read: cbu.read, show: cbu.show }
  authorised_corporate_director:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: authorised-corporate-director }
    cardinality: mandatory
    depends_on: [cbu]
    placeholder: ACD TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs:
      ensure: { verb: entity.ensure-or-placeholder, when: empty }
      assign: { verb: cbu.assign-role, when: placeholder }
      search: { verb: party.search, when: [placeholder, filled] }
      add: { verb: party.add, when: empty }
      show: { verb: entity.read, when: filled }
  depositary:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: depositary }
    cardinality: mandatory
    depends_on: [cbu]
    placeholder: Depositary TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs:
      ensure: { verb: entity.ensure-or-placeholder, when: empty }
      assign: { verb: cbu.assign-role, when: placeholder }
      search: { verb: party.search, when: [placeholder, filled] }
      add: { verb: party.add, when: empty }
      show: { verb: entity.read, when: filled }
  investment_manager:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: investment-manager }
    cardinality: optional
    depends_on: [cbu]
    placeholder: Investment Manager TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs: { ensure: { verb: entity.ensure-or-placeholder, when: empty }, assign: { verb: cbu.assign-role, when: placeholder }, search: { verb: party.search, when: [placeholder, filled] }, add: { verb: party.add, when: empty }, show: { verb: entity.read, when: filled } }
  administrator:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: administrator }
    cardinality: optional
    depends_on: [cbu]
    placeholder: Administrator TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs: { ensure: { verb: entity.ensure-or-placeholder, when: empty }, assign: { verb: cbu.assign-role, when: placeholder }, search: { verb: party.search, when: [placeholder, filled] }, add: { verb: party.add, when: empty }, show: { verb: entity.read, when: filled } }
  auditor:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: auditor }
    cardinality: optional
    depends_on: [cbu]
    placeholder: Auditor TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs: { ensure: { verb: entity.ensure-or-placeholder, when: empty }, assign: { verb: cbu.assign-role, when: placeholder }, search: { verb: party.search, when: [placeholder, filled] }, add: { verb: party.add, when: empty }, show: { verb: entity.read, when: filled } }
  registrar:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: registrar }
    cardinality: optional
    depends_on: [cbu]
    placeholder: Registrar TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs: { ensure: { verb: entity.ensure-or-placeholder, when: empty }, assign: { verb: cbu.assign-role, when: placeholder }, search: { verb: party.search, when: [placeholder, filled] }, add: { verb: party.add, when: empty }, show: { verb: entity.read, when: filled } }
  ownership_chain:
    type: entity_graph
    entity_kinds: [person, company]
    join: { via: entity_relationships, parent_fk: from_entity_id, child_fk: to_entity_id }
    cardinality: recursive
    max_depth: 5
    depends_on: [authorised_corporate_director]
    state_machine: ubo_epistemic_lifecycle
    overlays: [registry, evidence, screenings]
    edge_overlays: [ownership]
    verbs: { discover: ubo.discover, allege: ubo.allege, verify: ubo.verify, promote: ubo.promote, approve: ubo.approve, reject: ubo.reject }
  case:
    type: case
    table: cases
    pk: case_id
    join: { via: cases, parent_fk: cbu_id, child_fk: case_id }
    cardinality: optional
    depends_on: [authorised_corporate_director]
    state_machine: kyc_case_lifecycle
    verbs: { open: case.open, submit: case.submit, approve: case.approve, reject: case.reject, request_info: case.request-info }
    children:
      tollgate:
        type: tollgate
        table: tollgate_evaluations
        pk: evaluation_id
        join: { via: tollgate_evaluations, parent_fk: case_id, child_fk: evaluation_id }
        cardinality: optional
        depends_on: [{ slot: case, min_state: intake }]
        verbs: { evaluate: tollgate.evaluate }
  mandate:
    type: mandate
    table: cbu_trading_profiles
    pk: profile_id
    join: { via: cbu_trading_profiles, parent_fk: cbu_id, child_fk: profile_id }
    cardinality: optional
    depends_on: [{ slot: cbu, min_state: filled }, { slot: case, min_state: intake }]
    verbs: { create: mandate.create }
bulk_macros: [role_slots]
```

---
## File: rust/config/sem_os_seeds/constellation_maps/struct_uk_manager_llp.yaml
```yaml
constellation: struct.uk.manager.llp
description: United Kingdom manager LLP onboarding constellation
jurisdiction: UK
slots:
  cbu:
    type: cbu
    table: cbus
    pk: cbu_id
    cardinality: root
    verbs: { create: cbu.create, read: cbu.read, show: cbu.show }
  designated_member_1:
    type: entity
    entity_kinds: [company, person]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: designated-member }
    occurrence: 1
    cardinality: mandatory
    depends_on: [cbu]
    placeholder: Designated Member 1 TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs: { ensure: { verb: entity.ensure-or-placeholder, when: empty }, assign: { verb: cbu.assign-role, when: placeholder }, search: { verb: party.search, when: [placeholder, filled] }, add: { verb: party.add, when: empty }, show: { verb: entity.read, when: filled } }
  designated_member_2:
    type: entity
    entity_kinds: [company, person]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: designated-member }
    occurrence: 2
    cardinality: mandatory
    depends_on: [cbu]
    placeholder: Designated Member 2 TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs: { ensure: { verb: entity.ensure-or-placeholder, when: empty }, assign: { verb: cbu.assign-role, when: placeholder }, search: { verb: party.search, when: [placeholder, filled] }, add: { verb: party.add, when: empty }, show: { verb: entity.read, when: filled } }
  compliance_officer:
    type: entity
    entity_kinds: [person]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: compliance-officer }
    cardinality: optional
    depends_on: [cbu]
    placeholder: Compliance Officer TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs: { ensure: { verb: entity.ensure-or-placeholder, when: empty }, assign: { verb: cbu.assign-role, when: placeholder }, search: { verb: party.search, when: [placeholder, filled] }, add: { verb: party.add, when: empty }, show: { verb: entity.read, when: filled } }
  mlro:
    type: entity
    entity_kinds: [person]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: mlro }
    cardinality: optional
    depends_on: [cbu]
    placeholder: MLRO TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs: { ensure: { verb: entity.ensure-or-placeholder, when: empty }, assign: { verb: cbu.assign-role, when: placeholder }, search: { verb: party.search, when: [placeholder, filled] }, add: { verb: party.add, when: empty }, show: { verb: entity.read, when: filled } }
  auditor:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: auditor }
    cardinality: optional
    depends_on: [cbu]
    placeholder: Auditor TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs: { ensure: { verb: entity.ensure-or-placeholder, when: empty }, assign: { verb: cbu.assign-role, when: placeholder }, search: { verb: party.search, when: [placeholder, filled] }, add: { verb: party.add, when: empty }, show: { verb: entity.read, when: filled } }
  ownership_chain:
    type: entity_graph
    entity_kinds: [person, company]
    join: { via: entity_relationships, parent_fk: from_entity_id, child_fk: to_entity_id }
    cardinality: recursive
    max_depth: 5
    depends_on: [designated_member_1, designated_member_2]
    state_machine: ubo_epistemic_lifecycle
    overlays: [registry, evidence, screenings]
    edge_overlays: [ownership]
    verbs: { discover: ubo.discover, allege: ubo.allege, verify: ubo.verify, promote: ubo.promote, approve: ubo.approve, reject: ubo.reject }
  case:
    type: case
    table: cases
    pk: case_id
    join: { via: cases, parent_fk: cbu_id, child_fk: case_id }
    cardinality: optional
    depends_on: [designated_member_1]
    state_machine: kyc_case_lifecycle
    verbs: { open: case.open, submit: case.submit, approve: case.approve, reject: case.reject, request_info: case.request-info }
    children:
      tollgate:
        type: tollgate
        table: tollgate_evaluations
        pk: evaluation_id
        join: { via: tollgate_evaluations, parent_fk: case_id, child_fk: evaluation_id }
        cardinality: optional
        depends_on: [{ slot: case, min_state: intake }]
        verbs: { evaluate: tollgate.evaluate }
bulk_macros: [role_slots]
```

---
## File: rust/config/sem_os_seeds/constellation_maps/struct_uk_pe_lp.yaml
```yaml
constellation: struct.uk.private-equity.lp
description: United Kingdom private equity LP onboarding constellation
jurisdiction: UK
slots:
  cbu:
    type: cbu
    table: cbus
    pk: cbu_id
    cardinality: root
    verbs: { create: cbu.create, read: cbu.read, show: cbu.show }
  general_partner:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: general-partner }
    cardinality: mandatory
    depends_on: [cbu]
    placeholder: GP TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs: { ensure: { verb: entity.ensure-or-placeholder, when: empty }, assign: { verb: cbu.assign-role, when: placeholder }, search: { verb: party.search, when: [placeholder, filled] }, add: { verb: party.add, when: empty }, show: { verb: entity.read, when: filled } }
  aifm:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: aifm }
    cardinality: optional
    depends_on: [cbu]
    placeholder: AIFM TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs: { ensure: { verb: entity.ensure-or-placeholder, when: empty }, assign: { verb: cbu.assign-role, when: placeholder }, search: { verb: party.search, when: [placeholder, filled] }, add: { verb: party.add, when: empty }, show: { verb: entity.read, when: filled } }
  depositary:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: depositary }
    cardinality: optional
    depends_on: [cbu]
    placeholder: Depositary TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs: { ensure: { verb: entity.ensure-or-placeholder, when: empty }, assign: { verb: cbu.assign-role, when: placeholder }, search: { verb: party.search, when: [placeholder, filled] }, add: { verb: party.add, when: empty }, show: { verb: entity.read, when: filled } }
  administrator:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: administrator }
    cardinality: optional
    depends_on: [cbu]
    placeholder: Administrator TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs: { ensure: { verb: entity.ensure-or-placeholder, when: empty }, assign: { verb: cbu.assign-role, when: placeholder }, search: { verb: party.search, when: [placeholder, filled] }, add: { verb: party.add, when: empty }, show: { verb: entity.read, when: filled } }
  auditor:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: auditor }
    cardinality: optional
    depends_on: [cbu]
    placeholder: Auditor TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs: { ensure: { verb: entity.ensure-or-placeholder, when: empty }, assign: { verb: cbu.assign-role, when: placeholder }, search: { verb: party.search, when: [placeholder, filled] }, add: { verb: party.add, when: empty }, show: { verb: entity.read, when: filled } }
  legal_counsel:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: legal-counsel }
    cardinality: optional
    depends_on: [cbu]
    placeholder: Legal Counsel TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs: { ensure: { verb: entity.ensure-or-placeholder, when: empty }, assign: { verb: cbu.assign-role, when: placeholder }, search: { verb: party.search, when: [placeholder, filled] }, add: { verb: party.add, when: empty }, show: { verb: entity.read, when: filled } }
  ownership_chain:
    type: entity_graph
    entity_kinds: [person, company]
    join: { via: entity_relationships, parent_fk: from_entity_id, child_fk: to_entity_id }
    cardinality: recursive
    max_depth: 5
    depends_on: [general_partner]
    state_machine: ubo_epistemic_lifecycle
    overlays: [registry, evidence, screenings]
    edge_overlays: [ownership]
    verbs: { discover: ubo.discover, allege: ubo.allege, verify: ubo.verify, promote: ubo.promote, approve: ubo.approve, reject: ubo.reject }
  case:
    type: case
    table: cases
    pk: case_id
    join: { via: cases, parent_fk: cbu_id, child_fk: case_id }
    cardinality: optional
    depends_on: [general_partner]
    state_machine: kyc_case_lifecycle
    verbs: { open: case.open, submit: case.submit, approve: case.approve, reject: case.reject, request_info: case.request-info }
    children:
      tollgate:
        type: tollgate
        table: tollgate_evaluations
        pk: evaluation_id
        join: { via: tollgate_evaluations, parent_fk: case_id, child_fk: evaluation_id }
        cardinality: optional
        depends_on: [{ slot: case, min_state: intake }]
        verbs: { evaluate: tollgate.evaluate }
  mandate:
    type: mandate
    table: cbu_trading_profiles
    pk: profile_id
    join: { via: cbu_trading_profiles, parent_fk: cbu_id, child_fk: profile_id }
    cardinality: optional
    depends_on: [{ slot: cbu, min_state: filled }, { slot: case, min_state: intake }]
    verbs: { create: mandate.create }
bulk_macros: [role_slots]
```

---
## File: rust/config/sem_os_seeds/constellation_maps/struct_us_40act_closed_end.yaml
```yaml
constellation: struct.us.40act.closed-end
description: United States 40 Act closed-end onboarding constellation
jurisdiction: US
slots:
  cbu:
    type: cbu
    table: cbus
    pk: cbu_id
    cardinality: root
    verbs: { create: cbu.create, read: cbu.read, show: cbu.show }
  investment_adviser:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: investment-adviser }
    cardinality: mandatory
    depends_on: [cbu]
    placeholder: Investment Adviser TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs: { ensure: { verb: entity.ensure-or-placeholder, when: empty }, assign: { verb: cbu.assign-role, when: placeholder }, search: { verb: party.search, when: [placeholder, filled] }, add: { verb: party.add, when: empty }, show: { verb: entity.read, when: filled } }
  custodian:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: custodian }
    cardinality: mandatory
    depends_on: [cbu]
    placeholder: Custodian TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs: { ensure: { verb: entity.ensure-or-placeholder, when: empty }, assign: { verb: cbu.assign-role, when: placeholder }, search: { verb: party.search, when: [placeholder, filled] }, add: { verb: party.add, when: empty }, show: { verb: entity.read, when: filled } }
  sub_adviser:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: sub-adviser }
    cardinality: optional
    depends_on: [cbu]
    placeholder: Sub-Adviser TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs: { ensure: { verb: entity.ensure-or-placeholder, when: empty }, assign: { verb: cbu.assign-role, when: placeholder }, search: { verb: party.search, when: [placeholder, filled] }, add: { verb: party.add, when: empty }, show: { verb: entity.read, when: filled } }
  administrator:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: administrator }
    cardinality: optional
    depends_on: [cbu]
    placeholder: Administrator TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs: { ensure: { verb: entity.ensure-or-placeholder, when: empty }, assign: { verb: cbu.assign-role, when: placeholder }, search: { verb: party.search, when: [placeholder, filled] }, add: { verb: party.add, when: empty }, show: { verb: entity.read, when: filled } }
  transfer_agent:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: transfer-agent }
    cardinality: optional
    depends_on: [cbu]
    placeholder: Transfer Agent TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs: { ensure: { verb: entity.ensure-or-placeholder, when: empty }, assign: { verb: cbu.assign-role, when: placeholder }, search: { verb: party.search, when: [placeholder, filled] }, add: { verb: party.add, when: empty }, show: { verb: entity.read, when: filled } }
  auditor:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: auditor }
    cardinality: optional
    depends_on: [cbu]
    placeholder: Auditor TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs: { ensure: { verb: entity.ensure-or-placeholder, when: empty }, assign: { verb: cbu.assign-role, when: placeholder }, search: { verb: party.search, when: [placeholder, filled] }, add: { verb: party.add, when: empty }, show: { verb: entity.read, when: filled } }
  legal_counsel:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: legal-counsel }
    cardinality: optional
    depends_on: [cbu]
    placeholder: Legal Counsel TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs: { ensure: { verb: entity.ensure-or-placeholder, when: empty }, assign: { verb: cbu.assign-role, when: placeholder }, search: { verb: party.search, when: [placeholder, filled] }, add: { verb: party.add, when: empty }, show: { verb: entity.read, when: filled } }
  ownership_chain:
    type: entity_graph
    entity_kinds: [person, company]
    join: { via: entity_relationships, parent_fk: from_entity_id, child_fk: to_entity_id }
    cardinality: recursive
    max_depth: 5
    depends_on: [investment_adviser]
    state_machine: ubo_epistemic_lifecycle
    overlays: [registry, evidence, screenings]
    edge_overlays: [ownership]
    verbs: { discover: ubo.discover, allege: ubo.allege, verify: ubo.verify, promote: ubo.promote, approve: ubo.approve, reject: ubo.reject }
  case:
    type: case
    table: cases
    pk: case_id
    join: { via: cases, parent_fk: cbu_id, child_fk: case_id }
    cardinality: optional
    depends_on: [investment_adviser]
    state_machine: kyc_case_lifecycle
    verbs: { open: case.open, submit: case.submit, approve: case.approve, reject: case.reject, request_info: case.request-info }
    children:
      tollgate:
        type: tollgate
        table: tollgate_evaluations
        pk: evaluation_id
        join: { via: tollgate_evaluations, parent_fk: case_id, child_fk: evaluation_id }
        cardinality: optional
        depends_on: [{ slot: case, min_state: intake }]
        verbs: { evaluate: tollgate.evaluate }
  mandate:
    type: mandate
    table: cbu_trading_profiles
    pk: profile_id
    join: { via: cbu_trading_profiles, parent_fk: cbu_id, child_fk: profile_id }
    cardinality: optional
    depends_on: [{ slot: cbu, min_state: filled }, { slot: case, min_state: intake }]
    verbs: { create: mandate.create }
bulk_macros: [role_slots]
```

---
## File: rust/config/sem_os_seeds/constellation_maps/struct_us_40act_open_end.yaml
```yaml
constellation: struct.us.40act.open-end
description: United States 40 Act open-end onboarding constellation
jurisdiction: US
slots:
  cbu:
    type: cbu
    table: cbus
    pk: cbu_id
    cardinality: root
    verbs: { create: cbu.create, read: cbu.read, show: cbu.show }
  investment_adviser:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: investment-adviser }
    cardinality: mandatory
    depends_on: [cbu]
    placeholder: Investment Adviser TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs: { ensure: { verb: entity.ensure-or-placeholder, when: empty }, assign: { verb: cbu.assign-role, when: placeholder }, search: { verb: party.search, when: [placeholder, filled] }, add: { verb: party.add, when: empty }, show: { verb: entity.read, when: filled } }
  custodian:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: custodian }
    cardinality: mandatory
    depends_on: [cbu]
    placeholder: Custodian TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs: { ensure: { verb: entity.ensure-or-placeholder, when: empty }, assign: { verb: cbu.assign-role, when: placeholder }, search: { verb: party.search, when: [placeholder, filled] }, add: { verb: party.add, when: empty }, show: { verb: entity.read, when: filled } }
  sub_adviser:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: sub-adviser }
    cardinality: optional
    depends_on: [cbu]
    placeholder: Sub-Adviser TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs: { ensure: { verb: entity.ensure-or-placeholder, when: empty }, assign: { verb: cbu.assign-role, when: placeholder }, search: { verb: party.search, when: [placeholder, filled] }, add: { verb: party.add, when: empty }, show: { verb: entity.read, when: filled } }
  administrator:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: administrator }
    cardinality: optional
    depends_on: [cbu]
    placeholder: Administrator TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs: { ensure: { verb: entity.ensure-or-placeholder, when: empty }, assign: { verb: cbu.assign-role, when: placeholder }, search: { verb: party.search, when: [placeholder, filled] }, add: { verb: party.add, when: empty }, show: { verb: entity.read, when: filled } }
  transfer_agent:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: transfer-agent }
    cardinality: optional
    depends_on: [cbu]
    placeholder: Transfer Agent TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs: { ensure: { verb: entity.ensure-or-placeholder, when: empty }, assign: { verb: cbu.assign-role, when: placeholder }, search: { verb: party.search, when: [placeholder, filled] }, add: { verb: party.add, when: empty }, show: { verb: entity.read, when: filled } }
  distributor:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: distributor }
    cardinality: optional
    depends_on: [cbu]
    placeholder: Distributor TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs: { ensure: { verb: entity.ensure-or-placeholder, when: empty }, assign: { verb: cbu.assign-role, when: placeholder }, search: { verb: party.search, when: [placeholder, filled] }, add: { verb: party.add, when: empty }, show: { verb: entity.read, when: filled } }
  auditor:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: auditor }
    cardinality: optional
    depends_on: [cbu]
    placeholder: Auditor TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs: { ensure: { verb: entity.ensure-or-placeholder, when: empty }, assign: { verb: cbu.assign-role, when: placeholder }, search: { verb: party.search, when: [placeholder, filled] }, add: { verb: party.add, when: empty }, show: { verb: entity.read, when: filled } }
  legal_counsel:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: legal-counsel }
    cardinality: optional
    depends_on: [cbu]
    placeholder: Legal Counsel TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs: { ensure: { verb: entity.ensure-or-placeholder, when: empty }, assign: { verb: cbu.assign-role, when: placeholder }, search: { verb: party.search, when: [placeholder, filled] }, add: { verb: party.add, when: empty }, show: { verb: entity.read, when: filled } }
  ownership_chain:
    type: entity_graph
    entity_kinds: [person, company]
    join: { via: entity_relationships, parent_fk: from_entity_id, child_fk: to_entity_id }
    cardinality: recursive
    max_depth: 5
    depends_on: [investment_adviser]
    state_machine: ubo_epistemic_lifecycle
    overlays: [registry, evidence, screenings]
    edge_overlays: [ownership]
    verbs: { discover: ubo.discover, allege: ubo.allege, verify: ubo.verify, promote: ubo.promote, approve: ubo.approve, reject: ubo.reject }
  case:
    type: case
    table: cases
    pk: case_id
    join: { via: cases, parent_fk: cbu_id, child_fk: case_id }
    cardinality: optional
    depends_on: [investment_adviser]
    state_machine: kyc_case_lifecycle
    verbs: { open: case.open, submit: case.submit, approve: case.approve, reject: case.reject, request_info: case.request-info }
    children:
      tollgate:
        type: tollgate
        table: tollgate_evaluations
        pk: evaluation_id
        join: { via: tollgate_evaluations, parent_fk: case_id, child_fk: evaluation_id }
        cardinality: optional
        depends_on: [{ slot: case, min_state: intake }]
        verbs: { evaluate: tollgate.evaluate }
  mandate:
    type: mandate
    table: cbu_trading_profiles
    pk: profile_id
    join: { via: cbu_trading_profiles, parent_fk: cbu_id, child_fk: profile_id }
    cardinality: optional
    depends_on: [{ slot: cbu, min_state: filled }, { slot: case, min_state: intake }]
    verbs: { create: mandate.create }
bulk_macros: [role_slots]
```

---
## File: rust/config/sem_os_seeds/constellation_maps/struct_us_etf_40act.yaml
```yaml
constellation: struct.us.etf.40act
description: United States 40 Act ETF onboarding constellation
jurisdiction: US
slots:
  cbu:
    type: cbu
    table: cbus
    pk: cbu_id
    cardinality: root
    verbs: { create: cbu.create, read: cbu.read, show: cbu.show }
  investment_adviser:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: investment-adviser }
    cardinality: mandatory
    depends_on: [cbu]
    placeholder: Investment Adviser TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs: { ensure: { verb: entity.ensure-or-placeholder, when: empty }, assign: { verb: cbu.assign-role, when: placeholder }, search: { verb: party.search, when: [placeholder, filled] }, add: { verb: party.add, when: empty }, show: { verb: entity.read, when: filled } }
  custodian:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: custodian }
    cardinality: mandatory
    depends_on: [cbu]
    placeholder: Custodian TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs: { ensure: { verb: entity.ensure-or-placeholder, when: empty }, assign: { verb: cbu.assign-role, when: placeholder }, search: { verb: party.search, when: [placeholder, filled] }, add: { verb: party.add, when: empty }, show: { verb: entity.read, when: filled } }
  authorized_participant:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: authorized-participant }
    cardinality: mandatory
    depends_on: [cbu]
    placeholder: Authorized Participant TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs: { ensure: { verb: entity.ensure-or-placeholder, when: empty }, assign: { verb: cbu.assign-role, when: placeholder }, search: { verb: party.search, when: [placeholder, filled] }, add: { verb: party.add, when: empty }, show: { verb: entity.read, when: filled } }
  sub_adviser:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: sub-adviser }
    cardinality: optional
    depends_on: [cbu]
    placeholder: Sub-Adviser TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs: { ensure: { verb: entity.ensure-or-placeholder, when: empty }, assign: { verb: cbu.assign-role, when: placeholder }, search: { verb: party.search, when: [placeholder, filled] }, add: { verb: party.add, when: empty }, show: { verb: entity.read, when: filled } }
  administrator:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: administrator }
    cardinality: optional
    depends_on: [cbu]
    placeholder: Administrator TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs: { ensure: { verb: entity.ensure-or-placeholder, when: empty }, assign: { verb: cbu.assign-role, when: placeholder }, search: { verb: party.search, when: [placeholder, filled] }, add: { verb: party.add, when: empty }, show: { verb: entity.read, when: filled } }
  transfer_agent:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: transfer-agent }
    cardinality: optional
    depends_on: [cbu]
    placeholder: Transfer Agent TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs: { ensure: { verb: entity.ensure-or-placeholder, when: empty }, assign: { verb: cbu.assign-role, when: placeholder }, search: { verb: party.search, when: [placeholder, filled] }, add: { verb: party.add, when: empty }, show: { verb: entity.read, when: filled } }
  distributor:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: distributor }
    cardinality: optional
    depends_on: [cbu]
    placeholder: Distributor TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs: { ensure: { verb: entity.ensure-or-placeholder, when: empty }, assign: { verb: cbu.assign-role, when: placeholder }, search: { verb: party.search, when: [placeholder, filled] }, add: { verb: party.add, when: empty }, show: { verb: entity.read, when: filled } }
  auditor:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: auditor }
    cardinality: optional
    depends_on: [cbu]
    placeholder: Auditor TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs: { ensure: { verb: entity.ensure-or-placeholder, when: empty }, assign: { verb: cbu.assign-role, when: placeholder }, search: { verb: party.search, when: [placeholder, filled] }, add: { verb: party.add, when: empty }, show: { verb: entity.read, when: filled } }
  market_maker:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: market-maker }
    cardinality: optional
    depends_on: [cbu]
    placeholder: Market Maker TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs: { ensure: { verb: entity.ensure-or-placeholder, when: empty }, assign: { verb: cbu.assign-role, when: placeholder }, search: { verb: party.search, when: [placeholder, filled] }, add: { verb: party.add, when: empty }, show: { verb: entity.read, when: filled } }
  ownership_chain:
    type: entity_graph
    entity_kinds: [person, company]
    join: { via: entity_relationships, parent_fk: from_entity_id, child_fk: to_entity_id }
    cardinality: recursive
    max_depth: 5
    depends_on: [investment_adviser]
    state_machine: ubo_epistemic_lifecycle
    overlays: [registry, evidence, screenings]
    edge_overlays: [ownership]
    verbs: { discover: ubo.discover, allege: ubo.allege, verify: ubo.verify, promote: ubo.promote, approve: ubo.approve, reject: ubo.reject }
  case:
    type: case
    table: cases
    pk: case_id
    join: { via: cases, parent_fk: cbu_id, child_fk: case_id }
    cardinality: optional
    depends_on: [investment_adviser]
    state_machine: kyc_case_lifecycle
    verbs: { open: case.open, submit: case.submit, approve: case.approve, reject: case.reject, request_info: case.request-info }
    children:
      tollgate:
        type: tollgate
        table: tollgate_evaluations
        pk: evaluation_id
        join: { via: tollgate_evaluations, parent_fk: case_id, child_fk: evaluation_id }
        cardinality: optional
        depends_on: [{ slot: case, min_state: intake }]
        verbs: { evaluate: tollgate.evaluate }
  mandate:
    type: mandate
    table: cbu_trading_profiles
    pk: profile_id
    join: { via: cbu_trading_profiles, parent_fk: cbu_id, child_fk: profile_id }
    cardinality: optional
    depends_on: [{ slot: cbu, min_state: filled }, { slot: case, min_state: intake }]
    verbs: { create: mandate.create }
bulk_macros: [role_slots]
```

---
## File: rust/config/sem_os_seeds/constellation_maps/struct_us_private_fund_delaware_lp.yaml
```yaml
constellation: struct.us.private-fund.delaware-lp
description: United States private fund Delaware LP onboarding constellation
jurisdiction: US
slots:
  cbu:
    type: cbu
    table: cbus
    pk: cbu_id
    cardinality: root
    verbs: { create: cbu.create, read: cbu.read, show: cbu.show }
  general_partner:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: general-partner }
    cardinality: mandatory
    depends_on: [cbu]
    placeholder: GP TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs: { ensure: { verb: entity.ensure-or-placeholder, when: empty }, assign: { verb: cbu.assign-role, when: placeholder }, search: { verb: party.search, when: [placeholder, filled] }, add: { verb: party.add, when: empty }, show: { verb: entity.read, when: filled } }
  investment_manager:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: investment-manager }
    cardinality: mandatory
    depends_on: [cbu]
    placeholder: Investment Manager TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs: { ensure: { verb: entity.ensure-or-placeholder, when: empty }, assign: { verb: cbu.assign-role, when: placeholder }, search: { verb: party.search, when: [placeholder, filled] }, add: { verb: party.add, when: empty }, show: { verb: entity.read, when: filled } }
  custodian:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: custodian }
    cardinality: optional
    depends_on: [cbu]
    placeholder: Custodian TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs: { ensure: { verb: entity.ensure-or-placeholder, when: empty }, assign: { verb: cbu.assign-role, when: placeholder }, search: { verb: party.search, when: [placeholder, filled] }, add: { verb: party.add, when: empty }, show: { verb: entity.read, when: filled } }
  administrator:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: administrator }
    cardinality: optional
    depends_on: [cbu]
    placeholder: Administrator TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs: { ensure: { verb: entity.ensure-or-placeholder, when: empty }, assign: { verb: cbu.assign-role, when: placeholder }, search: { verb: party.search, when: [placeholder, filled] }, add: { verb: party.add, when: empty }, show: { verb: entity.read, when: filled } }
  prime_broker:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: prime-broker }
    cardinality: optional
    depends_on: [cbu]
    placeholder: Prime Broker TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs: { ensure: { verb: entity.ensure-or-placeholder, when: empty }, assign: { verb: cbu.assign-role, when: placeholder }, search: { verb: party.search, when: [placeholder, filled] }, add: { verb: party.add, when: empty }, show: { verb: entity.read, when: filled } }
  auditor:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: auditor }
    cardinality: optional
    depends_on: [cbu]
    placeholder: Auditor TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs: { ensure: { verb: entity.ensure-or-placeholder, when: empty }, assign: { verb: cbu.assign-role, when: placeholder }, search: { verb: party.search, when: [placeholder, filled] }, add: { verb: party.add, when: empty }, show: { verb: entity.read, when: filled } }
  legal_counsel:
    type: entity
    entity_kinds: [company]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: legal-counsel }
    cardinality: optional
    depends_on: [cbu]
    placeholder: Legal Counsel TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs: { ensure: { verb: entity.ensure-or-placeholder, when: empty }, assign: { verb: cbu.assign-role, when: placeholder }, search: { verb: party.search, when: [placeholder, filled] }, add: { verb: party.add, when: empty }, show: { verb: entity.read, when: filled } }
  tax_advisor:
    type: entity
    entity_kinds: [company, person]
    join: { via: cbu_entity_roles, parent_fk: cbu_id, child_fk: entity_id, filter_column: role, filter_value: tax-advisor }
    cardinality: optional
    depends_on: [cbu]
    placeholder: Tax Advisor TBD
    state_machine: entity_kyc_lifecycle
    overlays: [entity_ref, workstream, screenings, evidence, red_flags, doc_requests]
    verbs: { ensure: { verb: entity.ensure-or-placeholder, when: empty }, assign: { verb: cbu.assign-role, when: placeholder }, search: { verb: party.search, when: [placeholder, filled] }, add: { verb: party.add, when: empty }, show: { verb: entity.read, when: filled } }
  ownership_chain:
    type: entity_graph
    entity_kinds: [person, company]
    join: { via: entity_relationships, parent_fk: from_entity_id, child_fk: to_entity_id }
    cardinality: recursive
    max_depth: 5
    depends_on: [general_partner]
    state_machine: ubo_epistemic_lifecycle
    overlays: [registry, evidence, screenings]
    edge_overlays: [ownership]
    verbs: { discover: ubo.discover, allege: ubo.allege, verify: ubo.verify, promote: ubo.promote, approve: ubo.approve, reject: ubo.reject }
  case:
    type: case
    table: cases
    pk: case_id
    join: { via: cases, parent_fk: cbu_id, child_fk: case_id }
    cardinality: optional
    depends_on: [general_partner]
    state_machine: kyc_case_lifecycle
    verbs: { open: case.open, submit: case.submit, approve: case.approve, reject: case.reject, request_info: case.request-info }
    children:
      tollgate:
        type: tollgate
        table: tollgate_evaluations
        pk: evaluation_id
        join: { via: tollgate_evaluations, parent_fk: case_id, child_fk: evaluation_id }
        cardinality: optional
        depends_on: [{ slot: case, min_state: intake }]
        verbs: { evaluate: tollgate.evaluate }
  mandate:
    type: mandate
    table: cbu_trading_profiles
    pk: profile_id
    join: { via: cbu_trading_profiles, parent_fk: cbu_id, child_fk: profile_id }
    cardinality: optional
    depends_on: [{ slot: cbu, min_state: filled }, { slot: case, min_state: intake }]
    verbs: { create: mandate.create }
bulk_macros: [role_slots]
```

---
## File: rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml
```yaml
constellation: trading.streetside
description: Street-side operations constellation — trading profiles, custody, settlement, booking principals, service delivery. Post-KYC approval operations.
jurisdiction: ALL
slots:
  cbu:
    type: cbu
    table: cbus
    pk: cbu_id
    cardinality: root
    verbs:
      read: { verb: cbu.read, when: filled }
  trading_profile:
    type: mandate
    table: cbu_trading_profiles
    pk: profile_id
    join: { via: cbu_trading_profiles, parent_fk: cbu_id, child_fk: profile_id }
    cardinality: optional
    depends_on: [cbu]
    state_machine: trading_profile_lifecycle
    verbs:
      import: { verb: trading-profile.import, when: empty }
      create_draft: { verb: trading-profile.create-draft, when: [empty, filled] }
      read: { verb: trading-profile.read, when: filled }
      get_active: { verb: trading-profile.get-active, when: filled }
      list_versions: { verb: trading-profile.list-versions, when: filled }
      materialize: { verb: trading-profile.materialize, when: filled }
      activate: { verb: trading-profile.activate, when: filled }
      diff: { verb: trading-profile.diff, when: filled }
      clone: { verb: trading-profile.clone-to, when: filled }
      new_version: { verb: trading-profile.create-new-version, when: filled }
      add_component: { verb: trading-profile.add-component, when: filled }
      remove_component: { verb: trading-profile.remove-component, when: filled }
      set_base_currency: { verb: trading-profile.set-base-currency, when: filled }
      link_csa_ssi: { verb: trading-profile.link-csa-ssi, when: filled }
      update_im_scope: { verb: trading-profile.update-im-scope, when: filled }
      ca_add_cutoff_rule: { verb: trading-profile.ca.add-cutoff-rule, when: filled }
      ca_remove_cutoff_rule: { verb: trading-profile.ca.remove-cutoff-rule, when: filled }
      ca_enable_event_types: { verb: trading-profile.ca.enable-event-types, when: filled }
      ca_disable_event_types: { verb: trading-profile.ca.disable-event-types, when: filled }
      ca_set_default_option: { verb: trading-profile.ca.set-default-option, when: filled }
      ca_remove_default_option: { verb: trading-profile.ca.remove-default-option, when: filled }
      ca_link_proceeds_ssi: { verb: trading-profile.ca.link-proceeds-ssi, when: filled }
      ca_remove_proceeds_ssi: { verb: trading-profile.ca.remove-proceeds-ssi, when: filled }
      validate_golive: { verb: trading-profile.validate-go-live-ready, when: filled }
      validate_coverage: { verb: trading-profile.validate-universe-coverage, when: filled }
      submit: { verb: trading-profile.submit, when: filled }
      approve: { verb: trading-profile.approve, when: filled }
      reject: { verb: trading-profile.reject, when: filled }
      archive: { verb: trading-profile.archive, when: filled }
      # Matrix overlay — configuration layer on the trading profile
      overlay_create: { verb: matrix-overlay.create, when: [empty, filled] }
      overlay_read: { verb: matrix-overlay.read, when: filled }
      overlay_list: { verb: matrix-overlay.list, when: filled }
      overlay_update: { verb: matrix-overlay.update, when: filled }
      overlay_apply: { verb: matrix-overlay.apply, when: filled }
      overlay_remove: { verb: matrix-overlay.remove, when: filled }
      overlay_diff: { verb: matrix-overlay.diff, when: filled }
      overlay_preview: { verb: matrix-overlay.preview, when: filled }
      overlay_list_active: { verb: matrix-overlay.list-active, when: filled }
    overlays: [matrix_overlay]
  custody:
    type: entity
    entity_kinds: [cbu]
    join: { via: cbu_custody_profiles, parent_fk: cbu_id, child_fk: custody_id }
    cardinality: optional
    depends_on: [trading_profile]
    verbs:
      list_universe: { verb: custody.list-universe, when: [empty, filled] }
      list_ssis: { verb: custody.list-ssis, when: filled }
      list_booking_rules: { verb: custody.list-booking-rules, when: filled }
      list_overrides: { verb: custody.list-agent-overrides, when: filled }
      derive_coverage: { verb: custody.derive-required-coverage, when: filled }
      validate: { verb: custody.validate-booking-coverage, when: filled }
      lookup_ssi: { verb: custody.lookup-ssi, when: filled }
      setup_ssi: { verb: custody.setup-ssi, when: [empty, filled] }
  booking_principal:
    type: entity
    entity_kinds: [company]
    join: { via: booking_principals, parent_fk: cbu_id, child_fk: principal_id }
    cardinality: optional
    depends_on: [cbu]
    verbs:
      create: { verb: booking-principal.create, when: empty }
      update: { verb: booking-principal.update, when: filled }
      retire: { verb: booking-principal.retire, when: filled }
      evaluate: { verb: booking-principal.evaluate, when: filled }
      select: { verb: booking-principal.select, when: filled }
      explain: { verb: booking-principal.explain, when: filled }
      coverage: { verb: booking-principal.coverage-matrix, when: filled }
      gaps: { verb: booking-principal.gap-report, when: filled }
      impact: { verb: booking-principal.impact-analysis, when: filled }
  cash_sweep:
    type: entity
    entity_kinds: [entity]
    join: { via: cash_sweep_configs, parent_fk: cbu_id, child_fk: sweep_id }
    cardinality: optional
    depends_on: [custody]
    verbs:
      configure: { verb: cash-sweep.configure, when: empty }
      link: { verb: cash-sweep.link-resource, when: filled }
      list: { verb: cash-sweep.list, when: [empty, filled] }
      update_threshold: { verb: cash-sweep.update-threshold, when: filled }
      update_timing: { verb: cash-sweep.update-timing, when: filled }
      change_vehicle: { verb: cash-sweep.change-vehicle, when: filled }
      suspend: { verb: cash-sweep.suspend, when: filled }
      reactivate: { verb: cash-sweep.reactivate, when: filled }
      remove: { verb: cash-sweep.remove, when: filled }
  service_resource:
    type: entity
    entity_kinds: [entity]
    join: { via: service_resources, parent_fk: cbu_id, child_fk: resource_id }
    cardinality: optional
    depends_on: [cbu]
    verbs:
      read: { verb: service-resource.read, when: filled }
      list: { verb: service-resource.list, when: [empty, filled] }
      provision: { verb: service-resource.provision, when: [empty, filled] }
      set_attr: { verb: service-resource.set-attr, when: filled }
      activate: { verb: service-resource.activate, when: filled }
      suspend: { verb: service-resource.suspend, when: filled }
      decommission: { verb: service-resource.decommission, when: filled }
      validate: { verb: service-resource.validate-attrs, when: filled }
  service_intent:
    type: entity
    entity_kinds: [entity]
    join: { via: service_intents, parent_fk: cbu_id, child_fk: intent_id }
    cardinality: optional
    depends_on: [cbu]
    verbs:
      create: { verb: service-intent.create, when: empty }
      read: { verb: service-intent.read, when: filled }
      list: { verb: service-intent.list, when: [empty, filled] }
      update: { verb: service-intent.update, when: filled }
      approve: { verb: service-intent.approve, when: filled }
      reject: { verb: service-intent.reject, when: filled }
      cancel: { verb: service-intent.cancel, when: filled }
      list_available: { verb: service-intent.list-available, when: [empty, filled] }
      list_by_status: { verb: service-intent.list-by-status, when: filled }
      activate: { verb: service-intent.activate, when: filled }
      deactivate: { verb: service-intent.deactivate, when: filled }
      clone: { verb: service-intent.clone, when: filled }
  # matrix_overlay is a configuration layer ON the trading profile,
  # not a separate positioned entity. Verbs moved to trading_profile slot.
  booking_location:
    type: entity
    entity_kinds: [company]
    join: { via: booking_locations, parent_fk: principal_id, child_fk: location_id }
    cardinality: optional
    depends_on: [booking_principal]
    verbs:
      create: { verb: booking-location.create, when: empty }
      read: { verb: booking-location.read, when: filled }
      list: { verb: booking-location.list, when: [empty, filled] }
  legal_entity:
    type: entity
    entity_kinds: [company]
    join: { via: legal_entities, parent_fk: principal_id, child_fk: legal_entity_id }
    cardinality: optional
    depends_on: [booking_principal]
    verbs:
      create: { verb: legal-entity.create, when: empty }
      read: { verb: legal-entity.read, when: filled }
      list: { verb: legal-entity.list, when: [empty, filled] }
  product:
    type: entity
    entity_kinds: [entity]
    join: { via: products, parent_fk: cbu_id, child_fk: product_id }
    cardinality: optional
    depends_on: [cbu]
    verbs:
      create: { verb: product.create, when: empty }
      list: { verb: product.list, when: [empty, filled] }
  delivery:
    type: entity
    entity_kinds: [entity]
    join: { via: delivery_channels, parent_fk: cbu_id, child_fk: channel_id }
    cardinality: optional
    depends_on: [cbu]
    verbs:
      create: { verb: delivery.create, when: empty }
      read: { verb: delivery.read, when: filled }
      list: { verb: delivery.list, when: [empty, filled] }
```

