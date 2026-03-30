-- Board compositions and appointment rights tables
-- Required by board.* verbs (kyc.extended constellation, board slot)
-- Referenced by: board_ops.rs (BoardAnalyzeControlOp, BoardAppointOp, etc.)

CREATE TABLE IF NOT EXISTS "ob-poc".board_compositions (
    id              uuid DEFAULT gen_random_uuid() PRIMARY KEY,
    entity_id       uuid NOT NULL REFERENCES "ob-poc".entities(entity_id),
    person_entity_id uuid NOT NULL REFERENCES "ob-poc".entities(entity_id),
    role_id         uuid NOT NULL REFERENCES "ob-poc".roles(role_id),
    appointed_by_entity_id uuid REFERENCES "ob-poc".entities(entity_id),
    appointment_date date,
    resignation_date date,
    is_active       boolean DEFAULT true NOT NULL,
    source          varchar(50) DEFAULT 'manual' NOT NULL,
    source_document_ref varchar(255),
    notes           text,
    created_at      timestamptz DEFAULT now() NOT NULL,
    updated_at      timestamptz DEFAULT now() NOT NULL,
    CONSTRAINT board_comp_no_self_appointment CHECK (entity_id != person_entity_id)
);

CREATE INDEX IF NOT EXISTS idx_board_comp_entity ON "ob-poc".board_compositions(entity_id) WHERE is_active = true;
CREATE INDEX IF NOT EXISTS idx_board_comp_person ON "ob-poc".board_compositions(person_entity_id) WHERE is_active = true;

CREATE TABLE IF NOT EXISTS "ob-poc".appointment_rights (
    id              uuid DEFAULT gen_random_uuid() PRIMARY KEY,
    target_entity_id uuid NOT NULL REFERENCES "ob-poc".entities(entity_id),
    holder_entity_id uuid NOT NULL REFERENCES "ob-poc".entities(entity_id),
    right_type      varchar(30) NOT NULL,
    max_appointments integer,
    is_active       boolean DEFAULT true NOT NULL,
    effective_from  date,
    effective_to    date,
    source          varchar(50) DEFAULT 'manual' NOT NULL,
    source_document_ref varchar(255),
    created_at      timestamptz DEFAULT now() NOT NULL,
    updated_at      timestamptz DEFAULT now() NOT NULL,
    CONSTRAINT appt_rights_chk_type CHECK (right_type IN (
        'APPOINT', 'REMOVE', 'APPOINT_AND_REMOVE', 'VETO_APPOINTMENT', 'OBSERVER'
    ))
);

CREATE INDEX IF NOT EXISTS idx_appt_rights_target ON "ob-poc".appointment_rights(target_entity_id) WHERE is_active = true;
CREATE INDEX IF NOT EXISTS idx_appt_rights_holder ON "ob-poc".appointment_rights(holder_entity_id) WHERE is_active = true;
