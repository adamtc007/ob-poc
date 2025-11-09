-- 08_semantic_verb_registry.sql
-- Phase 3: Semantic Verb Registry for Agentic DSL Construction
--
-- This schema extends the existing vocabulary system with rich semantic metadata
-- to enable deterministic agentic DSL construction and editing.
--
-- OBJECTIVES:
-- 1. Provide AI agents with complete verb context (not just syntax)
-- 2. Enable deterministic DSL generation through semantic understanding
-- 3. Support complex verb relationships and workflows
-- 4. Maintain compatibility with existing EBNF grammar system

-- ============================================================================
-- CORE SEMANTIC VERB REGISTRY
-- ============================================================================

-- Enhanced verb definitions with comprehensive semantic metadata
CREATE TABLE IF NOT EXISTS "ob-poc".verb_semantics (
    semantic_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    domain VARCHAR(100) NOT NULL,
    verb VARCHAR(100) NOT NULL,

    -- Core Semantics
    semantic_description TEXT NOT NULL,    -- What this verb actually does in business terms
    intent_category VARCHAR(50) NOT NULL, -- 'create', 'update', 'validate', 'transform', 'query'
    business_purpose TEXT NOT NULL,       -- Why would someone use this verb
    side_effects TEXT[],                  -- What happens when this verb executes

    -- Execution Context
    prerequisites TEXT[],                 -- What must be true before using this verb
    postconditions TEXT[],               -- What will be true after using this verb
    resource_requirements TEXT[],        -- What resources/data this verb needs
    performance_characteristics JSONB,   -- Timing, cost, complexity metadata

    -- Agent Guidance
    agent_prompt TEXT NOT NULL,          -- How to explain this to an AI agent
    usage_patterns TEXT[],               -- Common patterns where this verb appears
    common_mistakes TEXT[],              -- What agents get wrong with this verb
    selection_criteria TEXT,             -- When should an agent choose this verb

    -- Parameter Semantics (extends beyond EBNF types)
    parameter_semantics JSONB NOT NULL,  -- Rich parameter metadata for each argument
    parameter_validation JSONB,          -- Semantic validation rules beyond type checking
    parameter_examples JSONB,            -- Example parameter values with context

    -- Workflow Integration
    typical_predecessors TEXT[],         -- Verbs that commonly come before this one
    typical_successors TEXT[],           -- Verbs that commonly come after this one
    workflow_stage VARCHAR(100),         -- Which stage of the process this belongs to
    parallel_compatibility TEXT[],       -- Verbs that can run in parallel with this one

    -- Business Rules & Compliance
    compliance_implications TEXT[],      -- Regulatory/compliance considerations
    risk_factors TEXT[],                -- What can go wrong
    approval_requirements TEXT[],        -- What approvals might be needed
    audit_significance VARCHAR(50),      -- 'high', 'medium', 'low', 'none'

    -- Quality & Reliability
    confidence_score DECIMAL(3,2) DEFAULT 1.0,  -- How confident we are in this definition
    last_validated TIMESTAMPTZ,                  -- When this definition was last verified
    validation_notes TEXT,                       -- Notes from last validation

    -- Versioning & Lifecycle
    version VARCHAR(20) DEFAULT '1.0.0',
    status VARCHAR(20) DEFAULT 'active',  -- 'active', 'deprecated', 'experimental'
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),

    -- Link to existing vocabulary system
    FOREIGN KEY (domain, verb) REFERENCES "ob-poc".domain_vocabularies (domain, verb) ON DELETE CASCADE
);

-- Indexes for efficient agent queries
CREATE INDEX IF NOT EXISTS idx_verb_semantics_domain_verb ON "ob-poc".verb_semantics (domain, verb);
CREATE INDEX IF NOT EXISTS idx_verb_semantics_intent ON "ob-poc".verb_semantics (intent_category);
CREATE INDEX IF NOT EXISTS idx_verb_semantics_workflow_stage ON "ob-poc".verb_semantics (workflow_stage);
CREATE INDEX IF NOT EXISTS idx_verb_semantics_status ON "ob-poc".verb_semantics (status);
CREATE INDEX IF NOT EXISTS idx_verb_semantics_confidence ON "ob-poc".verb_semantics (confidence_score DESC);

