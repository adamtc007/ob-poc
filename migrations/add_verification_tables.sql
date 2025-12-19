-- Adversarial Verification Model Tables
-- Part of Phase 2-3 implementation

--
-- Pattern detection audit trail
--
CREATE TABLE IF NOT EXISTS "ob-poc".detected_patterns (
    pattern_id uuid DEFAULT gen_random_uuid() NOT NULL,
    cbu_id uuid NOT NULL,
    case_id uuid,
    pattern_type varchar(50) NOT NULL,
    severity varchar(20) NOT NULL,
    description text NOT NULL,
    involved_entities uuid[] NOT NULL,
    evidence jsonb,
    status varchar(20) DEFAULT 'DETECTED'::varchar NOT NULL,
    detected_at timestamptz DEFAULT now() NOT NULL,
    resolved_at timestamptz,
    resolved_by varchar(100),
    resolution_notes text,
    CONSTRAINT detected_patterns_pkey PRIMARY KEY (pattern_id),
    CONSTRAINT detected_patterns_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id),
    CONSTRAINT detected_patterns_case_id_fkey FOREIGN KEY (case_id) REFERENCES kyc.cases(case_id),
    CONSTRAINT detected_patterns_pattern_type_check CHECK (pattern_type IN ('CIRCULAR_OWNERSHIP', 'LAYERING', 'NOMINEE_USAGE', 'OPACITY_JURISDICTION', 'REGISTRY_MISMATCH', 'OWNERSHIP_GAPS', 'RECENT_RESTRUCTURING', 'ROLE_CONCENTRATION')),
    CONSTRAINT detected_patterns_severity_check CHECK (severity IN ('INFO', 'LOW', 'MEDIUM', 'HIGH', 'CRITICAL')),
    CONSTRAINT detected_patterns_status_check CHECK (status IN ('DETECTED', 'INVESTIGATING', 'RESOLVED', 'FALSE_POSITIVE'))
);

CREATE INDEX IF NOT EXISTS idx_detected_patterns_cbu ON "ob-poc".detected_patterns(cbu_id);
CREATE INDEX IF NOT EXISTS idx_detected_patterns_type ON "ob-poc".detected_patterns(pattern_type);
CREATE INDEX IF NOT EXISTS idx_detected_patterns_status ON "ob-poc".detected_patterns(status);

COMMENT ON TABLE "ob-poc".detected_patterns IS 'Audit trail for adversarial pattern detection (circular ownership, layering, nominee usage, etc.)';

--
-- Challenge/response workflow for adversarial verification
--
CREATE TABLE IF NOT EXISTS "ob-poc".verification_challenges (
    challenge_id uuid DEFAULT gen_random_uuid() NOT NULL,
    cbu_id uuid NOT NULL,
    case_id uuid,
    entity_id uuid,
    allegation_id uuid,
    observation_id uuid,
    challenge_type varchar(30) NOT NULL,
    challenge_reason text NOT NULL,
    severity varchar(20) NOT NULL,
    status varchar(20) DEFAULT 'OPEN'::varchar NOT NULL,
    response_text text,
    response_evidence_ids uuid[],
    raised_at timestamptz DEFAULT now() NOT NULL,
    raised_by varchar(100),
    responded_at timestamptz,
    resolved_at timestamptz,
    resolved_by varchar(100),
    resolution_type varchar(30),
    resolution_notes text,
    CONSTRAINT verification_challenges_pkey PRIMARY KEY (challenge_id),
    CONSTRAINT verification_challenges_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id),
    CONSTRAINT verification_challenges_case_id_fkey FOREIGN KEY (case_id) REFERENCES kyc.cases(case_id),
    CONSTRAINT verification_challenges_entity_id_fkey FOREIGN KEY (entity_id) REFERENCES "ob-poc".entities(entity_id),
    CONSTRAINT verification_challenges_allegation_id_fkey FOREIGN KEY (allegation_id) REFERENCES "ob-poc".client_allegations(allegation_id),
    CONSTRAINT verification_challenges_observation_id_fkey FOREIGN KEY (observation_id) REFERENCES "ob-poc".attribute_observations(observation_id),
    CONSTRAINT verification_challenges_type_check CHECK (challenge_type IN ('INCONSISTENCY', 'LOW_CONFIDENCE', 'MISSING_CORROBORATION', 'PATTERN_DETECTED', 'EVASION_SIGNAL', 'REGISTRY_MISMATCH')),
    CONSTRAINT verification_challenges_severity_check CHECK (severity IN ('INFO', 'LOW', 'MEDIUM', 'HIGH', 'CRITICAL')),
    CONSTRAINT verification_challenges_status_check CHECK (status IN ('OPEN', 'RESPONDED', 'RESOLVED', 'ESCALATED')),
    CONSTRAINT verification_challenges_resolution_type_check CHECK (resolution_type IS NULL OR resolution_type IN ('ACCEPTED', 'REJECTED', 'WAIVED', 'ESCALATED'))
);

