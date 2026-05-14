//! BPMN-Lite domain model — leaf crate.
//!
//! Holds the value types every other bpmn-lite crate operates on:
//! scalar id aliases (`Addr`, `JoinId`, `WaitId`, `RaceId`, `FlagKey`,
//! `Timestamp`), the bytecode instruction set, `ProcessInstance`,
//! `Fiber`, `CompiledProgram`, `RuntimeEvent`, and the various wait /
//! activation / completion DTOs that flow between the engine and the
//! gRPC + persistence boundaries.
//!
//! Capability claim: schema-only. No bytecode interpretation, no
//! parsing, no persistence. Leaves the consumer crate (engine, vm,
//! store-*, authoring, compiler) free to layer behaviour over a
//! single agreed type vocabulary.
//!
//! Empty at Phase 1 skeleton — the actual types live in
//! `bpmn-lite-core/src/types.rs` + `events.rs` until the Phase 2
//! migration slices move them in. The skeleton lands first so the
//! workspace dep edges can be drawn against a known-empty target,
//! exactly the way the ob-poc capability-crate-restructure-v1
//! plan staged its `ob-poc-types`, `ob-poc-sage`, `ob-poc-journey`,
//! `ob-poc-domain`, `ob-poc-authoring` skeletons in commits
//! 7e146afc / 9cd119d7 / 163599aa / 0318f89c.
