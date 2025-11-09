-- 09_populate_semantic_verbs_fixed.sql
-- Phase 3: Populate Semantic Verb Registry with Rich Metadata (Fixed)
--
-- This script populates the semantic verb registry with comprehensive metadata
-- for existing verbs, enabling deterministic agentic DSL construction.

-- First, ensure we have the base vocabulary entries
INSERT INTO "ob-poc".domain_vocabularies (domain, verb, category, description, parameters, examples, active)
VALUES
-- Core Onboarding Verbs
('onboarding', 'case.create', 'case_management', 'Create a new onboarding case',
 '{"cbu.id": {"type": "string", "required": true}, "nature-purpose": {"type": "string", "required": true}}',
 '[{"usage": "(case.create (cbu.id \"CBU-1234\") (nature-purpose \"UCITS fund\"))"}]', true),

('onboarding', 'products.add', 'product_configuration', 'Add products to an existing case',
 '{"products": {"type": "array", "items": "string", "required": true}}',
 '[{"usage": "(products.add \"CUSTODY\" \"FUND_ACCOUNTING\")"}]', true),

('onboarding', 'services.discover', 'service_planning', 'Discover required services for products',
 '{"for.product": {"type": "string", "required": true}, "service": {"type": "array", "items": "string"}}',
 '[{"usage": "(services.discover (for.product \"CUSTODY\" (service \"CustodyService\")))"}]', true),

('onboarding', 'resources.plan', 'resource_management', 'Plan resource allocation and provisioning',
 '{"resource.create": {"type": "string", "required": true}, "owner": {"type": "string"}}',
 '[{"usage": "(resources.plan (resource.create \"Account\" (owner \"CustodyTech\")))"}]', true),

('onboarding', 'values.bind', 'data_binding', 'Bind attribute values to variables',
 '{"bind": {"type": "object", "required": true}, "attr-id": {"type": "uuid"}, "value": {"type": "any"}}',
 '[{"usage": "(values.bind (bind (attr-id \"uuid\") (value \"CBU-1234\")))"}]', true),

-- KYC Domain Verbs
('kyc', 'kyc.start', 'compliance', 'Initialize KYC process with required documents',
 '{"documents": {"type": "array", "items": "string"}, "jurisdictions": {"type": "array", "items": "string"}}',
 '[{"usage": "(kyc.start (documents (document \"W8BEN-E\")) (jurisdictions (jurisdiction \"US\")))"}]', true),

('kyc', 'kyc.validate', 'compliance', 'Validate KYC documentation completeness',
 '{"documents": {"type": "array", "items": "string"}, "criteria": {"type": "string"}}',
 '[{"usage": "(kyc.validate (documents (document \"W8BEN-E\")) (criteria \"completeness\"))"}]', true),

-- UBO Domain Verbs
('ubo', 'ubo.collect-entity-data', 'data_collection', 'Collect entity ownership data',
 '{"entity": {"type": "string", "required": true}, "jurisdiction": {"type": "string"}}',
 '[{"usage": "(ubo.collect-entity-data (entity \"Holdings Ltd\") (jurisdiction \"GB\"))"}]', true),

('ubo', 'ubo.get-ownership-structure', 'data_analysis', 'Retrieve ownership structure for entity',
 '{"entity": {"type": "string", "required": true}}',
 '[{"usage": "(ubo.get-ownership-structure (entity \"Holdings Ltd\"))"}]', true),

('ubo', 'ubo.calculate-indirect-ownership', 'calculation', 'Calculate indirect ownership percentages',
 '{}',
 '[{"usage": "(ubo.calculate-indirect-ownership)"}]', true),

('ubo', 'ubo.apply-thresholds', 'compliance', 'Apply UBO identification thresholds',
 '{"threshold": {"type": "number", "required": true}, "framework": {"type": "string"}}',
 '[{"usage": "(ubo.apply-thresholds (threshold 25.0) (framework \"EU_5MLD\"))"}]', true),

-- Workflow Management Verbs
('workflow', 'workflow.transition', 'state_management', 'Transition workflow to new state',
 '{"state": {"type": "string", "required": true}}',
 '[{"usage": "(workflow.transition \"KYC_COMPLETE\")"}]', true),

