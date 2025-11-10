-- 13_isda_dsl_domain_fixed.sql
-- ISDA DSL Domain and Derivative Workflow Verbs
--
-- This script creates a comprehensive ISDA DSL domain for managing
-- derivative contracts, master agreements, and related workflows.

-- ============================================================================
-- ISDA DOMAIN REGISTRATION
-- ============================================================================

-- Register the ISDA domain
INSERT INTO "ob-poc".dsl_domains (domain_name, description, base_grammar_version, vocabulary_version)
VALUES ('ISDA', 'ISDA Master Agreements and derivative contract workflows', '3.0.0', '1.0.0')
ON CONFLICT (domain_name) DO NOTHING;

-- ============================================================================
-- ISDA DSL VERBS
-- ============================================================================

-- Core ISDA workflow verbs
INSERT INTO "ob-poc".domain_vocabularies (domain, verb, category, description, parameters, examples, active) VALUES

-- Master Agreement establishment
('isda', 'isda.establish_master', 'agreement_setup', 'Establish ISDA Master Agreement between counterparties',
 '{
   ":agreement-id": {"type": "string", "required": true, "description": "Unique identifier for master agreement"},
   ":party-a": {"type": "string", "required": true, "description": "Legal entity identifier for Party A"},
   ":party-b": {"type": "string", "required": true, "description": "Legal entity identifier for Party B"},
   ":version": {"type": "string", "required": true, "description": "ISDA Master Agreement version (e.g. 2002, 1992)"},
   ":governing-law": {"type": "string", "required": true, "description": "Governing law jurisdiction"},
   ":agreement-date": {"type": "date", "required": true, "description": "Date of agreement execution"},
   ":multicurrency": {"type": "boolean", "required": false, "description": "Whether multicurrency cross-default applies"},
   ":cross-default": {"type": "boolean", "required": false, "description": "Whether cross-default provisions apply"},
   ":termination-currency": {"type": "string", "required": false, "description": "Currency for termination payments"},
   ":document-id": {"type": "string", "required": false, "description": "Reference to cataloged document"},
   ":effective-date": {"type": "date", "required": false, "description": "When agreement becomes effective"}
 }',
 '[{"usage": "(isda.establish_master :agreement-id \"ISDA-ZENITH-JPM-001\" :party-a \"company-zenith-spv-001\" :party-b \"jpmorgan-chase-entity\" :version \"2002\" :governing-law \"NY\" :agreement-date \"2023-01-15\" :multicurrency true :cross-default true :document-id \"doc-isda-master-zenith-001\")"}]',
 true),

-- Credit Support Annex setup
('isda', 'isda.establish_csa', 'collateral_management', 'Establish Credit Support Annex for collateral management',
 '{
   ":csa-id": {"type": "string", "required": true, "description": "Unique identifier for CSA"},
   ":master-agreement-id": {"type": "string", "required": true, "description": "Reference to master agreement"},
   ":base-currency": {"type": "string", "required": true, "description": "Base currency for calculations"},
   ":threshold-party-a": {"type": "number", "required": true, "description": "Threshold amount for Party A"},
   ":threshold-party-b": {"type": "number", "required": true, "description": "Threshold amount for Party B"},
   ":minimum-transfer": {"type": "number", "required": true, "description": "Minimum transfer amount"},
   ":rounding-amount": {"type": "number", "required": false, "description": "Amount to round transfers"},
   ":eligible-collateral": {"type": "array", "required": true, "description": "Types of eligible collateral"},
   ":valuation-percentage": {"type": "map", "required": false, "description": "Haircuts by collateral type"},
   ":margin-approach": {"type": "string", "required": true, "description": "VM (variation margin) or IM (initial margin)"},
   ":document-id": {"type": "string", "required": false, "description": "Reference to cataloged CSA document"},
   ":effective-date": {"type": "date", "required": true, "description": "CSA effective date"}
 }',
 '[{"usage": "(isda.establish_csa :csa-id \"CSA-ZENITH-JPM-001\" :master-agreement-id \"ISDA-ZENITH-JPM-001\" :base-currency \"USD\" :threshold-party-a 0 :threshold-party-b 5000000 :minimum-transfer 100000 :eligible-collateral [\"cash_usd\" \"us_treasury\"] :margin-approach \"VM\" :effective-date \"2023-01-15\")"}]',
 true),

