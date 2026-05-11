# Workspace Dependency Graph Target

Status: Gate A target recommendation.

Target shape for Slice 1:

```text
sem_os_registry
  -> sem_os_diagnostics

acp_context_envelope
  -> sem_os_registry
  -> sem_os_diagnostics

sage_utterance
  -> acp_context_envelope
  -> sem_os_registry
  -> sem_os_diagnostics

sem_os_execution
  -> sem_os_registry
  -> sem_os_diagnostics

web/api ingress
  -> sage_utterance
  -> sem_os_execution only after envelope-verified route decision
```

Rules:

- Utterance parsing must not depend on execution/database crates.
- Registry projection must not depend on web/API crates.
- Diagnostics must be low-level and shared.
- Execution may consume registry metadata, but registry must not call execution.
- Envelope generation must be deterministic and free of runtime state instances.

Gate B decision:

Use this as the direction of travel, but do not force crate names until the root crate export audit confirms whether existing crates can be narrowed without churn.