-- ============================================================================
-- VERB RELATIONSHIP MODELING
-- ============================================================================

-- Explicit verb-to-verb relationships for workflow understanding
CREATE TABLE IF NOT EXISTS "ob-poc".verb_relationships (
    relationship_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    source_domain VARCHAR(100) NOT NULL,
    source_verb VARCHAR(100) NOT NULL,
    target_domain VARCHAR(100) NOT NULL,
    target_verb VARCHAR(100) NOT NULL,

    relationship_type VARCHAR(50) NOT NULL, -- 'requires', 'enables', 'conflicts', 'suggests', 'alternative'
    relationship_strength DECIMAL(3,2) DEFAULT 0.8, -- How strong is this relationship (0.0-1.0)

    context_conditions TEXT[],           -- When does this relationship apply
    business_rationale TEXT,            -- Why does this relationship exist

    -- Workflow sequencing
    sequence_type VARCHAR(20),           -- 'before', 'after', 'parallel', 'alternative'
    timing_constraints TEXT,             -- How close together should these execute

    -- Agent guidance
    agent_explanation TEXT,              -- How to explain this relationship to an AI
    violation_consequences TEXT,         -- What happens if this relationship is ignored

    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),

    FOREIGN KEY (source_domain, source_verb) REFERENCES "ob-poc".domain_vocabularies (domain, verb),
    FOREIGN KEY (target_domain, target_verb) REFERENCES "ob-poc".domain_vocabularies (domain, verb)
);

CREATE INDEX IF NOT EXISTS idx_verb_relationships_source ON "ob-poc".verb_relationships (source_domain, source_verb);
CREATE INDEX IF NOT EXISTS idx_verb_relationships_target ON "ob-poc".verb_relationships (target_domain, target_verb);
CREATE INDEX IF NOT EXISTS idx_verb_relationships_type ON "ob-poc".verb_relationships (relationship_type);
CREATE INDEX IF NOT EXISTS idx_verb_relationships_strength ON "ob-poc".verb_relationships (relationship_strength DESC);

-- ============================================================================
-- CONTEXT PATTERN LIBRARY
-- ============================================================================

-- Common usage patterns and templates for agent reference
CREATE TABLE IF NOT EXISTS "ob-poc".verb_patterns (
    pattern_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    pattern_name VARCHAR(200) NOT NULL,
    pattern_category VARCHAR(100) NOT NULL, -- 'workflow', 'error_handling', 'validation', 'initialization'

    -- Pattern Definition
    pattern_description TEXT NOT NULL,
    pattern_template TEXT NOT NULL,       -- DSL template with placeholders
    pattern_variables JSONB,              -- Variables that can be substituted

    -- Usage Context
    use_cases TEXT[],                     -- When to apply this pattern
    business_scenarios TEXT[],            -- Real-world scenarios where this applies
    complexity_level VARCHAR(20),         -- 'beginner', 'intermediate', 'advanced'

    -- Pattern Validation
    required_verbs TEXT[] NOT NULL,       -- Verbs that must appear in this pattern
    optional_verbs TEXT[],                -- Verbs that might appear
    forbidden_verbs TEXT[],               -- Verbs that shouldn't appear with this pattern

    -- Agent Guidance
    agent_selection_rules TEXT,           -- When should an agent choose this pattern
    customization_guidance TEXT,          -- How to adapt this pattern
    common_adaptations JSONB,             -- Common ways this pattern gets modified

    -- Quality Metrics
    success_rate DECIMAL(5,2),            -- Historical success rate of this pattern
    usage_frequency INTEGER DEFAULT 0,    -- How often this pattern is used

    -- Metadata
    domain_applicability TEXT[],          -- Which domains this pattern applies to
    tags TEXT[],                          -- Searchable tags
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);

