BEGIN;

ALTER TABLE IF EXISTS agent.entity_aliases SET SCHEMA "ob-poc";
ALTER TABLE IF EXISTS agent.events SET SCHEMA "ob-poc";
ALTER TABLE IF EXISTS agent.invocation_phrases SET SCHEMA "ob-poc";
ALTER TABLE IF EXISTS agent.learning_audit SET SCHEMA "ob-poc";
ALTER TABLE IF EXISTS agent.learning_candidates SET SCHEMA "ob-poc";
ALTER TABLE IF EXISTS agent.lexicon_tokens SET SCHEMA "ob-poc";
ALTER TABLE IF EXISTS agent.phrase_blocklist SET SCHEMA "ob-poc";
ALTER TABLE IF EXISTS agent.user_learned_phrases SET SCHEMA "ob-poc";

ALTER SEQUENCE IF EXISTS agent.entity_aliases_id_seq SET SCHEMA "ob-poc";
ALTER SEQUENCE IF EXISTS agent.events_id_seq SET SCHEMA "ob-poc";
ALTER SEQUENCE IF EXISTS agent.invocation_phrases_id_seq SET SCHEMA "ob-poc";
ALTER SEQUENCE IF EXISTS agent.learning_audit_id_seq SET SCHEMA "ob-poc";
ALTER SEQUENCE IF EXISTS agent.learning_candidates_id_seq SET SCHEMA "ob-poc";
ALTER SEQUENCE IF EXISTS agent.lexicon_tokens_id_seq SET SCHEMA "ob-poc";
ALTER SEQUENCE IF EXISTS agent.phrase_blocklist_id_seq SET SCHEMA "ob-poc";
ALTER SEQUENCE IF EXISTS agent.user_learned_phrases_id_seq SET SCHEMA "ob-poc";

ALTER FUNCTION agent.apply_promotion(bigint, text) SET SCHEMA "ob-poc";
ALTER FUNCTION agent.batch_upsert_invocation_phrases(text[], text[], vector[], text) SET SCHEMA "ob-poc";
ALTER FUNCTION agent.check_blocklist_semantic(vector, text, uuid, real) SET SCHEMA "ob-poc";
ALTER FUNCTION agent.check_pattern_collision_basic(bigint) SET SCHEMA "ob-poc";
ALTER FUNCTION agent.expire_pending_outcomes(integer) SET SCHEMA "ob-poc";
ALTER FUNCTION agent.get_auto_applicable_candidates() SET SCHEMA "ob-poc";
ALTER FUNCTION agent.get_promotable_candidates(integer, real, integer, integer) SET SCHEMA "ob-poc";
ALTER FUNCTION agent.get_review_candidates(integer, integer, integer) SET SCHEMA "ob-poc";
ALTER FUNCTION agent.get_taught_pending_embeddings() SET SCHEMA "ob-poc";
ALTER FUNCTION agent.is_verb_blocked(vector, text, uuid, real) SET SCHEMA "ob-poc";
ALTER FUNCTION agent.learn_user_phrase(uuid, text, text, vector) SET SCHEMA "ob-poc";
ALTER FUNCTION agent.record_learning_signal(text, text, boolean, text, text) SET SCHEMA "ob-poc";
ALTER FUNCTION agent.reject_candidate(bigint, text, text) SET SCHEMA "ob-poc";
ALTER FUNCTION agent.search_learned_phrases_semantic(vector, real, integer) SET SCHEMA "ob-poc";
ALTER FUNCTION agent.search_user_phrases_semantic(uuid, vector, real, integer) SET SCHEMA "ob-poc";
ALTER FUNCTION agent.search_verbs_semantic(vector, uuid, real, integer) SET SCHEMA "ob-poc";
ALTER FUNCTION agent.teach_phrase(text, text, text) SET SCHEMA "ob-poc";
ALTER FUNCTION agent.teach_phrases_batch(jsonb, text) SET SCHEMA "ob-poc";
ALTER FUNCTION agent.unteach_phrase(text, text, text, text) SET SCHEMA "ob-poc";
ALTER FUNCTION agent.upsert_entity_alias(text, text, uuid, text) SET SCHEMA "ob-poc";
ALTER FUNCTION agent.upsert_invocation_phrase(text, text, vector, text) SET SCHEMA "ob-poc";
ALTER FUNCTION agent.upsert_lexicon_token(text, text, text, text) SET SCHEMA "ob-poc";

ALTER VIEW IF EXISTS agent.v_candidate_pipeline SET SCHEMA "ob-poc";
ALTER VIEW IF EXISTS agent.v_learning_health_weekly SET SCHEMA "ob-poc";
ALTER VIEW IF EXISTS agent.v_recently_taught SET SCHEMA "ob-poc";
ALTER VIEW IF EXISTS agent.v_teaching_stats SET SCHEMA "ob-poc";
ALTER VIEW IF EXISTS agent.v_top_pending_candidates SET SCHEMA "ob-poc";

DROP SCHEMA agent;

COMMIT;