CREATE INDEX IF NOT EXISTS idx_verification_challenges_cbu ON "ob-poc".verification_challenges(cbu_id);
CREATE INDEX IF NOT EXISTS idx_verification_challenges_status ON "ob-poc".verification_challenges(status);
CREATE INDEX IF NOT EXISTS idx_verification_challenges_case ON "ob-poc".verification_challenges(case_id);

COMMENT ON TABLE "ob-poc".verification_challenges IS 'Challenge/response workflow for adversarial verification - tracks formal challenges requiring client response';

--
-- Risk-based escalation routing
--
CREATE TABLE IF NOT EXISTS "ob-poc".verification_escalations (
    escalation_id uuid DEFAULT gen_random_uuid() NOT NULL,
    cbu_id uuid NOT NULL,
    case_id uuid,
    challenge_id uuid,
    escalation_level varchar(30) NOT NULL,
    escalation_reason text NOT NULL,
    risk_indicators jsonb,
    status varchar(20) DEFAULT 'PENDING'::varchar NOT NULL,
    decision varchar(20),
    decision_notes text,
    escalated_at timestamptz DEFAULT now() NOT NULL,
    escalated_by varchar(100),
    decided_at timestamptz,
    decided_by varchar(100),
    CONSTRAINT verification_escalations_pkey PRIMARY KEY (escalation_id),
    CONSTRAINT verification_escalations_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id),
    CONSTRAINT verification_escalations_case_id_fkey FOREIGN KEY (case_id) REFERENCES kyc.cases(case_id),
    CONSTRAINT verification_escalations_challenge_id_fkey FOREIGN KEY (challenge_id) REFERENCES "ob-poc".verification_challenges(challenge_id),
    CONSTRAINT verification_escalations_level_check CHECK (escalation_level IN ('SENIOR_ANALYST', 'COMPLIANCE_OFFICER', 'MLRO', 'COMMITTEE')),
    CONSTRAINT verification_escalations_status_check CHECK (status IN ('PENDING', 'UNDER_REVIEW', 'DECIDED')),
    CONSTRAINT verification_escalations_decision_check CHECK (decision IS NULL OR decision IN ('APPROVE', 'REJECT', 'REQUIRE_MORE_INFO', 'ESCALATE_FURTHER'))
);

CREATE INDEX IF NOT EXISTS idx_verification_escalations_cbu ON "ob-poc".verification_escalations(cbu_id);
CREATE INDEX IF NOT EXISTS idx_verification_escalations_status ON "ob-poc".verification_escalations(status);
CREATE INDEX IF NOT EXISTS idx_verification_escalations_level ON "ob-poc".verification_escalations(escalation_level);

COMMENT ON TABLE "ob-poc".verification_escalations IS 'Risk-based escalation routing for verification challenges requiring higher authority review';