CREATE INDEX IF NOT EXISTS idx_verb_patterns_category ON "ob-poc".verb_patterns (pattern_category);
CREATE INDEX IF NOT EXISTS idx_verb_patterns_complexity ON "ob-poc".verb_patterns (complexity_level);
CREATE INDEX IF NOT EXISTS idx_verb_patterns_success_rate ON "ob-poc".verb_patterns (success_rate DESC);
CREATE INDEX IF NOT EXISTS idx_verb_patterns_usage ON "ob-poc".verb_patterns (usage_frequency DESC);

-- ============================================================================
-- AGENT DECISION SUPPORT
-- ============================================================================

-- Decision trees and rules for verb selection
CREATE TABLE IF NOT EXISTS "ob-poc".verb_decision_rules (
    rule_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    rule_name VARCHAR(200) NOT NULL,
    rule_type VARCHAR(50) NOT NULL,       -- 'selection', 'validation', 'sequencing', 'parameter_binding'

    -- Rule Logic
    condition_expression TEXT NOT NULL,   -- Boolean expression for when this rule applies
    action_expression TEXT NOT NULL,      -- What action to take when condition is true
    priority_weight INTEGER DEFAULT 100,  -- Rule priority (higher = more important)

    -- Context
    applicable_domains TEXT[],            -- Which domains this rule applies to
    applicable_verbs TEXT[],              -- Which verbs this rule affects
    business_context TEXT,               -- Business reason for this rule

    -- Agent Integration
    llm_prompt_addition TEXT,             -- Additional context to add to LLM prompts
    error_message TEXT,                   -- Message to show when rule is violated
    suggestion_text TEXT,                 -- Suggestion for how to fix violations

    -- Lifecycle
    confidence_level DECIMAL(3,2) DEFAULT 0.8,
    active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);

CREATE INDEX IF NOT EXISTS idx_verb_decision_rules_type ON "ob-poc".verb_decision_rules (rule_type);
CREATE INDEX IF NOT EXISTS idx_verb_decision_rules_priority ON "ob-poc".verb_decision_rules (priority_weight DESC);
CREATE INDEX IF NOT EXISTS idx_verb_decision_rules_active ON "ob-poc".verb_decision_rules (active);

-- ============================================================================
-- AGENT INTERACTION HISTORY
-- ============================================================================

-- Track how agents use verbs to improve recommendations
CREATE TABLE IF NOT EXISTS "ob-poc".agent_verb_usage (
    usage_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    session_id VARCHAR(200),              -- Agent session identifier
    agent_type VARCHAR(100),              -- 'openai_gpt4', 'gemini', 'claude', etc.

    -- Usage Details
    domain VARCHAR(100) NOT NULL,
    verb VARCHAR(100) NOT NULL,
    context_prompt TEXT,                  -- What prompt led to this verb selection
    selected_parameters JSONB,            -- Parameters the agent chose

    -- Decision Process
    alternative_verbs_considered TEXT[],   -- Other verbs the agent considered
    selection_reasoning TEXT,             -- Agent's explanation for choice
    confidence_reported DECIMAL(3,2),     -- Agent's confidence in the choice

    -- Outcome
    execution_success BOOLEAN,            -- Did the DSL execute successfully
    user_feedback VARCHAR(20),            -- 'positive', 'negative', 'neutral', 'corrected'
    correction_applied TEXT,              -- If user corrected, what was the correction

    -- Context
    preceding_verbs TEXT[],               -- Verbs that came before in the session
    workflow_stage VARCHAR(100),          -- What stage of workflow this was part of

    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),

    FOREIGN KEY (domain, verb) REFERENCES "ob-poc".domain_vocabularies (domain, verb)
);

CREATE INDEX IF NOT EXISTS idx_agent_verb_usage_domain_verb ON "ob-poc".agent_verb_usage (domain, verb);
CREATE INDEX IF NOT EXISTS idx_agent_verb_usage_success ON "ob-poc".agent_verb_usage (execution_success);
CREATE INDEX IF NOT EXISTS idx_agent_verb_usage_feedback ON "ob-poc".agent_verb_usage (user_feedback);
CREATE INDEX IF NOT EXISTS idx_agent_verb_usage_created_at ON "ob-poc".agent_verb_usage (created_at DESC);