-- Trade execution and confirmation
('isda', 'isda.execute_trade', 'trade_execution', 'Execute derivative trade under ISDA Master Agreement',
 '{
   ":trade-id": {"type": "string", "required": true, "description": "Unique trade identifier"},
   ":master-agreement-id": {"type": "string", "required": true, "description": "Governing master agreement"},
   ":product-type": {"type": "string", "required": true, "description": "Derivative product type (IRS, CDS, FX_Forward, etc.)"},
   ":trade-date": {"type": "date", "required": true, "description": "Date trade was executed"},
   ":effective-date": {"type": "date", "required": true, "description": "Trade start date"},
   ":termination-date": {"type": "date", "required": true, "description": "Trade maturity date"},
   ":notional-amount": {"type": "number", "required": true, "description": "Trade notional amount"},
   ":currency": {"type": "string", "required": true, "description": "Trade currency"},
   ":payer": {"type": "string", "required": true, "description": "Entity paying fixed/premium"},
   ":receiver": {"type": "string", "required": true, "description": "Entity receiving fixed/premium"},
   ":underlying": {"type": "string", "required": false, "description": "Underlying reference (rate, bond, etc.)"},
   ":calculation-agent": {"type": "string", "required": false, "description": "Entity responsible for calculations"},
   ":settlement-terms": {"type": "map", "required": false, "description": "Settlement and payment terms"},
   ":confirmation-id": {"type": "string", "required": false, "description": "Trade confirmation document ID"}
 }',
 '[{"usage": "(isda.execute_trade :trade-id \"TRADE-IRS-001\" :master-agreement-id \"ISDA-ZENITH-JPM-001\" :product-type \"IRS\" :trade-date \"2024-03-15\" :effective-date \"2024-03-17\" :termination-date \"2029-03-17\" :notional-amount 50000000 :currency \"USD\" :payer \"company-zenith-spv-001\" :receiver \"jpmorgan-chase-entity\" :underlying \"USD-LIBOR-3M\" :calculation-agent \"jpmorgan-chase-entity\")"}]',
 true),

-- Margin call and collateral posting
('isda', 'isda.margin_call', 'collateral_management', 'Issue margin call for collateral posting',
 '{
   ":call-id": {"type": "string", "required": true, "description": "Unique margin call identifier"},
   ":csa-id": {"type": "string", "required": true, "description": "Governing CSA"},
   ":call-date": {"type": "date", "required": true, "description": "Date of margin call"},
   ":valuation-date": {"type": "date", "required": true, "description": "Portfolio valuation date"},
   ":calling-party": {"type": "string", "required": true, "description": "Party making the call"},
   ":called-party": {"type": "string", "required": true, "description": "Party receiving the call"},
   ":exposure-amount": {"type": "number", "required": true, "description": "Current mark-to-market exposure"},
   ":existing-collateral": {"type": "number", "required": true, "description": "Value of existing collateral"},
   ":call-amount": {"type": "number", "required": true, "description": "Amount being called"},
   ":currency": {"type": "string", "required": true, "description": "Call currency"},
   ":deadline": {"type": "datetime", "required": true, "description": "Deadline for posting collateral"},
   ":calculation-details": {"type": "map", "required": false, "description": "Detailed calculation breakdown"}
 }',
 '[{"usage": "(isda.margin_call :call-id \"MC-001-20241115\" :csa-id \"CSA-ZENITH-JPM-001\" :call-date \"2024-11-15\" :valuation-date \"2024-11-14\" :calling-party \"jpmorgan-chase-entity\" :called-party \"company-zenith-spv-001\" :exposure-amount 8500000 :existing-collateral 3000000 :call-amount 5000000 :currency \"USD\" :deadline \"2024-11-16T17:00:00Z\")"}]',
 true),