('compliance', 'compliance.screen', 'compliance', 'Screen entities against sanctions lists',
 '{"target": {"type": "string", "required": true}, "type": {"type": "string"}}',
 '[{"usage": "(compliance.screen (target \"UBOs\") (type \"AML\"))"}]', true)

ON CONFLICT (domain, verb) DO NOTHING;

-- Now populate rich semantic metadata
INSERT INTO "ob-poc".verb_semantics (
    domain, verb, semantic_description, intent_category, business_purpose,
    side_effects, prerequisites, postconditions, resource_requirements,
    agent_prompt, usage_patterns, selection_criteria,
    parameter_semantics, workflow_stage,
    typical_predecessors, typical_successors,
    compliance_implications, confidence_score
) VALUES

-- case.create semantic metadata
('onboarding', 'case.create',
 'Initiates a new client onboarding case by creating the foundational business unit record with identifying information',
 'create',
 'Establishes the legal and business foundation for a new client relationship, enabling all subsequent onboarding activities',
 ARRAY['Creates new CBU record in database', 'Generates unique case identifier', 'Sets initial onboarding state'],
 ARRAY[]::text[],
 ARRAY['CBU record exists', 'Case state is CREATED', 'Subsequent operations can reference this CBU'],
 ARRAY['Database write access', 'CBU identifier uniqueness validation'],
 'Use this verb to start any new client onboarding process. It creates the fundamental business record that all other operations will reference.',
 ARRAY['Always first verb in onboarding workflow', 'Followed by products.add', 'Required before any other case operations'],
 'Choose this verb when starting a completely new client onboarding case. Never use if case already exists.',
 '{"cbu.id": {"semantic_type": "business_identifier", "purpose": "Unique identifier for this business unit", "validation": "Must be unique across system"}, "nature-purpose": {"semantic_type": "business_description", "purpose": "Describes the fundamental business nature of this entity", "examples": ["UCITS fund", "Hedge fund", "Corporate entity"]}}',
 'initialization',
 ARRAY[]::text[],
 ARRAY['products.add', 'kyc.start'],
 ARRAY['Establishes audit trail', 'Creates regulatory reporting obligation'],
 0.95),

-- products.add semantic metadata
('onboarding', 'products.add',
 'Configures the specific financial products and services that will be provided to the client',
 'update',
 'Defines the scope of services and drives downstream KYC, compliance, and operational requirements',
 ARRAY['Updates case product configuration', 'Triggers product-specific compliance requirements', 'Enables service discovery'],
 ARRAY['case.create must be completed', 'Products must be valid and available'],
 ARRAY['Product list updated', 'Service requirements can be determined', 'KYC requirements may change'],
 ARRAY['Product catalog access', 'Service mapping data'],
 'Add this verb after case creation to specify what products the client will use. Each product has different regulatory and operational implications.',
 ARRAY['Always after case.create', 'Before services.discover', 'Can be updated multiple times'],
 'Use when you need to specify or modify the products a client will receive. Essential for determining compliance requirements.',
 '{"products": {"semantic_type": "product_list", "purpose": "List of financial products to provide", "validation": "Must be valid product names from catalog", "business_impact": "Determines regulatory requirements and operational complexity"}}',
 'configuration',
 ARRAY['case.create'],
 ARRAY['services.discover', 'kyc.start'],
 ARRAY['Product-specific regulations apply', 'May trigger enhanced due diligence'],
 0.90),

-- kyc.start semantic metadata
('kyc', 'kyc.start',
 'Initiates the Know Your Customer compliance process with specific documentation and jurisdictional requirements',
 'validate',
 'Ensures regulatory compliance by collecting and validating required client identification and verification documents',
 ARRAY['Creates KYC obligation record', 'Triggers document collection workflow', 'Sets compliance timeline'],
 ARRAY['Client entity identified', 'Products selected', 'Jurisdictional scope determined'],
 ARRAY['KYC process active', 'Document requirements defined', 'Compliance timeline established'],
 ARRAY['Document validation system', 'Regulatory rule engine', 'Jurisdiction-specific requirements'],
 'Start KYC process when you have identified the client and their products. This determines what documents are needed and starts the compliance clock.',
 ARRAY['After products.add', 'Before services can be activated', 'May trigger enhanced due diligence'],
 'Use when compliance documentation is required. Mandatory for most financial services. Choose based on entity type and jurisdiction.',
 '{"documents": {"semantic_type": "compliance_documents", "purpose": "Required legal documents for verification", "validation": "Must be valid document types for jurisdiction"}, "jurisdictions": {"semantic_type": "regulatory_scope", "purpose": "Regulatory jurisdictions that apply", "business_impact": "Determines applicable regulations and requirements"}}',
 'compliance',
 ARRAY['products.add'],
 ARRAY['kyc.validate', 'compliance.screen'],
 ARRAY['Regulatory compliance requirement', 'Timeline obligations', 'Audit trail creation', 'Privacy regulations apply'],
 0.98),