-- ============================================================================
-- RAG ENHANCEMENT FOR VERBS
-- ============================================================================

-- Vector embeddings for semantic verb search and clustering
CREATE TABLE IF NOT EXISTS "ob-poc".verb_embeddings (
    embedding_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    domain VARCHAR(100) NOT NULL,
    verb VARCHAR(100) NOT NULL,

    -- Vector Embeddings
    semantic_embedding VECTOR(1536),      -- OpenAI ada-002 dimensions (adjust as needed)
    context_embedding VECTOR(1536),       -- Embedding of usage context
    parameter_embedding VECTOR(1536),     -- Embedding of parameter semantics

    -- Embedding Metadata
    embedding_model VARCHAR(100),         -- Which model generated the embeddings
    embedding_version VARCHAR(20),        -- Version of embeddings
    last_updated TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),

    FOREIGN KEY (domain, verb) REFERENCES "ob-poc".domain_vocabularies (domain, verb) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_verb_embeddings_domain_verb ON "ob-poc".verb_embeddings (domain, verb);
-- Note: Vector similarity indexes would be added based on your vector database choice

-- ============================================================================
-- AGENT PROMPT TEMPLATES
-- ============================================================================

-- Structured prompt templates for different agent interactions
CREATE TABLE IF NOT EXISTS "ob-poc".agent_prompt_templates (
    template_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    template_name VARCHAR(200) NOT NULL,
    template_type VARCHAR(100) NOT NULL,  -- 'verb_selection', 'parameter_binding', 'validation', 'workflow_generation'

    -- Template Content
    base_prompt TEXT NOT NULL,            -- Base prompt template with placeholders
    context_sections JSONB,               -- Different sections that can be included
    variable_definitions JSONB,           -- Variables that can be substituted

    -- Usage Context
    applicable_domains TEXT[],
    applicable_verbs TEXT[],
    use_case_description TEXT,

    -- Template Quality
    effectiveness_score DECIMAL(3,2),     -- How effective is this template
    usage_count INTEGER DEFAULT 0,
    last_used TIMESTAMPTZ,

    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);

CREATE INDEX IF NOT EXISTS idx_agent_prompt_templates_type ON "ob-poc".agent_prompt_templates (template_type);
CREATE INDEX IF NOT EXISTS idx_agent_prompt_templates_effectiveness ON "ob-poc".agent_prompt_templates (effectiveness_score DESC);

-- ============================================================================
-- VIEWS FOR AGENT CONSUMPTION
-- ============================================================================

-- Comprehensive view combining all verb metadata for agent consumption
CREATE OR REPLACE VIEW "ob-poc".v_agent_verb_context AS
SELECT
    dv.domain,
    dv.verb,
    dv.category,
    dv.description as syntax_description,
    dv.parameters as syntax_parameters,
    dv.examples as syntax_examples,

    -- Semantic enrichment
    vs.semantic_description,
    vs.intent_category,
    vs.business_purpose,
    vs.side_effects,
    vs.prerequisites,
    vs.postconditions,
    vs.agent_prompt,
    vs.usage_patterns,
    vs.selection_criteria,
    vs.parameter_semantics,
    vs.workflow_stage,
    vs.compliance_implications,
    vs.confidence_score,

    -- Relationship context
    array_agg(DISTINCT vr_out.target_verb || ' (' || vr_out.relationship_type || ')')
        FILTER (WHERE vr_out.target_verb IS NOT NULL) as related_verbs,
    array_agg(DISTINCT vr_in.source_verb || ' (prerequisite)')
        FILTER (WHERE vr_in.source_verb IS NOT NULL AND vr_in.relationship_type = 'requires') as required_by,

    -- Usage statistics
    COALESCE(usage_stats.usage_count, 0) as historical_usage_count,
    COALESCE(usage_stats.success_rate, 0) as historical_success_rate,
    COALESCE(usage_stats.avg_confidence, 0) as avg_agent_confidence