-- Collateral posting response
('isda', 'isda.post_collateral', 'collateral_management', 'Post collateral in response to margin call',
 '{
   ":posting-id": {"type": "string", "required": true, "description": "Unique collateral posting identifier"},
   ":call-id": {"type": "string", "required": true, "description": "Reference to margin call"},
   ":posting-party": {"type": "string", "required": true, "description": "Party posting collateral"},
   ":receiving-party": {"type": "string", "required": true, "description": "Party receiving collateral"},
   ":collateral-type": {"type": "string", "required": true, "description": "Type of collateral posted"},
   ":amount": {"type": "number", "required": true, "description": "Amount of collateral posted"},
   ":currency": {"type": "string", "required": true, "description": "Collateral currency"},
   ":posting-date": {"type": "date", "required": true, "description": "Date collateral was posted"},
   ":settlement-date": {"type": "date", "required": true, "description": "Date collateral settles"},
   ":custodian": {"type": "string", "required": false, "description": "Collateral custodian"},
   ":valuation": {"type": "number", "required": false, "description": "Collateral valuation after haircuts"}
 }',
 '[{"usage": "(isda.post_collateral :posting-id \"POST-001-20241116\" :call-id \"MC-001-20241115\" :posting-party \"company-zenith-spv-001\" :receiving-party \"jpmorgan-chase-entity\" :collateral-type \"cash_usd\" :amount 5000000 :currency \"USD\" :posting-date \"2024-11-16\" :settlement-date \"2024-11-16\" :valuation 5000000)"}]',
 true),

-- Portfolio valuation and mark-to-market
('isda', 'isda.value_portfolio', 'valuation', 'Value derivative portfolio for risk management and collateral calculations',
 '{
   ":valuation-id": {"type": "string", "required": true, "description": "Unique valuation identifier"},
   ":portfolio-id": {"type": "string", "required": true, "description": "Portfolio being valued"},
   ":valuation-date": {"type": "date", "required": true, "description": "Date of valuation"},
   ":valuation-agent": {"type": "string", "required": true, "description": "Entity performing valuation"},
   ":methodology": {"type": "string", "required": true, "description": "Valuation methodology used"},
   ":base-currency": {"type": "string", "required": true, "description": "Base currency for reporting"},
   ":trades-valued": {"type": "array", "required": true, "description": "List of trade IDs valued"},
   ":gross-mtm": {"type": "number", "required": true, "description": "Gross mark-to-market value"},
   ":net-mtm": {"type": "number", "required": true, "description": "Net mark-to-market after netting"},
   ":market-data-sources": {"type": "array", "required": false, "description": "Sources of market data used"},
   ":calculation-details": {"type": "map", "required": false, "description": "Detailed valuation breakdown"}
 }',
 '[{"usage": "(isda.value_portfolio :valuation-id \"VAL-20241115\" :portfolio-id \"PORTFOLIO-ZENITH-JPM\" :valuation-date \"2024-11-15\" :valuation-agent \"jpmorgan-chase-entity\" :methodology \"market_standard\" :base-currency \"USD\" :trades-valued [\"TRADE-IRS-001\", \"TRADE-IRS-002\"] :gross-mtm 8500000 :net-mtm 8500000)"}]',
 true),