-- services.discover semantic metadata
('onboarding', 'services.discover',
 'Automatically determines and configures the technical services required to support the selected products',
 'transform',
 'Translates business product selections into specific operational service requirements and technical implementations',
 ARRAY['Creates service configuration', 'Triggers resource allocation', 'Defines operational dependencies'],
 ARRAY['Products defined', 'Service catalog available'],
 ARRAY['Services mapped to products', 'Resource requirements identified', 'Operational plan established'],
 ARRAY['Service catalog', 'Product-service mapping rules', 'Resource planning system'],
 'Use this to automatically determine what technical services are needed based on the products selected. It creates the operational blueprint.',
 ARRAY['After products.add', 'Before resources.plan', 'Drives operational planning'],
 'Choose when you need to translate product selections into specific service requirements. Essential for operational planning.',
 '{"for.product": {"semantic_type": "product_reference", "purpose": "Product to discover services for", "validation": "Must be previously added product"}, "service": {"semantic_type": "service_list", "purpose": "Technical services required", "business_impact": "Determines operational complexity and cost"}}',
 'service_planning',
 ARRAY['products.add'],
 ARRAY['resources.plan'],
 ARRAY['Service level agreements apply', 'Operational risk considerations'],
 0.85),

-- ubo.apply-thresholds semantic metadata
('ubo', 'ubo.apply-thresholds',
 'Applies regulatory threshold rules to identify Ultimate Beneficial Owners based on ownership percentages and control factors',
 'validate',
 'Ensures compliance with anti-money laundering regulations by correctly identifying individuals who ultimately own or control the entity',
 ARRAY['Identifies UBO candidates', 'Creates compliance determination', 'Triggers enhanced due diligence if needed'],
 ARRAY['Ownership structure calculated', 'Control factors identified', 'Regulatory framework specified'],
 ARRAY['UBOs identified', 'Compliance status determined', 'Enhanced due diligence requirements established'],
 ARRAY['Regulatory rules engine', 'Ownership calculation data', 'Control assessment data'],
 'Apply UBO thresholds after calculating ownership percentages to determine who must be identified as Ultimate Beneficial Owners under regulations.',
 ARRAY['After ubo.calculate-indirect-ownership', 'Before ubo.resolve-ubos', 'Critical for AML compliance'],
 'Use when you have complete ownership data and need to determine UBO status. Essential for regulatory compliance.',
 '{"threshold": {"semantic_type": "percentage_threshold", "purpose": "Ownership percentage that triggers UBO status", "validation": "Must be valid regulatory threshold (typically 25%)"}, "framework": {"semantic_type": "regulatory_framework", "purpose": "Applicable regulatory standard", "examples": ["EU_5MLD", "US_CDD", "UK_MLR"]}}',
 'compliance',
 ARRAY['ubo.calculate-indirect-ownership'],
 ARRAY['ubo.resolve-ubos', 'compliance.screen'],
 ARRAY['AML regulation compliance', 'UBO disclosure requirements', 'Enhanced due diligence triggers'],
 0.95),

