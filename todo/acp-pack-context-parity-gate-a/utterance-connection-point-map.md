# Utterance Connection Point Map

Status: audit draft for Gate A replan.

Current map:

```text
HTTP client
  -> POST /api/session/:id/input
  -> session_input
  -> SessionInputRequest::Utterance
  -> try_route_supported_acp_prompt
      -> resolve_acp_dag_semantic_prompt
      -> ACP semantic result / refusal / draft
  -> try_route_through_repl
      -> REPL proposal/matching/session flow
      -> staged DSL / pending question / response
  -> session state response
```

Alternate/callable surfaces:

```text
HTTP client -> POST /api/session/:id/execute -> execute_session_dsl_legacy_raw_only
ACP client  -> acp_protocol prompt handlers -> prompt_utterance_text -> ACP-specific handling
Tests       -> direct resolver/proposal/REPL calls
```

Required Gate B invariant:

Production utterance routing must become:

```text
utterance ingress -> verified ACP context envelope -> pack-scoped route -> draft/refusal/pending question
```

Any non-envelope path must be deleted, same-slice replaced, or quarantined with an explicit exclusion mechanism.