-- Termination event declaration
('isda', 'isda.declare_termination_event', 'risk_management', 'Declare termination event under ISDA Master Agreement',
 '{
   ":event-id": {"type": "string", "required": true, "description": "Unique termination event identifier"},
   ":master-agreement-id": {"type": "string", "required": true, "description": "Affected master agreement"},
   ":event-type": {"type": "string", "required": true, "description": "Type of termination event"},
   ":affected-party": {"type": "string", "required": true, "description": "Party subject to termination event"},
   ":declaring-party": {"type": "string", "required": true, "description": "Party declaring the event"},
   ":event-date": {"type": "date", "required": true, "description": "Date event occurred"},
   ":declaration-date": {"type": "date", "required": true, "description": "Date event was declared"},
   ":cure-period": {"type": "number", "required": false, "description": "Days to cure the event"},
   ":event-description": {"type": "string", "required": true, "description": "Description of the event"},
   ":supporting-evidence": {"type": "array", "required": false, "description": "Supporting documentation"},
   ":automatic": {"type": "boolean", "required": false, "description": "Whether termination is automatic"}
 }',
 '[{"usage": "(isda.declare_termination_event :event-id \"TERM-EVENT-001\" :master-agreement-id \"ISDA-ZENITH-JPM-001\" :event-type \"failure_to_pay\" :affected-party \"company-zenith-spv-001\" :declaring-party \"jpmorgan-chase-entity\" :event-date \"2024-11-10\" :declaration-date \"2024-11-15\" :cure-period 3 :event-description \"Failure to post required collateral within deadline\")"}]',
 true),

-- Early termination and close-out
('isda', 'isda.close_out', 'termination', 'Perform close-out calculation and early termination',
 '{
   ":closeout-id": {"type": "string", "required": true, "description": "Unique close-out identifier"},
   ":master-agreement-id": {"type": "string", "required": true, "description": "Master agreement being terminated"},
   ":termination-date": {"type": "date", "required": true, "description": "Early termination date"},
   ":calculation-agent": {"type": "string", "required": true, "description": "Entity performing close-out calculation"},
   ":terminated-trades": {"type": "array", "required": true, "description": "Trades being terminated"},
   ":valuation-method": {"type": "string", "required": true, "description": "Close-out valuation method"},
   ":market-quotations": {"type": "array", "required": false, "description": "Market quotations obtained"},
   ":loss-calculation": {"type": "string", "required": true, "description": "Loss calculation methodology"},
   ":closeout-amount": {"type": "number", "required": true, "description": "Net close-out amount"},
   ":payment-currency": {"type": "string", "required": true, "description": "Currency for settlement"},
   ":payment-date": {"type": "date", "required": true, "description": "Date payment is due"},
   ":calculation-statement": {"type": "string", "required": false, "description": "Reference to calculation statement document"}
 }',
 '[{"usage": "(isda.close_out :closeout-id \"CLOSEOUT-001\" :master-agreement-id \"ISDA-ZENITH-JPM-001\" :termination-date \"2024-11-18\" :calculation-agent \"jpmorgan-chase-entity\" :terminated-trades [\"TRADE-IRS-001\"] :valuation-method \"market_quotation\" :loss-calculation \"first_method\" :closeout-amount 2500000 :payment-currency \"USD\" :payment-date \"2024-11-20\")"}]',
 true),

-- Amendment to agreements
('isda', 'isda.amend_agreement', 'agreement_modification', 'Amend existing ISDA Master Agreement or related documentation',
 '{
   ":amendment-id": {"type": "string", "required": true, "description": "Unique amendment identifier"},
   ":original-agreement-id": {"type": "string", "required": true, "description": "Agreement being amended"},
   ":amendment-type": {"type": "string", "required": true, "description": "Type of amendment"},
   ":amendment-date": {"type": "date", "required": true, "description": "Date of amendment"},
   ":effective-date": {"type": "date", "required": true, "description": "When amendment becomes effective"},
   ":sections-amended": {"type": "array", "required": true, "description": "Sections being modified"},
   ":amendment-description": {"type": "string", "required": true, "description": "Description of changes"},
   ":party-a-consent": {"type": "boolean", "required": true, "description": "Party A consent status"},
   ":party-b-consent": {"type": "boolean", "required": true, "description": "Party B consent status"},
   ":document-id": {"type": "string", "required": false, "description": "Reference to amendment document"},
   ":supersedes-prior": {"type": "boolean", "required": false, "description": "Whether this supersedes prior amendments"}
 }',
 '[{"usage": "(isda.amend_agreement :amendment-id \"AMEND-001\" :original-agreement-id \"ISDA-ZENITH-JPM-001\" :amendment-type \"schedule_modification\" :amendment-date \"2024-06-15\" :effective-date \"2024-07-01\" :sections-amended [\"Part 4(h)\", \"Part 5(a)\"] :amendment-description \"Updated threshold amounts and eligible collateral\" :party-a-consent true :party-b-consent true)"}]',
 true),

