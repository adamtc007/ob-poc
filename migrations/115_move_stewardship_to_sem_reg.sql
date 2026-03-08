ALTER TABLE stewardship.basis_claims SET SCHEMA sem_reg;
ALTER TABLE stewardship.basis_records SET SCHEMA sem_reg;
ALTER TABLE stewardship.conflict_records SET SCHEMA sem_reg;
ALTER TABLE stewardship.events SET SCHEMA sem_reg;
ALTER TABLE stewardship.focus_states SET SCHEMA sem_reg;
ALTER TABLE stewardship.idempotency_keys SET SCHEMA sem_reg;
ALTER TABLE stewardship.templates SET SCHEMA sem_reg;
ALTER TABLE stewardship.verb_implementation_bindings SET SCHEMA sem_reg;
ALTER TABLE stewardship.viewport_manifests SET SCHEMA sem_reg;

DROP SCHEMA stewardship;
