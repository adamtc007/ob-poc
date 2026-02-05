```mermaid
erDiagram
  agent__entity_aliases {
    uuid entity_id FK
    text alias
    text canonical_name
    numeric_3_2 confidence
    timestamp_with_time_zone created_at
    public_vector_384 embedding
    text embedding_model
    bigint id
    integer occurrence_count
    text source
    timestamp_with_time_zone updated_at
  }
  agent__esper_aliases {
    boolean auto_approved
    text command_key
    numeric_3_2 confidence
    timestamp_with_time_zone created_at
    uuid id
    integer occurrence_count
    text phrase
    text source
    timestamp_with_time_zone updated_at
  }
  agent__events {
    text corrected_dsl
    text correction_type
    integer duration_ms
    jsonb entities_resolved
    text error_message
    text event_type
    boolean execution_success
    text generated_dsl
    bigint id
    text llm_model
    integer llm_tokens_used
    jsonb parsed_intents
    jsonb resolution_failures
    text selected_verb
    uuid session_id
    timestamp_with_time_zone timestamp
    text user_message
    boolean was_corrected
  }
  agent__invocation_phrases {
    numeric_3_2 confidence
    timestamp_with_time_zone created_at
    public_vector_384 embedding
    text embedding_model
    bigint id
    integer occurrence_count
    text phrase
    text source
    timestamp_with_time_zone updated_at
    text verb
  }
  agent__learning_audit {
    bigint candidate_id FK
    text action
    text actor
    boolean can_rollback
    jsonb details
    bigint id
    bigint learning_id
    text learning_type
    jsonb previous_state
    timestamp_with_time_zone timestamp
  }
  agent__learning_candidates {
    timestamp_with_time_zone applied_at
    boolean auto_applicable
    timestamp_with_time_zone collision_check_at
    boolean collision_safe
    text collision_verb
    timestamp_with_time_zone created_at
    text domain_hint
    bigint example_events
    text fingerprint
    timestamp_with_time_zone first_seen
    bigint id
    text input_pattern
    timestamp_with_time_zone last_seen
    timestamp_with_time_zone last_success_at
    text learning_type
    integer occurrence_count
    timestamp_with_time_zone reviewed_at
    text reviewed_by
    text risk_level
    text status
    integer success_count
    text suggested_output
    integer total_count
    timestamp_with_time_zone updated_at
  }
  agent__lexicon_tokens {
    numeric_3_2 confidence
    timestamp_with_time_zone created_at
    bigint id
    integer occurrence_count
    text source
    text token
    text token_subtype
    text token_type
    timestamp_with_time_zone updated_at
  }
  agent__phrase_blocklist {
    text blocked_verb
    timestamp_with_time_zone created_at
    public_vector_384 embedding
    text embedding_model
    timestamp_with_time_zone expires_at
    bigint id
    text phrase
    text reason
    uuid user_id
  }
  agent__stopwords {
    text category
    text word
  }
  agent__user_learned_phrases {
    numeric_3_2 confidence
    timestamp_with_time_zone created_at
    public_vector_384 embedding
    text embedding_model
    bigint id
    integer occurrence_count
    text phrase
    text source
    timestamp_with_time_zone updated_at
    uuid user_id
    text verb
  }
  client_portal__clients {
    uuid employer_entity_id FK
    uuid accessible_cbus
    uuid client_id
    timestamp_with_time_zone created_at
    character_varying_255 email
    character_varying_50 identity_provider
    boolean is_active
    timestamp_with_time_zone last_login_at
    character_varying_255 name
    character_varying_50 offboard_reason
    timestamp_with_time_zone offboarded_at
    character_varying_50 status
    timestamp_with_time_zone updated_at
  }
  client_portal__commitments {
    uuid client_id FK
    uuid commitment_id
    text commitment_text
    timestamp_with_time_zone created_at
    date expected_date
    timestamp_with_time_zone fulfilled_at
    date reminder_date
    timestamp_with_time_zone reminder_sent_at
    uuid request_id
    character_varying_20 status
    timestamp_with_time_zone updated_at
  }
  client_portal__credentials {
    uuid client_id FK
    timestamp_with_time_zone created_at
    text credential_hash
    uuid credential_id
    timestamp_with_time_zone expires_at
    boolean is_active
  }
  client_portal__escalations {
    uuid client_id FK
    uuid session_id FK
    timestamp_with_time_zone assigned_at
    uuid assigned_to_user_id
    uuid cbu_id
    jsonb conversation_context
    timestamp_with_time_zone created_at
    uuid escalation_id
    character_varying_20 preferred_contact
    text reason
    text resolution_notes
    timestamp_with_time_zone resolved_at
    character_varying_20 status
    timestamp_with_time_zone updated_at
  }
  client_portal__sessions {
    uuid client_id FK
    uuid active_cbu_id
    jsonb collection_state
    timestamp_with_time_zone created_at
    timestamp_with_time_zone expires_at
    timestamp_with_time_zone last_active_at
    uuid session_id
  }
  client_portal__submissions {
    uuid client_id FK
    uuid cataloged_document_id
    timestamp_with_time_zone created_at
    character_varying_100 document_type
    character_varying_255 file_name
    text file_reference
    bigint file_size_bytes
    jsonb info_data
    character_varying_100 info_type
    character_varying_100 mime_type
    text note_text
    uuid request_id
    text review_notes
    timestamp_with_time_zone reviewed_at
    uuid reviewed_by
    character_varying_20 status
    uuid submission_id
    character_varying_50 submission_type
    timestamp_with_time_zone updated_at
  }
  custody__ca_event_types {
    text category
    timestamp_with_time_zone created_at
    text default_election
    text event_code
    text event_name
    uuid event_type_id
    boolean is_elective
    text iso_event_code
    timestamp_with_time_zone updated_at
  }
  custody__cbu_ca_instruction_windows {
    uuid cbu_id FK
    uuid event_type_id FK
    uuid market_id FK
    timestamp_with_time_zone created_at
    integer cutoff_days_before
    text escalation_contact
    integer escalation_days
    timestamp_with_time_zone updated_at
    integer warning_days
    uuid window_id
  }
  custody__cbu_ca_preferences {
    uuid cbu_id FK
    uuid event_type_id FK
    uuid instrument_class_id FK
    timestamp_with_time_zone created_at
    text default_election
    text notification_email
    uuid preference_id
    text processing_mode
    text threshold_currency
    numeric_18_4 threshold_value
    timestamp_with_time_zone updated_at
  }
  custody__cbu_ca_ssi_mappings {
    uuid cbu_id FK
    uuid event_type_id FK
    uuid ssi_id FK
    timestamp_with_time_zone created_at
    text currency
    uuid mapping_id
    text proceeds_type
    timestamp_with_time_zone updated_at
  }
  custody__cbu_cash_sweep_config {
    uuid cbu_id FK
    uuid profile_id FK
    uuid sweep_resource_id FK
    timestamp_with_time_zone created_at
    character_varying_3 currency
    date effective_date
    uuid interest_account_id
    character_varying_20 interest_allocation
    boolean is_active
    character_varying_20 sweep_frequency
    uuid sweep_id
    time_without_time_zone sweep_time
    character_varying_50 sweep_timezone
    numeric_18_2 threshold_amount
    character_varying_50 vehicle_id
    character_varying_255 vehicle_name
    character_varying_20 vehicle_type
  }
  custody__cbu_cross_border_config {
    uuid bridge_location_id FK
    uuid cbu_id FK
    uuid source_market_id FK
    uuid target_market_id FK
    integer additional_days
    uuid config_id
    timestamp_with_time_zone created_at
    character_varying_20 fx_timing
    boolean is_active
    character_varying_3 preferred_currency
    character_varying_20 settlement_method
    text special_instructions
    timestamp_with_time_zone updated_at
  }
  custody__cbu_im_assignments {
    uuid cbu_id FK
    uuid instruction_resource_id FK
    uuid manager_entity_id FK
    uuid profile_id FK
    uuid assignment_id
    boolean can_affirm
    boolean can_settle
    boolean can_trade
    timestamp_with_time_zone created_at
    date effective_date
    character_varying_20 instruction_method
    character_varying_11 manager_bic
    character_varying_20 manager_lei
    character_varying_255 manager_name
    character_varying_30 manager_role
    integer priority
    boolean scope_all
    text scope_currencies
    text scope_instrument_classes
    text scope_isda_asset_classes
    text scope_markets
    character_varying_20 status
    date termination_date
    timestamp_with_time_zone updated_at
  }
  custody__cbu_instrument_universe {
    uuid cbu_id FK
    uuid counterparty_entity_id FK
    uuid instrument_class_id FK
    uuid market_id FK
    uuid counterparty_key
    timestamp_with_time_zone created_at
    character_varying_3 currencies
    date effective_date
    boolean is_active
    boolean is_held
    boolean is_traded
    character_varying_10 settlement_types
    uuid universe_id
  }
  custody__cbu_pricing_config {
    uuid cbu_id FK
    uuid instrument_class_id FK
    uuid market_id FK
    uuid pricing_resource_id FK
    uuid profile_id FK
    uuid config_id
    timestamp_with_time_zone created_at
    character_varying_3 currency
    date effective_date
    character_varying_30 fallback_source
    boolean is_active
    integer max_age_hours
    character_varying_20 price_type
    integer priority
    character_varying_30 source
    character_varying_20 stale_action
    numeric_5_2 tolerance_pct
  }
  custody__cbu_settlement_chains {
    uuid cbu_id FK
    uuid instrument_class_id FK
    uuid market_id FK
    uuid chain_id
    character_varying_100 chain_name
    timestamp_with_time_zone created_at
    character_varying_3 currency
    date effective_date
    boolean is_active
    boolean is_default
    text notes
    character_varying_10 settlement_type
    timestamp_with_time_zone updated_at
  }
  custody__cbu_settlement_location_preferences {
    uuid cbu_id FK
    uuid instrument_class_id FK
    uuid market_id FK
    uuid preferred_location_id FK
    timestamp_with_time_zone created_at
    boolean is_active
    uuid preference_id
    integer priority
    text reason
    timestamp_with_time_zone updated_at
  }
  custody__cbu_ssi {
    uuid cbu_id FK
    uuid market_id FK
    character_varying_35 cash_account
    character_varying_11 cash_account_bic
    character_varying_3 cash_currency
    character_varying_35 collateral_account
    character_varying_11 collateral_account_bic
    timestamp_with_time_zone created_at
    character_varying_100 created_by
    character_varying_11 delivering_agent_bic
    date effective_date
    date expiry_date
    character_varying_11 pset_bic
    character_varying_11 receiving_agent_bic
    character_varying_35 safekeeping_account
    character_varying_100 safekeeping_account_name
    character_varying_11 safekeeping_bic
    character_varying_20 source
    character_varying_100 source_reference
    uuid ssi_id
    character_varying_100 ssi_name
    character_varying_20 ssi_type
    character_varying_20 status
    timestamp_with_time_zone updated_at
  }
  custody__cbu_ssi_agent_override {
    uuid ssi_id FK
    character_varying_35 agent_account
    character_varying_11 agent_bic
    character_varying_100 agent_name
    character_varying_10 agent_role
    timestamp_with_time_zone created_at
    boolean is_active
    uuid override_id
    character_varying_255 reason
    integer sequence_order
  }
  custody__cbu_tax_reclaim_config {
    uuid cbu_id FK
    uuid service_provider_entity_id FK
    uuid source_jurisdiction_id FK
    character_varying_20 batch_frequency
    uuid config_id
    timestamp_with_time_zone created_at
    integer expected_recovery_days
    jsonb fee_structure
    boolean is_active
    numeric_15_2 minimum_reclaim_amount
    character_varying_3 minimum_reclaim_currency
    character_varying_20 reclaim_method
    timestamp_with_time_zone updated_at
  }
  custody__cbu_tax_reporting {
    uuid cbu_id FK
    uuid reporting_entity_id FK
    uuid reporting_jurisdiction_id FK
    uuid sponsor_entity_id FK
    timestamp_with_time_zone created_at
    character_varying_30 giin
    boolean is_active
    text notes
    date registration_date
    uuid reporting_id
    character_varying_20 reporting_regime
    character_varying_20 reporting_status
    timestamp_with_time_zone updated_at
  }
  custody__cbu_tax_status {
    uuid cbu_id FK
    uuid tax_jurisdiction_id FK
    numeric_5_3 applicable_treaty_rate
    timestamp_with_time_zone created_at
    character_varying_20 crs_status
    date documentation_expiry
    character_varying_20 documentation_status
    text exempt_reason
    character_varying_20 fatca_status
    character_varying_20 investor_type
    boolean is_active
    character_varying_20 qi_ein
    boolean qualified_intermediary
    uuid status_id
    boolean tax_exempt
    timestamp_with_time_zone updated_at
  }
  custody__cfi_codes {
    uuid class_id FK
    uuid security_type_id FK
    character_1 attribute_1
    character_1 attribute_2
    character_1 attribute_3
    character_1 attribute_4
    character_1 category
    character_varying_50 category_name
    character_6 cfi_code
    timestamp_with_time_zone created_at
    character_2 group_code
    character_varying_50 group_name
  }
  custody__csa_agreements {
    uuid collateral_ssi_id FK
    uuid isda_id FK
    timestamp_with_time_zone created_at
    uuid csa_id
    character_varying_20 csa_type
    date effective_date
    boolean is_active
    numeric_18_2 minimum_transfer_amount
    numeric_18_2 rounding_amount
    numeric_18_2 threshold_amount
    character_varying_3 threshold_currency
    timestamp_with_time_zone updated_at
  }
  custody__entity_settlement_identity {
    uuid entity_id FK
    character_varying_50 alert_participant_id
    timestamp_with_time_zone created_at
    character_varying_50 ctm_participant_id
    uuid identity_id
    boolean is_active
    character_varying_20 lei
    character_varying_11 primary_bic
    timestamp_with_time_zone updated_at
  }
  custody__entity_ssi {
    uuid entity_id FK
    uuid instrument_class_id FK
    uuid market_id FK
    uuid security_type_id FK
    character_varying_11 counterparty_bic
    timestamp_with_time_zone created_at
    character_varying_3 currency
    date effective_date
    uuid entity_ssi_id
    date expiry_date
    character_varying_35 safekeeping_account
    character_varying_20 source
    character_varying_100 source_reference
    character_varying_20 status
    timestamp_with_time_zone updated_at
  }
  custody__instruction_paths {
    uuid instruction_type_id FK
    uuid instrument_class_id FK
    uuid market_id FK
    uuid resource_id FK
    timestamp_with_time_zone created_at
    character_varying_3 currency
    jsonb enrichment_sources
    boolean is_active
    uuid path_id
    integer routing_priority
    timestamp_with_time_zone updated_at
    jsonb validation_rules
  }
  custody__instruction_types {
    timestamp_with_time_zone created_at
    character_varying_10 direction
    boolean is_active
    character_varying_50 iso20022_msg_type
    character_varying_100 name
    character_varying_10 payment_type
    character_varying_10 swift_mt_code
    character_varying_30 type_code
    uuid type_id
  }
  custody__instrument_classes {
    uuid parent_class_id FK
    character_1 cfi_category
    character_2 cfi_group
    uuid class_id
    character_varying_20 code
    timestamp_with_time_zone created_at
    character_varying_10 default_settlement_cycle
    boolean is_active
    character_varying_30 isda_asset_class
    character_varying_100 name
    boolean requires_collateral
    boolean requires_isda
    character_varying_20 smpg_group
    character_varying_10 swift_message_family
    timestamp_with_time_zone updated_at
  }
  custody__isda_agreements {
    uuid cbu_id FK
    uuid counterparty_entity_id FK
    date agreement_date
    timestamp_with_time_zone created_at
    date effective_date
    character_varying_20 governing_law
    boolean is_active
    uuid isda_id
    date termination_date
    timestamp_with_time_zone updated_at
  }
  custody__isda_product_coverage {
    uuid instrument_class_id FK
    uuid isda_id FK
    uuid isda_taxonomy_id FK
    uuid coverage_id
    timestamp_with_time_zone created_at
    boolean is_active
  }
  custody__isda_product_taxonomy {
    uuid class_id FK
    character_varying_30 asset_class
    character_varying_50 base_product
    character_varying_6 cfi_pattern
    timestamp_with_time_zone created_at
    boolean is_active
    character_varying_50 sub_product
    character_varying_100 taxonomy_code
    uuid taxonomy_id
    character_varying_50 upi_template
  }
  custody__markets {
    character_varying_2 country_code
    timestamp_with_time_zone created_at
    character_varying_11 csd_bic
    time_without_time_zone cut_off_time
    boolean is_active
    uuid market_id
    character_varying_4 mic
    character_varying_255 name
    character_varying_4 operating_mic
    character_varying_3 primary_currency
    character_varying_3 supported_currencies
    character_varying_50 timezone
    timestamp_with_time_zone updated_at
  }
  custody__security_types {
    uuid class_id FK
    character_varying_6 cfi_pattern
    character_varying_4 code
    timestamp_with_time_zone created_at
    boolean is_active
    character_varying_100 name
    uuid security_type_id
  }
  custody__settlement_chain_hops {
    uuid chain_id FK
    uuid intermediary_entity_id FK
    uuid ssi_id FK
    character_varying_50 account_number
    timestamp_with_time_zone created_at
    uuid hop_id
    integer hop_sequence
    text instructions
    character_varying_11 intermediary_bic
    character_varying_200 intermediary_name
    character_varying_20 role
    timestamp_with_time_zone updated_at
  }
  custody__settlement_locations {
    character_varying_11 bic
    character_varying_2 country_code
    timestamp_with_time_zone created_at
    boolean is_active
    character_varying_20 location_code
    uuid location_id
    character_varying_200 location_name
    character_varying_20 location_type
    jsonb operating_hours
    jsonb settlement_cycles
    timestamp_with_time_zone updated_at
  }
  custody__ssi_booking_rules {
    uuid cbu_id FK
    uuid counterparty_entity_id FK
    uuid instrument_class_id FK
    uuid market_id FK
    uuid security_type_id FK
    uuid ssi_id FK
    timestamp_with_time_zone created_at
    character_varying_3 currency
    date effective_date
    date expiry_date
    boolean is_active
    character_varying_30 isda_asset_class
    character_varying_50 isda_base_product
    integer priority
    uuid rule_id
    character_varying_100 rule_name
    character_varying_10 settlement_type
    integer specificity_score
    timestamp_with_time_zone updated_at
  }
  custody__subcustodian_network {
    uuid market_id FK
    timestamp_with_time_zone created_at
    character_varying_35 csd_participant_id
    character_varying_3 currency
    date effective_date
    date expiry_date
    boolean is_active
    boolean is_primary
    character_varying_35 local_agent_account
    character_varying_11 local_agent_bic
    character_varying_255 local_agent_name
    uuid network_id
    character_varying_11 place_of_settlement_bic
    character_varying_11 subcustodian_bic
    character_varying_255 subcustodian_name
    timestamp_with_time_zone updated_at
  }
  custody__tax_jurisdictions {
    character_varying_2 country_code
    timestamp_with_time_zone created_at
    numeric_5_3 default_withholding_rate
    jsonb documentation_requirements
    boolean is_active
    character_varying_10 jurisdiction_code
    uuid jurisdiction_id
    character_varying_200 jurisdiction_name
    boolean reclaim_available
    integer reclaim_deadline_days
    character_varying_50 tax_authority_code
    character_varying_200 tax_authority_name
    timestamp_with_time_zone updated_at
  }
  custody__tax_treaty_rates {
    uuid instrument_class_id FK
    uuid investor_jurisdiction_id FK
    uuid source_jurisdiction_id FK
    boolean beneficial_owner_required
    timestamp_with_time_zone created_at
    text documentation_codes
    date effective_date
    date expiry_date
    character_varying_20 income_type
    boolean is_active
    numeric_5_3 standard_rate
    uuid treaty_id
    numeric_5_3 treaty_rate
    character_varying_100 treaty_reference
    timestamp_with_time_zone updated_at
  }
  events__log {
    text event_type
    bigint id
    jsonb payload
    uuid session_id
    timestamp_with_time_zone timestamp
  }
  feedback__audit_log {
    uuid failure_id FK
    feedback_audit_action action
    text actor_id
    feedback_actor_type actor_type
    timestamp_with_time_zone created_at
    jsonb details
    text evidence
    text evidence_hash
    uuid id
    feedback_issue_status new_status
    feedback_issue_status previous_status
  }
  feedback__failures {
    text command_sequence
    timestamp_with_time_zone created_at
    jsonb error_context
    text error_message
    feedback_error_type error_type
    text fingerprint
    smallint fingerprint_version
    timestamp_with_time_zone first_seen_at
    text fix_commit
    text fix_notes
    uuid id
    timestamp_with_time_zone last_seen_at
    integer occurrence_count
    feedback_remediation_path remediation_path
    text repro_path
    text repro_type
    boolean repro_verified
    timestamp_with_time_zone resolved_at
    text source
    feedback_issue_status status
    timestamp_with_time_zone updated_at
    text user_intent
    text verb
  }
  feedback__occurrences {
    uuid failure_id FK
    timestamp_with_time_zone created_at
    bigint duration_ms
    text error_backtrace
    text error_message
    uuid event_id
    timestamp_with_time_zone event_timestamp
    uuid id
    uuid session_id
    text verb
  }
  kyc__approval_requests {
    uuid case_id FK
    uuid workstream_id FK
    uuid approval_id
    character_varying_255 approver
    text comments
    character_varying_20 decision
    timestamp_with_time_zone decision_at
    character_varying_50 request_type
    timestamp_with_time_zone requested_at
    character_varying_255 requested_by
  }
  kyc__bods_right_type_mapping {
    character_varying_50 bods_interest_type
    timestamp_with_time_zone created_at
    boolean maps_to_control
    boolean maps_to_economic
    character_varying_30 maps_to_right_type
    boolean maps_to_voting
    text notes
  }
  kyc__case_events {
    uuid case_id FK
    uuid workstream_id FK
    uuid actor_id
    character_varying_20 actor_type
    text comment
    jsonb event_data
    uuid event_id
    character_varying_50 event_type
    timestamp_with_time_zone occurred_at
  }
  kyc__cases {
    uuid cbu_id FK
    uuid service_agreement_id FK
    uuid sponsor_cbu_id FK
    uuid subject_entity_id FK
    uuid assigned_analyst_id
    uuid assigned_reviewer_id
    uuid case_id
    character_varying_30 case_type
    timestamp_with_time_zone closed_at
    character_varying_30 escalation_level
    character_varying_50 kyc_standard
    timestamp_with_time_zone last_activity_at
    text notes
    timestamp_with_time_zone opened_at
    character_varying_20 risk_rating
    character_varying_50 service_context
    timestamp_with_time_zone sla_deadline
    character_varying_30 status
    timestamp_with_time_zone updated_at
  }
  kyc__dilution_exercise_events {
    uuid instrument_id FK
    uuid resulting_holding_id FK
    timestamp_with_time_zone created_at
    date exercise_date
    uuid exercise_id
    numeric_20_6 exercise_price_paid
    character_varying_100 idempotency_key
    boolean is_cashless
    text notes
    numeric_20_6 shares_issued
    numeric_20_6 shares_withheld_for_tax
    numeric_20_6 units_exercised
  }
  kyc__dilution_instruments {
    uuid converts_to_share_class_id FK
    uuid grant_document_id FK
    uuid holder_entity_id FK
    uuid issuer_entity_id FK
    character_varying_100 board_approval_ref
    numeric_10_4 conversion_ratio
    timestamp_with_time_zone created_at
    numeric_5_2 discount_pct
    date exercisable_from
    character_varying_3 exercise_currency
    numeric_20_6 exercise_price
    date expiration_date
    uuid instrument_id
    character_varying_30 instrument_type
    text notes
    character_varying_100 plan_name
    numeric_20_2 principal_amount
    character_varying_20 status
    numeric_20_6 units_exercised
    numeric_20_6 units_forfeited
    numeric_20_6 units_granted
    timestamp_with_time_zone updated_at
    numeric_20_2 valuation_cap
    integer vesting_cliff_months
    date vesting_end_date
    date vesting_start_date
  }
  kyc__doc_request_acceptable_types {
    uuid document_type_id FK
    uuid request_id FK
    timestamp_with_time_zone created_at
    uuid link_id
  }
  kyc__doc_requests {
    uuid workstream_id FK
    uuid batch_id
    character_varying_50 batch_reference
    character_varying_50 doc_type
    uuid document_id
    date due_date
    character_varying_30 generation_source
    boolean is_mandatory
    character_varying_10 priority
    timestamp_with_time_zone received_at
    text rejection_reason
    uuid request_id
    timestamp_with_time_zone requested_at
    timestamp_with_time_zone required_at
    timestamp_with_time_zone reviewed_at
    uuid reviewer_id
    character_varying_20 status
    text verification_notes
    timestamp_with_time_zone verified_at
  }
  kyc__entity_workstreams {
    uuid blocker_request_id FK
    uuid case_id FK
    uuid discovery_source_workstream_id FK
    uuid entity_id FK
    timestamp_with_time_zone blocked_at
    integer blocked_days_total
    text blocked_reason
    character_varying_500 blocker_message
    character_varying_50 blocker_type
    timestamp_with_time_zone completed_at
    timestamp_with_time_zone created_at
    integer discovery_depth
    character_varying_100 discovery_reason
    boolean is_ubo
    numeric_5_2 ownership_percentage
    boolean requires_enhanced_dd
    jsonb risk_factors
    character_varying_20 risk_rating
    timestamp_with_time_zone started_at
    character_varying_30 status
    timestamp_with_time_zone updated_at
    uuid workstream_id
  }
  kyc__fund_compartments {
    uuid compartment_entity_id FK
    uuid umbrella_fund_entity_id FK
    text compartment_code
    text compartment_name
    timestamp_with_time_zone created_at
    uuid id
    jsonb meta
    timestamp_with_time_zone updated_at
  }
  kyc__fund_vehicles {
    uuid fund_entity_id FK
    uuid manager_entity_id FK
    uuid umbrella_entity_id FK
    timestamp_with_time_zone created_at
    character_varying_100 created_by
    character_2 domicile_country
    boolean is_umbrella
    jsonb meta
    timestamp_with_time_zone updated_at
    character_varying_30 vehicle_type
  }
  kyc__holding_control_links {
    uuid holder_entity_id FK
    uuid issuer_entity_id FK
    uuid share_class_id FK
    date as_of_date
    integer chain_depth
    character_varying_30 control_type
    timestamp_with_time_zone created_at
    numeric_8_4 economic_pct
    boolean is_direct
    uuid link_id
    uuid source_holding_ids
    numeric_5_2 threshold_pct
    numeric_20_6 total_units
    timestamp_with_time_zone updated_at
    numeric_8_4 voting_pct
  }
  kyc__holdings {
    uuid investor_entity_id FK
    uuid investor_id FK
    uuid share_class_id FK
    date acquisition_date
    numeric_20_2 cost_basis
    timestamp_with_time_zone created_at
    character_varying_50 holding_status
    uuid id
    character_varying_50 provider
    character_varying_100 provider_reference
    timestamp_with_time_zone provider_sync_at
    character_varying_50 status
    numeric_20_6 units
    timestamp_with_time_zone updated_at
    character_varying_20 usage_type
  }
  kyc__instrument_identifier_schemes {
    timestamp_with_time_zone created_at
    integer display_order
    character_varying_200 format_regex
    boolean is_global
    character_varying_100 issuing_authority
    character_varying_20 scheme_code
    character_varying_100 scheme_name
    character_varying_500 validation_url
  }
  kyc__investor_lifecycle_history {
    uuid investor_id FK
    character_varying_50 from_state
    uuid history_id
    jsonb metadata
    text notes
    character_varying_50 to_state
    timestamp_with_time_zone transitioned_at
    character_varying_100 triggered_by
  }
  kyc__investor_lifecycle_transitions {
    character_varying_100 auto_trigger
    character_varying_50 from_state
    text requires_document
    boolean requires_kyc_approved
    character_varying_50 to_state
  }
  kyc__investor_role_profiles {
    uuid group_container_entity_id FK
    uuid holder_entity_id FK
    uuid issuer_entity_id FK
    uuid share_class_id FK
    boolean beneficial_owner_data_available
    timestamp_with_time_zone created_at
    character_varying_100 created_by
    date effective_from
    date effective_to
    text group_label
    character_varying_20 holder_affiliation
    uuid id
    boolean is_ubo_eligible
    character_varying_30 lookthrough_policy
    text notes
    character_varying_50 role_type
    character_varying_50 source
    text source_reference
    timestamp_with_time_zone updated_at
  }
  kyc__investors {
    uuid entity_id FK
    uuid owning_cbu_id FK
    timestamp_with_time_zone created_at
    character_varying_50 crs_status
    text eligible_fund_types
    character_varying_50 fatca_status
    timestamp_with_time_zone first_subscription_at
    character_varying_50 investor_category
    uuid investor_id
    character_varying_50 investor_type
    timestamp_with_time_zone kyc_approved_at
    uuid kyc_case_id
    timestamp_with_time_zone kyc_expires_at
    character_varying_20 kyc_risk_rating
    character_varying_50 kyc_status
    text lifecycle_notes
    character_varying_50 lifecycle_state
    timestamp_with_time_zone lifecycle_state_at
    text offboard_reason
    timestamp_with_time_zone offboarded_at
    character_varying_50 pre_suspension_state
    character_varying_50 provider
    character_varying_100 provider_reference
    timestamp_with_time_zone provider_sync_at
    character_varying_50 redemption_type
    text rejection_reason
    text restricted_jurisdictions
    timestamp_with_time_zone suspended_at
    text suspended_reason
    character_varying_10 tax_jurisdiction
    character_varying_50 tax_status
    timestamp_with_time_zone updated_at
  }
  kyc__issuance_events {
    uuid issuer_entity_id FK
    uuid share_class_id FK
    uuid source_document_id FK
    date announcement_date
    character_varying_100 board_resolution_ref
    timestamp_with_time_zone created_at
    character_varying_100 created_by
    date effective_date
    uuid event_id
    character_varying_30 event_type
    character_varying_100 idempotency_key
    text notes
    character_varying_3 price_currency
    numeric_20_6 price_per_unit
    integer ratio_from
    integer ratio_to
    date record_date
    character_varying_100 regulatory_filing_ref
    character_varying_20 status
    numeric_20_2 total_amount
    numeric_20_6 units_delta
  }
  kyc__issuer_control_config {
    uuid issuer_entity_id FK
    boolean applies_voting_caps
    uuid config_id
    character_varying_20 control_basis
    numeric_5_2 control_threshold_pct
    timestamp_with_time_zone created_at
    character_varying_20 disclosure_basis
    numeric_5_2 disclosure_threshold_pct
    date effective_from
    date effective_to
    character_varying_10 jurisdiction
    numeric_5_2 material_threshold_pct
    numeric_5_2 significant_threshold_pct
    character_varying_20 voting_basis
  }
  kyc__movements {
    uuid holding_id FK
    numeric_20_2 amount
    integer call_number
    uuid commitment_id
    timestamp_with_time_zone created_at
    character_3 currency
    character_varying_50 distribution_type
    uuid id
    character_varying_50 movement_type
    text notes
    numeric_20_6 price_per_unit
    character_varying_100 reference
    date settlement_date
    character_varying_50 status
    date trade_date
    numeric_20_6 units
    timestamp_with_time_zone updated_at
  }
  kyc__outreach_requests {
    uuid recipient_entity_id FK
    uuid target_entity_id FK
    timestamp_with_time_zone created_at
    uuid created_by
    date deadline_date
    character_varying_255 recipient_email
    character_varying_255 recipient_name
    timestamp_with_time_zone reminder_sent_at
    uuid request_id
    text request_notes
    character_varying_30 request_type
    text resolution_notes
    uuid response_document_id
    timestamp_with_time_zone response_received_at
    character_varying_30 response_type
    timestamp_with_time_zone sent_at
    character_varying_20 status
    uuid trigger_id
    timestamp_with_time_zone updated_at
  }
  kyc__outstanding_requests {
    uuid case_id FK
    uuid cbu_id FK
    uuid entity_id FK
    uuid workstream_id FK
    text acceptable_alternatives
    character_varying_500 blocker_message
    boolean blocks_subject
    text client_notes
    boolean client_visible
    jsonb communication_log
    text compliance_context
    timestamp_with_time_zone created_at
    uuid created_by_execution_id
    character_varying_100 created_by_verb
    date due_date
    timestamp_with_time_zone escalated_at
    uuid escalated_to_user_id
    integer escalation_level
    character_varying_255 escalation_reason
    timestamp_with_time_zone fulfilled_at
    uuid fulfilled_by_user_id
    text fulfillment_notes
    uuid fulfillment_reference_id
    character_varying_50 fulfillment_reference_type
    character_varying_50 fulfillment_type
    integer grace_period_days
    timestamp_with_time_zone last_reminder_at
    integer max_reminders
    text reason_for_request
    integer reminder_count
    jsonb request_details
    uuid request_id
    character_varying_100 request_subtype
    character_varying_50 request_type
    timestamp_with_time_zone requested_at
    boolean requested_by_agent
    uuid requested_by_user_id
    uuid requested_from_entity_id
    character_varying_255 requested_from_label
    character_varying_50 requested_from_type
    character_varying_50 status
    text status_reason
    uuid subject_id
    character_varying_50 subject_type
    timestamp_with_time_zone updated_at
  }
  kyc__ownership_reconciliation_findings {
    uuid owner_entity_id FK
    uuid run_id FK
    timestamp_with_time_zone created_at
    integer delta_bps
    uuid finding_id
    character_varying_30 finding_type
    text resolution_notes
    character_varying_20 resolution_status
    timestamp_with_time_zone resolved_at
    character_varying_100 resolved_by
    character_varying_10 severity
    numeric_8_4 source_a_pct
    numeric_8_4 source_b_pct
  }
  kyc__ownership_reconciliation_runs {
    uuid issuer_entity_id FK
    date as_of_date
    character_varying_20 basis
    timestamp_with_time_zone completed_at
    integer matched_count
    integer mismatched_count
    integer missing_in_a_count
    integer missing_in_b_count
    text notes
    uuid run_id
    character_varying_20 source_a
    character_varying_20 source_b
    timestamp_with_time_zone started_at
    character_varying_20 status
    integer tolerance_bps
    integer total_entities
    character_varying_100 triggered_by
  }
  kyc__ownership_snapshots {
    uuid issuer_entity_id FK
    uuid owner_entity_id FK
    uuid share_class_id FK
    uuid superseded_by FK
    date as_of_date
    character_varying_20 basis
    character_varying_20 confidence
    timestamp_with_time_zone created_at
    numeric_20_6 denominator
    character_varying_20 derived_from
    boolean is_aggregated
    boolean is_direct
    numeric_20_6 numerator
    numeric_8_4 percentage
    numeric_8_4 percentage_max
    numeric_8_4 percentage_min
    uuid snapshot_id
    character_varying_100 source_bods_statement_id
    uuid source_document_id
    uuid source_gleif_rel_id
    uuid source_holding_ids
    timestamp_with_time_zone superseded_at
    numeric_20_6 units
  }
  kyc__red_flags {
    uuid case_id FK
    uuid workstream_id FK
    text description
    character_varying_50 flag_type
    timestamp_with_time_zone raised_at
    uuid raised_by
    uuid red_flag_id
    text resolution_notes
    character_varying_30 resolution_type
    timestamp_with_time_zone resolved_at
    uuid resolved_by
    timestamp_with_time_zone reviewed_at
    uuid reviewed_by
    character_varying_20 severity
    character_varying_50 source
    text source_reference
    character_varying_20 status
    uuid waiver_approved_by
    text waiver_justification
  }
  kyc__research_actions {
    uuid decision_id FK
    uuid target_entity_id FK
    uuid action_id
    character_varying_50 action_type
    integer api_calls_made
    integer duration_ms
    integer entities_created
    integer entities_updated
    character_varying_50 error_code
    text error_message
    timestamp_with_time_zone executed_at
    uuid executed_by
    jsonb fields_updated
    boolean is_rolled_back
    integer relationships_created
    text rollback_reason
    timestamp_with_time_zone rolled_back_at
    uuid rolled_back_by
    uuid session_id
    character_varying_100 source_key
    character_varying_20 source_key_type
    character_varying_30 source_provider
    boolean success
    jsonb verb_args
    character_varying_30 verb_domain
    character_varying_50 verb_name
  }
  kyc__research_anomalies {
    uuid action_id FK
    uuid entity_id FK
    text actual_value
    uuid anomaly_id
    text description
    timestamp_with_time_zone detected_at
    text expected_value
    text resolution
    timestamp_with_time_zone resolved_at
    uuid resolved_by
    character_varying_50 rule_code
    character_varying_10 severity
    character_varying_20 status
  }
  kyc__research_confidence_config {
    numeric_3_2 ambiguous_threshold
    numeric_3_2 auto_proceed_threshold
    text checkpoint_contexts
    uuid config_id
    date effective_from
    date effective_to
    integer max_auto_imports_per_session
    integer max_chain_depth
    numeric_3_2 reject_threshold
    boolean require_human_checkpoint
    character_varying_30 source_provider
  }
  kyc__research_corrections {
    uuid new_action_id FK
    uuid original_action_id FK
    uuid original_decision_id FK
    character_varying_100 correct_key
    character_varying_20 correct_key_type
    timestamp_with_time_zone corrected_at
    uuid corrected_by
    uuid correction_id
    text correction_reason
    character_varying_20 correction_type
    character_varying_100 wrong_key
    character_varying_20 wrong_key_type
  }
  kyc__research_decisions {
    uuid target_entity_id FK
    boolean auto_selected
    integer candidates_count
    jsonb candidates_found
    timestamp_with_time_zone created_at
    uuid decision_id
    character_varying_20 decision_type
    uuid resulting_action_id
    jsonb search_context
    text search_query
    character_varying_100 selected_key
    character_varying_20 selected_key_type
    numeric_3_2 selection_confidence
    text selection_reasoning
    uuid session_id
    character_varying_30 source_provider
    uuid trigger_id
    timestamp_with_time_zone verified_at
    uuid verified_by
  }
  kyc__rule_executions {
    uuid case_id FK
    uuid workstream_id FK
    jsonb actions_executed
    boolean condition_matched
    jsonb context_snapshot
    timestamp_with_time_zone executed_at
    uuid execution_id
    character_varying_100 rule_name
    character_varying_50 trigger_event
  }
  kyc__screenings {
    uuid red_flag_id FK
    uuid workstream_id FK
    timestamp_with_time_zone completed_at
    timestamp_with_time_zone expires_at
    integer match_count
    character_varying_50 provider
    timestamp_with_time_zone requested_at
    jsonb result_data
    character_varying_100 result_summary
    text review_notes
    timestamp_with_time_zone reviewed_at
    uuid reviewed_by
    uuid screening_id
    character_varying_30 screening_type
    character_varying_20 status
  }
  kyc__share_class_identifiers {
    character_varying_20 scheme_code FK
    uuid share_class_id FK
    timestamp_with_time_zone created_at
    uuid identifier_id
    character_varying_100 identifier_value
    boolean is_primary
    character_varying_50 source
    date valid_from
    date valid_to
    timestamp_with_time_zone verified_at
  }
  kyc__share_class_supply {
    uuid share_class_id FK
    date as_of_date
    uuid as_of_event_id
    numeric_20_6 authorized_units
    timestamp_with_time_zone created_at
    numeric_20_6 issued_units
    numeric_20_6 outstanding_units
    numeric_20_6 reserved_units
    uuid supply_id
    numeric_20_6 treasury_units
    timestamp_with_time_zone updated_at
  }
  kyc__share_classes {
    uuid cbu_id FK
    uuid compartment_id FK
    uuid converts_to_share_class_id FK
    uuid entity_id FK
    uuid issuer_entity_id FK
    character_varying_20 class_category
    character_varying_3 commitment_currency
    numeric_20_6 conversion_price
    numeric_10_4 conversion_ratio_num
    timestamp_with_time_zone created_at
    character_3 currency
    numeric_10_4 dividend_rate
    numeric_10_4 economic_per_unit
    character_varying_50 fund_structure
    character_varying_50 fund_type
    numeric_5_2 gate_percentage
    boolean high_water_mark
    numeric_5_2 hurdle_rate
    uuid id
    character_varying_30 instrument_kind
    character_varying_30 instrument_type
    character_varying_50 investor_eligibility
    boolean is_carried_interest
    character_varying_12 isin
    integer liquidation_rank
    integer lock_up_period_months
    integer management_fee_bps
    numeric_20_2 minimum_investment
    character_varying_255 name
    date nav_date
    numeric_20_6 nav_per_share
    integer performance_fee_bps
    character_varying_50 redemption_frequency
    integer redemption_notice_days
    character_varying_50 status
    character_varying_50 subscription_frequency
    timestamp_with_time_zone updated_at
    integer vintage_year
    numeric_10_4 votes_per_unit
    numeric_5_2 voting_cap_pct
    numeric_5_2 voting_threshold_pct
  }
  kyc__special_rights {
    uuid holder_entity_id FK
    uuid issuer_entity_id FK
    uuid share_class_id FK
    uuid source_document_id FK
    character_varying_20 board_seat_type
    integer board_seats
    timestamp_with_time_zone created_at
    date effective_from
    date effective_to
    text notes
    boolean requires_class_vote
    uuid right_id
    character_varying_30 right_type
    character_varying_50 source_clause_ref
    character_varying_20 source_type
    character_varying_20 threshold_basis
    numeric_5_2 threshold_pct
  }
  ob_poc__attribute_dictionary {
    character_varying_100 attr_id
    character_varying_255 attr_name
    uuid attribute_id
    timestamp_with_time_zone created_at
    character_varying_50 data_type
    text description
    character_varying_50 domain
    boolean is_active
    boolean is_required
    character_varying_255 validation_pattern
  }
  ob_poc__attribute_observations {
    uuid attribute_id FK
    uuid entity_id FK
    uuid source_document_id FK
    uuid source_screening_id FK
    uuid source_workstream_id FK
    uuid superseded_by FK
    numeric_3_2 confidence
    timestamp_with_time_zone created_at
    date effective_from
    date effective_to
    character_varying_50 extraction_method
    boolean is_authoritative
    uuid observation_id
    timestamp_with_time_zone observed_at
    text observed_by
    jsonb source_metadata
    text source_reference
    character_varying_30 source_type
    character_varying_30 status
    timestamp_with_time_zone superseded_at
    timestamp_with_time_zone updated_at
    boolean value_boolean
    date value_date
    timestamp_with_time_zone value_datetime
    jsonb value_json
    numeric value_number
    text value_text
  }
  ob_poc__attribute_registry {
    numeric_3_2 acceptable_variation_threshold
    jsonb applicability
    text category
    timestamp_with_time_zone created_at
    text default_value
    text display_name
    character_varying_100 domain
    public_vector_1536 embedding
    character_varying_100 embedding_model
    timestamp_with_time_zone embedding_updated_at
    character_varying_100 group_id
    text id
    boolean is_required
    jsonb metadata
    jsonb reconciliation_rules
    boolean requires_authoritative_source
    timestamp_with_time_zone updated_at
    uuid uuid
    jsonb validation_rules
    text value_type
  }
  ob_poc__attribute_values_typed {
    text attribute_id FK
    uuid attribute_uuid FK
    timestamp_with_time_zone created_at
    text created_by
    timestamp_with_time_zone effective_from
    timestamp_with_time_zone effective_to
    uuid entity_id
    integer id
    jsonb source
    boolean value_boolean
    date value_date
    timestamp_with_time_zone value_datetime
    bigint value_integer
    jsonb value_json
    numeric value_number
    text value_text
  }
  ob_poc__board_control_evidence {
    uuid cbu_board_controller_id FK
    date as_of
    timestamp_with_time_zone created_at
    text description
    jsonb details
    uuid id
    text source_id
    text source_register
    text source_type
  }
  ob_poc__bods_entity_statements {
    character_varying_100 company_number
    timestamp_with_time_zone created_at
    character_varying_50 entity_type
    jsonb identifiers
    character_varying_10 jurisdiction
    character_varying_20 lei
    text name
    character_varying_200 opencorporates_id
    character_varying_100 source_register
    text source_url
    date statement_date
    character_varying_100 statement_id
  }
  ob_poc__bods_entity_types {
    text description
    character_varying_100 display_name
    integer display_order
    jsonb subtypes
    character_varying_30 type_code
  }
  ob_poc__bods_interest_types {
    boolean bods_standard
    character_varying_30 category
    timestamp_with_time_zone created_at
    text description
    character_varying_100 display_name
    integer display_order
    boolean requires_percentage
    character_varying_50 type_code
  }
  ob_poc__bods_ownership_statements {
    character_varying_50 control_types
    timestamp_with_time_zone created_at
    date end_date
    text interested_party_name
    character_varying_100 interested_party_statement_id
    character_varying_20 interested_party_type
    boolean is_direct
    character_varying_50 ownership_type
    numeric share_exact
    numeric share_max
    numeric share_min
    text source_description
    character_varying_100 source_register
    date start_date
    date statement_date
    character_varying_100 statement_id
    character_varying_100 subject_entity_statement_id
    character_varying_20 subject_lei
    text subject_name
  }
  ob_poc__bods_person_statements {
    jsonb addresses
    date birth_date
    character_varying_20 birth_date_precision
    character_varying_10 country_of_residence
    timestamp_with_time_zone created_at
    date death_date
    character_varying_200 family_name
    text full_name
    character_varying_200 given_name
    jsonb names
    character_varying_10 nationalities
    character_varying_50 person_type
    character_varying_100 source_register
    date statement_date
    character_varying_100 statement_id
    character_varying_10 tax_residencies
  }
  ob_poc__case_decision_thresholds {
    timestamp_with_time_zone created_at
    text description
    character_varying_30 escalation_level
    boolean has_hard_stop
    boolean is_active
    integer max_score
    integer min_score
    character_varying_50 recommended_action
    uuid threshold_id
    character_varying_100 threshold_name
  }
  ob_poc__case_evaluation_snapshots {
    uuid case_id FK
    uuid matched_threshold_id FK
    character_varying_50 decision_made
    timestamp_with_time_zone decision_made_at
    character_varying_255 decision_made_by
    text decision_notes
    integer escalate_count
    integer escalate_score
    timestamp_with_time_zone evaluated_at
    character_varying_255 evaluated_by
    integer hard_stop_count
    boolean has_hard_stop
    integer mitigated_flags
    text notes
    integer open_flags
    character_varying_50 recommended_action
    character_varying_30 required_escalation_level
    uuid snapshot_id
    integer soft_count
    integer soft_score
    integer total_score
    integer waived_flags
  }
  ob_poc__case_types {
    character_varying_50 code
    text description
    integer display_order
    boolean is_active
    character_varying_100 name
  }
  ob_poc__cbu_attr_values {
    uuid attr_id FK
    uuid cbu_id FK
    timestamp_with_time_zone as_of
    timestamp_with_time_zone created_at
    jsonb evidence_refs
    jsonb explain_refs
    text source
    timestamp_with_time_zone updated_at
    jsonb value
  }
  ob_poc__cbu_board_controller {
    uuid cbu_id FK
    uuid controller_entity_id FK
    date as_of
    timestamp_with_time_zone computed_at
    text computed_by
    text confidence
    text controller_name
    jsonb explanation
    uuid id
    text method
    numeric_3_2 score
  }
  ob_poc__cbu_change_log {
    uuid cbu_id FK
    uuid case_id
    character_varying_50 change_type
    timestamp_with_time_zone changed_at
    character_varying_255 changed_by
    uuid evidence_ids
    character_varying_100 field_name
    uuid log_id
    jsonb new_value
    jsonb old_value
    text reason
  }
  ob_poc__cbu_control_anchors {
    uuid cbu_id FK
    uuid entity_id FK
    text anchor_role
    timestamp_with_time_zone created_at
    text display_name
    uuid id
    text jurisdiction
    timestamp_with_time_zone updated_at
  }
  ob_poc__cbu_creation_log {
    uuid cbu_id FK
    text ai_instruction
    timestamp_with_time_zone created_at
    text generated_dsl
    uuid log_id
    text nature_purpose
    text source_of_funds
  }
  ob_poc__cbu_entity_roles {
    uuid cbu_id FK
    uuid entity_id FK
    uuid role_id FK
    uuid target_entity_id FK
    character_varying_3 authority_currency
    numeric_18_2 authority_limit
    uuid cbu_entity_role_id
    timestamp_with_time_zone created_at
    date effective_from
    date effective_to
    numeric_5_2 ownership_percentage
    boolean requires_co_signatory
    timestamp_with_time_zone updated_at
  }
  ob_poc__cbu_entity_roles_history {
    uuid cbu_entity_role_id
    uuid cbu_id
    timestamp_with_time_zone changed_at
    uuid changed_by
    timestamp_with_time_zone created_at
    date effective_from
    date effective_to
    uuid entity_id
    uuid history_id
    character_varying_10 operation
    numeric_5_2 ownership_percentage
    uuid role_id
    uuid target_entity_id
    timestamp_with_time_zone updated_at
  }
  ob_poc__cbu_evidence {
    uuid cbu_id FK
    uuid document_id FK
    timestamp_with_time_zone attached_at
    character_varying_255 attached_by
    character_varying_255 attestation_ref
    text description
    character_varying_50 evidence_category
    uuid evidence_id
    character_varying_50 evidence_type
    text verification_notes
    character_varying_30 verification_status
    timestamp_with_time_zone verified_at
    character_varying_255 verified_by
  }
  ob_poc__cbu_group_members {
    uuid cbu_id FK
    uuid group_id FK
    timestamp_with_time_zone created_at
    integer display_order
    date effective_from
    date effective_to
    uuid membership_id
    character_varying_30 source
  }
  ob_poc__cbu_groups {
    uuid manco_entity_id FK
    uuid ultimate_parent_entity_id FK
    timestamp_with_time_zone created_at
    character_varying_100 created_by
    text description
    date effective_from
    date effective_to
    character_varying_50 group_code
    uuid group_id
    character_varying_255 group_name
    character_varying_30 group_type
    boolean is_auto_derived
    character_varying_10 jurisdiction
    timestamp_with_time_zone updated_at
  }
  ob_poc__cbu_layout_overrides {
    uuid cbu_id
    jsonb positions
    jsonb sizes
    timestamp_with_time_zone updated_at
    uuid user_id
    text view_mode
  }
  ob_poc__cbu_lifecycle_instances {
    uuid cbu_id FK
    uuid resource_type_id FK
    timestamp_with_time_zone activated_at
    jsonb config
    uuid counterparty_entity_id
    timestamp_with_time_zone created_at
    character_varying_3 currency
    timestamp_with_time_zone decommissioned_at
    jsonb depends_on_urls
    uuid instance_id
    character_varying_255 instance_identifier
    character_varying_500 instance_url
    uuid market_id
    character_varying_100 provider_account
    character_varying_11 provider_bic
    character_varying_50 provider_code
    timestamp_with_time_zone provisioned_at
    character_varying_50 status
    timestamp_with_time_zone suspended_at
    timestamp_with_time_zone updated_at
  }
  ob_poc__cbu_matrix_product_overlay {
    uuid cbu_id FK
    uuid counterparty_entity_id FK
    uuid instrument_class_id FK
    uuid market_id FK
    uuid subscription_id FK
    jsonb additional_resources
    jsonb additional_services
    jsonb additional_slas
    timestamp_with_time_zone created_at
    character_varying_3 currency
    uuid overlay_id
    jsonb product_specific_config
    character_varying_20 status
    timestamp_with_time_zone updated_at
  }
  ob_poc__cbu_product_subscriptions {
    uuid cbu_id FK
    uuid product_id FK
    jsonb config
    timestamp_with_time_zone created_at
    date effective_from
    date effective_to
    character_varying_20 status
    uuid subscription_id
    timestamp_with_time_zone updated_at
  }
  ob_poc__cbu_relationship_verification {
    uuid cbu_id FK
    uuid proof_document_id FK
    uuid relationship_id FK
    character_varying_100 allegation_source
    timestamp_with_time_zone alleged_at
    uuid alleged_by
    numeric_5_2 alleged_percentage
    timestamp_with_time_zone created_at
    text discrepancy_notes
    numeric_5_2 observed_percentage
    text resolution_notes
    timestamp_with_time_zone resolved_at
    uuid resolved_by
    character_varying_20 status
    timestamp_with_time_zone updated_at
    uuid verification_id
  }
  ob_poc__cbu_resource_instances {
    uuid cbu_id FK
    uuid counterparty_entity_id FK
    uuid last_request_id FK
    uuid market_id FK
    uuid product_id FK
    uuid resource_type_id FK
    uuid service_id FK
    timestamp_with_time_zone activated_at
    timestamp_with_time_zone created_at
    character_varying_3 currency
    timestamp_with_time_zone decommissioned_at
    jsonb instance_config
    uuid instance_id
    character_varying_255 instance_identifier
    character_varying_255 instance_name
    character_varying_1024 instance_url
    timestamp_with_time_zone last_event_at
    text owner_ticket_id
    character_varying_50 provider_code
    jsonb provider_config
    timestamp_with_time_zone provisioned_at
    timestamp_with_time_zone requested_at
    text resource_url
    text srdef_id
    character_varying_50 status
    timestamp_with_time_zone updated_at
  }
  ob_poc__cbu_service_contexts {
    uuid cbu_id FK
    date effective_date
    character_varying_50 service_context
  }
  ob_poc__cbu_service_readiness {
    uuid cbu_id FK
    uuid product_id FK
    uuid service_id FK
    jsonb active_srids
    timestamp_with_time_zone as_of
    jsonb blocking_reasons
    boolean is_stale
    timestamp_with_time_zone last_recomputed_at
    text recomputation_trigger
    jsonb required_srdefs
    text status
  }
  ob_poc__cbu_sla_commitments {
    uuid bound_resource_instance_id FK
    uuid bound_service_id FK
    uuid cbu_id FK
    uuid profile_id FK
    uuid source_document_id FK
    uuid template_id FK
    uuid bound_csa_id
    uuid bound_isda_id
    uuid commitment_id
    timestamp_with_time_zone created_at
    date effective_date
    jsonb incentive_structure
    character_varying_255 negotiated_by
    date negotiated_date
    numeric_10_4 override_target_value
    numeric_10_4 override_warning_threshold
    jsonb penalty_structure
    uuid scope_counterparties
    text scope_currencies
    text scope_instrument_classes
    text scope_markets
    character_varying_20 status
    date termination_date
    timestamp_with_time_zone updated_at
  }
  ob_poc__cbu_subscriptions {
    uuid cbu_id FK
    uuid contract_id FK
    character_varying_50 product_code FK
    timestamp_with_time_zone created_at
    character_varying_20 status
    timestamp_with_time_zone subscribed_at
    timestamp_with_time_zone updated_at
  }
  ob_poc__cbu_trading_profiles {
    uuid cbu_id FK
    uuid source_document_id FK
    timestamp_with_time_zone activated_at
    character_varying_255 activated_by
    timestamp_with_time_zone created_at
    character_varying_255 created_by
    jsonb document
    text document_hash
    text materialization_hash
    character_varying_20 materialization_status
    timestamp_with_time_zone materialized_at
    text notes
    uuid profile_id
    timestamp_with_time_zone rejected_at
    character_varying_255 rejected_by
    text rejection_reason
    uuid sla_profile_id
    character_varying_20 status
    timestamp_with_time_zone submitted_at
    character_varying_255 submitted_by
    timestamp_with_time_zone superseded_at
    integer superseded_by_version
    timestamp_with_time_zone validated_at
    character_varying_255 validated_by
    integer version
  }
  ob_poc__cbu_unified_attr_requirements {
    uuid attr_id FK
    uuid cbu_id FK
    jsonb conflict
    timestamp_with_time_zone created_at
    jsonb merged_constraints
    text preferred_source
    jsonb required_by_srdefs
    text requirement_strength
    timestamp_with_time_zone updated_at
  }
  ob_poc__cbus {
    uuid commercial_client_entity_id FK
    uuid product_id FK
    character_varying_50 cbu_category
    uuid cbu_id
    character_varying_100 client_type
    timestamp_with_time_zone created_at
    text description
    public_vector_1536 embedding
    character_varying_100 embedding_model
    timestamp_with_time_zone embedding_updated_at
    character_varying_50 jurisdiction
    character_varying_50 kyc_scope_template
    character_varying_255 name
    text nature_purpose
    jsonb onboarding_context
    jsonb risk_context
    jsonb semantic_context
    text source_of_funds
    character_varying_30 status
    timestamp_with_time_zone updated_at
  }
  ob_poc__client_allegations {
    uuid attribute_id FK
    uuid case_id FK
    uuid cbu_id FK
    uuid entity_id FK
    uuid verified_by_observation_id FK
    uuid workstream_id FK
    uuid allegation_id
    text allegation_reference
    character_varying_50 allegation_source
    timestamp_with_time_zone alleged_at
    text alleged_by
    jsonb alleged_value
    text alleged_value_display
    timestamp_with_time_zone created_at
    timestamp_with_time_zone updated_at
    text verification_notes
    character_varying_30 verification_result
    character_varying_30 verification_status
    timestamp_with_time_zone verified_at
    text verified_by
  }
  ob_poc__client_group {
    text canonical_name
    timestamp_with_time_zone created_at
    text description
    timestamp_with_time_zone discovery_completed_at
    character_varying_20 discovery_root_lei
    character_varying_50 discovery_source
    timestamp_with_time_zone discovery_started_at
    character_varying_20 discovery_status
    integer entity_count
    uuid id
    integer pending_review_count
    text short_code
    timestamp_with_time_zone updated_at
  }
  ob_poc__client_group_alias {
    uuid group_id FK
    text alias
    text alias_norm
    double_precision confidence
    timestamp_with_time_zone created_at
    uuid id
    boolean is_primary
    text source
  }
  ob_poc__client_group_alias_embedding {
    uuid alias_id FK
    timestamp_with_time_zone created_at
    integer dimension
    text embedder_id
    public_vector_384 embedding
    boolean normalize
    text pooling
  }
  ob_poc__client_group_anchor {
    uuid anchor_entity_id FK
    uuid group_id FK
    text anchor_role
    double_precision confidence
    timestamp_with_time_zone created_at
    uuid id
    text jurisdiction
    text notes
    integer priority
    date valid_from
    date valid_to
  }
  ob_poc__client_group_anchor_role {
    text default_for_domains
    text description
    text role_code
  }
  ob_poc__client_group_entity {
    uuid cbu_id FK
    uuid entity_id FK
    uuid group_id FK
    text added_by
    timestamp_with_time_zone created_at
    uuid id
    text membership_type
    text notes
    text review_notes
    character_varying_20 review_status
    timestamp_with_time_zone reviewed_at
    character_varying_100 reviewed_by
    character_varying_255 source_record_id
    timestamp_with_time_zone updated_at
  }
  ob_poc__client_group_entity_roles {
    uuid cge_id FK
    uuid role_id FK
    uuid target_entity_id FK
    text assigned_by
    timestamp_with_time_zone created_at
    date effective_from
    date effective_to
    uuid id
    character_varying_255 source_record_id
    timestamp_with_time_zone updated_at
  }
  ob_poc__client_group_entity_tag {
    uuid entity_id FK
    uuid group_id FK
    double_precision confidence
    timestamp_with_time_zone created_at
    text created_by
    uuid id
    text persona
    text source
    text tag
    text tag_norm
  }
  ob_poc__client_group_entity_tag_embedding {
    uuid tag_id FK
    timestamp_with_time_zone created_at
    integer dimension
    text embedder_id
    public_vector_384 embedding
    boolean normalize
    text pooling
  }
  ob_poc__client_group_relationship {
    uuid child_entity_id FK
    uuid group_id FK
    uuid parent_entity_id FK
    uuid promoted_to_relationship_id FK
    timestamp_with_time_zone created_at
    date effective_from
    date effective_to
    uuid id
    timestamp_with_time_zone promoted_at
    character_varying_30 relationship_kind
    text review_notes
    character_varying_20 review_status
    timestamp_with_time_zone reviewed_at
    character_varying_100 reviewed_by
    timestamp_with_time_zone updated_at
  }
  ob_poc__client_group_relationship_sources {
    uuid relationship_id FK
    uuid verifies_source_id FK
    text canonical_notes
    timestamp_with_time_zone canonical_set_at
    character_varying_100 canonical_set_by
    numeric_3_2 confidence_score
    numeric_5_2 control_pct
    timestamp_with_time_zone created_at
    numeric_5_2 discrepancy_pct
    uuid id
    boolean is_canonical
    boolean is_direct_evidence
    numeric_5_2 ownership_pct
    jsonb raw_payload
    character_varying_50 source
    date source_document_date
    character_varying_255 source_document_ref
    character_varying_100 source_document_type
    date source_effective_date
    timestamp_with_time_zone source_retrieved_at
    character_varying_100 source_retrieved_by
    character_varying_20 source_type
    timestamp_with_time_zone updated_at
    text verification_notes
    character_varying_20 verification_outcome
    character_varying_20 verification_status
    timestamp_with_time_zone verified_at
    character_varying_100 verified_by
    numeric_5_2 voting_pct
  }
  ob_poc__client_types {
    character_varying_50 code
    text description
    integer display_order
    boolean is_active
    character_varying_100 name
  }
  ob_poc__contract_products {
    uuid contract_id FK
    timestamp_with_time_zone created_at
    date effective_date
    character_varying_50 product_code
    uuid rate_card_id
    date termination_date
    timestamp_with_time_zone updated_at
  }
  ob_poc__control_edges {
    uuid from_entity_id FK
    uuid to_entity_id FK
    text bods_interest_type
    timestamp_with_time_zone created_at
    text created_by
    text edge_type
    date effective_date
    date end_date
    text gleif_relationship_type
    uuid id
    boolean is_beneficial
    boolean is_direct
    boolean is_legal
    numeric_5_2 percentage
    text psc_category
    uuid share_class_id
    uuid source_document_id
    text source_reference
    text source_register
    timestamp_with_time_zone updated_at
    numeric_10_4 votes_per_share
  }
  ob_poc__crud_operations {
    uuid parent_operation_id FK
    jsonb affected_records
    numeric_3_2 ai_confidence
    text ai_instruction
    character_varying_100 ai_model
    character_varying_50 ai_provider
    character_varying_50 asset_type
    timestamp_with_time_zone completed_at
    timestamp_with_time_zone created_at
    character_varying_255 created_by
    character_varying_100 entity_table_name
    text error_message
    character_varying_20 execution_status
    integer execution_time_ms
    text generated_dsl
    uuid operation_id
    character_varying_20 operation_type
    integer rows_affected
    uuid transaction_id
  }
  ob_poc__csg_validation_rules {
    timestamp_with_time_zone created_at
    character_varying_255 created_by
    text description
    text documentation_url
    timestamp_with_time_zone effective_from
    timestamp_with_time_zone effective_until
    character_varying_10 error_code
    text error_message_template
    boolean is_active
    text rationale
    character_varying_100 rule_code
    uuid rule_id
    character_varying_255 rule_name
    jsonb rule_params
    character_varying_50 rule_type
    integer rule_version
    character_varying_20 severity
    text suggestion_template
    character_varying_100 target_code
    character_varying_50 target_type
    timestamp_with_time_zone updated_at
  }
  ob_poc__currencies {
    timestamp_with_time_zone created_at
    uuid currency_id
    integer decimal_places
    boolean is_active
    character_varying_3 iso_code
    character_varying_100 name
    character_varying_10 symbol
  }
  ob_poc__delegation_relationships {
    uuid applies_to_cbu_id FK
    uuid contract_doc_id FK
    uuid delegate_entity_id FK
    uuid delegator_entity_id FK
    timestamp_with_time_zone created_at
    text delegation_description
    uuid delegation_id
    text delegation_scope
    date effective_from
    date effective_to
    date regulatory_approval_date
    boolean regulatory_approval_required
    date regulatory_notification_date
  }
  ob_poc__detected_patterns {
    uuid case_id FK
    uuid cbu_id FK
    text description
    timestamp_with_time_zone detected_at
    jsonb evidence
    uuid involved_entities
    uuid pattern_id
    character_varying_50 pattern_type
    text resolution_notes
    timestamp_with_time_zone resolved_at
    character_varying_100 resolved_by
    character_varying_20 severity
    character_varying_20 status
  }
  ob_poc__dictionary {
    uuid attribute_id
    timestamp_with_time_zone created_at
    character_varying_100 domain
    character_varying_100 group_id
    text long_description
    character_varying_50 mask
    character_varying_255 name
    jsonb sink
    jsonb source
    timestamp_with_time_zone updated_at
    text vector
  }
  ob_poc__document_attribute_links {
    uuid attribute_id FK
    uuid document_type_id FK
    uuid alternative_doc_types
    text client_types
    timestamp_with_time_zone created_at
    character_varying_10 direction
    text entity_types
    numeric_3_2 extraction_confidence_default
    jsonb extraction_field_path
    jsonb extraction_hints
    character_varying_50 extraction_method
    boolean is_active
    boolean is_authoritative
    text jurisdictions
    uuid link_id
    text notes
    character_varying_20 proof_strength
    timestamp_with_time_zone updated_at
  }
  ob_poc__document_attribute_mappings {
    uuid attribute_uuid FK
    uuid document_type_id FK
    numeric_3_2 confidence_threshold
    timestamp_with_time_zone created_at
    character_varying_50 extraction_method
    jsonb field_location
    character_varying_255 field_name
    boolean is_required
    uuid mapping_id
    timestamp_with_time_zone updated_at
    text validation_pattern
  }
  ob_poc__document_catalog {
    uuid cbu_id FK
    uuid document_type_id FK
    uuid entity_id FK
    timestamp_with_time_zone created_at
    uuid doc_id
    uuid document_id
    character_varying_255 document_name
    character_varying_100 document_type_code
    jsonb extracted_data
    numeric_5_4 extraction_confidence
    character_varying_50 extraction_status
    text file_hash_sha256
    bigint file_size_bytes
    timestamp_with_time_zone last_extracted_at
    jsonb metadata
    character_varying_100 mime_type
    character_varying_100 source_system
    character_varying_50 status
    text storage_key
    timestamp_with_time_zone updated_at
  }
  ob_poc__document_types {
    jsonb applicability
    character_varying_100 category
    timestamp_with_time_zone created_at
    text description
    character_varying_200 display_name
    character_varying_100 domain
    public_vector_768 embedding
    character_varying_100 embedding_model
    timestamp_with_time_zone embedding_updated_at
    jsonb required_attributes
    jsonb semantic_context
    character_varying_100 type_code
    uuid type_id
    timestamp_with_time_zone updated_at
  }
  ob_poc__document_validity_rules {
    uuid document_type_id FK
    text applies_to_entity_types
    text applies_to_jurisdictions
    timestamp_with_time_zone created_at
    boolean is_hard_requirement
    text notes
    character_varying_200 regulatory_source
    uuid rule_id
    jsonb rule_parameters
    character_varying_50 rule_type
    character_varying_20 rule_unit
    integer rule_value
    integer warning_days
  }
  ob_poc__dsl_domains {
    boolean active
    character_varying_20 base_grammar_version
    timestamp_with_time_zone created_at
    text description
    uuid domain_id
    character_varying_100 domain_name
    timestamp_with_time_zone updated_at
    character_varying_20 vocabulary_version
  }
  ob_poc__dsl_examples {
    character_varying_50 asset_type
    character_varying_20 complexity_level
    timestamp_with_time_zone created_at
    character_varying_255 created_by
    text description
    character_varying_100 entity_table_name
    text example_dsl
    uuid example_id
    text expected_outcome
    timestamp_with_time_zone last_used_at
    text natural_language_input
    character_varying_20 operation_type
    numeric_3_2 success_rate
    text tags
    character_varying_255 title
    timestamp_with_time_zone updated_at
    integer usage_count
  }
  ob_poc__dsl_execution_log {
    uuid version_id FK
    character_varying_255 cbu_id
    timestamp_with_time_zone completed_at
    integer duration_ms
    jsonb error_details
    character_varying_255 executed_by
    uuid execution_id
    character_varying_50 execution_phase
    jsonb performance_metrics
    jsonb result_data
    timestamp_with_time_zone started_at
    character_varying_50 status
    bytea verb_hashes
    text verb_names
  }
  ob_poc__dsl_generation_log {
    uuid instance_id FK
    bigint intent_feedback_id FK
    uuid affected_entity_ids
    uuid cbu_id
    timestamp_with_time_zone completed_at
    timestamp_with_time_zone created_at
    character_varying_50 domain_name
    timestamp_with_time_zone executed_at
    text execution_error
    ob_poc_execution_status execution_status
    text final_valid_dsl
    jsonb iterations
    uuid log_id
    character_varying_100 model_used
    uuid session_id
    boolean success
    integer total_attempts
    integer total_input_tokens
    integer total_latency_ms
    integer total_output_tokens
    text user_intent
  }
  ob_poc__dsl_graph_contexts {
    text context_code
    timestamp_with_time_zone created_at
    text description
    text label
    integer priority
  }
  ob_poc__dsl_idempotency {
    uuid actor_id
    character_varying_20 actor_type
    text args_hash
    timestamp_with_time_zone created_at
    uuid execution_id
    text idempotency_key
    uuid input_selection
    jsonb input_view_state
    jsonb output_view_state
    uuid request_id
    bigint result_affected
    uuid result_id
    jsonb result_json
    text result_type
    character_varying_30 source
    integer statement_index
    text verb
    bytea verb_hash
  }
  ob_poc__dsl_instance_versions {
    uuid instance_id FK
    jsonb ast_json
    character_varying_50 compilation_status
    timestamp_with_time_zone created_at
    text dsl_content
    character_varying_100 operation_type
    integer total_refs
    integer unresolved_count
    uuid version_id
    integer version_number
  }
  ob_poc__dsl_instances {
    character_varying_255 business_reference
    character_varying_255 case_id
    timestamp_with_time_zone created_at
    integer current_version
    character_varying_100 domain
    character_varying_100 domain_name
    text dsl_content
    integer id
    uuid instance_id
    character_varying_100 operation_type
    bigint processing_time_ms
    character_varying_50 status
    timestamp_with_time_zone updated_at
  }
  ob_poc__dsl_ob {
    uuid cbu_id FK
    timestamp_with_time_zone created_at
    text dsl_text
    uuid version_id
  }
  ob_poc__dsl_session_events {
    uuid session_id FK
    text dsl_source
    text error_message
    uuid event_id
    character_varying_30 event_type
    jsonb metadata
    timestamp_with_time_zone occurred_at
  }
  ob_poc__dsl_session_locks {
    uuid session_id FK
    timestamp_with_time_zone lock_timeout_at
    timestamp_with_time_zone locked_at
    character_varying_50 operation
  }
  ob_poc__dsl_sessions {
    uuid cbu_id FK
    uuid kyc_case_id FK
    uuid onboarding_request_id FK
    character_varying_50 client_type
    timestamp_with_time_zone completed_at
    timestamp_with_time_zone created_at
    jsonb current_view_state
    integer error_count
    timestamp_with_time_zone expires_at
    character_varying_10 jurisdiction
    timestamp_with_time_zone last_activity_at
    text last_error
    timestamp_with_time_zone last_error_at
    jsonb named_refs
    character_varying_30 primary_domain
    uuid session_id
    character_varying_20 status
    timestamp_with_time_zone view_updated_at
  }
  ob_poc__dsl_snapshots {
    uuid session_id FK
    jsonb bindings_captured
    text domains_used
    character_varying_64 dsl_checksum
    text dsl_source
    jsonb entities_created
    timestamp_with_time_zone executed_at
    integer execution_ms
    uuid snapshot_id
    boolean success
    integer version
  }
  ob_poc__dsl_verb_categories {
    text category_code
    timestamp_with_time_zone created_at
    text description
    integer display_order
    text label
  }
  ob_poc__dsl_verb_sync_log {
    integer duration_ms
    text error_message
    text source_hash
    uuid sync_id
    timestamp_with_time_zone synced_at
    integer verbs_added
    integer verbs_removed
    integer verbs_unchanged
    integer verbs_updated
  }
  ob_poc__dsl_verbs {
    text behavior
    text category
    timestamp_with_time_zone compiled_at
    bytea compiled_hash
    jsonb compiled_json
    character_varying_50 compiler_version
    jsonb consumes
    timestamp_with_time_zone created_at
    text description
    jsonb diagnostics_json
    text domain
    jsonb effective_config_json
    text example_dsl
    text example_short
    text full_name
    text graph_contexts
    text intent_patterns
    text lifecycle_entity_arg
    text produces_subtype
    text produces_type
    text requires_states
    text search_text
    text source
    text transitions_to
    text typical_next
    timestamp_with_time_zone updated_at
    uuid verb_id
    text verb_name
    text workflow_phases
    text yaml_hash
    text yaml_intent_patterns
  }
  ob_poc__dsl_versions {
    uuid domain_id FK
    uuid parent_version_id FK
    timestamp_with_time_zone activated_at
    text change_description
    character_varying_50 compilation_status
    timestamp_with_time_zone compiled_at
    timestamp_with_time_zone created_at
    character_varying_255 created_by
    text dsl_source_code
    character_varying_100 functional_state
    uuid version_id
    integer version_number
  }
  ob_poc__dsl_view_state_changes {
    text idempotency_key FK
    uuid session_id FK
    uuid audit_user_id
    uuid change_id
    timestamp_with_time_zone created_at
    jsonb refinements
    uuid request_id
    uuid selection
    integer selection_count
    character_varying_30 source
    integer stack_depth
    jsonb taxonomy_context
    character_varying_100 verb_name
    jsonb view_state_snapshot
  }
  ob_poc__dsl_workflow_phases {
    timestamp_with_time_zone created_at
    text description
    text label
    text phase_code
    integer phase_order
    text transitions_to
  }
  ob_poc__edge_types {
    character_varying_30 arrow_style
    character_varying_30 bundle_group
    character_varying_10 cardinality
    timestamp_with_time_zone created_at
    boolean creates_kyc_obligation
    integer cycle_break_priority
    text description
    character_varying_100 display_name
    character_varying_30 edge_color
    character_varying_30 edge_style
    character_varying_50 edge_type_code
    numeric_3_1 edge_width
    jsonb from_node_types
    numeric_6_1 ideal_length
    boolean is_control
    boolean is_hierarchical
    boolean is_ownership
    character_varying_50 is_primary_parent_rule
    boolean is_service_delivery
    boolean is_structural
    boolean is_trading
    character_varying_20 label_position
    character_varying_100 label_template
    character_varying_20 layout_direction
    numeric_4_1 parallel_edge_offset
    integer routing_priority
    character_varying_20 self_loop_position
    numeric_4_1 self_loop_radius
    boolean show_in_fund_structure_view
    boolean show_in_product_view
    boolean show_in_service_view
    boolean show_in_trading_view
    boolean show_in_ubo_view
    boolean shows_label
    boolean shows_percentage
    character_varying_30 sibling_sort_key
    integer sort_order
    character_varying_20 source_anchor
    numeric_4_3 spring_strength
    character_varying_20 target_anchor
    integer tier_delta
    jsonb to_node_types
    timestamp_with_time_zone updated_at
    integer z_order
  }
  ob_poc__entities {
    uuid entity_type_id FK
    character_varying_30 bods_entity_subtype
    character_varying_30 bods_entity_type
    timestamp_with_time_zone created_at
    date dissolution_date
    uuid entity_id
    character_varying_255 external_id
    date founding_date
    boolean is_publicly_listed
    character_varying_255 name
    text name_norm
    timestamp_with_time_zone updated_at
  }
  ob_poc__entity_addresses {
    uuid entity_id FK
    uuid address_id
    text address_lines
    character_varying_50 address_type
    character_varying_200 city
    character_varying_3 country
    timestamp_with_time_zone created_at
    boolean is_primary
    character_varying_10 language
    character_varying_50 postal_code
    character_varying_50 region
    character_varying_50 source
    timestamp_with_time_zone updated_at
  }
  ob_poc__entity_bods_links {
    character_varying_100 bods_entity_statement_id FK
    uuid entity_id FK
    timestamp_with_time_zone created_at
    uuid link_id
    numeric match_confidence
    character_varying_50 match_method
  }
  ob_poc__entity_concept_link {
    uuid entity_id FK
    text concept_id
    timestamp_with_time_zone created_at
    text provenance
    text relation
    real weight
  }
  ob_poc__entity_cooperatives {
    uuid entity_id FK
    uuid cooperative_id
    character_varying_255 cooperative_name
    character_varying_50 cooperative_type
    timestamp_with_time_zone created_at
    date formation_date
    character_varying_100 jurisdiction
    integer member_count
    text registered_address
    character_varying_100 registration_number
    timestamp_with_time_zone updated_at
  }
  ob_poc__entity_crud_rules {
    text constraint_description
    character_varying_50 constraint_type
    timestamp_with_time_zone created_at
    character_varying_100 entity_table_name
    text error_message
    character_varying_100 field_name
    boolean is_active
    character_varying_20 operation_type
    uuid rule_id
    timestamp_with_time_zone updated_at
    character_varying_500 validation_pattern
  }
  ob_poc__entity_feature {
    uuid entity_id FK
    text source
    text token_norm
    real weight
  }
  ob_poc__entity_foundations {
    uuid entity_id FK
    timestamp_with_time_zone created_at
    date establishment_date
    uuid foundation_id
    character_varying_255 foundation_name
    text foundation_purpose
    character_varying_50 foundation_type
    character_varying_100 governing_law
    character_varying_100 jurisdiction
    text registered_address
    character_varying_100 registration_number
    timestamp_with_time_zone updated_at
  }
  ob_poc__entity_funds {
    uuid entity_id FK
    uuid master_fund_id FK
    uuid parent_fund_id FK
    date authorization_date
    character_varying_3 base_currency
    timestamp_with_time_zone created_at
    character_varying_5 financial_year_end
    text fund_structure_type
    text fund_type
    character_varying_20 gleif_category
    character_varying_30 gleif_corroboration_level
    timestamp_with_time_zone gleif_last_update
    character_varying_10 gleif_legal_form_id
    character_varying_20 gleif_managing_lou
    character_varying_100 gleif_registered_as
    character_varying_20 gleif_registered_at
    character_varying_20 gleif_status
    character_varying_100 hq_address_city
    character_varying_2 hq_address_country
    date incorporation_date
    text investment_objective
    text investor_type
    character_varying_12 isin_base
    character_varying_10 jurisdiction
    date launch_date
    character_varying_100 legal_address_city
    character_varying_2 legal_address_country
    character_varying_20 lei
    character_varying_100 registration_number
    character_varying_100 regulator
    text regulatory_status
    timestamp_with_time_zone updated_at
  }
  ob_poc__entity_government {
    uuid entity_id FK
    character_varying_3 country_code
    timestamp_with_time_zone created_at
    character_varying_255 entity_name
    date establishment_date
    character_varying_255 governing_authority
    uuid government_id
    character_varying_50 government_type
    text registered_address
    timestamp_with_time_zone updated_at
  }
  ob_poc__entity_identifiers {
    uuid entity_id FK
    timestamp_with_time_zone created_at
    uuid identifier_id
    character_varying_30 identifier_type
    character_varying_100 identifier_value
    boolean is_primary
    boolean is_validated
    character_varying_100 issuing_authority
    date lei_initial_registration
    timestamp_with_time_zone lei_last_update
    character_varying_100 lei_managing_lou
    date lei_next_renewal
    character_varying_30 lei_status
    character_varying_100 scheme_name
    character_varying_50 source
    timestamp_with_time_zone updated_at
    character_varying_500 uri
    date valid_from
    date valid_until
    timestamp_with_time_zone validated_at
    jsonb validation_details
    character_varying_100 validation_source
  }
  ob_poc__entity_lifecycle_events {
    uuid entity_id FK
    jsonb affected_fields
    timestamp_with_time_zone created_at
    date effective_date
    uuid event_id
    character_varying_30 event_status
    character_varying_50 event_type
    jsonb new_values
    jsonb old_values
    date recorded_date
    character_varying_50 source
    character_varying_20 successor_lei
    text successor_name
    character_varying_50 validation_documents
    text validation_reference
  }
  ob_poc__entity_limited_companies {
    uuid entity_id FK
    text business_nature
    character_varying_255 company_name
    timestamp_with_time_zone created_at
    character_varying_20 direct_parent_lei
    date entity_creation_date
    character_varying_20 fund_manager_lei
    character_varying_30 fund_type
    character_varying_50 gleif_category
    character_varying_50 gleif_direct_parent_exception
    timestamp_with_time_zone gleif_last_update
    date gleif_next_renewal
    character_varying_20 gleif_status
    character_varying_50 gleif_subcategory
    character_varying_50 gleif_ultimate_parent_exception
    character_varying_30 gleif_validation_level
    text headquarters_address
    character_varying_200 headquarters_city
    character_varying_3 headquarters_country
    date incorporation_date
    boolean is_fund
    character_varying_100 jurisdiction
    character_varying_10 legal_form_code
    character_varying_200 legal_form_text
    character_varying_20 lei
    uuid limited_company_id
    character_varying_20 master_fund_lei
    text registered_address
    character_varying_100 registration_number
    character_varying_30 ubo_status
    character_varying_20 ultimate_parent_lei
    character_varying_20 umbrella_fund_lei
    timestamp_with_time_zone updated_at
  }
  ob_poc__entity_manco {
    uuid entity_id FK
    date authorization_date
    character_varying_10 authorized_jurisdiction
    boolean can_manage_aif
    boolean can_manage_ucits
    timestamp_with_time_zone created_at
    character_varying_20 lei
    text manco_type
    text passported_jurisdictions
    character_varying_100 regulator
    numeric_15_2 regulatory_capital_eur
    character_varying_100 regulatory_reference
    timestamp_with_time_zone updated_at
  }
  ob_poc__entity_names {
    uuid entity_id FK
    timestamp_with_time_zone created_at
    date effective_from
    date effective_to
    boolean is_primary
    character_varying_10 language
    text name
    uuid name_id
    character_varying_50 name_type
    character_varying_50 source
    timestamp_with_time_zone updated_at
  }
  ob_poc__entity_parent_relationships {
    uuid child_entity_id FK
    uuid parent_entity_id FK
    character_varying_20 accounting_standard
    timestamp_with_time_zone created_at
    character_varying_20 parent_lei
    text parent_name
    date relationship_end
    uuid relationship_id
    date relationship_start
    character_varying_30 relationship_status
    character_varying_50 relationship_type
    character_varying_50 source
    timestamp_with_time_zone updated_at
    text validation_reference
    character_varying_50 validation_source
  }
  ob_poc__entity_partnerships {
    uuid entity_id FK
    timestamp_with_time_zone created_at
    date formation_date
    character_varying_100 jurisdiction
    date partnership_agreement_date
    uuid partnership_id
    character_varying_255 partnership_name
    character_varying_100 partnership_type
    text principal_place_business
    timestamp_with_time_zone updated_at
  }
  ob_poc__entity_proper_persons {
    uuid entity_id FK
    timestamp_with_time_zone created_at
    date date_of_birth
    character_varying_255 first_name
    character_varying_100 id_document_number
    character_varying_100 id_document_type
    character_varying_255 last_name
    character_varying_255 middle_names
    character_varying_100 nationality
    character_varying_20 person_state
    uuid proper_person_id
    text residence_address
    text search_name
    timestamp_with_time_zone updated_at
  }
  ob_poc__entity_regulatory_profiles {
    uuid entity_id FK
    character_varying_20 regulator_code FK
    character_varying_20 regulatory_tier FK
    timestamp_with_time_zone created_at
    boolean is_regulated
    date next_verification_due
    character_varying_100 registration_number
    boolean registration_verified
    timestamp_with_time_zone updated_at
    date verification_date
    character_varying_50 verification_method
    character_varying_500 verification_reference
  }
  ob_poc__entity_relationships {
    uuid from_entity_id FK
    uuid to_entity_id FK
    uuid component_of_relationship_id
    character_varying_30 control_type
    timestamp_with_time_zone created_at
    uuid created_by
    character_varying_10 direct_or_indirect
    date effective_from
    date effective_to
    character_varying_50 interest_type
    boolean is_component
    boolean is_regulated
    text notes
    character_varying_30 ownership_type
    numeric_5_2 percentage
    character_varying_20 regulatory_jurisdiction
    uuid relationship_id
    character_varying_30 relationship_type
    uuid replaces_relationship_id
    boolean share_exclusive_maximum
    boolean share_exclusive_minimum
    numeric_5_2 share_maximum
    numeric_5_2 share_minimum
    character_varying_100 source
    character_varying_255 source_document_ref
    date statement_date
    text trust_class_description
    character_varying_30 trust_interest_type
    character_varying_30 trust_role
    timestamp_with_time_zone updated_at
  }
  ob_poc__entity_relationships_history {
    text change_reason
    timestamp_with_time_zone changed_at
    uuid changed_by
    character_varying_30 control_type
    timestamp_with_time_zone created_at
    uuid created_by
    date effective_from
    date effective_to
    uuid from_entity_id
    uuid history_id
    character_varying_50 interest_type
    boolean is_regulated
    text notes
    character_varying_10 operation
    character_varying_30 ownership_type
    numeric_5_2 percentage
    character_varying_20 regulatory_jurisdiction
    uuid relationship_id
    character_varying_30 relationship_type
    character_varying_100 source
    character_varying_255 source_document_ref
    uuid superseded_by
    uuid to_entity_id
    text trust_class_description
    character_varying_30 trust_interest_type
    character_varying_30 trust_role
    timestamp_with_time_zone updated_at
  }
  ob_poc__entity_share_classes {
    uuid entity_id FK
    uuid parent_fund_id FK
    timestamp_with_time_zone created_at
    character_varying_3 currency
    text distribution_type
    date hard_close_date
    boolean is_hedged
    character_varying_12 isin
    date launch_date
    integer management_fee_bps
    numeric_18_2 minimum_investment
    numeric_5_2 performance_fee_pct
    character_varying_20 share_class_code
    text share_class_type
    date soft_close_date
    timestamp_with_time_zone updated_at
  }
  ob_poc__entity_trusts {
    uuid entity_id FK
    timestamp_with_time_zone created_at
    date establishment_date
    character_varying_100 governing_law
    character_varying_100 jurisdiction
    date trust_deed_date
    uuid trust_id
    character_varying_255 trust_name
    text trust_purpose
    character_varying_100 trust_type
    timestamp_with_time_zone updated_at
  }
  ob_poc__entity_type_dependencies {
    text condition_expr
    timestamp_with_time_zone created_at
    uuid dependency_id
    character_varying_20 dependency_kind
    character_varying_50 from_subtype
    character_varying_50 from_type
    boolean is_active
    integer priority
    character_varying_50 to_subtype
    character_varying_50 to_type
    timestamp_with_time_zone updated_at
    character_varying_100 via_arg
  }
  ob_poc__entity_types {
    uuid parent_type_id FK
    timestamp_with_time_zone created_at
    boolean deprecated
    text deprecation_note
    text description
    public_vector_768 embedding
    character_varying_100 embedding_model
    timestamp_with_time_zone embedding_updated_at
    character_varying_20 entity_category
    uuid entity_type_id
    character_varying_255 name
    jsonb semantic_context
    character_varying_255 table_name
    character_varying_100 type_code
    text type_hierarchy_path
    timestamp_with_time_zone updated_at
  }
  ob_poc__entity_ubos {
    uuid entity_id FK
    integer chain_depth
    character_varying_20 confidence_level
    character_varying_50 control_types
    character_varying_10 country_of_residence
    timestamp_with_time_zone discovered_at
    boolean is_direct
    character_varying_10 nationalities
    jsonb ownership_chain
    numeric ownership_exact
    numeric ownership_max
    numeric ownership_min
    text person_name
    character_varying_100 person_statement_id
    character_varying_50 source
    character_varying_100 source_register
    uuid ubo_id
    character_varying_30 ubo_type
    timestamp_with_time_zone verified_at
    character_varying_255 verified_by
  }
  ob_poc__entity_validation_rules {
    timestamp_with_time_zone created_at
    character_varying_50 entity_type
    character_varying_500 error_message
    character_varying_100 field_name
    boolean is_active
    uuid rule_id
    character_varying_20 severity
    timestamp_with_time_zone updated_at
    jsonb validation_rule
    character_varying_50 validation_type
  }
  ob_poc__expansion_reports {
    character_varying_20 batch_policy
    timestamp_with_time_zone created_at
    jsonb derived_lock_set
    jsonb diagnostics
    timestamp_with_time_zone expanded_at
    character_varying_64 expanded_dsl_digest
    integer expanded_statement_count
    uuid expansion_id
    jsonb invocations
    uuid session_id
    character_varying_64 source_digest
    jsonb template_digests
  }
  ob_poc__fund_investments {
    uuid investee_entity_id FK
    uuid investor_entity_id FK
    timestamp_with_time_zone created_at
    date investment_date
    uuid investment_id
    text investment_type
    numeric_5_2 percentage_of_investee_aum
    numeric_5_2 percentage_of_investor_nav
    date redemption_date
    date valuation_date
  }
  ob_poc__fund_investors {
    uuid fund_cbu_id FK
    uuid investor_entity_id FK
    uuid kyc_case_id FK
    timestamp_with_time_zone created_at
    character_varying_3 currency
    numeric_20_2 investment_amount
    uuid investor_id
    character_varying_50 investor_type
    character_varying_50 kyc_status
    character_varying_50 kyc_tier
    date last_kyc_date
    date subscription_date
    timestamp_with_time_zone updated_at
  }
  ob_poc__fund_structure {
    uuid child_entity_id FK
    uuid parent_entity_id FK
    timestamp_with_time_zone created_at
    character_varying_100 created_by
    date effective_from
    date effective_to
    text relationship_type
    uuid structure_id
  }
  ob_poc__gleif_relationships {
    uuid child_entity_id FK
    uuid parent_entity_id FK
    character_varying_50 accounting_standard
    character_varying_20 child_lei
    timestamp_with_time_zone created_at
    date end_date
    timestamp_with_time_zone fetched_at
    character_varying_100 gleif_record_id
    character_varying_30 gleif_registration_status
    uuid gleif_rel_id
    numeric_5_2 ownership_percentage
    numeric_5_2 ownership_percentage_max
    numeric_5_2 ownership_percentage_min
    character_varying_20 parent_lei
    jsonb raw_data
    character_varying_50 relationship_qualifier
    character_varying_30 relationship_status
    character_varying_50 relationship_type
    date start_date
    timestamp_with_time_zone updated_at
  }
  ob_poc__gleif_sync_log {
    uuid entity_id FK
    timestamp_with_time_zone completed_at
    text error_message
    character_varying_20 lei
    integer records_created
    integer records_fetched
    integer records_updated
    timestamp_with_time_zone started_at
    uuid sync_id
    character_varying_30 sync_status
    character_varying_30 sync_type
  }
  ob_poc__instrument_lifecycles {
    uuid lifecycle_id FK
    jsonb configuration
    timestamp_with_time_zone created_at
    integer display_order
    uuid instrument_class_id
    uuid instrument_lifecycle_id
    boolean is_active
    boolean is_mandatory
    boolean requires_isda
  }
  ob_poc__intent_feedback {
    jsonb alternatives
    text correction_input
    timestamp_with_time_zone created_at
    text graph_context
    bigint id
    text input_source
    uuid interaction_id
    text match_confidence
    real match_score
    text matched_verb
    text outcome
    text outcome_verb
    real phonetic_score
    real semantic_score
    uuid session_id
    integer time_to_outcome_ms
    text user_input
    text user_input_hash
    text workflow_phase
  }
  ob_poc__intent_feedback_analysis {
    date analysis_date
    text analysis_type
    boolean applied
    timestamp_with_time_zone created_at
    jsonb data
    integer id
    boolean reviewed
    timestamp_with_time_zone reviewed_at
    text reviewed_by
  }
  ob_poc__kyc_case_sponsor_decisions {
    uuid case_id FK
    timestamp_with_time_zone created_at
    uuid decision_id
    date effective_date
    character_varying_50 final_status
    jsonb our_findings
    character_varying_50 our_recommendation
    uuid our_recommendation_by
    timestamp_with_time_zone our_recommendation_date
    text sponsor_comments
    character_varying_50 sponsor_decision
    character_varying_255 sponsor_decision_by
    timestamp_with_time_zone sponsor_decision_date
  }
  ob_poc__kyc_decisions {
    uuid case_id FK
    uuid cbu_id FK
    text conditions
    timestamp_with_time_zone created_at
    timestamp_with_time_zone decided_at
    uuid decided_by
    uuid decision_id
    text decision_rationale
    uuid dsl_execution_id
    jsonb evaluation_snapshot
    date next_review_date
    interval review_interval
    character_varying_20 status
  }
  ob_poc__kyc_service_agreements {
    uuid sponsor_cbu_id FK
    uuid sponsor_entity_id FK
    uuid agreement_id
    character_varying_100 agreement_reference
    character_varying_50 auto_accept_threshold
    timestamp_with_time_zone created_at
    date effective_date
    character_varying_50 kyc_standard
    boolean sponsor_review_required
    character_varying_50 status
    integer target_turnaround_days
    date termination_date
    timestamp_with_time_zone updated_at
  }
  ob_poc__layout_cache {
    character_varying_20 algorithm_version
    jsonb bounding_box
    uuid cache_id
    uuid cbu_id
    integer computation_time_ms
    timestamp_with_time_zone computed_at
    integer edge_count
    jsonb edge_paths
    character_varying_64 input_hash
    integer node_count
    jsonb node_positions
    jsonb tier_info
    uuid user_id
    timestamp_with_time_zone valid_until
    character_varying_30 view_mode
  }
  ob_poc__layout_config {
    character_varying_50 config_key
    jsonb config_value
    text description
    timestamp_with_time_zone updated_at
  }
  ob_poc__legal_contracts {
    character_varying_100 client_label
    uuid contract_id
    character_varying_100 contract_reference
    timestamp_with_time_zone created_at
    date effective_date
    character_varying_20 status
    date termination_date
    timestamp_with_time_zone updated_at
  }
  ob_poc__lifecycle_resource_capabilities {
    uuid lifecycle_id FK
    uuid resource_type_id FK
    uuid capability_id
    timestamp_with_time_zone created_at
    boolean is_active
    boolean is_required
    integer priority
    jsonb supported_options
  }
  ob_poc__lifecycle_resource_types {
    character_varying_50 code
    timestamp_with_time_zone created_at
    jsonb depends_on
    text description
    boolean is_active
    character_varying_100 location_type
    character_varying_255 name
    character_varying_100 owner
    boolean per_counterparty
    boolean per_currency
    boolean per_market
    jsonb provisioning_args
    character_varying_100 provisioning_verb
    character_varying_100 resource_type
    uuid resource_type_id
    timestamp_with_time_zone updated_at
    jsonb vendor_options
  }
  ob_poc__lifecycles {
    character_varying_100 category
    character_varying_50 code
    timestamp_with_time_zone created_at
    text description
    boolean is_active
    uuid lifecycle_id
    character_varying_255 name
    character_varying_100 owner
    character_varying_100 regulatory_driver
    jsonb sla_definition
    timestamp_with_time_zone updated_at
  }
  ob_poc__market_csd_mappings {
    timestamp_with_time_zone created_at
    character_varying_11 csd_bic
    character_varying_50 csd_code
    character_varying_255 csd_name
    boolean is_active
    boolean is_primary
    uuid mapping_id
    uuid market_id
  }
  ob_poc__master_entity_xref {
    character_varying_10 jurisdiction_code FK
    jsonb additional_metadata
    text business_purpose
    timestamp_with_time_zone created_at
    uuid entity_id
    character_varying_500 entity_name
    character_varying_50 entity_status
    character_varying_50 entity_type
    uuid primary_contact_person
    jsonb regulatory_numbers
    timestamp_with_time_zone updated_at
    uuid xref_id
  }
  ob_poc__master_jurisdictions {
    character_varying_3 country_code
    timestamp_with_time_zone created_at
    boolean entity_formation_allowed
    character_varying_10 jurisdiction_code
    character_varying_200 jurisdiction_name
    boolean offshore_jurisdiction
    character_varying_100 region
    character_varying_300 regulatory_authority
    character_varying_100 regulatory_framework
    timestamp_with_time_zone updated_at
  }
  ob_poc__node_types {
    boolean can_be_container
    character_varying_20 child_layout_mode
    numeric_3_2 collapse_below_zoom
    numeric_5_1 container_padding
    timestamp_with_time_zone created_at
    character_varying_20 dedupe_mode
    character_varying_30 default_color
    numeric_6_1 default_height
    character_varying_30 default_shape
    integer default_tier
    numeric_6_1 default_width
    text description
    character_varying_100 display_name
    numeric_3_2 hide_label_below_zoom
    character_varying_50 icon
    numeric_3_2 importance_weight
    boolean is_kyc_subject
    boolean is_operational
    boolean is_structural
    boolean is_trading
    integer max_visible_children
    numeric_5_1 min_separation
    character_varying_30 node_type_code
    character_varying_20 overflow_behavior
    numeric_3_2 show_detail_above_zoom
    boolean show_in_fund_structure_view
    boolean show_in_product_view
    boolean show_in_service_view
    boolean show_in_trading_view
    boolean show_in_ubo_view
    integer sort_order
    timestamp_with_time_zone updated_at
    integer z_order
  }
  ob_poc__observation_discrepancies {
    uuid accepted_observation_id FK
    uuid attribute_id FK
    uuid case_id FK
    uuid entity_id FK
    uuid observation_1_id FK
    uuid observation_2_id FK
    uuid red_flag_id FK
    uuid workstream_id FK
    timestamp_with_time_zone created_at
    text description
    timestamp_with_time_zone detected_at
    text detected_by
    uuid discrepancy_id
    character_varying_30 discrepancy_type
    text resolution_notes
    character_varying_30 resolution_status
    character_varying_30 resolution_type
    timestamp_with_time_zone resolved_at
    text resolved_by
    character_varying_20 severity
    timestamp_with_time_zone updated_at
    text value_1_display
    text value_2_display
  }
  ob_poc__onboarding_executions {
    uuid plan_id FK
    timestamp_with_time_zone completed_at
    text error_message
    uuid execution_id
    jsonb result_urls
    timestamp_with_time_zone started_at
    character_varying_20 status
  }
  ob_poc__onboarding_plans {
    uuid cbu_id FK
    jsonb attribute_overrides
    timestamp_with_time_zone created_at
    jsonb dependency_graph
    timestamp_with_time_zone expires_at
    text generated_dsl
    uuid plan_id
    text products
    integer resource_count
    character_varying_20 status
  }
  ob_poc__onboarding_products {
    uuid product_id FK
    uuid request_id FK
    uuid onboarding_product_id
    timestamp_with_time_zone selected_at
    integer selection_order
  }
  ob_poc__onboarding_requests {
    uuid cbu_id FK
    timestamp_with_time_zone completed_at
    timestamp_with_time_zone created_at
    character_varying_255 created_by
    character_varying_100 current_phase
    text dsl_draft
    integer dsl_version
    jsonb phase_metadata
    uuid request_id
    character_varying_50 request_state
    timestamp_with_time_zone updated_at
    jsonb validation_errors
  }
  ob_poc__onboarding_tasks {
    uuid execution_id FK
    uuid resource_instance_id FK
    timestamp_with_time_zone completed_at
    text error_message
    character_varying_50 resource_code
    integer retry_count
    integer stage
    timestamp_with_time_zone started_at
    character_varying_20 status
    uuid task_id
  }
  ob_poc__person_pep_status {
    uuid person_entity_id FK
    timestamp_with_time_zone created_at
    date end_date
    character_varying_10 jurisdiction
    character_varying_20 pep_risk_level
    uuid pep_status_id
    text position_held
    character_varying_30 position_level
    text reason
    uuid screening_id
    text source_reference
    character_varying_50 source_type
    date start_date
    character_varying_20 status
    timestamp_with_time_zone updated_at
    text verification_notes
    timestamp_with_time_zone verified_at
    character_varying_255 verified_by
  }
  ob_poc__product_services {
    uuid product_id FK
    uuid service_id FK
    jsonb configuration
    integer display_order
    boolean is_default
    boolean is_mandatory
  }
  ob_poc__products {
    timestamp_with_time_zone created_at
    text description
    boolean is_active
    character_varying_50 kyc_context
    character_varying_20 kyc_risk_rating
    jsonb metadata
    numeric_20_2 min_asset_requirement
    character_varying_255 name
    character_varying_100 product_category
    character_varying_50 product_code
    uuid product_id
    character_varying_100 regulatory_framework
    boolean requires_kyc
    timestamp_with_time_zone updated_at
  }
  ob_poc__proofs {
    uuid cbu_id FK
    uuid document_id FK
    timestamp_with_time_zone created_at
    character_varying_100 dirty_reason
    timestamp_with_time_zone marked_dirty_at
    uuid proof_id
    character_varying_50 proof_type
    character_varying_20 status
    timestamp_with_time_zone updated_at
    timestamp_with_time_zone uploaded_at
    uuid uploaded_by
    date valid_from
    date valid_until
    timestamp_with_time_zone verified_at
    uuid verified_by
  }
  ob_poc__provisioning_events {
    uuid request_id FK
    text content_hash
    text direction
    uuid event_id
    text kind
    timestamp_with_time_zone occurred_at
    jsonb payload
  }
  ob_poc__provisioning_requests {
    uuid cbu_id FK
    uuid instance_id FK
    text owner_system
    text owner_ticket_id
    jsonb parameters
    uuid request_id
    jsonb request_payload
    timestamp_with_time_zone requested_at
    text requested_by
    text srdef_id
    text status
    timestamp_with_time_zone status_changed_at
  }
  ob_poc__rate_cards {
    timestamp_with_time_zone created_at
    character_varying_3 currency
    text description
    date effective_date
    character_varying_100 name
    uuid rate_card_id
    timestamp_with_time_zone updated_at
  }
  ob_poc__red_flag_severities {
    character_varying_50 code
    text description
    integer display_order
    boolean is_active
    boolean is_blocking
    character_varying_100 name
  }
  ob_poc__redflag_score_config {
    uuid config_id
    timestamp_with_time_zone created_at
    text description
    boolean is_blocking
    character_varying_20 severity
    timestamp_with_time_zone updated_at
    integer weight
  }
  ob_poc__regulators {
    timestamp_with_time_zone created_at
    character_varying_10 jurisdiction
    character_varying_255 name
    character_varying_500 registry_url
    character_varying_20 regulator_code
    character_varying_20 tier
    timestamp_with_time_zone updated_at
  }
  ob_poc__regulatory_tiers {
    boolean allows_simplified_dd
    timestamp_with_time_zone created_at
    text description
    character_varying_20 reliance_level
    boolean requires_enhanced_screening
    character_varying_20 tier_code
  }
  ob_poc__requirement_acceptable_docs {
    character_varying_50 document_type_code FK
    uuid requirement_id FK
    integer priority
  }
  ob_poc__resolution_events {
    uuid snapshot_id FK
    inet client_ip
    timestamp_with_time_zone created_at
    text event_type
    uuid id
    jsonb payload
    uuid session_id
    text user_id
  }
  ob_poc__resource_attribute_requirements {
    uuid attribute_id FK
    uuid resource_id FK
    text condition_expression
    jsonb constraints
    text default_value
    integer display_order
    jsonb evidence_policy
    boolean is_mandatory
    uuid requirement_id
    text requirement_type
    character_varying_255 resource_field_name
    jsonb source_policy
    jsonb transformation_rule
    jsonb validation_override
  }
  ob_poc__resource_dependencies {
    uuid depends_on_type_id FK
    uuid resource_type_id FK
    text condition_expression
    timestamp_with_time_zone created_at
    uuid dependency_id
    character_varying_20 dependency_type
    character_varying_100 inject_arg
    boolean is_active
    integer priority
  }
  ob_poc__resource_instance_attributes {
    uuid attribute_id FK
    uuid instance_id FK
    timestamp_with_time_zone observed_at
    jsonb source
    character_varying_50 state
    boolean value_boolean
    date value_date
    uuid value_id
    jsonb value_json
    numeric value_number
    character_varying value_text
    timestamp_with_time_zone value_timestamp
  }
  ob_poc__resource_instance_dependencies {
    uuid depends_on_instance_id FK
    uuid instance_id FK
    timestamp_with_time_zone created_at
    character_varying_20 dependency_type
  }
  ob_poc__resource_profile_sources {
    uuid instance_id FK
    uuid profile_id FK
    timestamp_with_time_zone created_at
    uuid link_id
    text profile_path
    character_varying_50 profile_section
  }
  ob_poc__risk_bands {
    character_varying_20 band_code
    text description
    boolean escalation_required
    integer max_score
    integer min_score
    integer review_frequency_months
  }
  ob_poc__risk_ratings {
    character_varying_50 code
    text description
    integer display_order
    boolean is_active
    character_varying_100 name
    integer severity_level
  }
  ob_poc__role_categories {
    character_varying_30 category_code
    character_varying_100 category_name
    text description
    character_varying_30 layout_behavior
    boolean show_in_fund_structure_view
    boolean show_in_service_view
    boolean show_in_trading_view
    boolean show_in_ubo_view
    integer sort_order
  }
  ob_poc__role_incompatibilities {
    character_varying_255 role_a FK
    character_varying_255 role_b FK
    timestamp_with_time_zone created_at
    boolean exception_allowed
    text exception_condition
    uuid incompatibility_id
    text reason
  }
  ob_poc__role_requirements {
    character_varying_255 required_role FK
    character_varying_255 requiring_role FK
    text condition_description
    timestamp_with_time_zone created_at
    uuid requirement_id
    character_varying_30 requirement_type
    character_varying_30 scope
  }
  ob_poc__role_types {
    boolean cascade_to_entity_ubos
    boolean check_regulatory_status
    timestamp_with_time_zone created_at
    text description
    character_varying_50 if_regulated_obligation
    character_varying_100 name
    character_varying_50 role_code
    boolean threshold_based
    boolean triggers_full_kyc
    boolean triggers_id_verification
    boolean triggers_screening
  }
  ob_poc__roles {
    jsonb compatible_entity_categories
    timestamp_with_time_zone created_at
    text description
    integer display_priority
    boolean is_active
    character_varying_30 kyc_obligation
    character_varying_30 layout_category
    boolean legal_entity_only
    character_varying_255 name
    boolean natural_person_only
    boolean requires_percentage
    character_varying_30 role_category
    uuid role_id
    integer sort_order
    character_varying_30 ubo_treatment
    timestamp_with_time_zone updated_at
  }
  ob_poc__schema_changes {
    timestamp_with_time_zone applied_at
    character_varying_100 applied_by
    uuid change_id
    character_varying_50 change_type
    text description
    character_varying_255 script_name
  }
  ob_poc__scope_snapshots {
    uuid group_id FK
    uuid parent_snapshot_id FK
    timestamp_with_time_zone created_at
    text created_by
    text description
    text embedder_version
    integer entity_count
    jsonb filter_applied
    uuid id
    integer limit_requested
    text mode
    numeric_3_2 overall_confidence
    text resolution_method
    text role_tags_hash
    uuid selected_entity_ids
    uuid session_id
    jsonb top_k_candidates
  }
  ob_poc__screening_lists {
    timestamp_with_time_zone created_at
    text description
    boolean is_active
    character_varying_50 list_code
    character_varying_255 list_name
    character_varying_50 list_type
    character_varying_100 provider
    uuid screening_list_id
  }
  ob_poc__screening_requirements {
    character_varying_20 risk_band FK
    integer frequency_months
    boolean is_required
    character_varying_50 screening_type
  }
  ob_poc__screening_types {
    character_varying_50 code
    text description
    integer display_order
    boolean is_active
    character_varying_100 name
  }
  ob_poc__semantic_match_cache {
    timestamp_with_time_zone created_at
    integer hit_count
    uuid id
    timestamp_with_time_zone last_accessed_at
    character_varying_20 match_method
    character_varying_100 matched_verb
    real similarity_score
    text transcript_normalized
  }
  ob_poc__service_delivery_map {
    uuid cbu_id FK
    uuid instance_id FK
    uuid product_id FK
    uuid service_id FK
    timestamp_with_time_zone created_at
    timestamp_with_time_zone delivered_at
    uuid delivery_id
    character_varying_50 delivery_status
    timestamp_with_time_zone failed_at
    text failure_reason
    timestamp_with_time_zone requested_at
    jsonb service_config
    timestamp_with_time_zone started_at
    timestamp_with_time_zone updated_at
  }
  ob_poc__service_intents {
    uuid cbu_id FK
    uuid product_id FK
    uuid service_id FK
    timestamp_with_time_zone created_at
    text created_by
    uuid intent_id
    jsonb options
    text status
    timestamp_with_time_zone updated_at
  }
  ob_poc__service_option_choices {
    uuid option_def_id FK
    uuid choice_id
    character_varying_255 choice_label
    jsonb choice_metadata
    character_varying_255 choice_value
    integer display_order
    jsonb excludes_options
    boolean is_active
    boolean is_default
    jsonb requires_options
  }
  ob_poc__service_option_definitions {
    uuid service_id FK
    integer display_order
    text help_text
    boolean is_required
    uuid option_def_id
    character_varying_100 option_key
    character_varying_255 option_label
    character_varying_50 option_type
    jsonb validation_rules
  }
  ob_poc__service_resource_capabilities {
    uuid resource_id FK
    uuid service_id FK
    uuid capability_id
    numeric_10_4 cost_factor
    boolean is_active
    boolean is_required
    integer performance_rating
    integer priority
    jsonb resource_config
    jsonb supported_options
  }
  ob_poc__service_resource_types {
    text api_endpoint
    character_varying_20 api_version
    jsonb authentication_config
    character_varying_50 authentication_method
    jsonb capabilities
    jsonb capacity_limits
    timestamp_with_time_zone created_at
    jsonb depends_on
    text description
    character_varying_100 dictionary_group
    boolean is_active
    character_varying_50 location_type
    jsonb maintenance_windows
    character_varying_255 name
    character_varying_255 owner
    boolean per_counterparty
    boolean per_currency
    boolean per_market
    jsonb provisioning_args
    text provisioning_strategy
    character_varying_100 provisioning_verb
    character_varying_50 resource_code
    uuid resource_id
    text resource_purpose
    character_varying_100 resource_type
    text srdef_id
    timestamp_with_time_zone updated_at
    character_varying_255 vendor
    character_varying_50 version
  }
  ob_poc__services {
    timestamp_with_time_zone created_at
    text description
    boolean is_active
    character_varying_255 name
    character_varying_100 service_category
    character_varying_50 service_code
    uuid service_id
    jsonb sla_definition
    timestamp_with_time_zone updated_at
  }
  ob_poc__session_bookmarks {
    uuid bookmark_id
    character_varying_20 color
    timestamp_with_time_zone created_at
    text description
    character_varying_50 icon
    timestamp_with_time_zone last_used_at
    character_varying_100 name
    jsonb scope_snapshot
    uuid session_id
    integer use_count
    uuid user_id
  }
  ob_poc__session_scope_history {
    character_varying_50 change_source
    character_varying_100 change_verb
    timestamp_with_time_zone created_at
    uuid history_id
    integer position
    jsonb scope_snapshot
    uuid session_id
  }
  ob_poc__session_scopes {
    uuid apex_entity_id FK
    uuid cbu_id FK
    uuid cursor_entity_id FK
    uuid focal_entity_id FK
    uuid active_cbu_ids
    character_varying_255 apex_entity_name
    character_varying_255 cbu_name
    timestamp_with_time_zone created_at
    character_varying_255 cursor_entity_name
    timestamp_with_time_zone expires_at
    character_varying_255 focal_entity_name
    integer history_position
    character_varying_10 jurisdiction_code
    integer neighborhood_hops
    jsonb scope_filters
    character_varying_50 scope_type
    uuid session_id
    uuid session_scope_id
    integer total_cbus
    integer total_entities
    timestamp_with_time_zone updated_at
    uuid user_id
  }
  ob_poc__sessions {
    uuid cbu_ids
    timestamp_with_time_zone created_at
    timestamp_with_time_zone expires_at
    jsonb future
    jsonb history
    uuid id
    boolean intent_confirmed
    text name
    text repl_state
    text scope_dsl
    jsonb sheet
    text target_entity_type
    text template_dsl
    timestamp_with_time_zone updated_at
    uuid user_id
  }
  ob_poc__settlement_types {
    character_varying_20 code
    text description
    integer display_order
    boolean is_active
    character_varying_100 name
  }
  ob_poc__sheet_execution_audit {
    uuid session_id FK
    timestamp_with_time_zone completed_at
    jsonb dag_analysis
    bigint duration_ms
    uuid execution_id
    text overall_status
    integer phase_count
    integer phases_completed
    jsonb result
    text scope_dsl
    uuid sheet_id
    text source_statements
    timestamp_with_time_zone started_at
    integer statement_count
    timestamp_with_time_zone submitted_at
    text submitted_by
    text template_dsl
  }
  ob_poc__sla_breaches {
    uuid commitment_id FK
    uuid measurement_id FK
    date breach_date
    uuid breach_id
    character_varying_20 breach_severity
    timestamp_with_time_zone created_at
    timestamp_with_time_zone detected_at
    timestamp_with_time_zone escalated_at
    character_varying_255 escalated_to
    numeric_18_2 penalty_amount
    boolean penalty_applied
    character_varying_3 penalty_currency
    timestamp_with_time_zone remediation_completed_at
    date remediation_due_date
    text remediation_plan
    character_varying_20 remediation_status
    character_varying_50 root_cause_category
    text root_cause_description
    timestamp_with_time_zone updated_at
  }
  ob_poc__sla_measurements {
    uuid commitment_id FK
    timestamp_with_time_zone created_at
    numeric_10_4 measured_value
    uuid measurement_id
    character_varying_50 measurement_method
    text measurement_notes
    date period_end
    date period_start
    integer sample_size
    character_varying_20 status
    numeric_6_2 variance_pct
  }
  ob_poc__sla_metric_types {
    character_varying_20 aggregation_method
    text description
    boolean higher_is_better
    boolean is_active
    character_varying_30 metric_category
    character_varying_50 metric_code
    character_varying_100 name
    character_varying_20 unit
  }
  ob_poc__sla_templates {
    character_varying_50 metric_code FK
    character_varying_50 applies_to_code
    character_varying_30 applies_to_type
    timestamp_with_time_zone created_at
    text description
    text escalation_path
    boolean is_active
    character_varying_20 measurement_period
    character_varying_255 name
    text regulatory_reference
    boolean regulatory_requirement
    numeric_5_2 response_time_hours
    numeric_10_4 target_value
    character_varying_50 template_code
    uuid template_id
    numeric_10_4 warning_threshold
  }
  ob_poc__srdef_discovery_reasons {
    uuid cbu_id FK
    uuid resource_type_id FK
    timestamp_with_time_zone discovered_at
    uuid discovery_id
    jsonb discovery_reason
    text discovery_rule
    jsonb parameters
    text srdef_id
    timestamp_with_time_zone superseded_at
    jsonb triggered_by_intents
  }
  ob_poc__ssi_types {
    character_varying_50 code
    text description
    integer display_order
    boolean is_active
    character_varying_100 name
  }
  ob_poc__staged_command {
    uuid runbook_id FK
    timestamp_with_time_zone created_at
    integer dag_order
    uuid depends_on
    text description
    text dsl_raw
    text dsl_resolved
    uuid id
    text resolution_error
    text resolution_status
    integer source_order
    text source_prompt
    text verb
  }
  ob_poc__staged_command_candidate {
    uuid command_id FK
    uuid entity_id FK
    text arg_name
    double_precision confidence
    timestamp_with_time_zone created_at
    uuid id
    text match_type
    text matched_tag
  }
  ob_poc__staged_command_entity {
    uuid command_id FK
    uuid entity_id FK
    text arg_name
    double_precision confidence
    uuid id
    text original_ref
    text resolution_source
  }
  ob_poc__staged_runbook {
    uuid client_group_id FK
    timestamp_with_time_zone created_at
    uuid id
    text persona
    text session_id
    text status
    timestamp_with_time_zone updated_at
  }
  ob_poc__taxonomy_crud_log {
    timestamp_with_time_zone created_at
    uuid entity_id
    character_varying_50 entity_type
    text error_message
    jsonb execution_result
    integer execution_time_ms
    text natural_language_input
    uuid operation_id
    character_varying_20 operation_type
    text parsed_dsl
    boolean success
    character_varying_255 user_id
  }
  ob_poc__threshold_factors {
    timestamp_with_time_zone created_at
    text description
    character_varying_50 factor_code
    uuid factor_id
    character_varying_50 factor_type
    boolean is_active
    integer risk_weight
  }
  ob_poc__threshold_requirements {
    character_varying_20 risk_band FK
    character_varying_50 attribute_code
    numeric_3_2 confidence_min
    timestamp_with_time_zone created_at
    character_varying_50 entity_role
    boolean is_required
    integer max_age_days
    boolean must_be_authoritative
    text notes
    uuid requirement_id
  }
  ob_poc__trading_profile_documents {
    uuid doc_id FK
    uuid profile_id FK
    timestamp_with_time_zone created_at
    timestamp_with_time_zone extracted_at
    text extraction_notes
    character_varying_20 extraction_status
    uuid link_id
    character_varying_50 profile_section
  }
  ob_poc__trading_profile_materializations {
    uuid profile_id FK
    integer duration_ms
    jsonb errors
    uuid materialization_id
    timestamp_with_time_zone materialized_at
    character_varying_255 materialized_by
    jsonb records_created
    jsonb records_deleted
    jsonb records_updated
    text sections_materialized
  }
  ob_poc__trading_profile_migration_backup {
    uuid backup_id
    timestamp_with_time_zone migrated_at
    jsonb original_document
    uuid profile_id
  }
  ob_poc__trust_parties {
    uuid entity_id FK
    uuid trust_id FK
    date appointment_date
    timestamp_with_time_zone created_at
    boolean is_active
    character_varying_100 party_role
    character_varying_100 party_type
    date resignation_date
    uuid trust_party_id
    timestamp_with_time_zone updated_at
  }
  ob_poc__ubo_assertion_log {
    uuid case_id FK
    uuid cbu_id FK
    boolean actual_value
    timestamp_with_time_zone asserted_at
    uuid asserted_by
    character_varying_50 assertion_type
    uuid dsl_execution_id
    boolean expected_value
    jsonb failure_details
    uuid log_id
    boolean passed
  }
  ob_poc__ubo_evidence {
    uuid document_id FK
    uuid ubo_id FK
    timestamp_with_time_zone attached_at
    character_varying_255 attached_by
    character_varying_255 attestation_ref
    text description
    character_varying_50 evidence_role
    character_varying_50 evidence_type
    uuid ubo_evidence_id
    text verification_notes
    character_varying_30 verification_status
    timestamp_with_time_zone verified_at
    character_varying_255 verified_by
  }
  ob_poc__ubo_registry {
    uuid case_id FK
    uuid cbu_id FK
    uuid replacement_ubo_id FK
    uuid subject_entity_id FK
    uuid superseded_by FK
    uuid ubo_proper_person_id FK
    uuid workstream_id FK
    timestamp_with_time_zone closed_at
    character_varying_100 closed_reason
    character_varying_100 control_type
    timestamp_with_time_zone created_at
    character_varying_30 discovery_method
    uuid evidence_doc_ids
    timestamp_with_time_zone identified_at
    numeric_5_2 ownership_percentage
    timestamp_with_time_zone proof_date
    character_varying_50 proof_method
    text proof_notes
    character_varying_100 qualifying_reason
    character_varying_100 regulatory_framework
    character_varying_100 relationship_type
    character_varying_100 removal_reason
    character_varying_50 risk_rating
    character_varying_50 screening_result
    timestamp_with_time_zone superseded_at
    uuid ubo_id
    timestamp_with_time_zone updated_at
    character_varying_50 verification_status
    timestamp_with_time_zone verified_at
    character_varying_100 workflow_type
  }
  ob_poc__ubo_snapshot_comparisons {
    uuid baseline_snapshot_id FK
    uuid cbu_id FK
    uuid current_snapshot_id FK
    jsonb added_ubos
    jsonb change_summary
    jsonb changed_ubos
    timestamp_with_time_zone compared_at
    character_varying_255 compared_by
    uuid comparison_id
    jsonb control_changes
    timestamp_with_time_zone created_at
    boolean has_changes
    jsonb ownership_changes
    jsonb removed_ubos
  }
  ob_poc__ubo_snapshots {
    uuid case_id FK
    uuid cbu_id FK
    timestamp_with_time_zone captured_at
    character_varying_255 captured_by
    jsonb control_relationships
    timestamp_with_time_zone created_at
    text gap_summary
    boolean has_gaps
    text notes
    jsonb ownership_chains
    uuid snapshot_id
    character_varying_100 snapshot_reason
    character_varying_30 snapshot_type
    numeric_5_2 total_identified_ownership
    jsonb ubos
  }
  ob_poc__ubo_treatments {
    text description
    boolean requires_lookthrough
    boolean terminates_chain
    character_varying_30 treatment_code
    character_varying_100 treatment_name
  }
  ob_poc__verb_centroids {
    public_vector_384 embedding
    integer phrase_count
    timestamp_with_time_zone updated_at
    text verb_name
  }
  ob_poc__verb_pattern_embeddings {
    character_varying_50 category
    timestamp_with_time_zone created_at
    public_vector_384 embedding
    uuid id
    boolean is_agent_bound
    text match_method
    text pattern_normalized
    text pattern_phrase
    text phonetic_codes
    integer priority
    timestamp_with_time_zone updated_at
    character_varying_100 verb_name
  }
  ob_poc__verification_challenges {
    uuid allegation_id FK
    uuid case_id FK
    uuid cbu_id FK
    uuid entity_id FK
    uuid observation_id FK
    uuid challenge_id
    text challenge_reason
    character_varying_30 challenge_type
    timestamp_with_time_zone raised_at
    character_varying_100 raised_by
    text resolution_notes
    character_varying_30 resolution_type
    timestamp_with_time_zone resolved_at
    character_varying_100 resolved_by
    timestamp_with_time_zone responded_at
    uuid response_evidence_ids
    text response_text
    character_varying_20 severity
    character_varying_20 status
  }
  ob_poc__verification_escalations {
    uuid case_id FK
    uuid cbu_id FK
    uuid challenge_id FK
    timestamp_with_time_zone decided_at
    character_varying_100 decided_by
    character_varying_20 decision
    text decision_notes
    timestamp_with_time_zone escalated_at
    character_varying_100 escalated_by
    uuid escalation_id
    character_varying_30 escalation_level
    text escalation_reason
    jsonb risk_indicators
    character_varying_20 status
  }
  ob_poc__view_modes {
    jsonb algorithm_params
    boolean auto_cluster
    character_varying_50 cluster_attribute
    character_varying_20 cluster_visual_style
    timestamp_with_time_zone created_at
    character_varying_30 default_algorithm
    text description
    character_varying_100 display_name
    numeric_5_1 grid_size_x
    numeric_5_1 grid_size_y
    jsonb hierarchy_edge_types
    jsonb overlay_edge_types
    character_varying_10 primary_traversal_direction
    character_varying_50 root_identification_rule
    boolean snap_to_grid
    character_varying_50 swim_lane_attribute
    character_varying_10 swim_lane_direction
    character_varying_30 temporal_axis
    character_varying_10 temporal_axis_direction
    character_varying_30 view_mode_code
  }
  ob_poc__workflow_audit_log {
    uuid instance_id FK
    jsonb blockers_at_transition
    character_varying_100 from_state
    jsonb guard_results
    uuid log_id
    text reason
    character_varying_100 to_state
    character_varying_20 transition_type
    timestamp_with_time_zone transitioned_at
    character_varying_255 transitioned_by
  }
  ob_poc__workflow_definitions {
    character_varying_64 content_hash
    jsonb definition_json
    text description
    timestamp_with_time_zone loaded_at
    integer version
    character_varying_100 workflow_id
  }
  ob_poc__workflow_instances {
    jsonb blockers
    timestamp_with_time_zone created_at
    character_varying_255 created_by
    character_varying_100 current_state
    jsonb history
    uuid instance_id
    jsonb metadata
    timestamp_with_time_zone state_entered_at
    uuid subject_id
    character_varying_50 subject_type
    timestamp_with_time_zone updated_at
    integer version
    character_varying_100 workflow_id
  }
  ob_poc__workstream_statuses {
    character_varying_50 code
    text description
    integer display_order
    boolean is_active
    boolean is_terminal
    character_varying_100 name
  }
  ob_kyc__entity_regulatory_registrations {
    uuid entity_id FK
    character_varying_50 home_regulator_code FK
    character_varying_50 registration_type FK
    character_varying_50 regulator_code FK
    text activity_scope
    timestamp_without_time_zone created_at
    uuid created_by
    date effective_date
    date expiry_date
    character_varying_100 passport_reference
    uuid registration_id
    character_varying_100 registration_number
    boolean registration_verified
    character_varying_50 status
    timestamp_without_time_zone updated_at
    uuid updated_by
    date verification_date
    date verification_expires
    character_varying_50 verification_method
    character_varying_500 verification_reference
  }
  ob_ref__registration_types {
    boolean allows_reliance
    character_varying_255 description
    boolean is_primary
    character_varying_50 registration_type
  }
  ob_ref__regulators {
    character_varying_50 regulatory_tier FK
    boolean active
    timestamp_without_time_zone created_at
    character_varying_2 jurisdiction
    character_varying_500 registry_url
    character_varying_50 regulator_code
    character_varying_255 regulator_name
    character_varying_50 regulator_type
    timestamp_without_time_zone updated_at
  }
  ob_ref__regulatory_tiers {
    boolean allows_simplified_dd
    character_varying_255 description
    boolean requires_enhanced_screening
    character_varying_50 tier_code
  }
  ob_ref__request_types {
    boolean auto_fulfill_on_upload
    boolean blocks_by_default
    timestamp_with_time_zone created_at
    integer default_due_days
    integer default_grace_days
    character_varying_255 description
    integer escalation_after_days
    boolean escalation_enabled
    character_varying_50 fulfillment_sources
    integer max_reminders
    character_varying_100 request_subtype
    character_varying_50 request_type
    timestamp_with_time_zone updated_at
  }
  ob_ref__role_types {
    boolean active
    boolean cascade_to_entity_ubos
    character_varying_50 category
    boolean check_regulatory_status
    character_varying_50 code
    timestamp_without_time_zone created_at
    text description
    character_varying_50 if_regulated_obligation
    character_varying_255 name
    uuid role_type_id
    boolean threshold_based
    boolean triggers_full_kyc
    boolean triggers_id_verification
    boolean triggers_screening
    timestamp_without_time_zone updated_at
  }
  public___sqlx_migrations {
    bytea checksum
    text description
    bigint execution_time
    timestamp_with_time_zone installed_on
    boolean success
    bigint version
  }
  public__attribute_sources {
    timestamp_without_time_zone created_at
    text description
    integer id
    character_varying_100 name
    boolean requires_validation
    character_varying_50 source_key
    character_varying_20 trust_level
  }
  public__business_attributes {
    integer domain_id FK
    integer source_id FK
    character_varying_100 attribute_name
    timestamp_without_time_zone created_at
    character_varying_50 data_type
    text description
    boolean editable
    character_varying_100 entity_name
    character_varying_100 format_mask
    character_varying_200 full_path
    integer id
    integer max_length
    numeric max_value
    jsonb metadata
    integer min_length
    numeric min_value
    boolean required
    character_varying_100 rust_type
    character_varying_100 sql_type
    timestamp_without_time_zone updated_at
    text validation_pattern
  }
  public__credentials_vault {
    boolean active
    timestamp_with_time_zone created_at
    uuid credential_id
    character_varying_255 credential_name
    character_varying_50 credential_type
    bytea encrypted_data
    character_varying_50 environment
    timestamp_with_time_zone expires_at
  }
  public__data_domains {
    timestamp_without_time_zone created_at
    text description
    character_varying_100 domain_name
    integer id
    jsonb values
  }
  public__derived_attributes {
    integer domain_id FK
    character_varying_100 attribute_name
    timestamp_without_time_zone created_at
    character_varying_50 data_type
    text description
    character_varying_100 entity_name
    character_varying_200 full_path
    integer id
    jsonb metadata
    character_varying_100 rust_type
    character_varying_100 sql_type
    timestamp_without_time_zone updated_at
  }
  public__rule_categories {
    character_varying_50 category_key
    character_varying_7 color
    timestamp_without_time_zone created_at
    text description
    integer id
    character_varying_100 name
  }
  public__rule_dependencies {
    integer attribute_id FK
    integer rule_id FK
    character_varying_20 dependency_type
    integer id
  }
  public__rule_executions {
    integer rule_id FK
    jsonb context
    text error_message
    integer execution_duration_ms
    timestamp_without_time_zone execution_time
    uuid id
    jsonb input_data
    jsonb output_value
    boolean success
  }
  public__rule_versions {
    integer rule_id FK
    text change_description
    timestamp_without_time_zone created_at
    character_varying_100 created_by
    integer id
    text rule_definition
    integer version
  }
  public__rules {
    integer category_id FK
    integer target_attribute_id FK
    timestamp_without_time_zone created_at
    character_varying_100 created_by
    text description
    public_vector_1536 embedding
    jsonb embedding_data
    integer id
    jsonb parsed_ast
    jsonb performance_metrics
    text rule_definition
    character_varying_50 rule_id
    character_varying_200 rule_name
    tsvector search_vector
    character_varying_20 status
    text tags
    timestamp_without_time_zone updated_at
    character_varying_100 updated_by
    integer version
  }
  sessions__log {
    bigint event_id FK
    text content
    text entry_type
    bigint id
    jsonb metadata
    uuid session_id
    text source
    timestamp_with_time_zone timestamp
  }
  teams__access_attestations {
    uuid campaign_id FK
    uuid attestation_id
    character_varying_50 attestation_scope
    text attestation_text
    character_varying_20 attestation_version
    timestamp_with_time_zone attested_at
    character_varying_255 attester_email
    character_varying_255 attester_name
    character_varying_100 attester_role
    uuid attester_user_id
    inet ip_address
    uuid item_ids
    integer items_count
    uuid session_id
    text signature_hash
    text signature_input
    uuid team_id
    text user_agent
  }
  teams__access_domains {
    text description
    character_varying_50 domain_code
    boolean is_active
    character_varying_100 name
    text visualizer_views
  }
  teams__access_review_campaigns {
    uuid campaign_id
    timestamp_with_time_zone completed_at
    integer confirmed_items
    timestamp_with_time_zone created_at
    uuid created_by_user_id
    date deadline
    integer escalated_items
    integer extended_items
    timestamp_with_time_zone launched_at
    character_varying_255 name
    integer pending_items
    integer reminder_days
    date review_period_end
    date review_period_start
    character_varying_50 review_type
    integer reviewed_items
    integer revoked_items
    jsonb scope_filter
    character_varying_50 scope_type
    character_varying_50 status
    integer total_items
  }
  teams__access_review_items {
    uuid campaign_id FK
    uuid membership_id FK
    character_varying_50 access_domains
    timestamp_with_time_zone auto_action_at
    text auto_action_reason
    timestamp_with_time_zone created_at
    integer days_since_login
    character_varying_255 delegating_entity_name
    timestamp_with_time_zone escalated_at
    uuid escalated_to_user_id
    text escalation_reason
    date extended_to
    text extension_reason
    boolean flag_dormant_account
    boolean flag_legal_expired
    boolean flag_legal_expiring_soon
    boolean flag_never_logged_in
    boolean flag_no_legal_link
    boolean flag_orphaned_membership
    boolean flag_role_mismatch
    jsonb flags_json
    uuid item_id
    timestamp_with_time_zone last_login_at
    uuid legal_appointment_id
    date legal_effective_from
    date legal_effective_to
    character_varying_255 legal_entity_name
    character_varying_100 legal_position
    integer membership_age_days
    timestamp_with_time_zone membership_created_at
    character_varying_50 recommendation
    text recommendation_reason
    timestamp_with_time_zone reviewed_at
    character_varying_255 reviewer_email
    character_varying_255 reviewer_name
    text reviewer_notes
    uuid reviewer_user_id
    integer risk_score
    character_varying_100 role_key
    character_varying_50 status
    uuid team_id
    character_varying_255 team_name
    character_varying_50 team_type
    timestamp_with_time_zone updated_at
    character_varying_255 user_email
    character_varying_255 user_employer
    uuid user_id
    character_varying_255 user_name
  }
  teams__access_review_log {
    uuid campaign_id FK
    uuid item_id FK
    character_varying_50 action
    jsonb action_detail
    character_varying_255 actor_email
    character_varying_50 actor_type
    uuid actor_user_id
    timestamp_with_time_zone created_at
    inet ip_address
    uuid log_id
  }
  teams__function_domains {
    character_varying_50 access_domains
    text description
    character_varying_100 function_name
  }
  teams__membership_audit_log {
    uuid team_id FK
    character_varying_50 action
    uuid log_id
    timestamp_with_time_zone performed_at
    uuid performed_by_user_id
    text reason
    uuid user_id
  }
  teams__membership_history {
    character_varying_50 action
    timestamp_with_time_zone changed_at
    uuid changed_by_user_id
    uuid history_id
    uuid membership_id
    character_varying_100 new_role_key
    character_varying_100 old_role_key
    text reason
    uuid team_id
    uuid user_id
  }
  teams__memberships {
    uuid team_id FK
    uuid user_id FK
    timestamp_with_time_zone created_at
    uuid delegated_by_user_id
    date effective_from
    date effective_to
    character_varying_50 function_name
    uuid legal_appointment_id
    uuid membership_id
    jsonb permission_overrides
    boolean requires_legal_appointment
    character_varying_100 role_key
    character_varying_50 role_level
    character_varying_50 team_type
    timestamp_with_time_zone updated_at
  }
  teams__team_cbu_access {
    uuid cbu_id FK
    uuid team_id FK
    uuid access_id
    jsonb access_restrictions
    timestamp_with_time_zone granted_at
    uuid granted_by_user_id
  }
  teams__team_service_entitlements {
    uuid team_id FK
    jsonb config
    uuid entitlement_id
    timestamp_with_time_zone granted_at
    uuid granted_by_user_id
    character_varying_100 service_code
    timestamp_with_time_zone updated_at
  }
  teams__teams {
    uuid delegating_entity_id FK
    character_varying_50 access_mode
    text archive_reason
    timestamp_with_time_zone archived_at
    jsonb authority_scope
    character_varying_50 authority_type
    timestamp_with_time_zone created_at
    uuid created_by_user_id
    uuid explicit_cbus
    boolean is_active
    character_varying_255 name
    jsonb scope_filter
    jsonb service_entitlements
    uuid team_id
    character_varying_50 team_type
    timestamp_with_time_zone updated_at
  }

  agent__learning_candidates ||--o{ agent__learning_audit : "learning_audit_candidate_id_fkey:candidate_id"
  client_portal__clients ||--o{ client_portal__commitments : "commitments_client_id_fkey:client_id"
  client_portal__clients ||--o{ client_portal__credentials : "credentials_client_id_fkey:client_id"
  client_portal__clients ||--o{ client_portal__escalations : "escalations_client_id_fkey:client_id"
  client_portal__clients ||--o{ client_portal__sessions : "sessions_client_id_fkey:client_id"
  client_portal__clients ||--o{ client_portal__submissions : "submissions_client_id_fkey:client_id"
  client_portal__clients ||--o{ teams__memberships : "memberships_user_id_fkey:user_id"
  client_portal__sessions ||--o{ client_portal__escalations : "escalations_session_id_fkey:session_id"
  custody__ca_event_types ||--o{ custody__cbu_ca_instruction_windows : "cbu_ca_instruction_windows_event_type_id_fkey:event_type_id"
  custody__ca_event_types ||--o{ custody__cbu_ca_preferences : "cbu_ca_preferences_event_type_id_fkey:event_type_id"
  custody__ca_event_types ||--o{ custody__cbu_ca_ssi_mappings : "cbu_ca_ssi_mappings_event_type_id_fkey:event_type_id"
  custody__cbu_settlement_chains ||--o{ custody__settlement_chain_hops : "settlement_chain_hops_chain_id_fkey:chain_id"
  custody__cbu_ssi ||--o{ custody__cbu_ca_ssi_mappings : "cbu_ca_ssi_mappings_ssi_id_fkey:ssi_id"
  custody__cbu_ssi ||--o{ custody__cbu_ssi_agent_override : "cbu_ssi_agent_override_ssi_id_fkey:ssi_id"
  custody__cbu_ssi ||--o{ custody__csa_agreements : "csa_agreements_collateral_ssi_id_fkey:collateral_ssi_id"
  custody__cbu_ssi ||--o{ custody__settlement_chain_hops : "settlement_chain_hops_ssi_id_fkey:ssi_id"
  custody__cbu_ssi ||--o{ custody__ssi_booking_rules : "ssi_booking_rules_ssi_id_fkey:ssi_id"
  custody__instruction_types ||--o{ custody__instruction_paths : "instruction_paths_instruction_type_id_fkey:instruction_type_id"
  custody__instrument_classes ||--o{ custody__cbu_ca_preferences : "cbu_ca_preferences_instrument_class_id_fkey:instrument_class_id"
  custody__instrument_classes ||--o{ custody__cbu_instrument_universe : "cbu_instrument_universe_instrument_class_id_fkey:instrument_class_id"
  custody__instrument_classes ||--o{ custody__cbu_pricing_config : "cbu_pricing_config_instrument_class_id_fkey:instrument_class_id"
  custody__instrument_classes ||--o{ custody__cbu_settlement_chains : "cbu_settlement_chains_instrument_class_id_fkey:instrument_class_id"
  custody__instrument_classes ||--o{ custody__cbu_settlement_location_preferences : "cbu_settlement_location_preferences_instrument_class_id_fkey:instrument_class_id"
  custody__instrument_classes ||--o{ custody__cfi_codes : "cfi_codes_class_id_fkey:class_id"
  custody__instrument_classes ||--o{ custody__entity_ssi : "entity_ssi_instrument_class_id_fkey:instrument_class_id"
  custody__instrument_classes ||--o{ custody__instruction_paths : "instruction_paths_instrument_class_id_fkey:instrument_class_id"
  custody__instrument_classes ||--o{ custody__instrument_classes : "instrument_classes_parent_class_id_fkey:parent_class_id"
  custody__instrument_classes ||--o{ custody__isda_product_coverage : "isda_product_coverage_instrument_class_id_fkey:instrument_class_id"
  custody__instrument_classes ||--o{ custody__isda_product_taxonomy : "isda_product_taxonomy_class_id_fkey:class_id"
  custody__instrument_classes ||--o{ custody__security_types : "security_types_class_id_fkey:class_id"
  custody__instrument_classes ||--o{ custody__ssi_booking_rules : "ssi_booking_rules_instrument_class_id_fkey:instrument_class_id"
  custody__instrument_classes ||--o{ custody__tax_treaty_rates : "tax_treaty_rates_instrument_class_id_fkey:instrument_class_id"
  custody__instrument_classes ||--o{ ob_poc__cbu_matrix_product_overlay : "cbu_matrix_product_overlay_instrument_class_id_fkey:instrument_class_id"
  custody__isda_agreements ||--o{ custody__csa_agreements : "csa_agreements_isda_id_fkey:isda_id"
  custody__isda_agreements ||--o{ custody__isda_product_coverage : "isda_product_coverage_isda_id_fkey:isda_id"
  custody__isda_product_taxonomy ||--o{ custody__isda_product_coverage : "isda_product_coverage_isda_taxonomy_id_fkey:isda_taxonomy_id"
  custody__markets ||--o{ custody__cbu_ca_instruction_windows : "cbu_ca_instruction_windows_market_id_fkey:market_id"
  custody__markets ||--o{ custody__cbu_cross_border_config : "cbu_cross_border_config_source_market_id_fkey:source_market_id; cbu_cross_border_config_target_market_id_fkey:target_market_id"
  custody__markets ||--o{ custody__cbu_instrument_universe : "cbu_instrument_universe_market_id_fkey:market_id"
  custody__markets ||--o{ custody__cbu_pricing_config : "cbu_pricing_config_market_id_fkey:market_id"
  custody__markets ||--o{ custody__cbu_settlement_chains : "cbu_settlement_chains_market_id_fkey:market_id"
  custody__markets ||--o{ custody__cbu_settlement_location_preferences : "cbu_settlement_location_preferences_market_id_fkey:market_id"
  custody__markets ||--o{ custody__cbu_ssi : "cbu_ssi_market_id_fkey:market_id"
  custody__markets ||--o{ custody__entity_ssi : "entity_ssi_market_id_fkey:market_id"
  custody__markets ||--o{ custody__instruction_paths : "instruction_paths_market_id_fkey:market_id"
  custody__markets ||--o{ custody__ssi_booking_rules : "ssi_booking_rules_market_id_fkey:market_id"
  custody__markets ||--o{ custody__subcustodian_network : "subcustodian_network_market_id_fkey:market_id"
  custody__markets ||--o{ ob_poc__cbu_matrix_product_overlay : "cbu_matrix_product_overlay_market_id_fkey:market_id"
  custody__markets ||--o{ ob_poc__cbu_resource_instances : "cbu_resource_instances_market_id_fkey:market_id"
  custody__security_types ||--o{ custody__cfi_codes : "cfi_codes_security_type_id_fkey:security_type_id"
  custody__security_types ||--o{ custody__entity_ssi : "entity_ssi_security_type_id_fkey:security_type_id"
  custody__security_types ||--o{ custody__ssi_booking_rules : "ssi_booking_rules_security_type_id_fkey:security_type_id"
  custody__settlement_locations ||--o{ custody__cbu_cross_border_config : "cbu_cross_border_config_bridge_location_id_fkey:bridge_location_id"
  custody__settlement_locations ||--o{ custody__cbu_settlement_location_preferences : "cbu_settlement_location_preferences_preferred_location_id_fkey:preferred_location_id"
  custody__tax_jurisdictions ||--o{ custody__cbu_tax_reclaim_config : "cbu_tax_reclaim_config_source_jurisdiction_id_fkey:source_jurisdiction_id"
  custody__tax_jurisdictions ||--o{ custody__cbu_tax_reporting : "cbu_tax_reporting_reporting_jurisdiction_id_fkey:reporting_jurisdiction_id"
  custody__tax_jurisdictions ||--o{ custody__cbu_tax_status : "cbu_tax_status_tax_jurisdiction_id_fkey:tax_jurisdiction_id"
  custody__tax_jurisdictions ||--o{ custody__tax_treaty_rates : "tax_treaty_rates_investor_jurisdiction_id_fkey:investor_jurisdiction_id; tax_treaty_rates_source_jurisdiction_id_fkey:source_jurisdiction_id"
  events__log ||--o{ sessions__log : "log_event_id_fkey:event_id"
  feedback__failures ||--o{ feedback__audit_log : "audit_log_failure_id_fkey:failure_id"
  feedback__failures ||--o{ feedback__occurrences : "occurrences_failure_id_fkey:failure_id"
  kyc__cases ||--o{ kyc__approval_requests : "approval_requests_case_id_fkey:case_id"
  kyc__cases ||--o{ kyc__case_events : "case_events_case_id_fkey:case_id"
  kyc__cases ||--o{ kyc__entity_workstreams : "entity_workstreams_case_id_fkey:case_id"
  kyc__cases ||--o{ kyc__outstanding_requests : "outstanding_requests_case_id_fkey:case_id"
  kyc__cases ||--o{ kyc__red_flags : "red_flags_case_id_fkey:case_id"
  kyc__cases ||--o{ kyc__rule_executions : "rule_executions_case_id_fkey:case_id"
  kyc__cases ||--o{ ob_poc__case_evaluation_snapshots : "case_evaluation_snapshots_case_id_fkey:case_id"
  kyc__cases ||--o{ ob_poc__client_allegations : "client_allegations_case_id_fkey:case_id"
  kyc__cases ||--o{ ob_poc__detected_patterns : "detected_patterns_case_id_fkey:case_id"
  kyc__cases ||--o{ ob_poc__dsl_sessions : "dsl_sessions_kyc_case_id_fkey:kyc_case_id"
  kyc__cases ||--o{ ob_poc__fund_investors : "fund_investors_kyc_case_id_fkey:kyc_case_id"
  kyc__cases ||--o{ ob_poc__kyc_case_sponsor_decisions : "kyc_case_sponsor_decisions_case_id_fkey:case_id"
  kyc__cases ||--o{ ob_poc__kyc_decisions : "kyc_decisions_case_id_fkey:case_id"
  kyc__cases ||--o{ ob_poc__observation_discrepancies : "observation_discrepancies_case_id_fkey:case_id"
  kyc__cases ||--o{ ob_poc__ubo_assertion_log : "ubo_assertion_log_case_id_fkey:case_id"
  kyc__cases ||--o{ ob_poc__ubo_registry : "ubo_registry_case_id_fkey:case_id"
  kyc__cases ||--o{ ob_poc__ubo_snapshots : "ubo_snapshots_case_id_fkey:case_id"
  kyc__cases ||--o{ ob_poc__verification_challenges : "verification_challenges_case_id_fkey:case_id"
  kyc__cases ||--o{ ob_poc__verification_escalations : "verification_escalations_case_id_fkey:case_id"
  kyc__dilution_instruments ||--o{ kyc__dilution_exercise_events : "dilution_exercise_events_instrument_id_fkey:instrument_id"
  kyc__doc_requests ||--o{ kyc__doc_request_acceptable_types : "doc_request_acceptable_types_request_id_fkey:request_id"
  kyc__entity_workstreams ||--o{ kyc__approval_requests : "approval_requests_workstream_id_fkey:workstream_id"
  kyc__entity_workstreams ||--o{ kyc__case_events : "case_events_workstream_id_fkey:workstream_id"
  kyc__entity_workstreams ||--o{ kyc__doc_requests : "doc_requests_workstream_id_fkey:workstream_id"
  kyc__entity_workstreams ||--o{ kyc__entity_workstreams : "entity_workstreams_discovery_source_workstream_id_fkey:discovery_source_workstream_id"
  kyc__entity_workstreams ||--o{ kyc__outstanding_requests : "outstanding_requests_workstream_id_fkey:workstream_id"
  kyc__entity_workstreams ||--o{ kyc__red_flags : "red_flags_workstream_id_fkey:workstream_id"
  kyc__entity_workstreams ||--o{ kyc__rule_executions : "rule_executions_workstream_id_fkey:workstream_id"
  kyc__entity_workstreams ||--o{ kyc__screenings : "screenings_workstream_id_fkey:workstream_id"
  kyc__entity_workstreams ||--o{ ob_poc__attribute_observations : "attribute_observations_source_workstream_id_fkey:source_workstream_id"
  kyc__entity_workstreams ||--o{ ob_poc__client_allegations : "client_allegations_workstream_id_fkey:workstream_id"
  kyc__entity_workstreams ||--o{ ob_poc__observation_discrepancies : "observation_discrepancies_workstream_id_fkey:workstream_id"
  kyc__entity_workstreams ||--o{ ob_poc__ubo_registry : "ubo_registry_workstream_id_fkey:workstream_id"
  kyc__fund_compartments ||--o{ kyc__share_classes : "share_classes_compartment_id_fkey:compartment_id"
  kyc__holdings ||--o{ kyc__dilution_exercise_events : "dilution_exercise_events_resulting_holding_id_fkey:resulting_holding_id"
  kyc__holdings ||--o{ kyc__movements : "movements_holding_id_fkey:holding_id"
  kyc__instrument_identifier_schemes ||--o{ kyc__share_class_identifiers : "share_class_identifiers_scheme_code_fkey:scheme_code"
  kyc__investors ||--o{ kyc__holdings : "holdings_investor_id_fkey:investor_id"
  kyc__investors ||--o{ kyc__investor_lifecycle_history : "investor_lifecycle_history_investor_id_fkey:investor_id"
  kyc__outstanding_requests ||--o{ kyc__entity_workstreams : "entity_workstreams_blocker_request_id_fkey:blocker_request_id"
  kyc__ownership_reconciliation_runs ||--o{ kyc__ownership_reconciliation_findings : "ownership_reconciliation_findings_run_id_fkey:run_id"
  kyc__ownership_snapshots ||--o{ kyc__ownership_snapshots : "ownership_snapshots_superseded_by_fkey:superseded_by"
  kyc__red_flags ||--o{ kyc__screenings : "screenings_red_flag_id_fkey:red_flag_id"
  kyc__red_flags ||--o{ ob_poc__observation_discrepancies : "observation_discrepancies_red_flag_id_fkey:red_flag_id"
  kyc__research_actions ||--o{ kyc__research_anomalies : "research_anomalies_action_id_fkey:action_id"
  kyc__research_actions ||--o{ kyc__research_corrections : "research_corrections_new_action_id_fkey:new_action_id; research_corrections_original_action_id_fkey:original_action_id"
  kyc__research_decisions ||--o{ kyc__research_actions : "research_actions_decision_id_fkey:decision_id"
  kyc__research_decisions ||--o{ kyc__research_corrections : "research_corrections_original_decision_id_fkey:original_decision_id"
  kyc__screenings ||--o{ ob_poc__attribute_observations : "attribute_observations_source_screening_id_fkey:source_screening_id"
  kyc__share_classes ||--o{ kyc__dilution_instruments : "dilution_instruments_converts_to_share_class_id_fkey:converts_to_share_class_id"
  kyc__share_classes ||--o{ kyc__holding_control_links : "holding_control_links_share_class_id_fkey:share_class_id"
  kyc__share_classes ||--o{ kyc__holdings : "holdings_share_class_id_fkey:share_class_id"
  kyc__share_classes ||--o{ kyc__investor_role_profiles : "investor_role_profiles_share_class_id_fkey:share_class_id"
  kyc__share_classes ||--o{ kyc__issuance_events : "issuance_events_share_class_id_fkey:share_class_id"
  kyc__share_classes ||--o{ kyc__ownership_snapshots : "ownership_snapshots_share_class_id_fkey:share_class_id"
  kyc__share_classes ||--o{ kyc__share_class_identifiers : "share_class_identifiers_share_class_id_fkey:share_class_id"
  kyc__share_classes ||--o{ kyc__share_class_supply : "share_class_supply_share_class_id_fkey:share_class_id"
  kyc__share_classes ||--o{ kyc__share_classes : "share_classes_converts_to_share_class_id_fkey:converts_to_share_class_id"
  kyc__share_classes ||--o{ kyc__special_rights : "special_rights_share_class_id_fkey:share_class_id"
  ob_poc__attribute_observations ||--o{ ob_poc__attribute_observations : "attribute_observations_superseded_by_fkey:superseded_by"
  ob_poc__attribute_observations ||--o{ ob_poc__client_allegations : "client_allegations_verified_by_observation_id_fkey:verified_by_observation_id"
  ob_poc__attribute_observations ||--o{ ob_poc__observation_discrepancies : "observation_discrepancies_accepted_observation_id_fkey:accepted_observation_id; observation_discrepancies_observation_1_id_fkey:observation_1_id; observation_discrepancies_observation_2_id_fkey:observation_2_id"
  ob_poc__attribute_observations ||--o{ ob_poc__verification_challenges : "verification_challenges_observation_id_fkey:observation_id"
  ob_poc__attribute_registry ||--o{ ob_poc__attribute_observations : "attribute_observations_attribute_id_fkey:attribute_id"
  ob_poc__attribute_registry ||--o{ ob_poc__attribute_values_typed : "attribute_values_typed_attribute_id_fkey:attribute_id; fk_attribute_uuid:attribute_uuid"
  ob_poc__attribute_registry ||--o{ ob_poc__cbu_attr_values : "cbu_attr_values_attr_id_fkey:attr_id"
  ob_poc__attribute_registry ||--o{ ob_poc__cbu_unified_attr_requirements : "cbu_unified_attr_requirements_attr_id_fkey:attr_id"
  ob_poc__attribute_registry ||--o{ ob_poc__client_allegations : "client_allegations_attribute_id_fkey:attribute_id"
  ob_poc__attribute_registry ||--o{ ob_poc__document_attribute_links : "document_attribute_links_attribute_id_fkey:attribute_id"
  ob_poc__attribute_registry ||--o{ ob_poc__document_attribute_mappings : "document_attribute_mappings_attribute_uuid_fkey:attribute_uuid"
  ob_poc__attribute_registry ||--o{ ob_poc__observation_discrepancies : "observation_discrepancies_attribute_id_fkey:attribute_id"
  ob_poc__attribute_registry ||--o{ ob_poc__resource_attribute_requirements : "resource_attribute_requirements_attribute_uuid_fkey:attribute_id"
  ob_poc__attribute_registry ||--o{ ob_poc__resource_instance_attributes : "resource_instance_attributes_attribute_uuid_fkey:attribute_id"
  ob_poc__bods_entity_statements ||--o{ ob_poc__entity_bods_links : "entity_bods_links_bods_entity_statement_id_fkey:bods_entity_statement_id"
  ob_poc__case_decision_thresholds ||--o{ ob_poc__case_evaluation_snapshots : "case_evaluation_snapshots_matched_threshold_id_fkey:matched_threshold_id"
  ob_poc__cbu_board_controller ||--o{ ob_poc__board_control_evidence : "board_control_evidence_cbu_board_controller_id_fkey:cbu_board_controller_id"
  ob_poc__cbu_groups ||--o{ ob_poc__cbu_group_members : "cbu_group_members_group_id_fkey:group_id"
  ob_poc__cbu_product_subscriptions ||--o{ ob_poc__cbu_matrix_product_overlay : "cbu_matrix_product_overlay_subscription_id_fkey:subscription_id"
  ob_poc__cbu_resource_instances ||--o{ custody__cbu_cash_sweep_config : "cbu_cash_sweep_config_sweep_resource_id_fkey:sweep_resource_id"
  ob_poc__cbu_resource_instances ||--o{ custody__cbu_im_assignments : "cbu_im_assignments_instruction_resource_id_fkey:instruction_resource_id"
  ob_poc__cbu_resource_instances ||--o{ custody__cbu_pricing_config : "cbu_pricing_config_pricing_resource_id_fkey:pricing_resource_id"
  ob_poc__cbu_resource_instances ||--o{ ob_poc__cbu_sla_commitments : "cbu_sla_commitments_bound_resource_instance_id_fkey:bound_resource_instance_id"
  ob_poc__cbu_resource_instances ||--o{ ob_poc__onboarding_tasks : "onboarding_tasks_resource_instance_id_fkey:resource_instance_id"
  ob_poc__cbu_resource_instances ||--o{ ob_poc__provisioning_requests : "provisioning_requests_instance_id_fkey:instance_id"
  ob_poc__cbu_resource_instances ||--o{ ob_poc__resource_instance_attributes : "resource_instance_attributes_instance_id_fkey:instance_id"
  ob_poc__cbu_resource_instances ||--o{ ob_poc__resource_instance_dependencies : "resource_instance_dependencies_depends_on_instance_id_fkey:depends_on_instance_id; resource_instance_dependencies_instance_id_fkey:instance_id"
  ob_poc__cbu_resource_instances ||--o{ ob_poc__resource_profile_sources : "resource_profile_sources_instance_id_fkey:instance_id"
  ob_poc__cbu_resource_instances ||--o{ ob_poc__service_delivery_map : "service_delivery_map_instance_id_fkey:instance_id"
  ob_poc__cbu_sla_commitments ||--o{ ob_poc__sla_breaches : "sla_breaches_commitment_id_fkey:commitment_id"
  ob_poc__cbu_sla_commitments ||--o{ ob_poc__sla_measurements : "sla_measurements_commitment_id_fkey:commitment_id"
  ob_poc__cbu_trading_profiles ||--o{ custody__cbu_cash_sweep_config : "cbu_cash_sweep_config_profile_id_fkey:profile_id"
  ob_poc__cbu_trading_profiles ||--o{ custody__cbu_im_assignments : "cbu_im_assignments_profile_id_fkey:profile_id"
  ob_poc__cbu_trading_profiles ||--o{ custody__cbu_pricing_config : "cbu_pricing_config_profile_id_fkey:profile_id"
  ob_poc__cbu_trading_profiles ||--o{ ob_poc__cbu_sla_commitments : "cbu_sla_commitments_profile_id_fkey:profile_id"
  ob_poc__cbu_trading_profiles ||--o{ ob_poc__resource_profile_sources : "resource_profile_sources_profile_id_fkey:profile_id"
  ob_poc__cbu_trading_profiles ||--o{ ob_poc__trading_profile_documents : "trading_profile_documents_profile_id_fkey:profile_id"
  ob_poc__cbu_trading_profiles ||--o{ ob_poc__trading_profile_materializations : "trading_profile_materializations_profile_id_fkey:profile_id"
  ob_poc__cbus ||--o{ custody__cbu_ca_instruction_windows : "cbu_ca_instruction_windows_cbu_id_fkey:cbu_id"
  ob_poc__cbus ||--o{ custody__cbu_ca_preferences : "cbu_ca_preferences_cbu_id_fkey:cbu_id"
  ob_poc__cbus ||--o{ custody__cbu_ca_ssi_mappings : "cbu_ca_ssi_mappings_cbu_id_fkey:cbu_id"
  ob_poc__cbus ||--o{ custody__cbu_cash_sweep_config : "cbu_cash_sweep_config_cbu_id_fkey:cbu_id"
  ob_poc__cbus ||--o{ custody__cbu_cross_border_config : "cbu_cross_border_config_cbu_id_fkey:cbu_id"
  ob_poc__cbus ||--o{ custody__cbu_im_assignments : "cbu_im_assignments_cbu_id_fkey:cbu_id"
  ob_poc__cbus ||--o{ custody__cbu_instrument_universe : "cbu_instrument_universe_cbu_id_fkey:cbu_id"
  ob_poc__cbus ||--o{ custody__cbu_pricing_config : "cbu_pricing_config_cbu_id_fkey:cbu_id"
  ob_poc__cbus ||--o{ custody__cbu_settlement_chains : "cbu_settlement_chains_cbu_id_fkey:cbu_id"
  ob_poc__cbus ||--o{ custody__cbu_settlement_location_preferences : "cbu_settlement_location_preferences_cbu_id_fkey:cbu_id"
  ob_poc__cbus ||--o{ custody__cbu_ssi : "cbu_ssi_cbu_id_fkey:cbu_id"
  ob_poc__cbus ||--o{ custody__cbu_tax_reclaim_config : "cbu_tax_reclaim_config_cbu_id_fkey:cbu_id"
  ob_poc__cbus ||--o{ custody__cbu_tax_reporting : "cbu_tax_reporting_cbu_id_fkey:cbu_id"
  ob_poc__cbus ||--o{ custody__cbu_tax_status : "cbu_tax_status_cbu_id_fkey:cbu_id"
  ob_poc__cbus ||--o{ custody__isda_agreements : "isda_agreements_cbu_id_fkey:cbu_id"
  ob_poc__cbus ||--o{ custody__ssi_booking_rules : "ssi_booking_rules_cbu_id_fkey:cbu_id"
  ob_poc__cbus ||--o{ kyc__cases : "cases_cbu_id_fkey:cbu_id; cases_sponsor_cbu_id_fkey:sponsor_cbu_id"
  ob_poc__cbus ||--o{ kyc__investors : "investors_owning_cbu_id_fkey:owning_cbu_id"
  ob_poc__cbus ||--o{ kyc__outstanding_requests : "outstanding_requests_cbu_id_fkey:cbu_id"
  ob_poc__cbus ||--o{ kyc__share_classes : "share_classes_cbu_id_fkey:cbu_id"
  ob_poc__cbus ||--o{ ob_poc__cbu_attr_values : "cbu_attr_values_cbu_id_fkey:cbu_id"
  ob_poc__cbus ||--o{ ob_poc__cbu_board_controller : "cbu_board_controller_cbu_id_fkey:cbu_id"
  ob_poc__cbus ||--o{ ob_poc__cbu_change_log : "cbu_change_log_cbu_id_fkey:cbu_id"
  ob_poc__cbus ||--o{ ob_poc__cbu_control_anchors : "cbu_control_anchors_cbu_id_fkey:cbu_id"
  ob_poc__cbus ||--o{ ob_poc__cbu_creation_log : "fk_cbu_creation_log_cbu:cbu_id"
  ob_poc__cbus ||--o{ ob_poc__cbu_entity_roles : "cbu_entity_roles_cbu_id_fkey:cbu_id; fk_cbu_entity_roles_cbu_id:cbu_id"
  ob_poc__cbus ||--o{ ob_poc__cbu_evidence : "cbu_evidence_cbu_id_fkey:cbu_id"
  ob_poc__cbus ||--o{ ob_poc__cbu_group_members : "cbu_group_members_cbu_id_fkey:cbu_id"
  ob_poc__cbus ||--o{ ob_poc__cbu_lifecycle_instances : "cbu_lifecycle_instances_cbu_fk:cbu_id"
  ob_poc__cbus ||--o{ ob_poc__cbu_matrix_product_overlay : "cbu_matrix_product_overlay_cbu_id_fkey:cbu_id"
  ob_poc__cbus ||--o{ ob_poc__cbu_product_subscriptions : "cbu_product_subscriptions_cbu_id_fkey:cbu_id"
  ob_poc__cbus ||--o{ ob_poc__cbu_relationship_verification : "cbu_relationship_verification_cbu_id_fkey:cbu_id"
  ob_poc__cbus ||--o{ ob_poc__cbu_resource_instances : "cbu_resource_instances_cbu_id_fkey:cbu_id"
  ob_poc__cbus ||--o{ ob_poc__cbu_service_contexts : "cbu_service_contexts_cbu_id_fkey:cbu_id"
  ob_poc__cbus ||--o{ ob_poc__cbu_service_readiness : "cbu_service_readiness_cbu_id_fkey:cbu_id"
  ob_poc__cbus ||--o{ ob_poc__cbu_sla_commitments : "cbu_sla_commitments_cbu_id_fkey:cbu_id"
  ob_poc__cbus ||--o{ ob_poc__cbu_subscriptions : "cbu_subscriptions_cbu_id_fkey:cbu_id"
  ob_poc__cbus ||--o{ ob_poc__cbu_trading_profiles : "cbu_trading_profiles_cbu_id_fkey:cbu_id"
  ob_poc__cbus ||--o{ ob_poc__cbu_unified_attr_requirements : "cbu_unified_attr_requirements_cbu_id_fkey:cbu_id"
  ob_poc__cbus ||--o{ ob_poc__client_allegations : "client_allegations_cbu_id_fkey:cbu_id"
  ob_poc__cbus ||--o{ ob_poc__client_group_entity : "client_group_entity_cbu_id_fkey:cbu_id"
  ob_poc__cbus ||--o{ ob_poc__delegation_relationships : "delegation_relationships_applies_to_cbu_id_fkey:applies_to_cbu_id"
  ob_poc__cbus ||--o{ ob_poc__detected_patterns : "detected_patterns_cbu_id_fkey:cbu_id"
  ob_poc__cbus ||--o{ ob_poc__document_catalog : "fk_document_catalog_cbu:cbu_id"
  ob_poc__cbus ||--o{ ob_poc__dsl_ob : "fk_dsl_ob_cbu_id:cbu_id"
  ob_poc__cbus ||--o{ ob_poc__dsl_sessions : "dsl_sessions_cbu_id_fkey:cbu_id"
  ob_poc__cbus ||--o{ ob_poc__fund_investors : "fund_investors_fund_cbu_id_fkey:fund_cbu_id"
  ob_poc__cbus ||--o{ ob_poc__kyc_decisions : "kyc_decisions_cbu_id_fkey:cbu_id"
  ob_poc__cbus ||--o{ ob_poc__kyc_service_agreements : "kyc_service_agreements_sponsor_cbu_id_fkey:sponsor_cbu_id"
  ob_poc__cbus ||--o{ ob_poc__onboarding_plans : "onboarding_plans_cbu_id_fkey:cbu_id"
  ob_poc__cbus ||--o{ ob_poc__onboarding_requests : "onboarding_requests_cbu_id_fkey:cbu_id"
  ob_poc__cbus ||--o{ ob_poc__proofs : "proofs_cbu_id_fkey:cbu_id"
  ob_poc__cbus ||--o{ ob_poc__provisioning_requests : "provisioning_requests_cbu_id_fkey:cbu_id"
  ob_poc__cbus ||--o{ ob_poc__service_delivery_map : "service_delivery_map_cbu_id_fkey:cbu_id"
  ob_poc__cbus ||--o{ ob_poc__service_intents : "service_intents_cbu_id_fkey:cbu_id"
  ob_poc__cbus ||--o{ ob_poc__session_scopes : "session_scopes_cbu_id_fkey:cbu_id"
  ob_poc__cbus ||--o{ ob_poc__srdef_discovery_reasons : "srdef_discovery_reasons_cbu_id_fkey:cbu_id"
  ob_poc__cbus ||--o{ ob_poc__ubo_assertion_log : "ubo_assertion_log_cbu_id_fkey:cbu_id"
  ob_poc__cbus ||--o{ ob_poc__ubo_registry : "fk_ubo_registry_cbu_id:cbu_id; ubo_registry_cbu_id_fkey:cbu_id"
  ob_poc__cbus ||--o{ ob_poc__ubo_snapshot_comparisons : "ubo_snapshot_comparisons_cbu_id_fkey:cbu_id"
  ob_poc__cbus ||--o{ ob_poc__ubo_snapshots : "ubo_snapshots_cbu_id_fkey:cbu_id"
  ob_poc__cbus ||--o{ ob_poc__verification_challenges : "verification_challenges_cbu_id_fkey:cbu_id"
  ob_poc__cbus ||--o{ ob_poc__verification_escalations : "verification_escalations_cbu_id_fkey:cbu_id"
  ob_poc__cbus ||--o{ teams__team_cbu_access : "team_cbu_access_cbu_id_fkey:cbu_id"
  ob_poc__client_allegations ||--o{ ob_poc__verification_challenges : "verification_challenges_allegation_id_fkey:allegation_id"
  ob_poc__client_group ||--o{ ob_poc__client_group_alias : "client_group_alias_group_id_fkey:group_id"
  ob_poc__client_group ||--o{ ob_poc__client_group_anchor : "client_group_anchor_group_id_fkey:group_id"
  ob_poc__client_group ||--o{ ob_poc__client_group_entity : "client_group_entity_group_id_fkey:group_id"
  ob_poc__client_group ||--o{ ob_poc__client_group_entity_tag : "client_group_entity_tag_group_id_fkey:group_id"
  ob_poc__client_group ||--o{ ob_poc__client_group_relationship : "client_group_relationship_group_id_fkey:group_id"
  ob_poc__client_group ||--o{ ob_poc__scope_snapshots : "scope_snapshots_group_id_fkey:group_id"
  ob_poc__client_group ||--o{ ob_poc__staged_runbook : "staged_runbook_client_group_id_fkey:client_group_id"
  ob_poc__client_group_alias ||--o{ ob_poc__client_group_alias_embedding : "client_group_alias_embedding_alias_id_fkey:alias_id"
  ob_poc__client_group_entity ||--o{ ob_poc__client_group_entity_roles : "client_group_entity_roles_cge_id_fkey:cge_id"
  ob_poc__client_group_entity_tag ||--o{ ob_poc__client_group_entity_tag_embedding : "client_group_entity_tag_embedding_tag_id_fkey:tag_id"
  ob_poc__client_group_relationship ||--o{ ob_poc__client_group_relationship_sources : "client_group_relationship_sources_relationship_id_fkey:relationship_id"
  ob_poc__client_group_relationship_sources ||--o{ ob_poc__client_group_relationship_sources : "client_group_relationship_sources_verifies_source_id_fkey:verifies_source_id"
  ob_poc__contract_products ||--o{ ob_poc__cbu_subscriptions : "cbu_subscriptions_contract_id_product_code_fkey:contract_id,product_code"
  ob_poc__crud_operations ||--o{ ob_poc__crud_operations : "crud_operations_parent_operation_id_fkey:parent_operation_id"
  ob_poc__document_catalog ||--o{ kyc__dilution_instruments : "dilution_instruments_grant_document_id_fkey:grant_document_id"
  ob_poc__document_catalog ||--o{ kyc__issuance_events : "issuance_events_source_document_id_fkey:source_document_id"
  ob_poc__document_catalog ||--o{ kyc__special_rights : "special_rights_source_document_id_fkey:source_document_id"
  ob_poc__document_catalog ||--o{ ob_poc__attribute_observations : "attribute_observations_source_document_id_fkey:source_document_id"
  ob_poc__document_catalog ||--o{ ob_poc__cbu_evidence : "cbu_evidence_document_id_fkey:document_id"
  ob_poc__document_catalog ||--o{ ob_poc__cbu_relationship_verification : "cbu_relationship_verification_proof_document_id_fkey:proof_document_id"
  ob_poc__document_catalog ||--o{ ob_poc__cbu_sla_commitments : "cbu_sla_commitments_source_document_id_fkey:source_document_id"
  ob_poc__document_catalog ||--o{ ob_poc__cbu_trading_profiles : "cbu_trading_profiles_source_document_id_fkey:source_document_id"
  ob_poc__document_catalog ||--o{ ob_poc__delegation_relationships : "delegation_relationships_contract_doc_id_fkey:contract_doc_id"
  ob_poc__document_catalog ||--o{ ob_poc__proofs : "proofs_document_id_fkey:document_id"
  ob_poc__document_catalog ||--o{ ob_poc__trading_profile_documents : "trading_profile_documents_doc_id_fkey:doc_id"
  ob_poc__document_catalog ||--o{ ob_poc__ubo_evidence : "ubo_evidence_document_id_fkey:document_id"
  ob_poc__document_types ||--o{ kyc__doc_request_acceptable_types : "doc_request_acceptable_types_document_type_id_fkey:document_type_id"
  ob_poc__document_types ||--o{ ob_poc__document_attribute_links : "document_attribute_links_document_type_id_fkey:document_type_id"
  ob_poc__document_types ||--o{ ob_poc__document_attribute_mappings : "document_attribute_mappings_document_type_id_fkey:document_type_id"
  ob_poc__document_types ||--o{ ob_poc__document_catalog : "document_catalog_document_type_id_fkey:document_type_id"
  ob_poc__document_types ||--o{ ob_poc__document_validity_rules : "document_validity_rules_document_type_id_fkey:document_type_id"
  ob_poc__document_types ||--o{ ob_poc__requirement_acceptable_docs : "requirement_acceptable_docs_document_type_code_fkey:document_type_code"
  ob_poc__dsl_domains ||--o{ ob_poc__dsl_versions : "dsl_versions_domain_id_fkey:domain_id"
  ob_poc__dsl_idempotency ||--o{ ob_poc__dsl_view_state_changes : "fk_idempotency:idempotency_key"
  ob_poc__dsl_instances ||--o{ ob_poc__dsl_generation_log : "dsl_generation_log_instance_id_fkey:instance_id"
  ob_poc__dsl_instances ||--o{ ob_poc__dsl_instance_versions : "fk_instance:instance_id"
  ob_poc__dsl_sessions ||--o{ ob_poc__dsl_session_events : "dsl_session_events_session_id_fkey:session_id"
  ob_poc__dsl_sessions ||--o{ ob_poc__dsl_session_locks : "dsl_session_locks_session_id_fkey:session_id"
  ob_poc__dsl_sessions ||--o{ ob_poc__dsl_snapshots : "dsl_snapshots_session_id_fkey:session_id"
  ob_poc__dsl_sessions ||--o{ ob_poc__dsl_view_state_changes : "fk_session:session_id"
  ob_poc__dsl_versions ||--o{ ob_poc__dsl_execution_log : "dsl_execution_log_version_id_fkey:version_id"
  ob_poc__dsl_versions ||--o{ ob_poc__dsl_versions : "dsl_versions_parent_version_id_fkey:parent_version_id"
  ob_poc__entities ||--o{ agent__entity_aliases : "entity_aliases_entity_id_fkey:entity_id"
  ob_poc__entities ||--o{ client_portal__clients : "clients_employer_entity_id_fkey:employer_entity_id"
  ob_poc__entities ||--o{ custody__cbu_im_assignments : "cbu_im_assignments_manager_entity_id_fkey:manager_entity_id"
  ob_poc__entities ||--o{ custody__cbu_instrument_universe : "cbu_instrument_universe_counterparty_entity_id_fkey:counterparty_entity_id"
  ob_poc__entities ||--o{ custody__cbu_tax_reclaim_config : "cbu_tax_reclaim_config_service_provider_entity_id_fkey:service_provider_entity_id"
  ob_poc__entities ||--o{ custody__cbu_tax_reporting : "cbu_tax_reporting_reporting_entity_id_fkey:reporting_entity_id; cbu_tax_reporting_sponsor_entity_id_fkey:sponsor_entity_id"
  ob_poc__entities ||--o{ custody__entity_settlement_identity : "entity_settlement_identity_entity_id_fkey:entity_id"
  ob_poc__entities ||--o{ custody__entity_ssi : "entity_ssi_entity_id_fkey:entity_id"
  ob_poc__entities ||--o{ custody__isda_agreements : "isda_agreements_counterparty_entity_id_fkey:counterparty_entity_id"
  ob_poc__entities ||--o{ custody__settlement_chain_hops : "settlement_chain_hops_intermediary_entity_id_fkey:intermediary_entity_id"
  ob_poc__entities ||--o{ custody__ssi_booking_rules : "ssi_booking_rules_counterparty_entity_id_fkey:counterparty_entity_id"
  ob_poc__entities ||--o{ kyc__cases : "cases_subject_entity_id_fkey:subject_entity_id"
  ob_poc__entities ||--o{ kyc__dilution_instruments : "dilution_instruments_holder_entity_id_fkey:holder_entity_id; dilution_instruments_issuer_entity_id_fkey:issuer_entity_id"
  ob_poc__entities ||--o{ kyc__entity_workstreams : "entity_workstreams_entity_id_fkey:entity_id"
  ob_poc__entities ||--o{ kyc__fund_compartments : "fund_compartments_compartment_entity_id_fkey:compartment_entity_id; fund_compartments_umbrella_fund_entity_id_fkey:umbrella_fund_entity_id"
  ob_poc__entities ||--o{ kyc__fund_vehicles : "fund_vehicles_fund_entity_id_fkey:fund_entity_id; fund_vehicles_manager_entity_id_fkey:manager_entity_id; fund_vehicles_umbrella_entity_id_fkey:umbrella_entity_id"
  ob_poc__entities ||--o{ kyc__holding_control_links : "holding_control_links_holder_entity_id_fkey:holder_entity_id; holding_control_links_issuer_entity_id_fkey:issuer_entity_id"
  ob_poc__entities ||--o{ kyc__holdings : "holdings_investor_entity_id_fkey:investor_entity_id"
  ob_poc__entities ||--o{ kyc__investor_role_profiles : "investor_role_profiles_group_container_entity_id_fkey:group_container_entity_id; investor_role_profiles_holder_entity_id_fkey:holder_entity_id; investor_role_profiles_issuer_entity_id_fkey:issuer_entity_id"
  ob_poc__entities ||--o{ kyc__investors : "investors_entity_id_fkey:entity_id"
  ob_poc__entities ||--o{ kyc__issuance_events : "issuance_events_issuer_entity_id_fkey:issuer_entity_id"
  ob_poc__entities ||--o{ kyc__issuer_control_config : "issuer_control_config_issuer_entity_id_fkey:issuer_entity_id"
  ob_poc__entities ||--o{ kyc__outreach_requests : "outreach_requests_recipient_entity_id_fkey:recipient_entity_id; outreach_requests_target_entity_id_fkey:target_entity_id"
  ob_poc__entities ||--o{ kyc__outstanding_requests : "outstanding_requests_entity_id_fkey:entity_id"
  ob_poc__entities ||--o{ kyc__ownership_reconciliation_findings : "ownership_reconciliation_findings_owner_entity_id_fkey:owner_entity_id"
  ob_poc__entities ||--o{ kyc__ownership_reconciliation_runs : "ownership_reconciliation_runs_issuer_entity_id_fkey:issuer_entity_id"
  ob_poc__entities ||--o{ kyc__ownership_snapshots : "ownership_snapshots_issuer_entity_id_fkey:issuer_entity_id; ownership_snapshots_owner_entity_id_fkey:owner_entity_id"
  ob_poc__entities ||--o{ kyc__research_actions : "research_actions_target_entity_id_fkey:target_entity_id"
  ob_poc__entities ||--o{ kyc__research_anomalies : "research_anomalies_entity_id_fkey:entity_id"
  ob_poc__entities ||--o{ kyc__research_decisions : "research_decisions_target_entity_id_fkey:target_entity_id"
  ob_poc__entities ||--o{ kyc__share_classes : "share_classes_entity_id_fkey:entity_id; share_classes_issuer_entity_id_fkey:issuer_entity_id"
  ob_poc__entities ||--o{ kyc__special_rights : "special_rights_holder_entity_id_fkey:holder_entity_id; special_rights_issuer_entity_id_fkey:issuer_entity_id"
  ob_poc__entities ||--o{ ob_kyc__entity_regulatory_registrations : "entity_regulatory_registrations_entity_id_fkey:entity_id"
  ob_poc__entities ||--o{ ob_poc__attribute_observations : "attribute_observations_entity_id_fkey:entity_id"
  ob_poc__entities ||--o{ ob_poc__cbu_board_controller : "cbu_board_controller_controller_entity_id_fkey:controller_entity_id"
  ob_poc__entities ||--o{ ob_poc__cbu_control_anchors : "cbu_control_anchors_entity_id_fkey:entity_id"
  ob_poc__entities ||--o{ ob_poc__cbu_entity_roles : "cbu_entity_roles_entity_id_fkey:entity_id; cbu_entity_roles_target_entity_id_fkey:target_entity_id; fk_cbu_entity_roles_entity_id:entity_id"
  ob_poc__entities ||--o{ ob_poc__cbu_groups : "cbu_groups_manco_entity_id_fkey:manco_entity_id; cbu_groups_ultimate_parent_entity_id_fkey:ultimate_parent_entity_id"
  ob_poc__entities ||--o{ ob_poc__cbu_matrix_product_overlay : "cbu_matrix_product_overlay_counterparty_entity_id_fkey:counterparty_entity_id"
  ob_poc__entities ||--o{ ob_poc__cbu_resource_instances : "cbu_resource_instances_counterparty_entity_id_fkey:counterparty_entity_id"
  ob_poc__entities ||--o{ ob_poc__cbus : "cbus_commercial_client_entity_id_fkey:commercial_client_entity_id"
  ob_poc__entities ||--o{ ob_poc__client_allegations : "client_allegations_entity_id_fkey:entity_id"
  ob_poc__entities ||--o{ ob_poc__client_group_anchor : "client_group_anchor_anchor_entity_id_fkey:anchor_entity_id"
  ob_poc__entities ||--o{ ob_poc__client_group_entity : "client_group_entity_entity_id_fkey:entity_id"
  ob_poc__entities ||--o{ ob_poc__client_group_entity_roles : "client_group_entity_roles_target_entity_id_fkey:target_entity_id"
  ob_poc__entities ||--o{ ob_poc__client_group_entity_tag : "client_group_entity_tag_entity_id_fkey:entity_id"
  ob_poc__entities ||--o{ ob_poc__client_group_relationship : "client_group_relationship_child_entity_id_fkey:child_entity_id; client_group_relationship_parent_entity_id_fkey:parent_entity_id"
  ob_poc__entities ||--o{ ob_poc__control_edges : "control_edges_from_entity_id_fkey:from_entity_id; control_edges_to_entity_id_fkey:to_entity_id"
  ob_poc__entities ||--o{ ob_poc__delegation_relationships : "delegation_relationships_delegate_entity_id_fkey:delegate_entity_id; delegation_relationships_delegator_entity_id_fkey:delegator_entity_id"
  ob_poc__entities ||--o{ ob_poc__document_catalog : "document_catalog_entity_id_fkey:entity_id"
  ob_poc__entities ||--o{ ob_poc__entity_addresses : "entity_addresses_entity_id_fkey:entity_id"
  ob_poc__entities ||--o{ ob_poc__entity_bods_links : "entity_bods_links_entity_id_fkey:entity_id"
  ob_poc__entities ||--o{ ob_poc__entity_concept_link : "entity_concept_link_entity_id_fkey:entity_id"
  ob_poc__entities ||--o{ ob_poc__entity_cooperatives : "entity_cooperatives_entity_id_fkey:entity_id"
  ob_poc__entities ||--o{ ob_poc__entity_feature : "entity_feature_entity_id_fkey:entity_id"
  ob_poc__entities ||--o{ ob_poc__entity_foundations : "entity_foundations_entity_id_fkey:entity_id"
  ob_poc__entities ||--o{ ob_poc__entity_funds : "entity_funds_entity_id_fkey:entity_id; entity_funds_master_fund_id_fkey:master_fund_id; entity_funds_parent_fund_id_fkey:parent_fund_id"
  ob_poc__entities ||--o{ ob_poc__entity_government : "entity_government_entity_id_fkey:entity_id"
  ob_poc__entities ||--o{ ob_poc__entity_identifiers : "entity_identifiers_entity_id_fkey:entity_id"
  ob_poc__entities ||--o{ ob_poc__entity_lifecycle_events : "entity_lifecycle_events_entity_id_fkey:entity_id"
  ob_poc__entities ||--o{ ob_poc__entity_limited_companies : "entity_limited_companies_entity_id_fkey:entity_id"
  ob_poc__entities ||--o{ ob_poc__entity_manco : "entity_manco_entity_id_fkey:entity_id"
  ob_poc__entities ||--o{ ob_poc__entity_names : "entity_names_entity_id_fkey:entity_id"
  ob_poc__entities ||--o{ ob_poc__entity_parent_relationships : "entity_parent_relationships_child_entity_id_fkey:child_entity_id; entity_parent_relationships_parent_entity_id_fkey:parent_entity_id"
  ob_poc__entities ||--o{ ob_poc__entity_partnerships : "entity_partnerships_entity_id_fkey:entity_id"
  ob_poc__entities ||--o{ ob_poc__entity_proper_persons : "entity_proper_persons_entity_id_fkey:entity_id"
  ob_poc__entities ||--o{ ob_poc__entity_regulatory_profiles : "entity_regulatory_profiles_entity_id_fkey:entity_id"
  ob_poc__entities ||--o{ ob_poc__entity_relationships : "entity_relationships_from_entity_id_fkey:from_entity_id; entity_relationships_to_entity_id_fkey:to_entity_id"
  ob_poc__entities ||--o{ ob_poc__entity_share_classes : "entity_share_classes_entity_id_fkey:entity_id; entity_share_classes_parent_fund_id_fkey:parent_fund_id"
  ob_poc__entities ||--o{ ob_poc__entity_trusts : "entity_trusts_entity_id_fkey:entity_id"
  ob_poc__entities ||--o{ ob_poc__entity_ubos : "entity_ubos_entity_id_fkey:entity_id"
  ob_poc__entities ||--o{ ob_poc__fund_investments : "fund_investments_investee_entity_id_fkey:investee_entity_id; fund_investments_investor_entity_id_fkey:investor_entity_id"
  ob_poc__entities ||--o{ ob_poc__fund_investors : "fund_investors_investor_entity_id_fkey:investor_entity_id"
  ob_poc__entities ||--o{ ob_poc__fund_structure : "fund_structure_child_entity_id_fkey:child_entity_id; fund_structure_parent_entity_id_fkey:parent_entity_id"
  ob_poc__entities ||--o{ ob_poc__gleif_relationships : "gleif_relationships_child_entity_id_fkey:child_entity_id; gleif_relationships_parent_entity_id_fkey:parent_entity_id"
  ob_poc__entities ||--o{ ob_poc__gleif_sync_log : "gleif_sync_log_entity_id_fkey:entity_id"
  ob_poc__entities ||--o{ ob_poc__kyc_service_agreements : "kyc_service_agreements_sponsor_entity_id_fkey:sponsor_entity_id"
  ob_poc__entities ||--o{ ob_poc__observation_discrepancies : "observation_discrepancies_entity_id_fkey:entity_id"
  ob_poc__entities ||--o{ ob_poc__person_pep_status : "person_pep_status_person_entity_id_fkey:person_entity_id"
  ob_poc__entities ||--o{ ob_poc__session_scopes : "session_scopes_apex_entity_id_fkey:apex_entity_id; session_scopes_cursor_entity_id_fkey:cursor_entity_id; session_scopes_focal_entity_id_fkey:focal_entity_id"
  ob_poc__entities ||--o{ ob_poc__staged_command_candidate : "staged_command_candidate_entity_id_fkey:entity_id"
  ob_poc__entities ||--o{ ob_poc__staged_command_entity : "staged_command_entity_entity_id_fkey:entity_id"
  ob_poc__entities ||--o{ ob_poc__trust_parties : "fk_trust_parties_entity_id:entity_id; trust_parties_entity_id_fkey:entity_id"
  ob_poc__entities ||--o{ ob_poc__ubo_registry : "fk_ubo_registry_subject_entity_id:subject_entity_id; fk_ubo_registry_ubo_proper_person_id:ubo_proper_person_id; ubo_registry_subject_entity_id_fkey:subject_entity_id; ubo_registry_ubo_proper_person_id_fkey:ubo_proper_person_id"
  ob_poc__entities ||--o{ ob_poc__verification_challenges : "verification_challenges_entity_id_fkey:entity_id"
  ob_poc__entities ||--o{ teams__teams : "teams_delegating_entity_id_fkey:delegating_entity_id"
  ob_poc__entity_relationships ||--o{ ob_poc__cbu_relationship_verification : "cbu_relationship_verification_relationship_id_fkey:relationship_id"
  ob_poc__entity_relationships ||--o{ ob_poc__client_group_relationship : "client_group_relationship_promoted_to_relationship_id_fkey:promoted_to_relationship_id"
  ob_poc__entity_trusts ||--o{ ob_poc__trust_parties : "fk_trust_parties_trust_id:trust_id; trust_parties_trust_id_fkey:trust_id"
  ob_poc__entity_types ||--o{ ob_poc__entities : "entities_entity_type_id_fkey:entity_type_id; fk_entities_entity_type_id:entity_type_id"
  ob_poc__entity_types ||--o{ ob_poc__entity_types : "entity_types_parent_type_id_fkey:parent_type_id"
  ob_poc__intent_feedback ||--o{ ob_poc__dsl_generation_log : "fk_generation_log_feedback:intent_feedback_id"
  ob_poc__kyc_service_agreements ||--o{ kyc__cases : "cases_service_agreement_id_fkey:service_agreement_id"
  ob_poc__legal_contracts ||--o{ ob_poc__contract_products : "contract_products_contract_id_fkey:contract_id"
  ob_poc__lifecycle_resource_types ||--o{ ob_poc__cbu_lifecycle_instances : "cbu_lifecycle_instances_resource_fk:resource_type_id"
  ob_poc__lifecycle_resource_types ||--o{ ob_poc__lifecycle_resource_capabilities : "lifecycle_resource_capabilities_resource_fk:resource_type_id"
  ob_poc__lifecycles ||--o{ ob_poc__instrument_lifecycles : "instrument_lifecycles_lifecycle_fk:lifecycle_id"
  ob_poc__lifecycles ||--o{ ob_poc__lifecycle_resource_capabilities : "lifecycle_resource_capabilities_lifecycle_fk:lifecycle_id"
  ob_poc__master_jurisdictions ||--o{ ob_poc__master_entity_xref : "master_entity_xref_jurisdiction_code_fkey:jurisdiction_code"
  ob_poc__onboarding_executions ||--o{ ob_poc__onboarding_tasks : "onboarding_tasks_execution_id_fkey:execution_id"
  ob_poc__onboarding_plans ||--o{ ob_poc__onboarding_executions : "onboarding_executions_plan_id_fkey:plan_id"
  ob_poc__onboarding_requests ||--o{ ob_poc__dsl_sessions : "dsl_sessions_onboarding_request_id_fkey:onboarding_request_id"
  ob_poc__onboarding_requests ||--o{ ob_poc__onboarding_products : "onboarding_products_request_id_fkey:request_id"
  ob_poc__products ||--o{ ob_poc__cbu_product_subscriptions : "cbu_product_subscriptions_product_id_fkey:product_id"
  ob_poc__products ||--o{ ob_poc__cbu_resource_instances : "cbu_resource_instances_product_id_fkey:product_id"
  ob_poc__products ||--o{ ob_poc__cbu_service_readiness : "cbu_service_readiness_product_id_fkey:product_id"
  ob_poc__products ||--o{ ob_poc__cbus : "cbus_product_id_fkey:product_id"
  ob_poc__products ||--o{ ob_poc__onboarding_products : "onboarding_products_product_id_fkey:product_id"
  ob_poc__products ||--o{ ob_poc__product_services : "product_services_product_id_fkey:product_id"
  ob_poc__products ||--o{ ob_poc__service_delivery_map : "service_delivery_map_product_id_fkey:product_id"
  ob_poc__products ||--o{ ob_poc__service_intents : "service_intents_product_id_fkey:product_id"
  ob_poc__provisioning_requests ||--o{ ob_poc__cbu_resource_instances : "cbu_resource_instances_last_request_id_fkey:last_request_id"
  ob_poc__provisioning_requests ||--o{ ob_poc__provisioning_events : "provisioning_events_request_id_fkey:request_id"
  ob_poc__regulators ||--o{ ob_poc__entity_regulatory_profiles : "entity_regulatory_profiles_regulator_code_fkey:regulator_code"
  ob_poc__regulatory_tiers ||--o{ ob_poc__entity_regulatory_profiles : "entity_regulatory_profiles_regulatory_tier_fkey:regulatory_tier"
  ob_poc__risk_bands ||--o{ ob_poc__screening_requirements : "screening_requirements_risk_band_fkey:risk_band"
  ob_poc__risk_bands ||--o{ ob_poc__threshold_requirements : "threshold_requirements_risk_band_fkey:risk_band"
  ob_poc__roles ||--o{ ob_poc__cbu_entity_roles : "cbu_entity_roles_role_id_fkey:role_id; fk_cbu_entity_roles_role_id:role_id"
  ob_poc__roles ||--o{ ob_poc__client_group_entity_roles : "client_group_entity_roles_role_id_fkey:role_id"
  ob_poc__roles ||--o{ ob_poc__role_incompatibilities : "fk_role_a:role_a; fk_role_b:role_b"
  ob_poc__roles ||--o{ ob_poc__role_requirements : "fk_required_role:required_role; fk_requiring_role:requiring_role"
  ob_poc__scope_snapshots ||--o{ ob_poc__resolution_events : "resolution_events_snapshot_id_fkey:snapshot_id"
  ob_poc__scope_snapshots ||--o{ ob_poc__scope_snapshots : "scope_snapshots_parent_snapshot_id_fkey:parent_snapshot_id"
  ob_poc__service_option_definitions ||--o{ ob_poc__service_option_choices : "service_option_choices_option_def_id_fkey:option_def_id"
  ob_poc__service_resource_types ||--o{ custody__instruction_paths : "instruction_paths_resource_id_fkey:resource_id"
  ob_poc__service_resource_types ||--o{ ob_poc__cbu_resource_instances : "cbu_resource_instances_resource_type_id_fkey:resource_type_id"
  ob_poc__service_resource_types ||--o{ ob_poc__resource_attribute_requirements : "resource_attribute_requirements_resource_id_fkey:resource_id"
  ob_poc__service_resource_types ||--o{ ob_poc__resource_dependencies : "resource_dependencies_depends_on_type_id_fkey:depends_on_type_id; resource_dependencies_resource_type_id_fkey:resource_type_id"
  ob_poc__service_resource_types ||--o{ ob_poc__service_resource_capabilities : "service_resource_capabilities_resource_id_fkey:resource_id"
  ob_poc__service_resource_types ||--o{ ob_poc__srdef_discovery_reasons : "srdef_discovery_reasons_resource_type_id_fkey:resource_type_id"
  ob_poc__services ||--o{ ob_poc__cbu_resource_instances : "cbu_resource_instances_service_id_fkey:service_id"
  ob_poc__services ||--o{ ob_poc__cbu_service_readiness : "cbu_service_readiness_service_id_fkey:service_id"
  ob_poc__services ||--o{ ob_poc__cbu_sla_commitments : "cbu_sla_commitments_bound_service_id_fkey:bound_service_id"
  ob_poc__services ||--o{ ob_poc__product_services : "product_services_service_id_fkey:service_id"
  ob_poc__services ||--o{ ob_poc__service_delivery_map : "service_delivery_map_service_id_fkey:service_id"
  ob_poc__services ||--o{ ob_poc__service_intents : "service_intents_service_id_fkey:service_id"
  ob_poc__services ||--o{ ob_poc__service_option_definitions : "service_option_definitions_service_id_fkey:service_id"
  ob_poc__services ||--o{ ob_poc__service_resource_capabilities : "service_resource_capabilities_service_id_fkey:service_id"
  ob_poc__sessions ||--o{ ob_poc__sheet_execution_audit : "fk_session:session_id"
  ob_poc__sla_measurements ||--o{ ob_poc__sla_breaches : "sla_breaches_measurement_id_fkey:measurement_id"
  ob_poc__sla_metric_types ||--o{ ob_poc__sla_templates : "sla_templates_metric_code_fkey:metric_code"
  ob_poc__sla_templates ||--o{ ob_poc__cbu_sla_commitments : "cbu_sla_commitments_template_id_fkey:template_id"
  ob_poc__staged_command ||--o{ ob_poc__staged_command_candidate : "staged_command_candidate_command_id_fkey:command_id"
  ob_poc__staged_command ||--o{ ob_poc__staged_command_entity : "staged_command_entity_command_id_fkey:command_id"
  ob_poc__staged_runbook ||--o{ ob_poc__staged_command : "staged_command_runbook_id_fkey:runbook_id"
  ob_poc__threshold_requirements ||--o{ ob_poc__requirement_acceptable_docs : "requirement_acceptable_docs_requirement_id_fkey:requirement_id"
  ob_poc__ubo_registry ||--o{ ob_poc__ubo_evidence : "ubo_evidence_ubo_id_fkey:ubo_id"
  ob_poc__ubo_registry ||--o{ ob_poc__ubo_registry : "ubo_registry_replacement_ubo_id_fkey:replacement_ubo_id; ubo_registry_superseded_by_fkey:superseded_by"
  ob_poc__ubo_snapshots ||--o{ ob_poc__ubo_snapshot_comparisons : "ubo_snapshot_comparisons_baseline_snapshot_id_fkey:baseline_snapshot_id; ubo_snapshot_comparisons_current_snapshot_id_fkey:current_snapshot_id"
  ob_poc__verification_challenges ||--o{ ob_poc__verification_escalations : "verification_escalations_challenge_id_fkey:challenge_id"
  ob_poc__workflow_instances ||--o{ ob_poc__workflow_audit_log : "workflow_audit_log_instance_id_fkey:instance_id"
  ob_ref__registration_types ||--o{ ob_kyc__entity_regulatory_registrations : "entity_regulatory_registrations_registration_type_fkey:registration_type"
  ob_ref__regulators ||--o{ ob_kyc__entity_regulatory_registrations : "entity_regulatory_registrations_home_regulator_code_fkey:home_regulator_code; entity_regulatory_registrations_regulator_code_fkey:regulator_code"
  ob_ref__regulatory_tiers ||--o{ ob_ref__regulators : "regulators_regulatory_tier_fkey:regulatory_tier"
  public__attribute_sources ||--o{ public__business_attributes : "business_attributes_source_id_fkey:source_id"
  public__business_attributes ||--o{ public__rule_dependencies : "rule_dependencies_attribute_id_fkey:attribute_id"
  public__data_domains ||--o{ public__business_attributes : "business_attributes_domain_id_fkey:domain_id"
  public__data_domains ||--o{ public__derived_attributes : "derived_attributes_domain_id_fkey:domain_id"
  public__derived_attributes ||--o{ public__rules : "rules_target_attribute_id_fkey:target_attribute_id"
  public__rule_categories ||--o{ public__rules : "rules_category_id_fkey:category_id"
  public__rules ||--o{ public__rule_dependencies : "rule_dependencies_rule_id_fkey:rule_id"
  public__rules ||--o{ public__rule_executions : "rule_executions_rule_id_fkey:rule_id"
  public__rules ||--o{ public__rule_versions : "rule_versions_rule_id_fkey:rule_id"
  teams__access_review_campaigns ||--o{ teams__access_attestations : "access_attestations_campaign_id_fkey:campaign_id"
  teams__access_review_campaigns ||--o{ teams__access_review_items : "access_review_items_campaign_id_fkey:campaign_id"
  teams__access_review_campaigns ||--o{ teams__access_review_log : "access_review_log_campaign_id_fkey:campaign_id"
  teams__access_review_items ||--o{ teams__access_review_log : "access_review_log_item_id_fkey:item_id"
  teams__memberships ||--o{ teams__access_review_items : "access_review_items_membership_id_fkey:membership_id"
  teams__teams ||--o{ teams__membership_audit_log : "membership_audit_log_team_id_fkey:team_id"
  teams__teams ||--o{ teams__memberships : "memberships_team_id_fkey:team_id"
  teams__teams ||--o{ teams__team_cbu_access : "team_cbu_access_team_id_fkey:team_id"
  teams__teams ||--o{ teams__team_service_entitlements : "team_service_entitlements_team_id_fkey:team_id"
```