-- Novation (trade transfer)
('isda', 'isda.novate_trade', 'trade_transfer', 'Transfer derivative trade to new counterparty via novation',
 '{
   ":novation-id": {"type": "string", "required": true, "description": "Unique novation identifier"},
   ":original-trade-id": {"type": "string", "required": true, "description": "Trade being transferred"},
   ":transferor": {"type": "string", "required": true, "description": "Party transferring the trade"},
   ":transferee": {"type": "string", "required": true, "description": "Party receiving the trade"},
   ":remaining-party": {"type": "string", "required": true, "description": "Non-transferring counterparty"},
   ":novation-date": {"type": "date", "required": true, "description": "Date of novation"},
   ":effective-date": {"type": "date", "required": true, "description": "When transfer becomes effective"},
   ":consent-required": {"type": "boolean", "required": true, "description": "Whether remaining party consent needed"},
   ":consent-obtained": {"type": "boolean", "required": false, "description": "Whether consent was obtained"},
   ":new-master-agreement": {"type": "string", "required": false, "description": "Master agreement between transferee and remaining party"},
   ":transfer-fee": {"type": "number", "required": false, "description": "Fee for the transfer"},
   ":novation-document": {"type": "string", "required": false, "description": "Reference to novation agreement document"}
 }',
 '[{"usage": "(isda.novate_trade :novation-id \"NOV-001\" :original-trade-id \"TRADE-IRS-001\" :transferor \"company-zenith-spv-001\" :transferee \"alpha-holdings-sg\" :remaining-party \"jpmorgan-chase-entity\" :novation-date \"2024-12-01\" :effective-date \"2024-12-03\" :consent-required true :consent-obtained true :new-master-agreement \"ISDA-ALPHA-JPM-001\")"}]',
 true),

-- Dispute resolution
('isda', 'isda.dispute', 'dispute_management', 'Initiate or manage dispute under ISDA agreement',
 '{
   ":dispute-id": {"type": "string", "required": true, "description": "Unique dispute identifier"},
   ":master-agreement-id": {"type": "string", "required": true, "description": "Governing master agreement"},
   ":dispute-type": {"type": "string", "required": true, "description": "Type of dispute (valuation, calculation, payment, etc.)"},
   ":initiating-party": {"type": "string", "required": true, "description": "Party initiating dispute"},
   ":responding-party": {"type": "string", "required": true, "description": "Party responding to dispute"},
   ":dispute-date": {"type": "date", "required": true, "description": "Date dispute was initiated"},
   ":subject-matter": {"type": "string", "required": true, "description": "Subject of the dispute"},
   ":amount-in-dispute": {"type": "number", "required": false, "description": "Monetary amount in dispute"},
   ":currency": {"type": "string", "required": false, "description": "Currency of disputed amount"},
   ":resolution-method": {"type": "string", "required": false, "description": "Proposed resolution method"},
   ":deadline": {"type": "date", "required": false, "description": "Deadline for resolution"},
   ":status": {"type": "string", "required": true, "description": "Current dispute status"},
   ":supporting-documents": {"type": "array", "required": false, "description": "Supporting documentation"}
 }',
 '[{"usage": "(isda.dispute :dispute-id \"DISP-001\" :master-agreement-id \"ISDA-ZENITH-JPM-001\" :dispute-type \"valuation\" :initiating-party \"company-zenith-spv-001\" :responding-party \"jpmorgan-chase-entity\" :dispute-date \"2024-11-20\" :subject-matter \"Disagreement on IRS valuation methodology\" :amount-in-dispute 1500000 :currency \"USD\" :status \"pending\")"}]',
 true),

