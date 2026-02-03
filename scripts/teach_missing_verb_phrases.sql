-- Teach invocation phrases for verbs missing patterns
-- Generated based on verb descriptions and common usage patterns
-- Run with: psql -d data_designer -f scripts/teach_missing_verb_phrases.sql

SELECT agent.teach_phrases_batch('[
  {"phrase": "apply document bundle", "verb": "docs-bundle.apply"},
  {"phrase": "apply docs bundle", "verb": "docs-bundle.apply"},
  {"phrase": "add document bundle to cbu", "verb": "docs-bundle.apply"},
  {"phrase": "attach document bundle", "verb": "docs-bundle.apply"},
  {"phrase": "assign document requirements", "verb": "docs-bundle.apply"},

  {"phrase": "list applied bundles", "verb": "docs-bundle.list-applied"},
  {"phrase": "show document bundles for cbu", "verb": "docs-bundle.list-applied"},
  {"phrase": "what bundles are applied", "verb": "docs-bundle.list-applied"},
  {"phrase": "applied document bundles", "verb": "docs-bundle.list-applied"},

  {"phrase": "list available bundles", "verb": "docs-bundle.list-available"},
  {"phrase": "show document bundles", "verb": "docs-bundle.list-available"},
  {"phrase": "available document bundles", "verb": "docs-bundle.list-available"},
  {"phrase": "what bundles exist", "verb": "docs-bundle.list-available"},

  {"phrase": "ensure entity or placeholder", "verb": "entity.ensure-or-placeholder"},
  {"phrase": "create placeholder entity", "verb": "entity.ensure-or-placeholder"},
  {"phrase": "placeholder for entity", "verb": "entity.ensure-or-placeholder"},
  {"phrase": "ensure party exists", "verb": "entity.ensure-or-placeholder"},

  {"phrase": "list placeholder entities", "verb": "entity.list-placeholders"},
  {"phrase": "show placeholders", "verb": "entity.list-placeholders"},
  {"phrase": "pending placeholders", "verb": "entity.list-placeholders"},
  {"phrase": "unresolved entities", "verb": "entity.list-placeholders"},

  {"phrase": "placeholder summary", "verb": "entity.placeholder-summary"},
  {"phrase": "placeholder statistics", "verb": "entity.placeholder-summary"},
  {"phrase": "how many placeholders", "verb": "entity.placeholder-summary"},

  {"phrase": "resolve placeholder", "verb": "entity.resolve-placeholder"},
  {"phrase": "link placeholder to entity", "verb": "entity.resolve-placeholder"},
  {"phrase": "replace placeholder", "verb": "entity.resolve-placeholder"},
  {"phrase": "resolve pending entity", "verb": "entity.resolve-placeholder"},

  {"phrase": "cancel proposal", "verb": "exec.cancel"},
  {"phrase": "abort proposal", "verb": "exec.cancel"},
  {"phrase": "discard proposal", "verb": "exec.cancel"},
  {"phrase": "cancel pending execution", "verb": "exec.cancel"},

  {"phrase": "confirm proposal", "verb": "exec.confirm"},
  {"phrase": "execute proposal", "verb": "exec.confirm"},
  {"phrase": "approve proposal", "verb": "exec.confirm"},
  {"phrase": "confirm execution", "verb": "exec.confirm"},
  {"phrase": "run proposal", "verb": "exec.confirm"},

  {"phrase": "edit proposal", "verb": "exec.edit"},
  {"phrase": "modify proposal", "verb": "exec.edit"},
  {"phrase": "change proposal", "verb": "exec.edit"},
  {"phrase": "update pending proposal", "verb": "exec.edit"},

  {"phrase": "create proposal", "verb": "exec.proposal"},
  {"phrase": "preview execution", "verb": "exec.proposal"},
  {"phrase": "stage for execution", "verb": "exec.proposal"},
  {"phrase": "prepare proposal", "verb": "exec.proposal"},
  {"phrase": "dry run", "verb": "exec.proposal"},

  {"phrase": "proposal status", "verb": "exec.status"},
  {"phrase": "check proposal", "verb": "exec.status"},
  {"phrase": "list proposals", "verb": "exec.status"},
  {"phrase": "pending proposals", "verb": "exec.status"},

  {"phrase": "ensure standalone fund", "verb": "fund.ensure-standalone"},
  {"phrase": "create standalone fund", "verb": "fund.ensure-standalone"},
  {"phrase": "import fund from gleif", "verb": "fund.ensure-standalone"},
  {"phrase": "ensure fund exists", "verb": "fund.ensure-standalone"},

  {"phrase": "commit scope", "verb": "scope.commit"},
  {"phrase": "save entity scope", "verb": "scope.commit"},
  {"phrase": "bind scope to symbol", "verb": "scope.commit"},
  {"phrase": "create entity scope", "verb": "scope.commit"},
  {"phrase": "select entities", "verb": "scope.commit"},

  {"phrase": "narrow scope", "verb": "scope.narrow"},
  {"phrase": "filter scope", "verb": "scope.narrow"},
  {"phrase": "refine entity scope", "verb": "scope.narrow"},
  {"phrase": "narrow down entities", "verb": "scope.narrow"},

  {"phrase": "resolve scope", "verb": "scope.resolve"},
  {"phrase": "preview scope", "verb": "scope.resolve"},
  {"phrase": "dry run scope", "verb": "scope.resolve"},
  {"phrase": "test scope query", "verb": "scope.resolve"},

  {"phrase": "union scopes", "verb": "scope.union"},
  {"phrase": "combine scopes", "verb": "scope.union"},
  {"phrase": "merge entity scopes", "verb": "scope.union"},
  {"phrase": "join scopes", "verb": "scope.union"},

  {"phrase": "set case", "verb": "session.set-case"},
  {"phrase": "set kyc case", "verb": "session.set-case"},
  {"phrase": "switch to case", "verb": "session.set-case"},
  {"phrase": "work on case", "verb": "session.set-case"},
  {"phrase": "focus on case", "verb": "session.set-case"},

  {"phrase": "set mandate", "verb": "session.set-mandate"},
  {"phrase": "set trading profile", "verb": "session.set-mandate"},
  {"phrase": "switch to mandate", "verb": "session.set-mandate"},
  {"phrase": "work on mandate", "verb": "session.set-mandate"},
  {"phrase": "focus on mandate", "verb": "session.set-mandate"},

  {"phrase": "set structure", "verb": "session.set-structure"},
  {"phrase": "set cbu", "verb": "session.set-structure"},
  {"phrase": "switch to structure", "verb": "session.set-structure"},
  {"phrase": "work on structure", "verb": "session.set-structure"},
  {"phrase": "focus on structure", "verb": "session.set-structure"},
  {"phrase": "select structure", "verb": "session.set-structure"}
]'::jsonb, 'missing_verb_phrases');