-- workflow.transition semantic metadata
('workflow', 'workflow.transition',
 'Advances the onboarding case through its lifecycle states, enforcing business rules and triggering appropriate next actions',
 'update',
 'Manages the progression of onboarding cases through defined stages, ensuring proper sequencing and completeness checks',
 ARRAY['Updates case state', 'Triggers state-specific validations', 'Enables next workflow activities'],
 ARRAY['Current state requirements met', 'Transition rules satisfied'],
 ARRAY['New state active', 'State-specific capabilities enabled', 'Progress tracking updated'],
 ARRAY['Workflow engine', 'State validation rules', 'Business process definitions'],
 'Use this to move cases through the onboarding lifecycle. Each state has specific requirements and enables different activities.',
 ARRAY['Throughout workflow', 'After completing state requirements', 'Enables progress tracking'],
 'Choose when case has completed current state requirements and is ready for next stage. Critical for workflow management.',
 '{"state": {"semantic_type": "workflow_state", "purpose": "Target state to transition to", "validation": "Must be valid next state from current position", "examples": ["CREATED", "KYC_DISCOVERED", "SERVICES_PLANNED", "COMPLETE"]}}',
 'state_management',
 ARRAY[]::text[],
 ARRAY[]::text[],
 ARRAY['Audit trail requirements', 'Progress tracking obligations'],
 0.90);

-- Insert verb relationships to model workflow dependencies
INSERT INTO "ob-poc".verb_relationships (
    source_domain, source_verb, target_domain, target_verb,
    relationship_type, relationship_strength, sequence_type,
    business_rationale, agent_explanation
) VALUES

-- Core onboarding sequence
('onboarding', 'case.create', 'onboarding', 'products.add', 'enables', 0.95, 'before',
 'Case must exist before products can be added',
 'Always create the case first, then add products. Products cannot exist without a case.'),

('onboarding', 'products.add', 'onboarding', 'services.discover', 'enables', 0.90, 'before',
 'Product selection determines service requirements',
 'Services are discovered based on products selected. Define products before discovering services.'),

('onboarding', 'products.add', 'kyc', 'kyc.start', 'suggests', 0.85, 'before',
 'Product selection influences KYC requirements',
 'Different products have different KYC requirements. Select products before starting KYC.'),

('onboarding', 'services.discover', 'onboarding', 'resources.plan', 'enables', 0.88, 'before',
 'Service requirements determine resource needs',
 'Resources are planned based on discovered services. Discover services before planning resources.'),

-- UBO workflow sequence
('ubo', 'ubo.collect-entity-data', 'ubo', 'ubo.get-ownership-structure', 'enables', 0.95, 'before',
 'Entity data required for ownership analysis',
 'Collect entity data first, then analyze ownership structure from that data.'),

('ubo', 'ubo.get-ownership-structure', 'ubo', 'ubo.calculate-indirect-ownership', 'enables', 0.92, 'before',
 'Structure data required for ownership calculations',
 'Get the ownership structure before calculating indirect ownership percentages.'),

('ubo', 'ubo.calculate-indirect-ownership', 'ubo', 'ubo.apply-thresholds', 'enables', 0.98, 'before',
 'Ownership percentages required for threshold application',
 'Calculate all ownership percentages before applying regulatory thresholds to identify UBOs.'),

-- Cross-domain relationships
('kyc', 'kyc.start', 'ubo', 'ubo.collect-entity-data', 'suggests', 0.75, 'parallel',
 'KYC and UBO processes often run in parallel',
 'KYC and UBO discovery can happen simultaneously but both are often required for complete onboarding.'),

('ubo', 'ubo.apply-thresholds', 'compliance', 'compliance.screen', 'enables', 0.90, 'before',
 'UBO identification enables compliance screening',
 'Once UBOs are identified, they must be screened against sanctions and watch lists.');

-- Insert common usage patterns
INSERT INTO "ob-poc".verb_patterns (
    pattern_name, pattern_category, pattern_description, pattern_template,
    use_cases, business_scenarios, complexity_level,
    required_verbs, agent_selection_rules
) VALUES

('Basic Onboarding Flow', 'workflow',
 'Standard client onboarding sequence for most financial services clients',
 '(case.create (cbu.id "{cbu_id}") (nature-purpose "{nature_purpose}"))
(products.add {product_list})
(services.discover (for.product "{primary_product}" {service_list}))
(kyc.start (documents {document_list}) (jurisdictions {jurisdiction_list}))',
 ARRAY['New client onboarding', 'Standard financial services setup'],
 ARRAY['UCITS fund onboarding', 'Corporate banking setup', 'Investment management client'],
 'beginner',
 ARRAY['case.create', 'products.add', 'services.discover', 'kyc.start'],
 'Use for any new client that needs standard onboarding with products and KYC compliance.'),

