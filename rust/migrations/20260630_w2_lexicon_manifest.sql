-- EOP-DD-KYCUBO-001 §8.1 / W2 — reference-plane lexicon wiring.
--
-- Two additive changes:
--
-- 1. `lexicon_hash` column on `dsl_verbs` — the content-address of the
--    substrate LexiconEntry that governs this verb (Q7). Null for verbs
--    that are not part of the dsl.kyc determination vocabulary. Stream
--    replay uses this to dispatch per event.lexicon_hash via the FoldRegistry.
--
-- 2. `kyc_lexicon_manifest` table — one row per published whole-lexicon
--    version (Q7). The manifest hash is the SHA-256 over the sorted
--    concatenation of all LexiconEntry hashes. DeterminationPin and replay
--    pin to a manifest row; a changed lexicon is a new row, never an in-place
--    update (content-addressed, append-only).

ALTER TABLE "ob-poc".dsl_verbs
    ADD COLUMN IF NOT EXISTS lexicon_hash text NULL;

COMMENT ON COLUMN "ob-poc".dsl_verbs.lexicon_hash IS
    'SHA-256 hex content-address of the substrate LexiconEntry (Q7). '
    'Null for verbs outside the dsl.kyc determination vocabulary. '
    'Matches IntentEvent.lexicon_hash persisted in kyc_intent_events. '
    'EOP-DD-KYCUBO-001 §8.1.';

-- Append-only manifest table: one row per whole-lexicon version.
CREATE TABLE IF NOT EXISTS "ob-poc".kyc_lexicon_manifest (
    id               uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    manifest_hash    text NOT NULL UNIQUE,  -- SHA-256 hex over sorted entry hashes (Q7)
    entry_count      integer NOT NULL,
    -- Serialised BTreeMap<fqn → entry_hash> — the exact content that produced manifest_hash.
    -- Stored so an auditor can verify: SHA-256(sorted(entry_hashes)) == manifest_hash.
    entry_hashes     jsonb NOT NULL,
    published_at     timestamptz NOT NULL DEFAULT clock_timestamp(),
    published_by     text NULL
);

COMMENT ON TABLE "ob-poc".kyc_lexicon_manifest IS
    'Whole-lexicon version history (Q7). Each row is an immutable content-addressed '
    'snapshot of the dsl.kyc LexiconManifest. DeterminationPin.lexicon_manifest_hash '
    'references a row here. EOP-DD-KYCUBO-001 §8.1 / W2.';

CREATE INDEX IF NOT EXISTS kyc_lexicon_manifest_published_idx
    ON "ob-poc".kyc_lexicon_manifest (published_at DESC);