-- Netting set management
('isda', 'isda.manage_netting_set', 'netting', 'Manage netting set for close-out and collateral calculations',
 '{
   ":netting-set-id": {"type": "string", "required": true, "description": "Unique netting set identifier"},
   ":master-agreement-id": {"type": "string", "required": true, "description": "Governing master agreement"},
   ":included-trades": {"type": "array", "required": true, "description": "Trades included in netting set"},
   ":excluded-trades": {"type": "array", "required": false, "description": "Trades explicitly excluded"},
   ":netting-date": {"type": "date", "required": true, "description": "Date of netting calculation"},
   ":gross-exposure": {"type": "number", "required": true, "description": "Gross exposure before netting"},
   ":net-exposure": {"type": "number", "required": true, "description": "Net exposure after netting"},
   ":currency": {"type": "string", "required": true, "description": "Base currency for calculations"},
   ":calculation-method": {"type": "string", "required": false, "description": "Netting calculation methodology"},
   ":legal-opinion": {"type": "string", "required": false, "description": "Reference to supporting legal opinion"}
 }',
 '[{"usage": "(isda.manage_netting_set :netting-set-id \"NETTING-ZENITH-JPM\" :master-agreement-id \"ISDA-ZENITH-JPM-001\" :included-trades [\"TRADE-IRS-001\", \"TRADE-IRS-002\"] :netting-date \"2024-11-15\" :gross-exposure 12500000 :net-exposure 8500000 :currency \"USD\")"}]',
 true);

-- ============================================================================
-- VERB REGISTRY ENTRIES FOR ISDA
-- ============================================================================

-- Register ISDA verbs in the global verb registry
INSERT INTO "ob-poc".verb_registry (verb, primary_domain, shared, description) VALUES
('isda.establish_master', 'isda', false, 'Establish ISDA Master Agreement between counterparties'),
('isda.establish_csa', 'isda', false, 'Establish Credit Support Annex for collateral management'),
('isda.execute_trade', 'isda', false, 'Execute derivative trade under ISDA Master Agreement'),
('isda.margin_call', 'isda', false, 'Issue margin call for collateral posting'),
('isda.post_collateral', 'isda', false, 'Post collateral in response to margin call'),
('isda.value_portfolio', 'isda', true, 'Value derivative portfolio for risk and collateral calculations'),
('isda.declare_termination_event', 'isda', false, 'Declare termination event under Master Agreement'),
('isda.close_out', 'isda', false, 'Perform close-out calculation and early termination'),
('isda.amend_agreement', 'isda', false, 'Amend existing ISDA agreements or documentation'),
('isda.novate_trade', 'isda', false, 'Transfer derivative trade via novation'),
('isda.dispute', 'isda', false, 'Initiate or manage disputes under ISDA agreements'),
('isda.manage_netting_set', 'isda', false, 'Manage netting sets for exposure calculations')
ON CONFLICT (verb) DO NOTHING;

-- ============================================================================
-- SEMANTIC VERB METADATA FOR AI AGENTS
-- ============================================================================

-- Add comprehensive semantic metadata for ISDA verbs
INSERT INTO "ob-poc".verb_semantics (
    domain, verb, semantic_description, intent_category, business_purpose,
    side_effects, prerequisites, postconditions, agent_prompt,
    parameter_semantics, workflow_stage, compliance_implications, audit_significance
) VALUES