('UBO Discovery Complete', 'workflow',
 'Complete Ultimate Beneficial Owner identification workflow',
 '(ubo.collect-entity-data (entity "{entity_name}") (jurisdiction "{jurisdiction}"))
(ubo.get-ownership-structure (entity "{entity_name}"))
(ubo.calculate-indirect-ownership)
(ubo.apply-thresholds (threshold {threshold_percent}) (framework "{regulatory_framework}"))
(compliance.screen (target "UBOs") (type "AML"))',
 ARRAY['AML compliance', 'Corporate transparency', 'Enhanced due diligence'],
 ARRAY['Complex corporate structures', 'High-risk jurisdictions', 'Regulatory reporting'],
 'advanced',
 ARRAY['ubo.collect-entity-data', 'ubo.get-ownership-structure', 'ubo.calculate-indirect-ownership', 'ubo.apply-thresholds'],
 'Use when client has complex ownership structure and regulatory UBO identification is required.'),

('Product Configuration Update', 'update',
 'Pattern for modifying client product configuration after initial setup',
 '(products.add {additional_products})
(services.discover (for.product "{new_product}" {additional_services}))
(workflow.transition "SERVICES_UPDATED")',
 ARRAY['Product expansion', 'Service additions', 'Client requirements change'],
 ARRAY['Client adds custody to existing fund accounting', 'Adding transfer agency services'],
 'intermediate',
 ARRAY['products.add', 'services.discover'],
 'Use when existing client wants to add new products or services to their current setup.');

-- Insert decision rules for agent guidance
INSERT INTO "ob-poc".verb_decision_rules (
    rule_name, rule_type, condition_expression, action_expression,
    applicable_domains, business_context, llm_prompt_addition
) VALUES

('Case Creation First Rule', 'sequencing',
 'current_dsl_empty OR no_case_create_found',
 'suggest_verb("case.create") AND require_before_other_verbs',
 ARRAY['onboarding'],
 'All onboarding workflows must start with case creation',
 'Always start onboarding with case.create. No other onboarding verbs work without it.'),

('Products Before Services Rule', 'sequencing',
 'products_defined AND no_services_discovered',
 'suggest_verb("services.discover")',
 ARRAY['onboarding'],
 'Services must be discovered after products are defined',
 'After adding products, use services.discover to determine what technical services are needed.'),

('UBO Threshold Application Rule', 'validation',
 'ownership_calculated AND no_thresholds_applied',
 'suggest_verb("ubo.apply-thresholds")',
 ARRAY['ubo'],
 'UBO identification requires threshold application after ownership calculation',
 'After calculating ownership percentages, apply regulatory thresholds to identify UBOs.'),

('KYC Document Jurisdiction Match', 'parameter_binding',
 'jurisdiction_specified AND document_requirements_mismatch',
 'validate_documents_for_jurisdiction',
 ARRAY['kyc'],
 'Document requirements vary by jurisdiction',
 'Ensure document requirements match the specified jurisdictions. Different countries have different requirements.');

-- Success message
DO $$
BEGIN
    RAISE NOTICE 'Semantic verb registry populated successfully!';
    RAISE NOTICE '';
    RAISE NOTICE 'Data populated:';
    RAISE NOTICE '- % verb definitions with rich semantics', (SELECT COUNT(*) FROM "ob-poc".verb_semantics);
    RAISE NOTICE '- % verb relationships for workflow modeling', (SELECT COUNT(*) FROM "ob-poc".verb_relationships);
    RAISE NOTICE '- % usage patterns for agent guidance', (SELECT COUNT(*) FROM "ob-poc".verb_patterns);
    RAISE NOTICE '- % decision rules for agent validation', (SELECT COUNT(*) FROM "ob-poc".verb_decision_rules);
    RAISE NOTICE '';
    RAISE NOTICE 'Agents now have access to:';
    RAISE NOTICE '• Rich semantic context for each verb';
    RAISE NOTICE '• Workflow sequencing rules';
    RAISE NOTICE '• Parameter semantic validation';
    RAISE NOTICE '• Business context and compliance implications';
    RAISE NOTICE '• Historical usage patterns and success rates';
    RAISE NOTICE '';
    RAISE NOTICE 'Next: Update agent implementations to query v_agent_verb_context view';
END $$;