FROM "ob-poc".domain_vocabularies dv
LEFT JOIN "ob-poc".verb_semantics vs ON dv.domain = vs.domain AND dv.verb = vs.verb
LEFT JOIN "ob-poc".verb_relationships vr_out ON dv.domain = vr_out.source_domain AND dv.verb = vr_out.source_verb
LEFT JOIN "ob-poc".verb_relationships vr_in ON dv.domain = vr_in.target_domain AND dv.verb = vr_in.target_verb
LEFT JOIN (
    SELECT
        domain, verb,
        count(*) as usage_count,
        avg(CASE WHEN execution_success THEN 1.0 ELSE 0.0 END) as success_rate,
        avg(confidence_reported) as avg_confidence
    FROM "ob-poc".agent_verb_usage
    WHERE created_at > NOW() - INTERVAL '30 days'
    GROUP BY domain, verb
) usage_stats ON dv.domain = usage_stats.domain AND dv.verb = usage_stats.verb

GROUP BY dv.domain, dv.verb, dv.category, dv.description, dv.parameters, dv.examples,
         vs.semantic_description, vs.intent_category, vs.business_purpose, vs.side_effects,
         vs.prerequisites, vs.postconditions, vs.agent_prompt, vs.usage_patterns,
         vs.selection_criteria, vs.parameter_semantics, vs.workflow_stage,
         vs.compliance_implications, vs.confidence_score,
         usage_stats.usage_count, usage_stats.success_rate, usage_stats.avg_confidence;

-- Workflow-oriented view for sequence planning
CREATE OR REPLACE VIEW "ob-poc".v_workflow_sequences AS
SELECT
    vs.workflow_stage,
    vs.domain,
    array_agg(vs.verb ORDER BY vs.verb) as available_verbs,
    array_agg(DISTINCT vr.target_verb) FILTER (WHERE vr.relationship_type = 'enables') as enables_verbs,
    array_agg(DISTINCT vr2.source_verb) FILTER (WHERE vr2.relationship_type = 'requires') as required_by_verbs
FROM "ob-poc".verb_semantics vs
LEFT JOIN "ob-poc".verb_relationships vr ON vs.domain = vr.source_domain AND vs.verb = vr.source_verb
LEFT JOIN "ob-poc".verb_relationships vr2 ON vs.domain = vr2.target_domain AND vs.verb = vr2.target_verb
WHERE vs.status = 'active'
GROUP BY vs.workflow_stage, vs.domain
ORDER BY vs.workflow_stage, vs.domain;

-- ============================================================================
-- SUCCESS MESSAGE
-- ============================================================================

DO $$
BEGIN
    RAISE NOTICE 'Semantic Verb Registry created successfully!';
    RAISE NOTICE '';
    RAISE NOTICE 'New tables created:';
    RAISE NOTICE '- verb_semantics: Rich semantic metadata for each verb';
    RAISE NOTICE '- verb_relationships: Explicit verb-to-verb relationships';
    RAISE NOTICE '- verb_patterns: Common usage patterns and templates';
    RAISE NOTICE '- verb_decision_rules: Decision support for verb selection';
    RAISE NOTICE '- agent_verb_usage: Historical agent interaction tracking';
    RAISE NOTICE '- verb_embeddings: Vector embeddings for semantic search';
    RAISE NOTICE '- agent_prompt_templates: Structured prompt templates';
    RAISE NOTICE '';
    RAISE NOTICE 'Views created:';
    RAISE NOTICE '- v_agent_verb_context: Comprehensive verb metadata for agents';
    RAISE NOTICE '- v_workflow_sequences: Workflow-oriented verb sequences';
    RAISE NOTICE '';
    RAISE NOTICE 'Next steps:';
    RAISE NOTICE '1. Run the semantic data population script';
    RAISE NOTICE '2. Generate vector embeddings for existing verbs';
    RAISE NOTICE '3. Update agent implementations to use semantic context';
    RAISE NOTICE '4. Test deterministic DSL generation';
END $$;