-- isda.establish_master semantic metadata
('isda', 'isda.establish_master',
 'Creates legal framework for derivative trading by establishing ISDA Master Agreement between two counterparties, defining standard terms, conditions, and dispute resolution mechanisms',
 'create',
 'Establish standardized legal framework that enables efficient derivative trading while managing counterparty credit risk and operational complexity',
 ARRAY['Creates master agreement record', 'Enables derivative trading', 'Establishes legal relationship', 'Creates audit trail'],
 ARRAY['Both parties must be legally capable entities', 'Appropriate legal review and approval', 'Governing law jurisdiction must be specified'],
 ARRAY['Legal framework exists for derivative trading', 'Standard terms are established', 'Risk management framework is in place'],
 'Use this verb to establish the foundational legal agreement that will govern all derivative transactions between two parties. This is typically the first step in setting up a derivatives trading relationship. Ensure all required legal terms are specified.',
 '{
   ":party-a": {"business_meaning": "Legal entity that will be designated as Party A in all derivative transactions", "validation": "Must be valid legal entity with capacity to enter derivatives"},
   ":party-b": {"business_meaning": "Legal entity that will be designated as Party B in all derivative transactions", "validation": "Must be valid legal entity with capacity to enter derivatives"},
   ":governing-law": {"business_meaning": "Legal jurisdiction whose laws will govern the agreement and dispute resolution", "validation": "Must be recognized jurisdiction with derivatives law framework"},
   ":version": {"business_meaning": "Version of ISDA Master Agreement template used, affects available terms and provisions", "validation": "Must be valid ISDA version (1992, 2002, etc.)"}
 }',
 'legal_setup',
 ARRAY['Establishes legal basis for derivative trading', 'Creates enforceability framework', 'Defines credit event and termination procedures'],
 'high'),

-- isda.execute_trade semantic metadata
('isda', 'isda.execute_trade',
 'Records execution of specific derivative transaction under established ISDA Master Agreement, capturing all economic and operational terms required for trade lifecycle management',
 'create',
 'Document derivative trade execution with complete terms to enable accurate valuation, risk management, settlement, and regulatory reporting throughout trade lifecycle',
 ARRAY['Creates trade record', 'Establishes market risk exposure', 'Triggers collateral calculations', 'Creates regulatory reporting obligations'],
 ARRAY['Valid ISDA Master Agreement must exist between parties', 'Trading authorization must be in place', 'Risk limits must be available'],
 ARRAY['Trade is legally binding', 'Risk exposure is established', 'Valuation can be performed', 'Settlement obligations are created'],
 'Use this verb to record derivative trade execution. Ensure all economic terms are captured accurately as they will drive valuation, risk management, and settlement throughout the trade lifecycle. The trade must be executed under an existing ISDA Master Agreement.',
 '{
   ":product-type": {"business_meaning": "Type of derivative product which determines valuation methodology and risk characteristics", "validation": "Must be recognized derivative type (IRS, CDS, FX_Forward, etc.)"},
   ":notional-amount": {"business_meaning": "Reference amount for calculating payments and exposures", "validation": "Must be positive number"},
   ":termination-date": {"business_meaning": "Date when trade matures and final settlement occurs", "validation": "Must be future date and valid business day"},
   ":underlying": {"business_meaning": "Reference rate, security, or index that determines trade value", "validation": "Must be valid and observable market reference"}
 }',
 'trade_execution',
 ARRAY['Creates regulatory reporting obligations', 'Establishes capital requirements', 'May trigger risk limit monitoring'],
 'high'),

-- isda.margin_call semantic metadata
('isda', 'isda.margin_call',
 'Initiates formal request for collateral posting based on mark-to-market exposure calculations and Credit Support Annex terms, managing counterparty credit risk',
 'update',
 'Manage counterparty credit risk by ensuring appropriate collateral is posted to cover mark-to-market exposures as they fluctuate with market conditions',
 ARRAY['Creates collateral posting obligation', 'Triggers counterparty response deadline', 'Updates exposure tracking'],
 ARRAY['Valid CSA must exist', 'Portfolio valuation must be current', 'Exposure must exceed threshold amounts'],
 ARRAY['Counterparty has collateral posting obligation', 'Deadline for response is established', 'Credit risk exposure is documented'],
 'Use this verb when mark-to-market exposure exceeds threshold amounts defined in the CSA. Calculate the required collateral amount carefully as it affects counterparty credit risk. Ensure calculation methodology follows CSA terms.',
 '{
   ":exposure-amount": {"business_meaning": "Current
