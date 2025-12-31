//! Verb RAG Metadata
//!
//! Provides semantic metadata for verb discovery:
//! - intent_patterns: Natural language patterns that map to verbs
//! - workflow_phases: KYC lifecycle phases where verb is applicable
//! - graph_contexts: Graph UI contexts where verb is relevant
//! - typical_next: Common follow-up verbs in workflows
//!
//! COMPREHENSIVE UPDATE: 2024-12-31
//! - Added all Trading Matrix verbs (instruction-profile, trade-gateway, corporate-action, etc.)
//! - Added all KYC verbs (kyc-case, entity-workstream, doc-request, case-screening, red-flag)
//! - Added all Observation verbs (observation, allegation, discrepancy)
//! - Added all Verification verbs (verify, challenge, escalation, pattern detection)
//! - Added all Team/User verbs
//! - Added all Temporal query verbs
//! - Added all Screening verbs (PEP, sanctions, adverse-media)
//! - Added all SLA verbs
//! - Added all Regulatory verbs
//! - Added all Semantic stage verbs
//! - Added all Cash Sweep verbs
//! - Added all Investment Manager verbs
//! - Added all Fund Investor verbs
//! - Added all Delegation verbs
//! - Added all Delivery verbs
//! - Added all Service Resource verbs
//! - Added all Client Portal verbs
//! - Added all Batch verbs
//! - Added all KYC Agreement verbs
//! - Added all Registry verbs (holding, movement)
//! - Added all Request verbs
//! - Enhanced existing entries with corner cases and alternative phrasings

use std::collections::HashMap;

/// Intent patterns for natural language -> verb matching
pub fn get_intent_patterns() -> HashMap<&'static str, Vec<&'static str>> {
    let mut m = HashMap::new();

    // ==========================================================================
    // CBU VERBS
    // ==========================================================================
    m.insert(
        "cbu.create",
        vec![
            "create cbu",
            "new cbu",
            "add client",
            "onboard client",
            "create client business unit",
            "new client",
            "start onboarding",
            "new account",
            "open account",
            "register client",
            "new customer",
            "client setup",
        ],
    );
    m.insert(
        "cbu.ensure",
        vec![
            "ensure cbu exists",
            "upsert cbu",
            "create or update cbu",
            "idempotent cbu",
            "find or create cbu",
            "cbu if not exists",
        ],
    );
    m.insert(
        "cbu.assign-role",
        vec![
            "assign role",
            "add role",
            "give role",
            "set role",
            "make them",
            "appoint as",
            "designate as",
            "role assignment",
            "link entity to cbu",
        ],
    );
    m.insert(
        "cbu.remove-role",
        vec![
            "remove role",
            "revoke role",
            "unassign role",
            "take away role",
            "end role",
            "terminate role",
            "delete role assignment",
        ],
    );
    m.insert(
        "cbu.show",
        vec![
            "show cbu",
            "display cbu",
            "view cbu",
            "cbu details",
            "cbu info",
            "client details",
            "get cbu",
            "read cbu",
        ],
    );
    m.insert(
        "cbu.parties",
        vec![
            "list parties",
            "show parties",
            "who is involved",
            "all entities",
            "related parties",
            "cbu participants",
            "stakeholders",
            "cbu entities",
            "who are the parties",
        ],
    );
    m.insert(
        "cbu.add-product",
        vec![
            "add product",
            "assign product",
            "enable product",
            "activate product",
            "subscribe to product",
            "product subscription",
            "enroll in product",
        ],
    );
    m.insert(
        "cbu.decide",
        vec![
            "make decision",
            "approve cbu",
            "reject cbu",
            "decide on client",
            "final decision",
            "onboarding decision",
            "cbu approval",
        ],
    );

    // ==========================================================================
    // ENTITY VERBS
    // ==========================================================================
    m.insert(
        "entity.create-limited-company",
        vec![
            "create company",
            "new company",
            "add company",
            "create entity",
            "create ltd",
            "create limited company",
            "new legal entity",
            "incorporate company",
            "add corporation",
            "create gmbh",
            "create sarl",
            "create bv",
            "create ag",
            "create sa",
            "create plc",
            "register company",
        ],
    );
    m.insert(
        "entity.ensure-limited-company",
        vec![
            "ensure company",
            "upsert company",
            "create or update company",
            "find or create company",
            "idempotent company",
            "company if not exists",
        ],
    );
    m.insert(
        "entity.create-proper-person",
        vec![
            "create person",
            "add person",
            "new individual",
            "add individual",
            "create natural person",
            "add human",
            "new person record",
            "create director",
            "add shareholder person",
            "register individual",
            "new natural person",
        ],
    );
    m.insert(
        "entity.ensure-proper-person",
        vec![
            "ensure person",
            "upsert person",
            "create or update person",
            "find or create person",
            "idempotent person",
            "person if not exists",
        ],
    );
    m.insert(
        "entity.create-trust-discretionary",
        vec![
            "create trust",
            "new trust",
            "add trust",
            "discretionary trust",
            "family trust",
            "create settlement",
            "establish trust",
            "unit trust",
            "create trust structure",
        ],
    );
    m.insert(
        "entity.create-partnership-limited",
        vec![
            "create partnership",
            "new lp",
            "add limited partnership",
            "create lp",
            "new partnership",
            "create gp",
            "general partner",
            "limited partner",
            "create llp",
            "scottish partnership",
        ],
    );
    m.insert(
        "entity.update",
        vec![
            "update entity",
            "modify entity",
            "change entity details",
            "edit entity",
            "correct entity",
            "amend entity",
        ],
    );

    // ==========================================================================
    // FUND VERBS
    // ==========================================================================
    m.insert(
        "fund.create-umbrella",
        vec![
            "create umbrella",
            "new sicav",
            "create sicav",
            "new icav",
            "create fund umbrella",
            "umbrella fund",
            "create master fund",
            "new fund structure",
            "establish fund",
            "create vcic",
            "create oeic",
            "new umbrella structure",
            "register fund",
        ],
    );
    m.insert(
        "fund.ensure-umbrella",
        vec![
            "ensure umbrella",
            "upsert umbrella",
            "ensure sicav exists",
            "find or create umbrella",
            "umbrella if not exists",
        ],
    );
    m.insert(
        "fund.create-subfund",
        vec![
            "create subfund",
            "new subfund",
            "add compartment",
            "create compartment",
            "new sub-fund",
            "add portfolio",
            "create sleeve",
            "new fund compartment",
            "add sub-fund",
            "create cell",
        ],
    );
    m.insert(
        "fund.ensure-subfund",
        vec![
            "ensure subfund",
            "upsert subfund",
            "ensure compartment",
            "find or create subfund",
            "subfund if not exists",
        ],
    );
    m.insert(
        "fund.create-share-class",
        vec![
            "create share class",
            "new share class",
            "add share class",
            "create isin",
            "new isin",
            "add class",
            "institutional class",
            "retail class",
            "accumulating class",
            "distributing class",
            "hedged class",
            "unhedged class",
        ],
    );
    m.insert(
        "fund.ensure-share-class",
        vec![
            "ensure share class",
            "upsert share class",
            "find or create share class",
            "share class if not exists",
        ],
    );
    m.insert(
        "fund.link-feeder",
        vec![
            "link feeder",
            "connect feeder to master",
            "feeder master relationship",
            "master feeder",
            "feeder fund",
            "link to master",
            "feeder structure",
        ],
    );
    m.insert(
        "fund.list-subfunds",
        vec![
            "list subfunds",
            "show compartments",
            "subfunds under umbrella",
            "all compartments",
            "fund hierarchy",
            "umbrella compartments",
        ],
    );
    m.insert(
        "fund.list-share-classes",
        vec![
            "list share classes",
            "show share classes",
            "isins for fund",
            "all classes",
            "fund isins",
            "share class list",
        ],
    );

    // ==========================================================================
    // UBO/OWNERSHIP VERBS
    // ==========================================================================
    m.insert(
        "ubo.add-ownership",
        vec![
            "add owner",
            "add ownership",
            "owns",
            "shareholder of",
            "add shareholder",
            "ownership stake",
            "equity stake",
            "parent company",
            "holding company",
            "beneficial owner",
            "percentage holding",
            "ownership link",
            "owns percent",
            "shareholding",
            "equity holder",
            "ultimate owner",
            "direct ownership",
            "indirect ownership",
        ],
    );
    m.insert(
        "ubo.update-ownership",
        vec![
            "update ownership",
            "change percentage",
            "modify stake",
            "adjust ownership",
            "correct percentage",
            "ownership changed",
        ],
    );
    m.insert(
        "ubo.end-ownership",
        vec![
            "end ownership",
            "remove owner",
            "sold stake",
            "divested",
            "ownership ended",
            "no longer owns",
            "exit ownership",
            "disposed shares",
        ],
    );
    m.insert(
        "ubo.list-owners",
        vec![
            "list owners",
            "who owns",
            "shareholders",
            "ownership chain up",
            "direct owners",
            "immediate shareholders",
            "show owners",
            "parent entities",
        ],
    );
    m.insert(
        "ubo.list-owned",
        vec![
            "list owned",
            "subsidiaries",
            "what do they own",
            "ownership chain down",
            "investments",
            "holdings",
            "show subsidiaries",
            "child entities",
        ],
    );
    m.insert(
        "ubo.register-ubo",
        vec![
            "register ubo",
            "add beneficial owner",
            "ubo registration",
            "record ubo",
            "beneficial owner declaration",
            "ultimate beneficial owner",
            "declare ubo",
            "ubo identified",
        ],
    );
    m.insert(
        "ubo.mark-terminus",
        vec![
            "mark terminus",
            "end of chain",
            "public company",
            "no known person",
            "ubo terminus",
            "dispersed ownership",
            "listed company",
            "widely held",
            "regulated entity",
            "government owned",
            "natural person terminus",
            "chain termination",
            "ownership stops here",
        ],
    );
    m.insert(
        "ubo.calculate",
        vec![
            "calculate ubo",
            "ubo calculation",
            "beneficial ownership calculation",
            "who are the ubos",
            "25% threshold",
            "compute ownership",
            "derive ubos",
            "ubo analysis",
            "ownership rollup",
            "aggregate ownership",
        ],
    );
    m.insert(
        "ubo.trace-chains",
        vec![
            "trace chains",
            "trace ownership",
            "follow ownership",
            "ownership path",
            "ownership tree",
            "chain analysis",
            "ownership structure",
            "walk ownership",
            "ownership diagram",
        ],
    );

    // ==========================================================================
    // CONTROL VERBS
    // ==========================================================================
    m.insert(
        "control.add",
        vec![
            "add control",
            "controls",
            "controlling person",
            "significant control",
            "psc",
            "person of significant control",
            "control relationship",
            "voting control",
            "board control",
            "significant influence",
        ],
    );
    m.insert(
        "control.list-controllers",
        vec![
            "list controllers",
            "who controls",
            "controlling parties",
            "show control",
            "control chain",
            "persons with control",
        ],
    );

    // ==========================================================================
    // ROLE ASSIGNMENT (V2)
    // ==========================================================================
    m.insert(
        "cbu.role:assign",
        vec![
            "assign role",
            "add role to cbu",
            "entity role",
            "role assignment",
            "link with role",
        ],
    );
    m.insert(
        "cbu.role:assign-ownership",
        vec![
            "assign ownership role",
            "shareholder role",
            "owner role",
            "beneficial ownership role",
            "equity holder",
            "investor role",
        ],
    );
    m.insert(
        "cbu.role:assign-control",
        vec![
            "assign control role",
            "director role",
            "officer role",
            "board member",
            "executive role",
            "ceo",
            "cfo",
            "chairman",
            "managing director",
            "company secretary",
        ],
    );
    m.insert(
        "cbu.role:assign-trust-role",
        vec![
            "assign trust role",
            "trustee",
            "settlor",
            "beneficiary",
            "protector",
            "enforcer",
            "trust role",
            "trust beneficiary",
            "trust settlor",
        ],
    );
    m.insert(
        "cbu.role:assign-fund-role",
        vec![
            "assign fund role",
            "management company",
            "manco",
            "investment manager",
            "aifm",
            "fund admin",
            "portfolio manager",
            "sub-advisor",
            "investment advisor",
        ],
    );
    m.insert(
        "cbu.role:assign-service-provider",
        vec![
            "assign service provider",
            "depositary",
            "custodian",
            "auditor",
            "administrator",
            "transfer agent",
            "prime broker",
            "legal counsel",
            "tax advisor",
            "registrar",
            "fund accountant",
        ],
    );
    m.insert(
        "cbu.role:assign-signatory",
        vec![
            "assign signatory",
            "authorized signatory",
            "authorized trader",
            "power of attorney",
            "signing authority",
            "poa",
            "mandate holder",
            "signing rights",
        ],
    );

    // ==========================================================================
    // GRAPH/NAVIGATION VERBS
    // ==========================================================================
    m.insert(
        "graph.view",
        vec![
            "view graph",
            "show graph",
            "visualize",
            "display structure",
            "entity graph",
            "ownership graph",
            "structure visualization",
            "show structure",
        ],
    );
    m.insert(
        "graph.focus",
        vec![
            "focus on",
            "zoom to",
            "center on",
            "highlight entity",
            "select node",
            "navigate to",
        ],
    );
    m.insert(
        "graph.ancestors",
        vec![
            "show ancestors",
            "ownership chain up",
            "who owns this",
            "parent chain",
            "upstream owners",
            "trace up",
        ],
    );
    m.insert(
        "graph.descendants",
        vec![
            "show descendants",
            "ownership chain down",
            "what do they own",
            "child entities",
            "downstream holdings",
            "trace down",
        ],
    );
    m.insert(
        "graph.path",
        vec![
            "path between",
            "connection between",
            "how are they related",
            "relationship path",
            "find route",
            "link between",
        ],
    );
    m.insert(
        "graph.filter",
        vec![
            "filter graph",
            "show only",
            "hide",
            "filter by type",
            "show funds only",
            "show persons only",
            "filter entities",
        ],
    );
    m.insert(
        "graph.group-by",
        vec![
            "group by",
            "cluster by",
            "organize by",
            "group by jurisdiction",
            "group by type",
        ],
    );

    // ==========================================================================
    // KYC CASE VERBS
    // ==========================================================================
    m.insert(
        "kyc-case.create",
        vec![
            "create kyc case",
            "new kyc case",
            "start kyc",
            "open case",
            "initiate kyc",
            "begin kyc review",
            "onboarding case",
            "new case",
            "start review",
            "kick off kyc",
        ],
    );
    m.insert(
        "kyc-case.update-status",
        vec![
            "update case status",
            "change case status",
            "case progress",
            "move case forward",
            "advance case",
            "progress case",
            "case status change",
        ],
    );
    m.insert(
        "kyc-case.escalate",
        vec![
            "escalate case",
            "case escalation",
            "raise to senior",
            "escalate kyc",
            "send to compliance",
            "bump up case",
        ],
    );
    m.insert(
        "kyc-case.assign",
        vec![
            "assign case",
            "assign analyst",
            "assign reviewer",
            "case assignment",
            "allocate case",
            "who works on case",
        ],
    );
    m.insert(
        "kyc-case.set-risk-rating",
        vec![
            "set risk rating",
            "risk rate case",
            "case risk",
            "rate risk",
            "assign risk",
            "risk assessment",
            "high risk",
            "low risk",
            "medium risk",
        ],
    );
    m.insert(
        "kyc-case.close",
        vec![
            "close case",
            "complete case",
            "finalize case",
            "end case",
            "case completion",
            "finish kyc",
            "case done",
        ],
    );
    m.insert(
        "kyc-case.read",
        vec![
            "read case",
            "get case",
            "case details",
            "show case",
            "view case",
            "case info",
        ],
    );
    m.insert(
        "kyc-case.list-by-cbu",
        vec![
            "list cases",
            "cases for cbu",
            "cbu cases",
            "all cases",
            "case history",
        ],
    );
    m.insert(
        "kyc-case.reopen",
        vec![
            "reopen case",
            "case reopened",
            "remediation case",
            "review again",
            "periodic review",
            "event driven review",
        ],
    );
    m.insert(
        "kyc-case.state",
        vec![
            "case state",
            "full case status",
            "case with workstreams",
            "case summary",
            "case overview",
        ],
    );

    // ==========================================================================
    // ENTITY WORKSTREAM VERBS
    // ==========================================================================
    m.insert(
        "entity-workstream.create",
        vec![
            "create workstream",
            "entity workstream",
            "kyc workstream",
            "due diligence workstream",
            "new workstream",
            "add entity to case",
            "start entity review",
        ],
    );
    m.insert(
        "entity-workstream.update-status",
        vec![
            "update workstream",
            "workstream progress",
            "change workstream status",
            "advance workstream",
        ],
    );
    m.insert(
        "entity-workstream.block",
        vec![
            "block workstream",
            "workstream blocked",
            "pause workstream",
            "stop workstream",
        ],
    );
    m.insert(
        "entity-workstream.complete",
        vec![
            "complete workstream",
            "workstream done",
            "finish workstream",
            "workstream complete",
        ],
    );
    m.insert(
        "entity-workstream.set-enhanced-dd",
        vec![
            "enhanced dd",
            "enhanced due diligence",
            "edd required",
            "heightened dd",
            "extra scrutiny",
        ],
    );
    m.insert(
        "entity-workstream.set-ubo",
        vec![
            "mark as ubo",
            "workstream ubo",
            "ubo workstream",
            "identify ubo",
        ],
    );
    m.insert(
        "entity-workstream.list-by-case",
        vec![
            "list workstreams",
            "case workstreams",
            "all workstreams",
            "entities in case",
        ],
    );
    m.insert(
        "entity-workstream.state",
        vec![
            "workstream state",
            "workstream details",
            "workstream with requests",
        ],
    );

    // ==========================================================================
    // DOCUMENT REQUEST VERBS
    // ==========================================================================
    m.insert(
        "doc-request.create",
        vec![
            "request document",
            "ask for document",
            "doc request",
            "require document",
            "document requirement",
            "outstanding document",
            "need document",
            "document needed",
        ],
    );
    m.insert(
        "doc-request.mark-requested",
        vec![
            "mark requested",
            "formally request",
            "send request",
            "document requested",
        ],
    );
    m.insert(
        "doc-request.receive",
        vec![
            "receive document",
            "document received",
            "fulfilled request",
            "got document",
            "doc uploaded",
            "document submitted",
        ],
    );
    m.insert(
        "doc-request.verify",
        vec![
            "verify document",
            "validate document",
            "check document",
            "document verification",
            "doc verified",
            "approve document",
        ],
    );
    m.insert(
        "doc-request.reject",
        vec![
            "reject document",
            "document rejected",
            "invalid document",
            "doc not acceptable",
        ],
    );
    m.insert(
        "doc-request.waive",
        vec![
            "waive document",
            "document waived",
            "skip document",
            "not required",
            "waive requirement",
        ],
    );
    m.insert(
        "doc-request.list-by-workstream",
        vec![
            "list doc requests",
            "outstanding documents",
            "pending documents",
            "what documents needed",
        ],
    );

    // ==========================================================================
    // CASE SCREENING VERBS
    // ==========================================================================
    m.insert(
        "case-screening.run",
        vec![
            "run screening",
            "screen entity",
            "sanctions check",
            "pep check",
            "aml screening",
            "watchlist check",
            "adverse media",
            "compliance screening",
            "start screening",
            "initiate screening",
            "screen for sanctions",
            "screen for pep",
        ],
    );
    m.insert(
        "case-screening.complete",
        vec![
            "complete screening",
            "screening done",
            "screening finished",
            "screening result",
        ],
    );
    m.insert(
        "case-screening.review-hit",
        vec![
            "review hit",
            "screening hit",
            "hit review",
            "potential match",
            "review match",
            "hit confirmed",
            "hit dismissed",
            "false positive",
        ],
    );
    m.insert(
        "case-screening.list-by-workstream",
        vec![
            "list screenings",
            "screening history",
            "all screenings",
            "screening results",
        ],
    );

    // ==========================================================================
    // RED FLAG VERBS
    // ==========================================================================
    m.insert(
        "red-flag.raise",
        vec![
            "raise red flag",
            "flag issue",
            "compliance concern",
            "escalate issue",
            "alert",
            "raise concern",
            "report issue",
            "flag problem",
            "red flag identified",
        ],
    );
    m.insert(
        "red-flag.mitigate",
        vec![
            "mitigate red flag",
            "resolve flag",
            "address concern",
            "close red flag",
            "flag mitigated",
            "issue resolved",
        ],
    );
    m.insert(
        "red-flag.waive",
        vec![
            "waive red flag",
            "flag waived",
            "approve despite flag",
            "accept risk",
        ],
    );
    m.insert(
        "red-flag.dismiss",
        vec![
            "dismiss flag",
            "false positive flag",
            "flag dismissed",
            "not a concern",
        ],
    );
    m.insert(
        "red-flag.set-blocking",
        vec![
            "blocking flag",
            "flag blocks case",
            "hard stop",
            "case blocked",
        ],
    );
    m.insert(
        "red-flag.list-by-case",
        vec!["list red flags", "case flags", "all flags", "open flags"],
    );

    // ==========================================================================
    // SCREENING VERBS (PEP, Sanctions, Adverse Media)
    // ==========================================================================
    m.insert(
        "screening.pep",
        vec![
            "pep screening",
            "politically exposed",
            "pep check",
            "check for pep",
            "political exposure",
            "government official check",
        ],
    );
    m.insert(
        "screening.sanctions",
        vec![
            "sanctions screening",
            "sanctions check",
            "ofac check",
            "sanctions list",
            "restricted party",
            "blocked persons",
            "sdn list",
        ],
    );
    m.insert(
        "screening.adverse-media",
        vec![
            "adverse media",
            "negative news",
            "media screening",
            "news check",
            "reputation check",
            "bad press",
        ],
    );

    // ==========================================================================
    // DOCUMENT VERBS
    // ==========================================================================
    m.insert(
        "document.catalog",
        vec![
            "catalog document",
            "upload document",
            "add document",
            "attach file",
            "store document",
            "register document",
            "save document",
            "document uploaded",
        ],
    );
    m.insert(
        "document.extract",
        vec![
            "extract from document",
            "parse document",
            "read document",
            "document extraction",
            "ocr document",
            "extract data",
            "pull from document",
        ],
    );

    // ==========================================================================
    // SERVICE/PRODUCT VERBS
    // ==========================================================================
    m.insert(
        "service.list",
        vec![
            "list services",
            "available services",
            "what services",
            "service catalog",
            "show services",
        ],
    );
    m.insert(
        "product.list",
        vec![
            "list products",
            "available products",
            "what products",
            "product catalog",
            "show products",
        ],
    );
    m.insert(
        "product.subscribe",
        vec![
            "subscribe to product",
            "enable product",
            "activate product",
            "product subscription",
            "add product",
        ],
    );
    m.insert(
        "product.unsubscribe",
        vec![
            "unsubscribe product",
            "disable product",
            "deactivate product",
            "cancel subscription",
            "remove product",
        ],
    );

    // ==========================================================================
    // CUSTODY VERBS - UNIVERSE
    // ==========================================================================
    m.insert(
        "cbu-custody.add-universe",
        vec![
            "add to universe",
            "trading universe",
            "can trade",
            "allowed instruments",
            "add instrument class",
            "enable market",
            "what can we trade",
            "permitted instruments",
            "tradeable securities",
            "expand universe",
        ],
    );
    m.insert(
        "cbu-custody.list-universe",
        vec![
            "list universe",
            "show universe",
            "trading permissions",
            "what can cbu trade",
            "universe entries",
            "permitted instruments",
        ],
    );
    m.insert(
        "cbu-custody.remove-universe",
        vec![
            "remove from universe",
            "disable trading",
            "remove instrument class",
            "stop trading",
            "restrict universe",
        ],
    );

    // ==========================================================================
    // CUSTODY VERBS - SSI
    // ==========================================================================
    m.insert(
        "cbu-custody.create-ssi",
        vec![
            "create ssi",
            "standing settlement instruction",
            "settlement instruction",
            "new ssi",
            "add settlement details",
            "settlement account",
            "safekeeping account",
            "setup ssi",
        ],
    );
    m.insert(
        "cbu-custody.ensure-ssi",
        vec![
            "ensure ssi",
            "upsert ssi",
            "find or create ssi",
            "idempotent ssi",
            "ssi if not exists",
        ],
    );
    m.insert(
        "cbu-custody.activate-ssi",
        vec![
            "activate ssi",
            "enable ssi",
            "ssi active",
            "go live ssi",
            "ssi ready",
        ],
    );
    m.insert(
        "cbu-custody.suspend-ssi",
        vec![
            "suspend ssi",
            "disable ssi",
            "pause ssi",
            "ssi inactive",
            "deactivate ssi",
        ],
    );
    m.insert(
        "cbu-custody.list-ssis",
        vec![
            "list ssis",
            "show settlement instructions",
            "all ssis",
            "settlement accounts",
            "ssi list",
        ],
    );
    m.insert(
        "cbu-custody.setup-ssi",
        vec![
            "setup ssi",
            "bulk ssi import",
            "import settlement instructions",
            "load ssis",
            "ssi migration",
        ],
    );
    m.insert(
        "cbu-custody.lookup-ssi",
        vec![
            "lookup ssi",
            "find ssi",
            "resolve ssi",
            "which ssi",
            "ssi for trade",
            "ssi lookup",
        ],
    );

    // ==========================================================================
    // CUSTODY VERBS - BOOKING RULES
    // ==========================================================================
    m.insert(
        "cbu-custody.add-booking-rule",
        vec![
            "add booking rule",
            "routing rule",
            "settlement routing",
            "booking configuration",
            "trade routing",
            "alert rule",
            "ssi selection rule",
        ],
    );
    m.insert(
        "cbu-custody.ensure-booking-rule",
        vec![
            "ensure booking rule",
            "upsert booking rule",
            "idempotent booking rule",
        ],
    );
    m.insert(
        "cbu-custody.list-booking-rules",
        vec![
            "list booking rules",
            "show routing rules",
            "all booking rules",
            "routing configuration",
        ],
    );
    m.insert(
        "cbu-custody.update-rule-priority",
        vec![
            "update rule priority",
            "change rule order",
            "reorder rules",
            "rule precedence",
        ],
    );
    m.insert(
        "cbu-custody.deactivate-rule",
        vec![
            "deactivate rule",
            "disable booking rule",
            "remove routing rule",
        ],
    );

    // ==========================================================================
    // CUSTODY VERBS - AGENT OVERRIDES
    // ==========================================================================
    m.insert(
        "cbu-custody.add-agent-override",
        vec![
            "add agent override",
            "settlement chain override",
            "reag override",
            "deag override",
            "intermediary override",
            "agent chain",
        ],
    );
    m.insert(
        "cbu-custody.list-agent-overrides",
        vec![
            "list agent overrides",
            "show overrides",
            "settlement chain overrides",
        ],
    );
    m.insert(
        "cbu-custody.remove-agent-override",
        vec!["remove agent override", "delete override", "clear override"],
    );

    // ==========================================================================
    // CUSTODY VERBS - ANALYSIS
    // ==========================================================================
    m.insert(
        "cbu-custody.derive-required-coverage",
        vec![
            "derive required coverage",
            "what ssis needed",
            "coverage analysis",
            "ssi gap analysis",
            "what do we need",
        ],
    );
    m.insert(
        "cbu-custody.validate-booking-coverage",
        vec![
            "validate booking coverage",
            "check routing completeness",
            "booking gaps",
            "routing validation",
            "is routing complete",
        ],
    );

    // ==========================================================================
    // CUSTODY VERBS - SETTLEMENT EXTENSIONS
    // ==========================================================================
    m.insert(
        "cbu-custody.define-settlement-chain",
        vec![
            "define settlement chain",
            "settlement chain",
            "multi-hop settlement",
            "chain definition",
            "settlement path",
            "cross-border settlement",
        ],
    );
    m.insert(
        "cbu-custody.list-settlement-chains",
        vec!["list settlement chains", "show chains", "settlement paths"],
    );
    m.insert(
        "cbu-custody.set-fop-rules",
        vec![
            "set fop rules",
            "free of payment rules",
            "fop allowed",
            "fop threshold",
            "dvp vs fop",
            "fop configuration",
        ],
    );
    m.insert(
        "cbu-custody.list-fop-rules",
        vec!["list fop rules", "fop configuration", "show fop rules"],
    );
    m.insert(
        "cbu-custody.set-csd-preference",
        vec![
            "set csd preference",
            "preferred csd",
            "euroclear preference",
            "clearstream preference",
            "dtcc preference",
            "icsd preference",
        ],
    );
    m.insert(
        "cbu-custody.list-csd-preferences",
        vec![
            "list csd preferences",
            "csd configuration",
            "show csd preferences",
        ],
    );
    m.insert(
        "cbu-custody.set-settlement-cycle",
        vec![
            "set settlement cycle",
            "settlement cycle override",
            "t+1",
            "t+2",
            "t+3",
            "settlement timing",
        ],
    );
    m.insert(
        "cbu-custody.list-settlement-cycle-overrides",
        vec![
            "list settlement cycles",
            "cycle overrides",
            "settlement timing config",
        ],
    );

    // ==========================================================================
    // ENTITY SETTLEMENT VERBS
    // ==========================================================================
    m.insert(
        "entity-settlement.set-identity",
        vec![
            "set settlement identity",
            "counterparty identity",
            "settlement bic",
            "alert participant",
            "ctm participant",
            "counterparty setup",
        ],
    );
    m.insert(
        "entity-settlement.add-ssi",
        vec![
            "add counterparty ssi",
            "counterparty settlement",
            "their ssi",
            "broker ssi",
            "dealer ssi",
        ],
    );
    m.insert(
        "entity-settlement.remove-ssi",
        vec!["remove counterparty ssi", "delete their ssi"],
    );

    // ==========================================================================
    // PRICING CONFIG VERBS
    // ==========================================================================
    m.insert(
        "pricing-config.set",
        vec![
            "set pricing source",
            "pricing configuration",
            "valuation source",
            "bloomberg pricing",
            "reuters pricing",
            "price feed",
            "how to price",
            "pricing setup",
        ],
    );
    m.insert(
        "pricing-config.list",
        vec![
            "list pricing config",
            "show pricing sources",
            "pricing setup",
        ],
    );
    m.insert(
        "pricing-config.remove",
        vec!["remove pricing config", "delete pricing source"],
    );
    m.insert(
        "pricing-config.find-for-instrument",
        vec![
            "find pricing source",
            "which pricing",
            "pricing for instrument",
            "resolve pricing",
        ],
    );
    m.insert(
        "pricing-config.link-resource",
        vec![
            "link pricing resource",
            "connect price feed",
            "pricing resource",
        ],
    );
    m.insert(
        "pricing-config.set-valuation-schedule",
        vec![
            "set valuation schedule",
            "valuation frequency",
            "when to price",
            "eod pricing",
            "intraday pricing",
            "nav timing",
        ],
    );
    m.insert(
        "pricing-config.list-valuation-schedules",
        vec![
            "list valuation schedules",
            "show pricing schedules",
            "valuation timing",
        ],
    );
    m.insert(
        "pricing-config.set-fallback-chain",
        vec![
            "set fallback chain",
            "pricing fallback",
            "backup pricing source",
            "secondary pricing",
            "price fallback",
        ],
    );
    m.insert(
        "pricing-config.set-stale-policy",
        vec![
            "set stale policy",
            "stale price handling",
            "old price policy",
            "price staleness",
            "max price age",
        ],
    );
    m.insert(
        "pricing-config.set-nav-threshold",
        vec![
            "set nav threshold",
            "nav impact alert",
            "price movement alert",
            "nav tolerance",
        ],
    );
    m.insert(
        "pricing-config.validate-pricing-config",
        vec![
            "validate pricing config",
            "pricing gaps",
            "check pricing setup",
            "pricing completeness",
        ],
    );

    // ==========================================================================
    // INSTRUCTION PROFILE VERBS
    // ==========================================================================
    m.insert(
        "instruction-profile.define-message-type",
        vec![
            "define message type",
            "new message type",
            "mt message",
            "mx message",
            "swift message type",
            "fix message",
            "instruction type",
        ],
    );
    m.insert(
        "instruction-profile.list-message-types",
        vec![
            "list message types",
            "available messages",
            "swift messages",
            "message catalog",
        ],
    );
    m.insert(
        "instruction-profile.create-template",
        vec![
            "create instruction template",
            "new template",
            "message template",
            "swift template",
            "instruction format",
        ],
    );
    m.insert(
        "instruction-profile.read-template",
        vec!["read template", "show template", "template details"],
    );
    m.insert(
        "instruction-profile.list-templates",
        vec!["list templates", "available templates", "message templates"],
    );
    m.insert(
        "instruction-profile.assign-template",
        vec![
            "assign template",
            "map template",
            "which template",
            "template for instrument",
            "template assignment",
            "how to instruct",
        ],
    );
    m.insert(
        "instruction-profile.list-assignments",
        vec![
            "list template assignments",
            "show assignments",
            "template mappings",
        ],
    );
    m.insert(
        "instruction-profile.remove-assignment",
        vec![
            "remove assignment",
            "unassign template",
            "delete assignment",
        ],
    );
    m.insert(
        "instruction-profile.add-field-override",
        vec![
            "add field override",
            "override field",
            "custom field value",
            "field customization",
            "swift field override",
            "message field override",
        ],
    );
    m.insert(
        "instruction-profile.list-field-overrides",
        vec![
            "list field overrides",
            "show overrides",
            "field customizations",
        ],
    );
    m.insert(
        "instruction-profile.remove-field-override",
        vec!["remove field override", "delete override", "clear override"],
    );
    m.insert(
        "instruction-profile.find-template",
        vec![
            "find template",
            "which template for trade",
            "resolve template",
            "template lookup",
        ],
    );
    m.insert(
        "instruction-profile.validate-profile",
        vec![
            "validate instruction profile",
            "instruction gaps",
            "template coverage",
            "instruction completeness",
        ],
    );
    m.insert(
        "instruction-profile.derive-required-templates",
        vec![
            "derive required templates",
            "what templates needed",
            "template gap analysis",
        ],
    );

    // ==========================================================================
    // TRADE GATEWAY VERBS
    // ==========================================================================
    m.insert(
        "trade-gateway.define-gateway",
        vec![
            "define gateway",
            "new gateway",
            "add gateway",
            "create gateway",
            "trade gateway",
            "swift gateway",
            "fix gateway",
        ],
    );
    m.insert(
        "trade-gateway.read-gateway",
        vec!["read gateway", "gateway details", "show gateway"],
    );
    m.insert(
        "trade-gateway.list-gateways",
        vec![
            "list gateways",
            "available gateways",
            "all gateways",
            "gateway catalog",
        ],
    );
    m.insert(
        "trade-gateway.enable-gateway",
        vec![
            "enable gateway",
            "connect gateway",
            "gateway connectivity",
            "activate gateway connection",
            "setup gateway",
        ],
    );
    m.insert(
        "trade-gateway.activate-gateway",
        vec!["activate gateway", "go live gateway", "gateway active"],
    );
    m.insert(
        "trade-gateway.suspend-gateway",
        vec![
            "suspend gateway",
            "disable gateway",
            "pause gateway",
            "gateway inactive",
        ],
    );
    m.insert(
        "trade-gateway.list-cbu-gateways",
        vec![
            "list cbu gateways",
            "cbu connectivity",
            "connected gateways",
            "gateway status",
        ],
    );
    m.insert(
        "trade-gateway.add-routing-rule",
        vec![
            "add gateway routing",
            "route to gateway",
            "gateway rule",
            "which gateway",
            "trade routing",
            "instruction routing",
        ],
    );
    m.insert(
        "trade-gateway.list-routing-rules",
        vec!["list routing rules", "gateway routing", "show routing"],
    );
    m.insert(
        "trade-gateway.remove-routing-rule",
        vec!["remove routing rule", "delete gateway route"],
    );
    m.insert(
        "trade-gateway.set-fallback",
        vec![
            "set gateway fallback",
            "fallback gateway",
            "backup gateway",
            "gateway failover",
        ],
    );
    m.insert(
        "trade-gateway.list-fallbacks",
        vec!["list fallbacks", "gateway fallbacks", "failover config"],
    );
    m.insert(
        "trade-gateway.find-gateway",
        vec![
            "find gateway",
            "which gateway for trade",
            "resolve gateway",
            "gateway lookup",
        ],
    );
    m.insert(
        "trade-gateway.validate-routing",
        vec![
            "validate gateway routing",
            "routing gaps",
            "gateway coverage",
            "routing completeness",
        ],
    );
    m.insert(
        "trade-gateway.derive-required-routes",
        vec![
            "derive required routes",
            "what routes needed",
            "routing gap analysis",
        ],
    );

    // ==========================================================================
    // CORPORATE ACTION VERBS
    // ==========================================================================
    m.insert(
        "corporate-action.define-event-type",
        vec![
            "define ca event",
            "corporate action type",
            "new event type",
            "ca event definition",
        ],
    );
    m.insert(
        "corporate-action.list-event-types",
        vec![
            "list ca events",
            "corporate action types",
            "event catalog",
            "ca types",
        ],
    );
    m.insert(
        "corporate-action.set-preferences",
        vec![
            "set ca preferences",
            "corporate action preferences",
            "ca processing mode",
            "how to handle ca",
            "dividend preferences",
            "auto instruct ca",
            "manual ca",
        ],
    );
    m.insert(
        "corporate-action.list-preferences",
        vec!["list ca preferences", "show ca config", "ca setup"],
    );
    m.insert(
        "corporate-action.remove-preference",
        vec!["remove ca preference", "delete ca config"],
    );
    m.insert(
        "corporate-action.set-instruction-window",
        vec![
            "set instruction window",
            "ca deadline",
            "response window",
            "ca cutoff",
            "instruction deadline",
            "ca timing",
        ],
    );
    m.insert(
        "corporate-action.list-instruction-windows",
        vec![
            "list instruction windows",
            "ca deadlines",
            "response deadlines",
        ],
    );
    m.insert(
        "corporate-action.link-ca-ssi",
        vec![
            "link ca ssi",
            "ca payment ssi",
            "dividend ssi",
            "ca proceeds account",
            "where to receive ca",
        ],
    );
    m.insert(
        "corporate-action.list-ca-ssi-mappings",
        vec![
            "list ca ssi mappings",
            "ca payment accounts",
            "dividend accounts",
        ],
    );
    m.insert(
        "corporate-action.validate-ca-config",
        vec![
            "validate ca config",
            "ca gaps",
            "corporate action completeness",
            "ca readiness",
        ],
    );
    m.insert(
        "corporate-action.derive-required-config",
        vec![
            "derive ca config",
            "what ca config needed",
            "ca gap analysis",
        ],
    );

    // ==========================================================================
    // TAX CONFIG VERBS
    // ==========================================================================
    m.insert(
        "tax-config.set-withholding-profile",
        vec![
            "set withholding profile",
            "withholding tax",
            "tax rate",
            "treaty rate",
            "statutory rate",
            "qi status",
            "nqi status",
            "tax setup",
        ],
    );
    m.insert(
        "tax-config.list-withholding-profiles",
        vec![
            "list withholding profiles",
            "tax configuration",
            "withholding rates",
        ],
    );
    m.insert(
        "tax-config.set-reclaim-preferences",
        vec![
            "set reclaim preferences",
            "tax reclaim",
            "reclaim method",
            "quick refund",
            "standard reclaim",
            "tax recovery",
        ],
    );
    m.insert(
        "tax-config.list-reclaim-preferences",
        vec![
            "list reclaim preferences",
            "reclaim configuration",
            "tax recovery setup",
        ],
    );
    m.insert(
        "tax-config.link-tax-documentation",
        vec![
            "link tax documentation",
            "tax docs",
            "w8 form",
            "w9 form",
            "crs form",
            "fatca form",
            "tax residency certificate",
            "beneficial owner certificate",
        ],
    );
    m.insert(
        "tax-config.list-tax-documentation",
        vec![
            "list tax documentation",
            "tax docs status",
            "expiring tax docs",
        ],
    );
    m.insert(
        "tax-config.set-rate-override",
        vec![
            "set tax rate override",
            "override withholding",
            "custom tax rate",
            "tax exception",
        ],
    );
    m.insert(
        "tax-config.list-rate-overrides",
        vec!["list rate overrides", "tax exceptions", "custom rates"],
    );
    m.insert(
        "tax-config.validate-tax-config",
        vec![
            "validate tax config",
            "tax gaps",
            "tax completeness",
            "tax readiness",
            "expiring documentation",
        ],
    );
    m.insert(
        "tax-config.find-withholding-rate",
        vec![
            "find withholding rate",
            "which tax rate",
            "applicable rate",
            "tax rate lookup",
        ],
    );

    // ==========================================================================
    // TRADING PROFILE VERBS
    // ==========================================================================
    m.insert(
        "trading-profile.import",
        vec![
            "import trading profile",
            "load trading matrix",
            "upload trading config",
            "trading profile yaml",
        ],
    );
    m.insert(
        "trading-profile.read",
        vec![
            "read trading profile",
            "show trading matrix",
            "trading configuration",
        ],
    );
    m.insert(
        "trading-profile.get-active",
        vec![
            "get active profile",
            "current trading profile",
            "active trading matrix",
        ],
    );
    m.insert(
        "trading-profile.list-versions",
        vec![
            "list profile versions",
            "trading profile history",
            "version history",
        ],
    );
    m.insert(
        "trading-profile.activate",
        vec![
            "activate profile",
            "enable trading profile",
            "make profile active",
        ],
    );
    m.insert(
        "trading-profile.materialize",
        vec![
            "materialize profile",
            "sync profile to tables",
            "apply trading profile",
            "deploy profile",
        ],
    );
    m.insert(
        "trading-profile.validate",
        vec![
            "validate profile",
            "check trading profile",
            "profile validation",
        ],
    );
    m.insert(
        "trading-profile.diff",
        vec![
            "diff profiles",
            "compare profiles",
            "profile changes",
            "what changed",
        ],
    );
    m.insert(
        "trading-profile.export",
        vec!["export profile", "download trading matrix", "profile yaml"],
    );
    m.insert(
        "trading-profile.export-full-matrix",
        vec![
            "export full matrix",
            "complete trading matrix",
            "full trading document",
            "comprehensive matrix export",
            "matrix document",
        ],
    );
    m.insert(
        "trading-profile.validate-matrix-completeness",
        vec![
            "validate matrix completeness",
            "matrix ready",
            "go live check",
            "trading readiness",
            "matrix gaps",
        ],
    );
    m.insert(
        "trading-profile.generate-gap-remediation-plan",
        vec![
            "generate remediation plan",
            "fix matrix gaps",
            "gap remediation",
            "what to fix",
        ],
    );

    // ==========================================================================
    // LIFECYCLE VERBS
    // ==========================================================================
    m.insert(
        "lifecycle.read",
        vec![
            "read lifecycle",
            "lifecycle definition",
            "lifecycle details",
        ],
    );
    m.insert(
        "lifecycle.list",
        vec!["list lifecycles", "available lifecycles", "all lifecycles"],
    );
    m.insert(
        "lifecycle.list-by-instrument",
        vec![
            "lifecycles for instrument",
            "instrument lifecycles",
            "what lifecycles apply",
        ],
    );
    m.insert(
        "lifecycle.discover",
        vec![
            "discover lifecycle",
            "lifecycle discovery",
            "find lifecycles",
            "applicable lifecycles",
        ],
    );
    m.insert(
        "lifecycle.provision",
        vec![
            "provision lifecycle resource",
            "setup lifecycle",
            "enable lifecycle",
            "lifecycle provisioning",
        ],
    );
    m.insert(
        "lifecycle.activate",
        vec![
            "activate lifecycle",
            "go live lifecycle",
            "lifecycle active",
        ],
    );
    m.insert(
        "lifecycle.suspend",
        vec!["suspend lifecycle", "pause lifecycle", "lifecycle inactive"],
    );
    m.insert(
        "lifecycle.analyze-gaps",
        vec![
            "analyze lifecycle gaps",
            "lifecycle gap analysis",
            "missing lifecycles",
            "lifecycle readiness",
        ],
    );
    m.insert(
        "lifecycle.check-readiness",
        vec![
            "check lifecycle readiness",
            "lifecycle ready",
            "can go live",
        ],
    );
    m.insert(
        "lifecycle.generate-plan",
        vec![
            "generate lifecycle plan",
            "lifecycle remediation",
            "fix lifecycle gaps",
        ],
    );
    m.insert(
        "lifecycle.visualize-coverage",
        vec![
            "visualize lifecycle coverage",
            "lifecycle diagram",
            "lifecycle coverage chart",
        ],
    );

    // ==========================================================================
    // ISDA VERBS
    // ==========================================================================
    m.insert(
        "isda.create",
        vec![
            "create isda",
            "new isda agreement",
            "isda master",
            "derivatives agreement",
            "otc agreement",
        ],
    );
    m.insert(
        "isda.add-coverage",
        vec![
            "add isda coverage",
            "covered instruments",
            "isda instrument class",
            "derivatives coverage",
        ],
    );
    m.insert(
        "isda.remove-coverage",
        vec!["remove isda coverage", "uncovered instruments"],
    );
    m.insert(
        "isda.add-csa",
        vec![
            "add csa",
            "credit support annex",
            "collateral agreement",
            "margin agreement",
            "vm csa",
            "im csa",
        ],
    );
    m.insert("isda.remove-csa", vec!["remove csa", "delete csa"]);
    m.insert(
        "isda.list",
        vec![
            "list isda",
            "show isda agreements",
            "all isda",
            "derivatives agreements",
        ],
    );

    // ==========================================================================
    // MATRIX OVERLAY VERBS
    // ==========================================================================
    m.insert(
        "matrix-overlay.subscribe",
        vec![
            "subscribe to overlay",
            "product overlay",
            "add product to matrix",
            "enable product overlay",
        ],
    );
    m.insert(
        "matrix-overlay.unsubscribe",
        vec![
            "unsubscribe overlay",
            "remove product overlay",
            "disable overlay",
        ],
    );
    m.insert(
        "matrix-overlay.add",
        vec!["add overlay", "create overlay", "new matrix overlay"],
    );
    m.insert(
        "matrix-overlay.list",
        vec!["list overlays", "show overlays", "product overlays"],
    );
    m.insert(
        "matrix-overlay.effective-matrix",
        vec![
            "effective matrix",
            "computed matrix",
            "merged matrix",
            "final matrix",
        ],
    );
    m.insert(
        "matrix-overlay.unified-gaps",
        vec![
            "unified gaps",
            "all gaps",
            "combined gap analysis",
            "matrix gaps",
        ],
    );
    m.insert(
        "matrix-overlay.compare-products",
        vec![
            "compare products",
            "product comparison",
            "product differences",
        ],
    );

    // ==========================================================================
    // TEMPORAL VERBS
    // ==========================================================================
    m.insert(
        "temporal.ownership-as-of",
        vec![
            "ownership as of",
            "historical ownership",
            "ownership on date",
            "past ownership",
            "point in time ownership",
        ],
    );
    m.insert(
        "temporal.ubo-chain-as-of",
        vec![
            "ubo chain as of",
            "historical ubo",
            "ubo on date",
            "past ubo structure",
        ],
    );
    m.insert(
        "temporal.cbu-relationships-as-of",
        vec![
            "cbu relationships as of",
            "historical relationships",
            "relationships on date",
        ],
    );
    m.insert(
        "temporal.cbu-roles-as-of",
        vec!["roles as of", "historical roles", "roles on date"],
    );
    m.insert(
        "temporal.cbu-state-at-approval",
        vec![
            "state at approval",
            "snapshot at approval",
            "what was approved",
        ],
    );
    m.insert(
        "temporal.relationship-history",
        vec!["relationship history", "audit trail", "change history"],
    );
    m.insert(
        "temporal.entity-history",
        vec!["entity history", "entity changes", "entity audit"],
    );
    m.insert(
        "temporal.compare-ownership",
        vec![
            "compare ownership",
            "ownership diff",
            "what changed between dates",
        ],
    );

    // ==========================================================================
    // TEAM VERBS
    // ==========================================================================
    m.insert(
        "team.create",
        vec!["create team", "new team", "add team", "setup team"],
    );
    m.insert("team.read", vec!["read team", "team details", "show team"]);
    m.insert(
        "team.archive",
        vec!["archive team", "deactivate team", "remove team"],
    );
    m.insert(
        "team.add-member",
        vec![
            "add team member",
            "add user to team",
            "join team",
            "team membership",
        ],
    );
    m.insert(
        "team.remove-member",
        vec!["remove team member", "leave team", "remove from team"],
    );
    m.insert(
        "team.update-member",
        vec!["update member", "change member role", "modify membership"],
    );
    m.insert(
        "team.transfer-member",
        vec!["transfer member", "move to team", "reassign to team"],
    );
    m.insert(
        "team.add-governance-member",
        vec!["add governance member", "board member", "committee member"],
    );
    m.insert(
        "team.verify-governance-access",
        vec![
            "verify governance access",
            "audit governance",
            "governance check",
        ],
    );
    m.insert(
        "team.add-cbu-access",
        vec!["add cbu access", "team cbu access", "grant access"],
    );
    m.insert(
        "team.remove-cbu-access",
        vec!["remove cbu access", "revoke access"],
    );
    m.insert(
        "team.grant-service",
        vec!["grant service", "team entitlement", "enable service"],
    );
    m.insert(
        "team.revoke-service",
        vec!["revoke service", "remove entitlement", "disable service"],
    );
    m.insert(
        "team.list-members",
        vec!["list team members", "team roster", "who is on team"],
    );
    m.insert(
        "team.list-cbus",
        vec!["list team cbus", "team access", "cbus for team"],
    );

    // ==========================================================================
    // USER VERBS
    // ==========================================================================
    m.insert(
        "user.create",
        vec!["create user", "new user", "add user", "register user"],
    );
    m.insert(
        "user.suspend",
        vec!["suspend user", "disable user", "deactivate user"],
    );
    m.insert(
        "user.reactivate",
        vec!["reactivate user", "enable user", "activate user"],
    );
    m.insert(
        "user.offboard",
        vec!["offboard user", "terminate user", "user left company"],
    );
    m.insert(
        "user.list-teams",
        vec!["user teams", "which teams", "team membership"],
    );
    m.insert(
        "user.list-cbus",
        vec!["user cbus", "user access", "what can user access"],
    );
    m.insert(
        "user.check-access",
        vec!["check user access", "can user access", "access check"],
    );

    // ==========================================================================
    // SLA VERBS
    // ==========================================================================
    m.insert(
        "sla.list-templates",
        vec!["list sla templates", "sla catalog", "available slas"],
    );
    m.insert(
        "sla.read-template",
        vec!["read sla template", "sla details", "template details"],
    );
    m.insert(
        "sla.commit",
        vec![
            "commit sla",
            "sla commitment",
            "agree to sla",
            "sla agreement",
        ],
    );
    m.insert(
        "sla.bind-to-profile",
        vec!["bind sla to profile", "sla for profile", "link sla"],
    );
    m.insert(
        "sla.bind-to-service",
        vec!["bind sla to service", "service sla"],
    );
    m.insert(
        "sla.bind-to-resource",
        vec!["bind sla to resource", "resource sla"],
    );
    m.insert("sla.bind-to-isda", vec!["bind sla to isda", "isda sla"]);
    m.insert("sla.bind-to-csa", vec!["bind sla to csa", "csa sla"]);
    m.insert(
        "sla.list-commitments",
        vec!["list sla commitments", "cbu slas", "all commitments"],
    );
    m.insert(
        "sla.suspend-commitment",
        vec!["suspend sla", "pause commitment"],
    );
    m.insert(
        "sla.record-measurement",
        vec!["record sla measurement", "sla metric", "measure sla"],
    );
    m.insert(
        "sla.list-measurements",
        vec![
            "list sla measurements",
            "sla history",
            "measurement history",
        ],
    );
    m.insert(
        "sla.report-breach",
        vec!["report sla breach", "sla violation", "sla failure"],
    );
    m.insert(
        "sla.update-remediation",
        vec!["update remediation", "breach remediation", "fix sla breach"],
    );
    m.insert(
        "sla.resolve-breach",
        vec!["resolve breach", "breach resolved", "sla fixed"],
    );
    m.insert(
        "sla.escalate-breach",
        vec!["escalate breach", "sla escalation"],
    );
    m.insert(
        "sla.list-open-breaches",
        vec!["list open breaches", "active breaches", "unresolved slas"],
    );

    // ==========================================================================
    // REGULATORY VERBS
    // ==========================================================================
    m.insert(
        "regulatory.registration.add",
        vec![
            "add regulatory registration",
            "register with regulator",
            "fca registration",
            "sec registration",
            "regulatory license",
        ],
    );
    m.insert(
        "regulatory.registration.list",
        vec![
            "list registrations",
            "regulatory status",
            "all registrations",
        ],
    );
    m.insert(
        "regulatory.registration.verify",
        vec![
            "verify registration",
            "check registration",
            "registration verification",
        ],
    );
    m.insert(
        "regulatory.registration.remove",
        vec!["remove registration", "withdraw registration", "deregister"],
    );
    m.insert(
        "regulatory.status.check",
        vec![
            "check regulatory status",
            "is regulated",
            "regulatory check",
        ],
    );

    // ==========================================================================
    // SEMANTIC VERBS
    // ==========================================================================
    m.insert(
        "semantic.get-state",
        vec![
            "get semantic state",
            "onboarding progress",
            "where are we",
            "stage progress",
        ],
    );
    m.insert(
        "semantic.list-stages",
        vec!["list stages", "all stages", "stage definitions"],
    );
    m.insert(
        "semantic.stages-for-product",
        vec!["stages for product", "product stages", "required stages"],
    );
    m.insert(
        "semantic.next-actions",
        vec![
            "next actions",
            "what to do next",
            "suggested actions",
            "actionable stages",
        ],
    );
    m.insert(
        "semantic.missing-entities",
        vec!["missing entities", "what is missing", "gaps in structure"],
    );
    m.insert(
        "semantic.prompt-context",
        vec!["prompt context", "agent context", "session context"],
    );

    // ==========================================================================
    // CASH SWEEP VERBS
    // ==========================================================================
    m.insert(
        "cash-sweep.configure",
        vec![
            "configure cash sweep",
            "setup sweep",
            "stif configuration",
            "cash management",
        ],
    );
    m.insert(
        "cash-sweep.link-resource",
        vec!["link sweep resource", "sweep account"],
    );
    m.insert(
        "cash-sweep.list",
        vec!["list cash sweeps", "sweep configuration", "all sweeps"],
    );
    m.insert(
        "cash-sweep.update-threshold",
        vec!["update sweep threshold", "change threshold"],
    );
    m.insert(
        "cash-sweep.update-timing",
        vec!["update sweep timing", "change sweep time"],
    );
    m.insert(
        "cash-sweep.change-vehicle",
        vec!["change sweep vehicle", "different stif", "change mmf"],
    );
    m.insert("cash-sweep.suspend", vec!["suspend sweep", "pause sweep"]);
    m.insert(
        "cash-sweep.reactivate",
        vec!["reactivate sweep", "resume sweep"],
    );
    m.insert(
        "cash-sweep.remove",
        vec!["remove sweep", "delete sweep config"],
    );

    // ==========================================================================
    // INVESTMENT MANAGER VERBS
    // ==========================================================================
    m.insert(
        "investment-manager.assign",
        vec![
            "assign investment manager",
            "add im",
            "investment manager setup",
            "appoint im",
        ],
    );
    m.insert(
        "investment-manager.set-scope",
        vec!["set im scope", "im trading scope", "im permissions"],
    );
    m.insert(
        "investment-manager.link-connectivity",
        vec![
            "link im connectivity",
            "im instruction method",
            "how im sends trades",
        ],
    );
    m.insert(
        "investment-manager.list",
        vec!["list investment managers", "all ims", "im assignments"],
    );
    m.insert("investment-manager.suspend", vec!["suspend im", "pause im"]);
    m.insert(
        "investment-manager.terminate",
        vec!["terminate im", "end im relationship"],
    );
    m.insert(
        "investment-manager.find-for-trade",
        vec!["find im for trade", "which im", "im for instrument"],
    );

    // ==========================================================================
    // FUND INVESTOR VERBS
    // ==========================================================================
    m.insert(
        "fund-investor.create",
        vec![
            "create fund investor",
            "register investor",
            "new investor",
            "add investor to fund",
        ],
    );
    m.insert(
        "fund-investor.list",
        vec!["list fund investors", "all investors", "investor list"],
    );
    m.insert(
        "fund-investor.update-kyc-status",
        vec!["update investor kyc", "investor kyc status"],
    );
    m.insert(
        "fund-investor.get",
        vec!["get investor", "investor details"],
    );

    // ==========================================================================
    // DELEGATION VERBS
    // ==========================================================================
    m.insert(
        "delegation.add",
        vec![
            "add delegation",
            "delegate to",
            "sub-advisor",
            "outsource to",
            "delegation chain",
        ],
    );
    m.insert(
        "delegation.end",
        vec!["end delegation", "terminate delegation", "stop delegation"],
    );
    m.insert(
        "delegation.list-delegates",
        vec!["list delegates", "who do we delegate to", "our delegates"],
    );
    m.insert(
        "delegation.list-delegations-received",
        vec![
            "delegations received",
            "who delegates to us",
            "received delegations",
        ],
    );

    // ==========================================================================
    // DELIVERY VERBS
    // ==========================================================================
    m.insert(
        "delivery.record",
        vec!["record delivery", "service delivery", "delivered service"],
    );
    m.insert(
        "delivery.complete",
        vec!["complete delivery", "delivery done", "service delivered"],
    );
    m.insert(
        "delivery.fail",
        vec!["delivery failed", "service failure", "failed delivery"],
    );

    // ==========================================================================
    // SERVICE RESOURCE VERBS
    // ==========================================================================
    m.insert(
        "service-resource.read",
        vec![
            "read resource type",
            "resource details",
            "resource definition",
        ],
    );
    m.insert(
        "service-resource.list",
        vec![
            "list resource types",
            "available resources",
            "resource catalog",
        ],
    );
    m.insert(
        "service-resource.list-by-service",
        vec!["resources for service", "service resources"],
    );
    m.insert(
        "service-resource.list-attributes",
        vec![
            "list resource attributes",
            "required attributes",
            "attribute requirements",
        ],
    );
    m.insert(
        "service-resource.provision",
        vec![
            "provision resource",
            "create resource instance",
            "setup resource",
            "new resource instance",
        ],
    );
    m.insert(
        "service-resource.set-attr",
        vec![
            "set resource attribute",
            "configure resource",
            "resource setting",
        ],
    );
    m.insert(
        "service-resource.activate",
        vec!["activate resource", "resource active", "go live resource"],
    );
    m.insert(
        "service-resource.suspend",
        vec!["suspend resource", "pause resource"],
    );
    m.insert(
        "service-resource.decommission",
        vec![
            "decommission resource",
            "retire resource",
            "remove resource",
        ],
    );
    m.insert(
        "service-resource.validate-attrs",
        vec![
            "validate resource attrs",
            "check resource config",
            "resource validation",
        ],
    );

    // ==========================================================================
    // CLIENT PORTAL VERBS
    // ==========================================================================
    m.insert(
        "client.get-status",
        vec![
            "get onboarding status",
            "my status",
            "where am i",
            "onboarding progress",
        ],
    );
    m.insert(
        "client.get-outstanding",
        vec![
            "outstanding requests",
            "what do i need to do",
            "pending items",
        ],
    );
    m.insert(
        "client.get-request-detail",
        vec!["request detail", "why is this needed", "request info"],
    );
    m.insert(
        "client.get-entity-info",
        vec!["entity info", "my entity", "entity summary"],
    );
    m.insert(
        "client.submit-document",
        vec!["submit document", "upload document", "provide document"],
    );
    m.insert(
        "client.provide-info",
        vec!["provide info", "submit information", "answer question"],
    );
    m.insert(
        "client.add-note",
        vec!["add note", "leave comment", "note on request"],
    );
    m.insert(
        "client.request-clarification",
        vec![
            "request clarification",
            "ask question",
            "need help",
            "dont understand",
        ],
    );
    m.insert(
        "client.start-collection",
        vec![
            "start collection",
            "begin guided collection",
            "collect info",
        ],
    );
    m.insert(
        "client.collection-response",
        vec!["collection response", "answer field", "provide value"],
    );
    m.insert(
        "client.collection-confirm",
        vec![
            "confirm collection",
            "submit collected data",
            "finish collection",
        ],
    );
    m.insert(
        "client.escalate",
        vec![
            "escalate",
            "speak to human",
            "need help",
            "contact relationship manager",
        ],
    );

    // ==========================================================================
    // BATCH VERBS
    // ==========================================================================
    m.insert(
        "batch.pause",
        vec!["pause batch", "stop batch", "batch pause"],
    );
    m.insert(
        "batch.resume",
        vec!["resume batch", "continue batch", "restart batch"],
    );
    m.insert(
        "batch.continue",
        vec!["batch continue", "process more", "next batch item"],
    );
    m.insert(
        "batch.skip",
        vec!["skip batch item", "skip current", "next item"],
    );
    m.insert(
        "batch.abort",
        vec!["abort batch", "cancel batch", "stop all"],
    );
    m.insert(
        "batch.status",
        vec!["batch status", "batch progress", "how is batch doing"],
    );
    m.insert(
        "batch.add-products",
        vec![
            "batch add products",
            "bulk add products",
            "products to multiple cbus",
        ],
    );

    // ==========================================================================
    // KYC AGREEMENT VERBS
    // ==========================================================================
    m.insert(
        "kyc-agreement.create",
        vec![
            "create kyc agreement",
            "kyc service agreement",
            "sponsor agreement",
        ],
    );
    m.insert(
        "kyc-agreement.read",
        vec!["read kyc agreement", "agreement details"],
    );
    m.insert(
        "kyc-agreement.list",
        vec!["list kyc agreements", "sponsor agreements"],
    );
    m.insert(
        "kyc-agreement.update-status",
        vec!["update agreement status", "agreement status change"],
    );

    // ==========================================================================
    // KYC SCOPE VERBS
    // ==========================================================================
    m.insert(
        "kyc.preview-scope",
        vec![
            "preview kyc scope",
            "kyc obligations",
            "who needs kyc",
            "scope preview",
        ],
    );
    m.insert(
        "kyc.recommend",
        vec!["kyc recommendation", "recommend approval", "kyc decision"],
    );
    m.insert(
        "kyc.sponsor-decision",
        vec![
            "sponsor decision",
            "sponsor approval",
            "sponsor accept",
            "sponsor reject",
        ],
    );

    // ==========================================================================
    // REQUEST VERBS
    // ==========================================================================
    m.insert(
        "request.create",
        vec![
            "create request",
            "new request",
            "outstanding request",
            "need from client",
        ],
    );
    m.insert(
        "request.list",
        vec!["list requests", "outstanding requests", "pending requests"],
    );
    m.insert(
        "request.overdue",
        vec!["overdue requests", "late requests", "past due"],
    );
    m.insert(
        "request.fulfill",
        vec!["fulfill request", "request fulfilled", "request done"],
    );
    m.insert(
        "request.cancel",
        vec!["cancel request", "void request", "remove request"],
    );
    m.insert(
        "request.extend",
        vec!["extend request", "more time", "extend deadline"],
    );
    m.insert(
        "request.remind",
        vec!["remind", "send reminder", "follow up"],
    );
    m.insert(
        "request.escalate",
        vec!["escalate request", "bump request", "urgent request"],
    );
    m.insert(
        "request.waive",
        vec!["waive request", "not needed", "skip request"],
    );

    // ==========================================================================
    // CASE EVENT VERBS
    // ==========================================================================
    m.insert(
        "case-event.log",
        vec!["log event", "case event", "audit log", "record activity"],
    );
    m.insert(
        "case-event.list-by-case",
        vec!["list case events", "case history", "event log"],
    );

    // ==========================================================================
    // OBSERVATION VERBS
    // ==========================================================================
    m.insert(
        "observation.record",
        vec![
            "record observation",
            "capture observation",
            "observe attribute",
            "attribute observation",
        ],
    );
    m.insert(
        "observation.record-from-document",
        vec![
            "observation from document",
            "extract observation",
            "document observation",
        ],
    );
    m.insert(
        "observation.supersede",
        vec![
            "supersede observation",
            "replace observation",
            "newer observation",
        ],
    );
    m.insert(
        "observation.list-for-entity",
        vec![
            "list observations",
            "entity observations",
            "all observations",
        ],
    );
    m.insert(
        "observation.list-for-attribute",
        vec!["observations for attribute", "attribute history"],
    );
    m.insert(
        "observation.get-current",
        vec![
            "current observation",
            "best observation",
            "latest observation",
        ],
    );
    m.insert(
        "observation.reconcile",
        vec![
            "reconcile observations",
            "compare observations",
            "find conflicts",
        ],
    );
    m.insert(
        "observation.verify-allegations",
        vec![
            "verify allegations",
            "check allegations",
            "allegation verification",
        ],
    );

    // ==========================================================================
    // ALLEGATION VERBS
    // ==========================================================================
    m.insert(
        "allegation.record",
        vec![
            "record allegation",
            "client claim",
            "alleged value",
            "client says",
        ],
    );
    m.insert(
        "allegation.verify",
        vec!["verify allegation", "confirm claim", "allegation verified"],
    );
    m.insert(
        "allegation.contradict",
        vec!["contradict allegation", "allegation false", "not accurate"],
    );
    m.insert(
        "allegation.mark-partial",
        vec![
            "partial verification",
            "partially correct",
            "partly verified",
        ],
    );
    m.insert(
        "allegation.list-by-entity",
        vec!["list allegations", "entity allegations", "client claims"],
    );
    m.insert(
        "allegation.list-pending",
        vec![
            "pending allegations",
            "unverified claims",
            "needs verification",
        ],
    );

    // ==========================================================================
    // DISCREPANCY VERBS
    // ==========================================================================
    m.insert(
        "discrepancy.record",
        vec![
            "record discrepancy",
            "data conflict",
            "observation conflict",
            "mismatch",
        ],
    );
    m.insert(
        "discrepancy.resolve",
        vec![
            "resolve discrepancy",
            "fix conflict",
            "discrepancy resolved",
        ],
    );
    m.insert(
        "discrepancy.escalate",
        vec!["escalate discrepancy", "serious conflict"],
    );
    m.insert(
        "discrepancy.list-open",
        vec![
            "list discrepancies",
            "open conflicts",
            "unresolved discrepancies",
        ],
    );

    // ==========================================================================
    // VERIFICATION VERBS
    // ==========================================================================
    m.insert(
        "verify.detect-patterns",
        vec![
            "detect patterns",
            "find red flags",
            "circular ownership",
            "layering detection",
            "nominee detection",
            "opacity detection",
        ],
    );
    m.insert(
        "verify.detect-evasion",
        vec![
            "detect evasion",
            "evasion signals",
            "suspicious behavior",
            "document delays",
        ],
    );
    m.insert(
        "verify.challenge",
        vec![
            "raise challenge",
            "verification challenge",
            "question client",
            "formal challenge",
        ],
    );
    m.insert(
        "verify.respond-to-challenge",
        vec![
            "respond to challenge",
            "challenge response",
            "answer challenge",
        ],
    );
    m.insert(
        "verify.resolve-challenge",
        vec![
            "resolve challenge",
            "challenge resolved",
            "accept challenge",
            "reject challenge",
        ],
    );
    m.insert(
        "verify.list-challenges",
        vec!["list challenges", "open challenges", "all challenges"],
    );
    m.insert(
        "verify.escalate",
        vec![
            "escalate verification",
            "verification escalation",
            "senior review",
            "mlro review",
        ],
    );
    m.insert(
        "verify.resolve-escalation",
        vec![
            "resolve escalation",
            "escalation decision",
            "escalation resolved",
        ],
    );
    m.insert(
        "verify.list-escalations",
        vec!["list escalations", "open escalations", "pending decisions"],
    );
    m.insert(
        "verify.calculate-confidence",
        vec![
            "calculate confidence",
            "confidence score",
            "how confident",
            "data quality",
        ],
    );
    m.insert(
        "verify.get-status",
        vec!["verification status", "verification report", "how verified"],
    );
    m.insert(
        "verify.verify-against-registry",
        vec![
            "verify against registry",
            "registry check",
            "gleif check",
            "companies house check",
        ],
    );
    m.insert(
        "verify.assert",
        vec![
            "assert confidence",
            "minimum confidence",
            "confidence gate",
            "verification gate",
        ],
    );
    m.insert(
        "verify.record-pattern",
        vec!["record pattern", "log pattern", "pattern detected"],
    );
    m.insert(
        "verify.resolve-pattern",
        vec!["resolve pattern", "dismiss pattern", "pattern resolved"],
    );
    m.insert(
        "verify.list-patterns",
        vec!["list patterns", "detected patterns", "suspicious patterns"],
    );

    // ==========================================================================
    // ONBOARDING VERBS
    // ==========================================================================
    m.insert(
        "onboarding.auto-complete",
        vec![
            "auto complete onboarding",
            "generate missing entities",
            "fill gaps automatically",
            "autopilot onboarding",
        ],
    );

    // ==========================================================================
    // HOLDING VERBS
    // ==========================================================================
    m.insert(
        "holding.create",
        vec!["create holding", "new investor holding", "register holding"],
    );
    m.insert("holding.ensure", vec!["ensure holding", "upsert holding"]);
    m.insert(
        "holding.update-units",
        vec!["update holding units", "adjust position", "change units"],
    );
    m.insert("holding.read", vec!["read holding", "holding details"]);
    m.insert(
        "holding.list-by-share-class",
        vec![
            "holdings by share class",
            "share class investors",
            "who holds this class",
        ],
    );
    m.insert(
        "holding.list-by-investor",
        vec![
            "holdings by investor",
            "investor portfolio",
            "what does investor hold",
        ],
    );
    m.insert(
        "holding.close",
        vec!["close holding", "zero holding", "exit position"],
    );

    // ==========================================================================
    // MOVEMENT VERBS
    // ==========================================================================
    m.insert(
        "movement.subscribe",
        vec![
            "subscription",
            "buy units",
            "invest in fund",
            "new subscription",
        ],
    );
    m.insert(
        "movement.redeem",
        vec!["redemption", "sell units", "redeem holding", "cash out"],
    );
    m.insert(
        "movement.transfer-in",
        vec!["transfer in", "incoming transfer", "receive units"],
    );
    m.insert(
        "movement.transfer-out",
        vec!["transfer out", "outgoing transfer", "send units"],
    );
    m.insert(
        "movement.confirm",
        vec!["confirm movement", "movement confirmed", "trade confirmed"],
    );
    m.insert(
        "movement.settle",
        vec!["settle movement", "movement settled", "trade settled"],
    );
    m.insert(
        "movement.cancel",
        vec!["cancel movement", "void transaction", "movement cancelled"],
    );
    m.insert(
        "movement.list-by-holding",
        vec![
            "list movements",
            "transaction history",
            "holding transactions",
        ],
    );
    m.insert(
        "movement.read",
        vec!["read movement", "movement details", "transaction details"],
    );

    // ==========================================================================
    // REFERENCE DATA VERBS
    // ==========================================================================
    m.insert(
        "market.read",
        vec!["read market", "market details", "mic code", "exchange info"],
    );
    m.insert(
        "market.list",
        vec![
            "list markets",
            "available markets",
            "all exchanges",
            "market catalog",
        ],
    );
    m.insert(
        "market.set-holiday-calendar",
        vec![
            "set holiday calendar",
            "market holidays",
            "trading calendar",
            "business days",
        ],
    );
    m.insert(
        "market.calculate-settlement-date",
        vec![
            "calculate settlement date",
            "when does it settle",
            "settlement date calc",
            "value date",
        ],
    );
    m.insert(
        "instrument-class.read",
        vec![
            "read instrument class",
            "instrument class details",
            "asset class info",
        ],
    );
    m.insert(
        "instrument-class.list",
        vec![
            "list instrument classes",
            "asset classes",
            "instrument catalog",
        ],
    );
    m.insert(
        "security-type.read",
        vec!["read security type", "security type details", "smpg code"],
    );
    m.insert(
        "security-type.list",
        vec!["list security types", "security type catalog"],
    );
    m.insert(
        "subcustodian.read",
        vec!["read subcustodian", "subcustodian details", "agent details"],
    );
    m.insert(
        "subcustodian.list",
        vec!["list subcustodians", "network agents", "local agents"],
    );

    m
}

/// Workflow phases - which KYC/onboarding phase each verb belongs to
pub fn get_workflow_phases() -> HashMap<&'static str, &'static str> {
    let mut m = HashMap::new();

    // ==========================================================================
    // CBU SETUP PHASE
    // ==========================================================================
    m.insert("cbu.create", "cbu_setup");
    m.insert("cbu.ensure", "cbu_setup");
    m.insert("cbu.show", "cbu_setup");
    m.insert("cbu.parties", "cbu_setup");
    m.insert("cbu.add-product", "cbu_setup");
    m.insert("cbu.decide", "cbu_decision");

    // ==========================================================================
    // ENTITY SETUP PHASE
    // ==========================================================================
    m.insert("entity.create-limited-company", "entity_setup");
    m.insert("entity.ensure-limited-company", "entity_setup");
    m.insert("entity.create-proper-person", "entity_setup");
    m.insert("entity.ensure-proper-person", "entity_setup");
    m.insert("entity.create-trust-discretionary", "entity_setup");
    m.insert("entity.create-partnership-limited", "entity_setup");
    m.insert("entity.update", "entity_setup");

    // ==========================================================================
    // FUND SETUP PHASE
    // ==========================================================================
    m.insert("fund.create-umbrella", "fund_setup");
    m.insert("fund.ensure-umbrella", "fund_setup");
    m.insert("fund.create-subfund", "fund_setup");
    m.insert("fund.ensure-subfund", "fund_setup");
    m.insert("fund.create-share-class", "fund_setup");
    m.insert("fund.ensure-share-class", "fund_setup");
    m.insert("fund.link-feeder", "fund_setup");
    m.insert("fund.list-subfunds", "fund_setup");
    m.insert("fund.list-share-classes", "fund_setup");

    // ==========================================================================
    // STRUCTURE PHASE (UBO/Control/Ownership)
    // ==========================================================================
    m.insert("ubo.add-ownership", "structure");
    m.insert("ubo.update-ownership", "structure");
    m.insert("ubo.end-ownership", "structure");
    m.insert("ubo.list-owners", "structure");
    m.insert("ubo.list-owned", "structure");
    m.insert("ubo.register-ubo", "structure");
    m.insert("ubo.mark-terminus", "structure");
    m.insert("ubo.calculate", "structure");
    m.insert("ubo.trace-chains", "structure");
    m.insert("control.add", "structure");
    m.insert("control.list-controllers", "structure");

    // ==========================================================================
    // ROLE ASSIGNMENT PHASE
    // ==========================================================================
    m.insert("cbu.assign-role", "role_assignment");
    m.insert("cbu.remove-role", "role_assignment");
    m.insert("cbu.role:assign", "role_assignment");
    m.insert("cbu.role:assign-ownership", "role_assignment");
    m.insert("cbu.role:assign-control", "role_assignment");
    m.insert("cbu.role:assign-trust-role", "role_assignment");
    m.insert("cbu.role:assign-fund-role", "role_assignment");
    m.insert("cbu.role:assign-service-provider", "role_assignment");
    m.insert("cbu.role:assign-signatory", "role_assignment");

    // ==========================================================================
    // KYC CASE PHASE
    // ==========================================================================
    m.insert("kyc-case.create", "kyc_case");
    m.insert("kyc-case.update-status", "kyc_case");
    m.insert("kyc-case.escalate", "kyc_case");
    m.insert("kyc-case.assign", "kyc_case");
    m.insert("kyc-case.set-risk-rating", "kyc_case");
    m.insert("kyc-case.close", "kyc_case");
    m.insert("kyc-case.read", "kyc_case");
    m.insert("kyc-case.list-by-cbu", "kyc_case");
    m.insert("kyc-case.reopen", "kyc_case");
    m.insert("kyc-case.state", "kyc_case");

    // ==========================================================================
    // ENTITY WORKSTREAM PHASE
    // ==========================================================================
    m.insert("entity-workstream.create", "entity_workstream");
    m.insert("entity-workstream.update-status", "entity_workstream");
    m.insert("entity-workstream.block", "entity_workstream");
    m.insert("entity-workstream.complete", "entity_workstream");
    m.insert("entity-workstream.set-enhanced-dd", "entity_workstream");
    m.insert("entity-workstream.set-ubo", "entity_workstream");
    m.insert("entity-workstream.list-by-case", "entity_workstream");
    m.insert("entity-workstream.state", "entity_workstream");

    // ==========================================================================
    // DOCUMENT COLLECTION PHASE
    // ==========================================================================
    m.insert("doc-request.create", "document_collection");
    m.insert("doc-request.mark-requested", "document_collection");
    m.insert("doc-request.receive", "document_collection");
    m.insert("doc-request.verify", "document_collection");
    m.insert("doc-request.reject", "document_collection");
    m.insert("doc-request.waive", "document_collection");
    m.insert("doc-request.list-by-workstream", "document_collection");
    m.insert("document.catalog", "document_collection");
    m.insert("document.extract", "document_collection");

    // ==========================================================================
    // SCREENING PHASE
    // ==========================================================================
    m.insert("case-screening.run", "screening");
    m.insert("case-screening.complete", "screening");
    m.insert("case-screening.review-hit", "screening");
    m.insert("case-screening.list-by-workstream", "screening");
    m.insert("screening.pep", "screening");
    m.insert("screening.sanctions", "screening");
    m.insert("screening.adverse-media", "screening");

    // ==========================================================================
    // RED FLAG / RISK ASSESSMENT PHASE
    // ==========================================================================
    m.insert("red-flag.raise", "risk_assessment");
    m.insert("red-flag.mitigate", "risk_assessment");
    m.insert("red-flag.waive", "risk_assessment");
    m.insert("red-flag.dismiss", "risk_assessment");
    m.insert("red-flag.set-blocking", "risk_assessment");
    m.insert("red-flag.list-by-case", "risk_assessment");
    m.insert("red-flag.list-by-workstream", "risk_assessment");

    // ==========================================================================
    // OBSERVATION / VERIFICATION PHASE
    // ==========================================================================
    m.insert("observation.record", "verification");
    m.insert("observation.record-from-document", "verification");
    m.insert("observation.supersede", "verification");
    m.insert("observation.list-for-entity", "verification");
    m.insert("observation.list-for-attribute", "verification");
    m.insert("observation.get-current", "verification");
    m.insert("observation.reconcile", "verification");
    m.insert("observation.verify-allegations", "verification");
    m.insert("allegation.record", "verification");
    m.insert("allegation.verify", "verification");
    m.insert("allegation.contradict", "verification");
    m.insert("allegation.mark-partial", "verification");
    m.insert("allegation.list-by-entity", "verification");
    m.insert("allegation.list-pending", "verification");
    m.insert("discrepancy.record", "verification");
    m.insert("discrepancy.resolve", "verification");
    m.insert("discrepancy.escalate", "verification");
    m.insert("discrepancy.list-open", "verification");

    // ==========================================================================
    // ADVERSARIAL VERIFICATION PHASE
    // ==========================================================================
    m.insert("verify.detect-patterns", "adversarial_verification");
    m.insert("verify.detect-evasion", "adversarial_verification");
    m.insert("verify.challenge", "adversarial_verification");
    m.insert("verify.respond-to-challenge", "adversarial_verification");
    m.insert("verify.resolve-challenge", "adversarial_verification");
    m.insert("verify.list-challenges", "adversarial_verification");
    m.insert("verify.escalate", "adversarial_verification");
    m.insert("verify.resolve-escalation", "adversarial_verification");
    m.insert("verify.list-escalations", "adversarial_verification");
    m.insert("verify.calculate-confidence", "adversarial_verification");
    m.insert("verify.get-status", "adversarial_verification");
    m.insert("verify.verify-against-registry", "adversarial_verification");
    m.insert("verify.assert", "adversarial_verification");
    m.insert("verify.record-pattern", "adversarial_verification");
    m.insert("verify.resolve-pattern", "adversarial_verification");
    m.insert("verify.list-patterns", "adversarial_verification");

    // ==========================================================================
    // REQUEST MANAGEMENT PHASE
    // ==========================================================================
    m.insert("request.create", "request_management");
    m.insert("request.list", "request_management");
    m.insert("request.overdue", "request_management");
    m.insert("request.fulfill", "request_management");
    m.insert("request.cancel", "request_management");
    m.insert("request.extend", "request_management");
    m.insert("request.remind", "request_management");
    m.insert("request.escalate", "request_management");
    m.insert("request.waive", "request_management");

    // ==========================================================================
    // TRADING SETUP PHASE
    // ==========================================================================
    m.insert("cbu-custody.add-universe", "trading_setup");
    m.insert("cbu-custody.list-universe", "trading_setup");
    m.insert("cbu-custody.remove-universe", "trading_setup");
    m.insert("cbu-custody.create-ssi", "trading_setup");
    m.insert("cbu-custody.ensure-ssi", "trading_setup");
    m.insert("cbu-custody.activate-ssi", "trading_setup");
    m.insert("cbu-custody.suspend-ssi", "trading_setup");
    m.insert("cbu-custody.list-ssis", "trading_setup");
    m.insert("cbu-custody.setup-ssi", "trading_setup");
    m.insert("cbu-custody.lookup-ssi", "trading_setup");
    m.insert("cbu-custody.add-booking-rule", "trading_setup");
    m.insert("cbu-custody.ensure-booking-rule", "trading_setup");
    m.insert("cbu-custody.list-booking-rules", "trading_setup");
    m.insert("cbu-custody.update-rule-priority", "trading_setup");
    m.insert("cbu-custody.deactivate-rule", "trading_setup");
    m.insert("cbu-custody.add-agent-override", "trading_setup");
    m.insert("cbu-custody.list-agent-overrides", "trading_setup");
    m.insert("cbu-custody.remove-agent-override", "trading_setup");
    m.insert("cbu-custody.derive-required-coverage", "trading_setup");
    m.insert("cbu-custody.validate-booking-coverage", "trading_setup");

    // ==========================================================================
    // SETTLEMENT SETUP PHASE
    // ==========================================================================
    m.insert("cbu-custody.define-settlement-chain", "settlement_setup");
    m.insert("cbu-custody.list-settlement-chains", "settlement_setup");
    m.insert("cbu-custody.set-fop-rules", "settlement_setup");
    m.insert("cbu-custody.list-fop-rules", "settlement_setup");
    m.insert("cbu-custody.set-csd-preference", "settlement_setup");
    m.insert("cbu-custody.list-csd-preferences", "settlement_setup");
    m.insert("cbu-custody.set-settlement-cycle", "settlement_setup");
    m.insert(
        "cbu-custody.list-settlement-cycle-overrides",
        "settlement_setup",
    );
    m.insert("entity-settlement.set-identity", "settlement_setup");
    m.insert("entity-settlement.add-ssi", "settlement_setup");
    m.insert("entity-settlement.remove-ssi", "settlement_setup");

    // ==========================================================================
    // INSTRUCTION SETUP PHASE
    // ==========================================================================
    m.insert(
        "instruction-profile.define-message-type",
        "instruction_setup",
    );
    m.insert(
        "instruction-profile.list-message-types",
        "instruction_setup",
    );
    m.insert("instruction-profile.create-template", "instruction_setup");
    m.insert("instruction-profile.read-template", "instruction_setup");
    m.insert("instruction-profile.list-templates", "instruction_setup");
    m.insert("instruction-profile.assign-template", "instruction_setup");
    m.insert("instruction-profile.list-assignments", "instruction_setup");
    m.insert("instruction-profile.remove-assignment", "instruction_setup");
    m.insert(
        "instruction-profile.add-field-override",
        "instruction_setup",
    );
    m.insert(
        "instruction-profile.list-field-overrides",
        "instruction_setup",
    );
    m.insert(
        "instruction-profile.remove-field-override",
        "instruction_setup",
    );
    m.insert("instruction-profile.find-template", "instruction_setup");
    m.insert("instruction-profile.validate-profile", "instruction_setup");
    m.insert(
        "instruction-profile.derive-required-templates",
        "instruction_setup",
    );

    // ==========================================================================
    // GATEWAY SETUP PHASE
    // ==========================================================================
    m.insert("trade-gateway.define-gateway", "gateway_setup");
    m.insert("trade-gateway.read-gateway", "gateway_setup");
    m.insert("trade-gateway.list-gateways", "gateway_setup");
    m.insert("trade-gateway.enable-gateway", "gateway_setup");
    m.insert("trade-gateway.activate-gateway", "gateway_setup");
    m.insert("trade-gateway.suspend-gateway", "gateway_setup");
    m.insert("trade-gateway.list-cbu-gateways", "gateway_setup");
    m.insert("trade-gateway.add-routing-rule", "gateway_setup");
    m.insert("trade-gateway.list-routing-rules", "gateway_setup");
    m.insert("trade-gateway.remove-routing-rule", "gateway_setup");
    m.insert("trade-gateway.set-fallback", "gateway_setup");
    m.insert("trade-gateway.list-fallbacks", "gateway_setup");
    m.insert("trade-gateway.find-gateway", "gateway_setup");
    m.insert("trade-gateway.validate-routing", "gateway_setup");
    m.insert("trade-gateway.derive-required-routes", "gateway_setup");

    // ==========================================================================
    // PRICING SETUP PHASE
    // ==========================================================================
    m.insert("pricing-config.set", "pricing_setup");
    m.insert("pricing-config.list", "pricing_setup");
    m.insert("pricing-config.remove", "pricing_setup");
    m.insert("pricing-config.find-for-instrument", "pricing_setup");
    m.insert("pricing-config.link-resource", "pricing_setup");
    m.insert("pricing-config.set-valuation-schedule", "pricing_setup");
    m.insert("pricing-config.list-valuation-schedules", "pricing_setup");
    m.insert("pricing-config.set-fallback-chain", "pricing_setup");
    m.insert("pricing-config.set-stale-policy", "pricing_setup");
    m.insert("pricing-config.set-nav-threshold", "pricing_setup");
    m.insert("pricing-config.validate-pricing-config", "pricing_setup");

    // ==========================================================================
    // CORPORATE ACTION SETUP PHASE
    // ==========================================================================
    m.insert(
        "corporate-action.define-event-type",
        "corporate_action_setup",
    );
    m.insert(
        "corporate-action.list-event-types",
        "corporate_action_setup",
    );
    m.insert("corporate-action.set-preferences", "corporate_action_setup");
    m.insert(
        "corporate-action.list-preferences",
        "corporate_action_setup",
    );
    m.insert(
        "corporate-action.remove-preference",
        "corporate_action_setup",
    );
    m.insert(
        "corporate-action.set-instruction-window",
        "corporate_action_setup",
    );
    m.insert(
        "corporate-action.list-instruction-windows",
        "corporate_action_setup",
    );
    m.insert("corporate-action.link-ca-ssi", "corporate_action_setup");
    m.insert(
        "corporate-action.list-ca-ssi-mappings",
        "corporate_action_setup",
    );
    m.insert(
        "corporate-action.validate-ca-config",
        "corporate_action_setup",
    );
    m.insert(
        "corporate-action.derive-required-config",
        "corporate_action_setup",
    );

    // ==========================================================================
    // TAX SETUP PHASE
    // ==========================================================================
    m.insert("tax-config.set-withholding-profile", "tax_setup");
    m.insert("tax-config.list-withholding-profiles", "tax_setup");
    m.insert("tax-config.set-reclaim-preferences", "tax_setup");
    m.insert("tax-config.list-reclaim-preferences", "tax_setup");
    m.insert("tax-config.link-tax-documentation", "tax_setup");
    m.insert("tax-config.list-tax-documentation", "tax_setup");
    m.insert("tax-config.set-rate-override", "tax_setup");
    m.insert("tax-config.list-rate-overrides", "tax_setup");
    m.insert("tax-config.validate-tax-config", "tax_setup");
    m.insert("tax-config.find-withholding-rate", "tax_setup");

    // ==========================================================================
    // TRADING PROFILE MANAGEMENT PHASE
    // ==========================================================================
    m.insert("trading-profile.import", "trading_profile_management");
    m.insert("trading-profile.read", "trading_profile_management");
    m.insert("trading-profile.get-active", "trading_profile_management");
    m.insert(
        "trading-profile.list-versions",
        "trading_profile_management",
    );
    m.insert("trading-profile.activate", "trading_profile_management");
    m.insert("trading-profile.materialize", "trading_profile_management");
    m.insert("trading-profile.validate", "trading_profile_management");
    m.insert("trading-profile.diff", "trading_profile_management");
    m.insert("trading-profile.export", "trading_profile_management");
    m.insert(
        "trading-profile.export-full-matrix",
        "trading_profile_management",
    );
    m.insert(
        "trading-profile.validate-matrix-completeness",
        "trading_profile_management",
    );
    m.insert(
        "trading-profile.generate-gap-remediation-plan",
        "trading_profile_management",
    );

    // ==========================================================================
    // LIFECYCLE MANAGEMENT PHASE
    // ==========================================================================
    m.insert("lifecycle.read", "lifecycle_management");
    m.insert("lifecycle.list", "lifecycle_management");
    m.insert("lifecycle.list-by-instrument", "lifecycle_management");
    m.insert("lifecycle.discover", "lifecycle_management");
    m.insert("lifecycle.provision", "lifecycle_management");
    m.insert("lifecycle.activate", "lifecycle_management");
    m.insert("lifecycle.suspend", "lifecycle_management");
    m.insert("lifecycle.analyze-gaps", "lifecycle_management");
    m.insert("lifecycle.check-readiness", "lifecycle_management");
    m.insert("lifecycle.generate-plan", "lifecycle_management");
    m.insert("lifecycle.visualize-coverage", "lifecycle_management");

    // ==========================================================================
    // ISDA SETUP PHASE
    // ==========================================================================
    m.insert("isda.create", "isda_setup");
    m.insert("isda.add-coverage", "isda_setup");
    m.insert("isda.remove-coverage", "isda_setup");
    m.insert("isda.add-csa", "isda_setup");
    m.insert("isda.remove-csa", "isda_setup");
    m.insert("isda.list", "isda_setup");

    // ==========================================================================
    // MATRIX OVERLAY MANAGEMENT PHASE
    // ==========================================================================
    m.insert("matrix-overlay.subscribe", "matrix_overlay_management");
    m.insert("matrix-overlay.unsubscribe", "matrix_overlay_management");
    m.insert("matrix-overlay.add", "matrix_overlay_management");
    m.insert("matrix-overlay.list", "matrix_overlay_management");
    m.insert(
        "matrix-overlay.effective-matrix",
        "matrix_overlay_management",
    );
    m.insert("matrix-overlay.unified-gaps", "matrix_overlay_management");
    m.insert(
        "matrix-overlay.compare-products",
        "matrix_overlay_management",
    );

    // ==========================================================================
    // TEMPORAL QUERIES PHASE
    // ==========================================================================
    m.insert("temporal.ownership-as-of", "temporal_queries");
    m.insert("temporal.ubo-chain-as-of", "temporal_queries");
    m.insert("temporal.cbu-relationships-as-of", "temporal_queries");
    m.insert("temporal.cbu-roles-as-of", "temporal_queries");
    m.insert("temporal.cbu-state-at-approval", "temporal_queries");
    m.insert("temporal.relationship-history", "temporal_queries");
    m.insert("temporal.entity-history", "temporal_queries");
    m.insert("temporal.compare-ownership", "temporal_queries");

    // ==========================================================================
    // TEAM MANAGEMENT PHASE
    // ==========================================================================
    m.insert("team.create", "team_management");
    m.insert("team.read", "team_management");
    m.insert("team.archive", "team_management");
    m.insert("team.add-member", "team_management");
    m.insert("team.remove-member", "team_management");
    m.insert("team.update-member", "team_management");
    m.insert("team.transfer-member", "team_management");
    m.insert("team.add-governance-member", "team_management");
    m.insert("team.verify-governance-access", "team_management");
    m.insert("team.add-cbu-access", "team_management");
    m.insert("team.remove-cbu-access", "team_management");
    m.insert("team.grant-service", "team_management");
    m.insert("team.revoke-service", "team_management");
    m.insert("team.list-members", "team_management");
    m.insert("team.list-cbus", "team_management");

    // ==========================================================================
    // USER MANAGEMENT PHASE
    // ==========================================================================
    m.insert("user.create", "user_management");
    m.insert("user.suspend", "user_management");
    m.insert("user.reactivate", "user_management");
    m.insert("user.offboard", "user_management");
    m.insert("user.list-teams", "user_management");
    m.insert("user.list-cbus", "user_management");
    m.insert("user.check-access", "user_management");

    // ==========================================================================
    // SLA MANAGEMENT PHASE
    // ==========================================================================
    m.insert("sla.list-templates", "sla_management");
    m.insert("sla.read-template", "sla_management");
    m.insert("sla.commit", "sla_management");
    m.insert("sla.bind-to-profile", "sla_management");
    m.insert("sla.bind-to-service", "sla_management");
    m.insert("sla.bind-to-resource", "sla_management");
    m.insert("sla.bind-to-isda", "sla_management");
    m.insert("sla.bind-to-csa", "sla_management");
    m.insert("sla.list-commitments", "sla_management");
    m.insert("sla.suspend-commitment", "sla_management");
    m.insert("sla.record-measurement", "sla_management");
    m.insert("sla.list-measurements", "sla_management");
    m.insert("sla.report-breach", "sla_management");
    m.insert("sla.update-remediation", "sla_management");
    m.insert("sla.resolve-breach", "sla_management");
    m.insert("sla.escalate-breach", "sla_management");
    m.insert("sla.list-open-breaches", "sla_management");

    // ==========================================================================
    // REGULATORY REGISTRATION PHASE
    // ==========================================================================
    m.insert("regulatory.registration.add", "regulatory_registration");
    m.insert("regulatory.registration.list", "regulatory_registration");
    m.insert("regulatory.registration.verify", "regulatory_registration");
    m.insert("regulatory.registration.remove", "regulatory_registration");
    m.insert("regulatory.status.check", "regulatory_registration");

    // ==========================================================================
    // SEMANTIC STAGE QUERIES PHASE
    // ==========================================================================
    m.insert("semantic.get-state", "semantic_queries");
    m.insert("semantic.list-stages", "semantic_queries");
    m.insert("semantic.stages-for-product", "semantic_queries");
    m.insert("semantic.next-actions", "semantic_queries");
    m.insert("semantic.missing-entities", "semantic_queries");
    m.insert("semantic.prompt-context", "semantic_queries");

    // ==========================================================================
    // CASH SWEEP SETUP PHASE
    // ==========================================================================
    m.insert("cash-sweep.configure", "cash_sweep_setup");
    m.insert("cash-sweep.link-resource", "cash_sweep_setup");
    m.insert("cash-sweep.list", "cash_sweep_setup");
    m.insert("cash-sweep.update-threshold", "cash_sweep_setup");
    m.insert("cash-sweep.update-timing", "cash_sweep_setup");
    m.insert("cash-sweep.change-vehicle", "cash_sweep_setup");
    m.insert("cash-sweep.suspend", "cash_sweep_setup");
    m.insert("cash-sweep.reactivate", "cash_sweep_setup");
    m.insert("cash-sweep.remove", "cash_sweep_setup");

    // ==========================================================================
    // INVESTMENT MANAGER SETUP PHASE
    // ==========================================================================
    m.insert("investment-manager.assign", "im_setup");
    m.insert("investment-manager.set-scope", "im_setup");
    m.insert("investment-manager.link-connectivity", "im_setup");
    m.insert("investment-manager.list", "im_setup");
    m.insert("investment-manager.suspend", "im_setup");
    m.insert("investment-manager.terminate", "im_setup");
    m.insert("investment-manager.find-for-trade", "im_setup");

    // ==========================================================================
    // FUND INVESTOR MANAGEMENT PHASE
    // ==========================================================================
    m.insert("fund-investor.create", "fund_investor_management");
    m.insert("fund-investor.list", "fund_investor_management");
    m.insert(
        "fund-investor.update-kyc-status",
        "fund_investor_management",
    );
    m.insert("fund-investor.get", "fund_investor_management");

    // ==========================================================================
    // DELEGATION MANAGEMENT PHASE
    // ==========================================================================
    m.insert("delegation.add", "delegation_management");
    m.insert("delegation.end", "delegation_management");
    m.insert("delegation.list-delegates", "delegation_management");
    m.insert(
        "delegation.list-delegations-received",
        "delegation_management",
    );

    // ==========================================================================
    // DELIVERY TRACKING PHASE
    // ==========================================================================
    m.insert("delivery.record", "delivery_tracking");
    m.insert("delivery.complete", "delivery_tracking");
    m.insert("delivery.fail", "delivery_tracking");

    // ==========================================================================
    // SERVICE RESOURCE MANAGEMENT PHASE
    // ==========================================================================
    m.insert("service-resource.read", "resource_management");
    m.insert("service-resource.list", "resource_management");
    m.insert("service-resource.list-by-service", "resource_management");
    m.insert("service-resource.list-attributes", "resource_management");
    m.insert("service-resource.provision", "resource_management");
    m.insert("service-resource.set-attr", "resource_management");
    m.insert("service-resource.activate", "resource_management");
    m.insert("service-resource.suspend", "resource_management");
    m.insert("service-resource.decommission", "resource_management");
    m.insert("service-resource.validate-attrs", "resource_management");

    // ==========================================================================
    // CLIENT PORTAL PHASE
    // ==========================================================================
    m.insert("client.get-status", "client_portal");
    m.insert("client.get-outstanding", "client_portal");
    m.insert("client.get-request-detail", "client_portal");
    m.insert("client.get-entity-info", "client_portal");
    m.insert("client.submit-document", "client_portal");
    m.insert("client.provide-info", "client_portal");
    m.insert("client.add-note", "client_portal");
    m.insert("client.request-clarification", "client_portal");
    m.insert("client.start-collection", "client_portal");
    m.insert("client.collection-response", "client_portal");
    m.insert("client.collection-confirm", "client_portal");
    m.insert("client.escalate", "client_portal");

    // ==========================================================================
    // BATCH EXECUTION PHASE
    // ==========================================================================
    m.insert("batch.pause", "batch_execution");
    m.insert("batch.resume", "batch_execution");
    m.insert("batch.continue", "batch_execution");
    m.insert("batch.skip", "batch_execution");
    m.insert("batch.abort", "batch_execution");
    m.insert("batch.status", "batch_execution");
    m.insert("batch.add-products", "batch_execution");

    // ==========================================================================
    // KYC AGREEMENT PHASE
    // ==========================================================================
    m.insert("kyc-agreement.create", "kyc_agreement");
    m.insert("kyc-agreement.read", "kyc_agreement");
    m.insert("kyc-agreement.list", "kyc_agreement");
    m.insert("kyc-agreement.update-status", "kyc_agreement");

    // ==========================================================================
    // KYC SCOPE PHASE
    // ==========================================================================
    m.insert("kyc.preview-scope", "kyc_scope");
    m.insert("kyc.recommend", "kyc_scope");
    m.insert("kyc.sponsor-decision", "kyc_scope");

    // ==========================================================================
    // CASE EVENT AUDIT PHASE
    // ==========================================================================
    m.insert("case-event.log", "case_audit");
    m.insert("case-event.list-by-case", "case_audit");

    // ==========================================================================
    // GRAPH NAVIGATION PHASE
    // ==========================================================================
    m.insert("graph.view", "graph_navigation");
    m.insert("graph.focus", "graph_navigation");
    m.insert("graph.ancestors", "graph_navigation");
    m.insert("graph.descendants", "graph_navigation");
    m.insert("graph.path", "graph_navigation");
    m.insert("graph.filter", "graph_navigation");
    m.insert("graph.group-by", "graph_navigation");

    // ==========================================================================
    // SERVICE/PRODUCT CATALOG PHASE
    // ==========================================================================
    m.insert("service.list", "product_catalog");
    m.insert("product.list", "product_catalog");
    m.insert("product.subscribe", "product_catalog");
    m.insert("product.unsubscribe", "product_catalog");

    // ==========================================================================
    // REGISTRY (HOLDINGS/MOVEMENTS) PHASE
    // ==========================================================================
    m.insert("holding.create", "registry_management");
    m.insert("holding.ensure", "registry_management");
    m.insert("holding.update-units", "registry_management");
    m.insert("holding.read", "registry_management");
    m.insert("holding.list-by-share-class", "registry_management");
    m.insert("holding.list-by-investor", "registry_management");
    m.insert("holding.close", "registry_management");
    m.insert("movement.subscribe", "registry_management");
    m.insert("movement.redeem", "registry_management");
    m.insert("movement.transfer-in", "registry_management");
    m.insert("movement.transfer-out", "registry_management");
    m.insert("movement.confirm", "registry_management");
    m.insert("movement.settle", "registry_management");
    m.insert("movement.cancel", "registry_management");
    m.insert("movement.list-by-holding", "registry_management");
    m.insert("movement.read", "registry_management");

    // ==========================================================================
    // REFERENCE DATA PHASE
    // ==========================================================================
    m.insert("market.read", "reference_data");
    m.insert("market.list", "reference_data");
    m.insert("market.set-holiday-calendar", "reference_data");
    m.insert("market.calculate-settlement-date", "reference_data");
    m.insert("instrument-class.read", "reference_data");
    m.insert("instrument-class.list", "reference_data");
    m.insert("security-type.read", "reference_data");
    m.insert("security-type.list", "reference_data");
    m.insert("subcustodian.read", "reference_data");
    m.insert("subcustodian.list", "reference_data");

    // ==========================================================================
    // ONBOARDING AUTOMATION PHASE
    // ==========================================================================
    m.insert("onboarding.auto-complete", "onboarding_automation");

    m
}

/// Graph contexts - which graph UI view each verb is relevant to
pub fn get_graph_contexts() -> HashMap<&'static str, Vec<&'static str>> {
    let mut m = HashMap::new();

    // ==========================================================================
    // LAYER: OWNERSHIP STRUCTURE
    // ==========================================================================
    m.insert(
        "layer_ownership",
        vec![
            "ubo.add-ownership",
            "ubo.update-ownership",
            "ubo.end-ownership",
            "ubo.list-owners",
            "ubo.list-owned",
            "ubo.register-ubo",
            "ubo.mark-terminus",
            "ubo.calculate",
            "ubo.trace-chains",
            "control.add",
            "control.list-controllers",
            "graph.ancestors",
            "graph.descendants",
            "graph.path",
            "temporal.ownership-as-of",
            "temporal.ubo-chain-as-of",
            "temporal.compare-ownership",
        ],
    );

    // ==========================================================================
    // LAYER: CBU OVERVIEW
    // ==========================================================================
    m.insert(
        "layer_cbu",
        vec![
            "cbu.create",
            "cbu.ensure",
            "cbu.show",
            "cbu.parties",
            "cbu.add-product",
            "cbu.decide",
            "cbu.assign-role",
            "cbu.remove-role",
            "temporal.cbu-relationships-as-of",
            "temporal.cbu-roles-as-of",
            "temporal.cbu-state-at-approval",
        ],
    );

    // ==========================================================================
    // LAYER: ENTITY DETAILS
    // ==========================================================================
    m.insert(
        "layer_entity",
        vec![
            "entity.create-limited-company",
            "entity.ensure-limited-company",
            "entity.create-proper-person",
            "entity.ensure-proper-person",
            "entity.create-trust-discretionary",
            "entity.create-partnership-limited",
            "entity.update",
            "temporal.entity-history",
            "regulatory.registration.add",
            "regulatory.registration.list",
            "regulatory.status.check",
        ],
    );

    // ==========================================================================
    // LAYER: FUND STRUCTURE
    // ==========================================================================
    m.insert(
        "layer_fund",
        vec![
            "fund.create-umbrella",
            "fund.ensure-umbrella",
            "fund.create-subfund",
            "fund.ensure-subfund",
            "fund.create-share-class",
            "fund.ensure-share-class",
            "fund.link-feeder",
            "fund.list-subfunds",
            "fund.list-share-classes",
        ],
    );

    // ==========================================================================
    // LAYER: ROLES
    // ==========================================================================
    m.insert(
        "layer_roles",
        vec![
            "cbu.role:assign",
            "cbu.role:assign-ownership",
            "cbu.role:assign-control",
            "cbu.role:assign-trust-role",
            "cbu.role:assign-fund-role",
            "cbu.role:assign-service-provider",
            "cbu.role:assign-signatory",
            "delegation.add",
            "delegation.end",
            "delegation.list-delegates",
            "delegation.list-delegations-received",
        ],
    );

    // ==========================================================================
    // LAYER: KYC CASE
    // ==========================================================================
    m.insert(
        "layer_kyc_case",
        vec![
            "kyc-case.create",
            "kyc-case.update-status",
            "kyc-case.escalate",
            "kyc-case.assign",
            "kyc-case.set-risk-rating",
            "kyc-case.close",
            "kyc-case.read",
            "kyc-case.list-by-cbu",
            "kyc-case.reopen",
            "kyc-case.state",
            "entity-workstream.create",
            "entity-workstream.update-status",
            "entity-workstream.block",
            "entity-workstream.complete",
            "entity-workstream.set-enhanced-dd",
            "entity-workstream.set-ubo",
            "entity-workstream.list-by-case",
            "entity-workstream.state",
            "red-flag.raise",
            "red-flag.mitigate",
            "red-flag.waive",
            "red-flag.dismiss",
            "red-flag.set-blocking",
            "red-flag.list-by-case",
            "case-event.log",
            "case-event.list-by-case",
        ],
    );

    // ==========================================================================
    // LAYER: DOCUMENT COLLECTION
    // ==========================================================================
    m.insert(
        "layer_documents",
        vec![
            "doc-request.create",
            "doc-request.mark-requested",
            "doc-request.receive",
            "doc-request.verify",
            "doc-request.reject",
            "doc-request.waive",
            "doc-request.list-by-workstream",
            "document.catalog",
            "document.extract",
            "request.create",
            "request.list",
            "request.overdue",
            "request.fulfill",
            "request.cancel",
            "request.extend",
            "request.remind",
            "request.waive",
        ],
    );

    // ==========================================================================
    // LAYER: SCREENING
    // ==========================================================================
    m.insert(
        "layer_screening",
        vec![
            "case-screening.run",
            "case-screening.complete",
            "case-screening.review-hit",
            "case-screening.list-by-workstream",
            "screening.pep",
            "screening.sanctions",
            "screening.adverse-media",
        ],
    );

    // ==========================================================================
    // LAYER: VERIFICATION / OBSERVATIONS
    // ==========================================================================
    m.insert(
        "layer_verification",
        vec![
            "observation.record",
            "observation.record-from-document",
            "observation.supersede",
            "observation.list-for-entity",
            "observation.list-for-attribute",
            "observation.get-current",
            "observation.reconcile",
            "observation.verify-allegations",
            "allegation.record",
            "allegation.verify",
            "allegation.contradict",
            "allegation.mark-partial",
            "allegation.list-by-entity",
            "allegation.list-pending",
            "discrepancy.record",
            "discrepancy.resolve",
            "discrepancy.escalate",
            "discrepancy.list-open",
            "verify.detect-patterns",
            "verify.detect-evasion",
            "verify.challenge",
            "verify.respond-to-challenge",
            "verify.resolve-challenge",
            "verify.list-challenges",
            "verify.escalate",
            "verify.resolve-escalation",
            "verify.list-escalations",
            "verify.calculate-confidence",
            "verify.get-status",
            "verify.verify-against-registry",
            "verify.assert",
            "verify.record-pattern",
            "verify.resolve-pattern",
            "verify.list-patterns",
        ],
    );

    // ==========================================================================
    // LAYER: TRADING MATRIX
    // ==========================================================================
    m.insert(
        "layer_trading_matrix",
        vec![
            "trading-profile.import",
            "trading-profile.read",
            "trading-profile.get-active",
            "trading-profile.list-versions",
            "trading-profile.activate",
            "trading-profile.materialize",
            "trading-profile.validate",
            "trading-profile.diff",
            "trading-profile.export",
            "trading-profile.export-full-matrix",
            "trading-profile.validate-matrix-completeness",
            "trading-profile.generate-gap-remediation-plan",
            "matrix-overlay.subscribe",
            "matrix-overlay.unsubscribe",
            "matrix-overlay.add",
            "matrix-overlay.list",
            "matrix-overlay.effective-matrix",
            "matrix-overlay.unified-gaps",
            "matrix-overlay.compare-products",
        ],
    );

    // ==========================================================================
    // LAYER: CUSTODY / SETTLEMENT
    // ==========================================================================
    m.insert(
        "layer_custody_settlement",
        vec![
            "cbu-custody.add-universe",
            "cbu-custody.list-universe",
            "cbu-custody.remove-universe",
            "cbu-custody.create-ssi",
            "cbu-custody.ensure-ssi",
            "cbu-custody.activate-ssi",
            "cbu-custody.suspend-ssi",
            "cbu-custody.list-ssis",
            "cbu-custody.setup-ssi",
            "cbu-custody.lookup-ssi",
            "cbu-custody.add-booking-rule",
            "cbu-custody.ensure-booking-rule",
            "cbu-custody.list-booking-rules",
            "cbu-custody.update-rule-priority",
            "cbu-custody.deactivate-rule",
            "cbu-custody.add-agent-override",
            "cbu-custody.list-agent-overrides",
            "cbu-custody.remove-agent-override",
            "cbu-custody.derive-required-coverage",
            "cbu-custody.validate-booking-coverage",
            "cbu-custody.define-settlement-chain",
            "cbu-custody.list-settlement-chains",
            "cbu-custody.set-fop-rules",
            "cbu-custody.list-fop-rules",
            "cbu-custody.set-csd-preference",
            "cbu-custody.list-csd-preferences",
            "cbu-custody.set-settlement-cycle",
            "cbu-custody.list-settlement-cycle-overrides",
            "entity-settlement.set-identity",
            "entity-settlement.add-ssi",
            "entity-settlement.remove-ssi",
        ],
    );

    // ==========================================================================
    // LAYER: INSTRUCTION PROFILE
    // ==========================================================================
    m.insert(
        "layer_instruction_profile",
        vec![
            "instruction-profile.define-message-type",
            "instruction-profile.list-message-types",
            "instruction-profile.create-template",
            "instruction-profile.read-template",
            "instruction-profile.list-templates",
            "instruction-profile.assign-template",
            "instruction-profile.list-assignments",
            "instruction-profile.remove-assignment",
            "instruction-profile.add-field-override",
            "instruction-profile.list-field-overrides",
            "instruction-profile.remove-field-override",
            "instruction-profile.find-template",
            "instruction-profile.validate-profile",
            "instruction-profile.derive-required-templates",
        ],
    );

    // ==========================================================================
    // LAYER: GATEWAY ROUTING
    // ==========================================================================
    m.insert(
        "layer_gateway_routing",
        vec![
            "trade-gateway.define-gateway",
            "trade-gateway.read-gateway",
            "trade-gateway.list-gateways",
            "trade-gateway.enable-gateway",
            "trade-gateway.activate-gateway",
            "trade-gateway.suspend-gateway",
            "trade-gateway.list-cbu-gateways",
            "trade-gateway.add-routing-rule",
            "trade-gateway.list-routing-rules",
            "trade-gateway.remove-routing-rule",
            "trade-gateway.set-fallback",
            "trade-gateway.list-fallbacks",
            "trade-gateway.find-gateway",
            "trade-gateway.validate-routing",
            "trade-gateway.derive-required-routes",
        ],
    );

    // ==========================================================================
    // LAYER: PRICING
    // ==========================================================================
    m.insert(
        "layer_pricing",
        vec![
            "pricing-config.set",
            "pricing-config.list",
            "pricing-config.remove",
            "pricing-config.find-for-instrument",
            "pricing-config.link-resource",
            "pricing-config.set-valuation-schedule",
            "pricing-config.list-valuation-schedules",
            "pricing-config.set-fallback-chain",
            "pricing-config.set-stale-policy",
            "pricing-config.set-nav-threshold",
            "pricing-config.validate-pricing-config",
        ],
    );

    // ==========================================================================
    // LAYER: CORPORATE ACTIONS
    // ==========================================================================
    m.insert(
        "layer_corporate_actions",
        vec![
            "corporate-action.define-event-type",
            "corporate-action.list-event-types",
            "corporate-action.set-preferences",
            "corporate-action.list-preferences",
            "corporate-action.remove-preference",
            "corporate-action.set-instruction-window",
            "corporate-action.list-instruction-windows",
            "corporate-action.link-ca-ssi",
            "corporate-action.list-ca-ssi-mappings",
            "corporate-action.validate-ca-config",
            "corporate-action.derive-required-config",
        ],
    );

    // ==========================================================================
    // LAYER: TAX
    // ==========================================================================
    m.insert(
        "layer_tax",
        vec![
            "tax-config.set-withholding-profile",
            "tax-config.list-withholding-profiles",
            "tax-config.set-reclaim-preferences",
            "tax-config.list-reclaim-preferences",
            "tax-config.link-tax-documentation",
            "tax-config.list-tax-documentation",
            "tax-config.set-rate-override",
            "tax-config.list-rate-overrides",
            "tax-config.validate-tax-config",
            "tax-config.find-withholding-rate",
        ],
    );

    // ==========================================================================
    // LAYER: ISDA
    // ==========================================================================
    m.insert(
        "layer_isda",
        vec![
            "isda.create",
            "isda.add-coverage",
            "isda.remove-coverage",
            "isda.add-csa",
            "isda.remove-csa",
            "isda.list",
        ],
    );

    // ==========================================================================
    // LAYER: LIFECYCLE
    // ==========================================================================
    m.insert(
        "layer_lifecycle",
        vec![
            "lifecycle.read",
            "lifecycle.list",
            "lifecycle.list-by-instrument",
            "lifecycle.discover",
            "lifecycle.provision",
            "lifecycle.activate",
            "lifecycle.suspend",
            "lifecycle.analyze-gaps",
            "lifecycle.check-readiness",
            "lifecycle.generate-plan",
            "lifecycle.visualize-coverage",
        ],
    );

    // ==========================================================================
    // LAYER: TEAM / ACCESS MANAGEMENT
    // ==========================================================================
    m.insert(
        "layer_team",
        vec![
            "team.create",
            "team.read",
            "team.archive",
            "team.add-member",
            "team.remove-member",
            "team.update-member",
            "team.transfer-member",
            "team.add-governance-member",
            "team.verify-governance-access",
            "team.add-cbu-access",
            "team.remove-cbu-access",
            "team.grant-service",
            "team.revoke-service",
            "team.list-members",
            "team.list-cbus",
            "user.create",
            "user.suspend",
            "user.reactivate",
            "user.offboard",
            "user.list-teams",
            "user.list-cbus",
            "user.check-access",
        ],
    );

    // ==========================================================================
    // LAYER: SLA
    // ==========================================================================
    m.insert(
        "layer_sla",
        vec![
            "sla.list-templates",
            "sla.read-template",
            "sla.commit",
            "sla.bind-to-profile",
            "sla.bind-to-service",
            "sla.bind-to-resource",
            "sla.bind-to-isda",
            "sla.bind-to-csa",
            "sla.list-commitments",
            "sla.suspend-commitment",
            "sla.record-measurement",
            "sla.list-measurements",
            "sla.report-breach",
            "sla.update-remediation",
            "sla.resolve-breach",
            "sla.escalate-breach",
            "sla.list-open-breaches",
        ],
    );

    // ==========================================================================
    // LAYER: CASH MANAGEMENT
    // ==========================================================================
    m.insert(
        "layer_cash_management",
        vec![
            "cash-sweep.configure",
            "cash-sweep.link-resource",
            "cash-sweep.list",
            "cash-sweep.update-threshold",
            "cash-sweep.update-timing",
            "cash-sweep.change-vehicle",
            "cash-sweep.suspend",
            "cash-sweep.reactivate",
            "cash-sweep.remove",
        ],
    );

    // ==========================================================================
    // LAYER: INVESTMENT MANAGER
    // ==========================================================================
    m.insert(
        "layer_investment_manager",
        vec![
            "investment-manager.assign",
            "investment-manager.set-scope",
            "investment-manager.link-connectivity",
            "investment-manager.list",
            "investment-manager.suspend",
            "investment-manager.terminate",
            "investment-manager.find-for-trade",
        ],
    );

    // ==========================================================================
    // LAYER: REGISTRY (HOLDINGS / MOVEMENTS)
    // ==========================================================================
    m.insert(
        "layer_registry",
        vec![
            "holding.create",
            "holding.ensure",
            "holding.update-units",
            "holding.read",
            "holding.list-by-share-class",
            "holding.list-by-investor",
            "holding.close",
            "movement.subscribe",
            "movement.redeem",
            "movement.transfer-in",
            "movement.transfer-out",
            "movement.confirm",
            "movement.settle",
            "movement.cancel",
            "movement.list-by-holding",
            "movement.read",
            "fund-investor.create",
            "fund-investor.list",
            "fund-investor.update-kyc-status",
            "fund-investor.get",
        ],
    );

    // ==========================================================================
    // LAYER: CLIENT PORTAL
    // ==========================================================================
    m.insert(
        "layer_client_portal",
        vec![
            "client.get-status",
            "client.get-outstanding",
            "client.get-request-detail",
            "client.get-entity-info",
            "client.submit-document",
            "client.provide-info",
            "client.add-note",
            "client.request-clarification",
            "client.start-collection",
            "client.collection-response",
            "client.collection-confirm",
            "client.escalate",
        ],
    );

    // ==========================================================================
    // LAYER: SERVICE RESOURCES
    // ==========================================================================
    m.insert(
        "layer_service_resources",
        vec![
            "service-resource.read",
            "service-resource.list",
            "service-resource.list-by-service",
            "service-resource.list-attributes",
            "service-resource.provision",
            "service-resource.set-attr",
            "service-resource.activate",
            "service-resource.suspend",
            "service-resource.decommission",
            "service-resource.validate-attrs",
            "service.list",
            "product.list",
            "product.subscribe",
            "product.unsubscribe",
        ],
    );

    // ==========================================================================
    // LAYER: REFERENCE DATA
    // ==========================================================================
    m.insert(
        "layer_reference_data",
        vec![
            "market.read",
            "market.list",
            "market.set-holiday-calendar",
            "market.calculate-settlement-date",
            "instrument-class.read",
            "instrument-class.list",
            "security-type.read",
            "security-type.list",
            "subcustodian.read",
            "subcustodian.list",
        ],
    );

    m
}


/// Typical next verbs - what verb typically follows another in workflows
pub fn get_typical_next() -> HashMap<&'static str, Vec<&'static str>> {
    let mut m = HashMap::new();

    // ==========================================================================
    // CBU FLOW
    // ==========================================================================
    m.insert(
        "cbu.create",
        vec![
            "entity.create-limited-company",
            "entity.create-proper-person",
            "fund.create-umbrella",
            "cbu.assign-role",
            "cbu.add-product",
            "cbu-custody.add-universe",
        ],
    );
    m.insert(
        "cbu.ensure",
        vec![
            "entity.ensure-limited-company",
            "cbu.assign-role",
            "cbu.add-product",
        ],
    );
    m.insert(
        "cbu.add-product",
        vec![
            "kyc-case.create",
            "cbu-custody.add-universe",
            "service-resource.provision",
        ],
    );

    // ==========================================================================
    // ENTITY FLOW
    // ==========================================================================
    m.insert(
        "entity.create-limited-company",
        vec![
            "ubo.add-ownership",
            "control.add",
            "cbu.assign-role",
            "entity-settlement.set-identity",
            "regulatory.registration.add",
        ],
    );
    m.insert(
        "entity.create-proper-person",
        vec![
            "ubo.add-ownership",
            "cbu.role:assign-signatory",
            "cbu.role:assign-control",
        ],
    );
    m.insert(
        "entity.create-trust-discretionary",
        vec![
            "cbu.role:assign-trust-role",
            "entity.create-proper-person",
        ],
    );
    m.insert(
        "entity.create-partnership-limited",
        vec![
            "ubo.add-ownership",
            "cbu.assign-role",
        ],
    );

    // ==========================================================================
    // FUND FLOW
    // ==========================================================================
    m.insert(
        "fund.create-umbrella",
        vec![
            "fund.create-subfund",
            "cbu.role:assign-fund-role",
        ],
    );
    m.insert(
        "fund.create-subfund",
        vec![
            "fund.create-share-class",
            "cbu-custody.add-universe",
            "pricing-config.set",
        ],
    );
    m.insert(
        "fund.create-share-class",
        vec![
            "cbu-custody.add-universe",
            "pricing-config.set",
            "holding.create",
        ],
    );

    // ==========================================================================
    // OWNERSHIP FLOW
    // ==========================================================================
    m.insert(
        "ubo.add-ownership",
        vec![
            "ubo.add-ownership",
            "ubo.calculate",
            "ubo.register-ubo",
            "entity.create-limited-company",
            "entity.create-proper-person",
        ],
    );
    m.insert(
        "ubo.calculate",
        vec![
            "ubo.register-ubo",
            "ubo.mark-terminus",
            "entity-workstream.create",
        ],
    );
    m.insert(
        "ubo.register-ubo",
        vec![
            "entity-workstream.create",
            "doc-request.create",
        ],
    );

    // ==========================================================================
    // CONTROL FLOW
    // ==========================================================================
    m.insert(
        "control.add",
        vec![
            "entity.create-proper-person",
            "cbu.role:assign-control",
        ],
    );

    // ==========================================================================
    // ROLE FLOW
    // ==========================================================================
    m.insert(
        "cbu.assign-role",
        vec![
            "entity-workstream.create",
            "cbu.assign-role",
            "delegation.add",
        ],
    );
    m.insert(
        "cbu.role:assign-fund-role",
        vec![
            "delegation.add",
            "investment-manager.assign",
        ],
    );
    m.insert(
        "cbu.role:assign-signatory",
        vec![
            "doc-request.create",
            "entity-workstream.create",
        ],
    );

    // ==========================================================================
    // KYC CASE FLOW
    // ==========================================================================
    m.insert(
        "kyc-case.create",
        vec![
            "entity-workstream.create",
            "kyc-case.assign",
            "kyc.preview-scope",
        ],
    );
    m.insert(
        "kyc-case.assign",
        vec![
            "entity-workstream.create",
            "kyc-case.update-status",
        ],
    );
    m.insert(
        "kyc-case.update-status",
        vec![
            "entity-workstream.update-status",
            "kyc-case.set-risk-rating",
            "kyc-case.close",
        ],
    );
    m.insert(
        "kyc-case.set-risk-rating",
        vec![
            "kyc-case.close",
            "kyc-case.escalate",
        ],
    );
    m.insert(
        "kyc-case.close",
        vec![
            "cbu.decide",
            "trading-profile.activate",
        ],
    );
    m.insert(
        "kyc-case.escalate",
        vec![
            "verify.escalate",
            "red-flag.raise",
        ],
    );
    m.insert(
        "kyc-case.reopen",
        vec![
            "entity-workstream.create",
            "kyc-case.assign",
        ],
    );

    // ==========================================================================
    // ENTITY WORKSTREAM FLOW
    // ==========================================================================
    m.insert(
        "entity-workstream.create",
        vec![
            "doc-request.create",
            "case-screening.run",
            "entity-workstream.update-status",
        ],
    );
    m.insert(
        "entity-workstream.update-status",
        vec![
            "doc-request.create",
            "case-screening.run",
            "entity-workstream.complete",
            "entity-workstream.set-enhanced-dd",
        ],
    );
    m.insert(
        "entity-workstream.complete",
        vec![
            "entity-workstream.create",
            "kyc-case.update-status",
        ],
    );
    m.insert(
        "entity-workstream.set-enhanced-dd",
        vec![
            "doc-request.create",
            "verify.challenge",
        ],
    );
    m.insert(
        "entity-workstream.block",
        vec![
            "red-flag.raise",
            "verify.escalate",
        ],
    );

    // ==========================================================================
    // DOCUMENT REQUEST FLOW
    // ==========================================================================
    m.insert(
        "doc-request.create",
        vec![
            "doc-request.mark-requested",
            "request.create",
        ],
    );
    m.insert(
        "doc-request.mark-requested",
        vec![
            "doc-request.receive",
            "request.remind",
        ],
    );
    m.insert(
        "doc-request.receive",
        vec![
            "doc-request.verify",
            "document.extract",
            "observation.record-from-document",
        ],
    );
    m.insert(
        "doc-request.verify",
        vec![
            "entity-workstream.update-status",
            "allegation.verify",
        ],
    );
    m.insert(
        "doc-request.reject",
        vec![
            "doc-request.create",
            "request.create",
        ],
    );

    // ==========================================================================
    // SCREENING FLOW
    // ==========================================================================
    m.insert(
        "case-screening.run",
        vec![
            "case-screening.complete",
        ],
    );
    m.insert(
        "case-screening.complete",
        vec![
            "case-screening.review-hit",
            "entity-workstream.update-status",
        ],
    );
    m.insert(
        "case-screening.review-hit",
        vec![
            "red-flag.raise",
            "entity-workstream.update-status",
        ],
    );

    // ==========================================================================
    // RED FLAG FLOW
    // ==========================================================================
    m.insert(
        "red-flag.raise",
        vec![
            "red-flag.mitigate",
            "red-flag.set-blocking",
            "verify.escalate",
        ],
    );
    m.insert(
        "red-flag.mitigate",
        vec![
            "entity-workstream.update-status",
            "kyc-case.update-status",
        ],
    );
    m.insert(
        "red-flag.set-blocking",
        vec![
            "entity-workstream.block",
            "kyc-case.escalate",
        ],
    );

    // ==========================================================================
    // OBSERVATION FLOW
    // ==========================================================================
    m.insert(
        "observation.record",
        vec![
            "observation.reconcile",
            "allegation.verify",
            "verify.calculate-confidence",
        ],
    );
    m.insert(
        "observation.record-from-document",
        vec![
            "observation.reconcile",
            "allegation.verify",
        ],
    );
    m.insert(
        "observation.reconcile",
        vec![
            "discrepancy.record",
            "allegation.verify",
        ],
    );

    // ==========================================================================
    // ALLEGATION FLOW
    // ==========================================================================
    m.insert(
        "allegation.record",
        vec![
            "doc-request.create",
            "allegation.verify",
        ],
    );
    m.insert(
        "allegation.verify",
        vec![
            "entity-workstream.update-status",
        ],
    );
    m.insert(
        "allegation.contradict",
        vec![
            "verify.challenge",
            "red-flag.raise",
        ],
    );

    // ==========================================================================
    // DISCREPANCY FLOW
    // ==========================================================================
    m.insert(
        "discrepancy.record",
        vec![
            "discrepancy.resolve",
            "verify.challenge",
        ],
    );
    m.insert(
        "discrepancy.resolve",
        vec![
            "observation.supersede",
            "entity-workstream.update-status",
        ],
    );
    m.insert(
        "discrepancy.escalate",
        vec![
            "verify.escalate",
        ],
    );

    // ==========================================================================
    // VERIFICATION FLOW
    // ==========================================================================
    m.insert(
        "verify.detect-patterns",
        vec![
            "verify.record-pattern",
            "verify.challenge",
            "red-flag.raise",
        ],
    );
    m.insert(
        "verify.detect-evasion",
        vec![
            "verify.challenge",
            "verify.escalate",
        ],
    );
    m.insert(
        "verify.challenge",
        vec![
            "verify.respond-to-challenge",
        ],
    );
    m.insert(
        "verify.respond-to-challenge",
        vec![
            "verify.resolve-challenge",
        ],
    );
    m.insert(
        "verify.resolve-challenge",
        vec![
            "entity-workstream.update-status",
            "red-flag.raise",
        ],
    );
    m.insert(
        "verify.escalate",
        vec![
            "verify.resolve-escalation",
        ],
    );
    m.insert(
        "verify.resolve-escalation",
        vec![
            "kyc-case.update-status",
            "kyc-case.close",
        ],
    );
    m.insert(
        "verify.calculate-confidence",
        vec![
            "verify.assert",
            "entity-workstream.update-status",
        ],
    );
    m.insert(
        "verify.verify-against-registry",
        vec![
            "observation.record",
            "allegation.verify",
            "allegation.contradict",
        ],
    );
    m.insert(
        "verify.record-pattern",
        vec![
            "verify.challenge",
            "red-flag.raise",
        ],
    );
    m.insert(
        "verify.resolve-pattern",
        vec![
            "entity-workstream.update-status",
        ],
    );

    // ==========================================================================
    // REQUEST FLOW
    // ==========================================================================
    m.insert(
        "request.create",
        vec![
            "request.remind",
        ],
    );
    m.insert(
        "request.fulfill",
        vec![
            "doc-request.receive",
            "entity-workstream.update-status",
        ],
    );
    m.insert(
        "request.escalate",
        vec![
            "red-flag.raise",
        ],
    );

    // ==========================================================================
    // TRADING SETUP FLOW
    // ==========================================================================
    m.insert(
        "cbu-custody.add-universe",
        vec![
            "cbu-custody.create-ssi",
            "cbu-custody.add-booking-rule",
        ],
    );
    m.insert(
        "cbu-custody.create-ssi",
        vec![
            "cbu-custody.activate-ssi",
            "cbu-custody.add-booking-rule",
        ],
    );
    m.insert(
        "cbu-custody.ensure-ssi",
        vec![
            "cbu-custody.activate-ssi",
            "cbu-custody.add-booking-rule",
        ],
    );
    m.insert(
        "cbu-custody.activate-ssi",
        vec![
            "cbu-custody.add-booking-rule",
            "cbu-custody.add-agent-override",
        ],
    );
    m.insert(
        "cbu-custody.add-booking-rule",
        vec![
            "cbu-custody.add-agent-override",
            "cbu-custody.derive-required-coverage",
        ],
    );
    m.insert(
        "cbu-custody.add-agent-override",
        vec![
            "cbu-custody.validate-booking-coverage",
        ],
    );
    m.insert(
        "cbu-custody.derive-required-coverage",
        vec![
            "cbu-custody.create-ssi",
            "cbu-custody.add-booking-rule",
        ],
    );
    m.insert(
        "cbu-custody.validate-booking-coverage",
        vec![
            "trading-profile.validate-matrix-completeness",
        ],
    );

    // ==========================================================================
    // SETTLEMENT FLOW
    // ==========================================================================
    m.insert(
        "cbu-custody.define-settlement-chain",
        vec![
            "cbu-custody.set-fop-rules",
            "cbu-custody.set-csd-preference",
        ],
    );
    m.insert(
        "cbu-custody.set-fop-rules",
        vec![
            "cbu-custody.set-csd-preference",
            "cbu-custody.set-settlement-cycle",
        ],
    );
    m.insert(
        "cbu-custody.set-csd-preference",
        vec![
            "cbu-custody.set-settlement-cycle",
        ],
    );
    m.insert(
        "entity-settlement.set-identity",
        vec![
            "entity-settlement.add-ssi",
        ],
    );

    // ==========================================================================
    // INSTRUCTION PROFILE FLOW
    // ==========================================================================
    m.insert(
        "instruction-profile.define-message-type",
        vec![
            "instruction-profile.create-template",
        ],
    );
    m.insert(
        "instruction-profile.create-template",
        vec![
            "instruction-profile.assign-template",
        ],
    );
    m.insert(
        "instruction-profile.assign-template",
        vec![
            "instruction-profile.add-field-override",
            "instruction-profile.validate-profile",
        ],
    );
    m.insert(
        "instruction-profile.add-field-override",
        vec![
            "instruction-profile.validate-profile",
        ],
    );
    m.insert(
        "instruction-profile.validate-profile",
        vec![
            "instruction-profile.derive-required-templates",
        ],
    );
    m.insert(
        "instruction-profile.derive-required-templates",
        vec![
            "instruction-profile.create-template",
            "instruction-profile.assign-template",
        ],
    );

    // ==========================================================================
    // GATEWAY FLOW
    // ==========================================================================
    m.insert(
        "trade-gateway.define-gateway",
        vec![
            "trade-gateway.enable-gateway",
        ],
    );
    m.insert(
        "trade-gateway.enable-gateway",
        vec![
            "trade-gateway.activate-gateway",
        ],
    );
    m.insert(
        "trade-gateway.activate-gateway",
        vec![
            "trade-gateway.add-routing-rule",
        ],
    );
    m.insert(
        "trade-gateway.add-routing-rule",
        vec![
            "trade-gateway.set-fallback",
            "trade-gateway.validate-routing",
        ],
    );
    m.insert(
        "trade-gateway.set-fallback",
        vec![
            "trade-gateway.validate-routing",
        ],
    );
    m.insert(
        "trade-gateway.validate-routing",
        vec![
            "trade-gateway.derive-required-routes",
        ],
    );
    m.insert(
        "trade-gateway.derive-required-routes",
        vec![
            "trade-gateway.add-routing-rule",
        ],
    );

    // ==========================================================================
    // PRICING FLOW
    // ==========================================================================
    m.insert(
        "pricing-config.set",
        vec![
            "pricing-config.link-resource",
            "pricing-config.set-valuation-schedule",
        ],
    );
    m.insert(
        "pricing-config.link-resource",
        vec![
            "pricing-config.set-valuation-schedule",
        ],
    );
    m.insert(
        "pricing-config.set-valuation-schedule",
        vec![
            "pricing-config.set-fallback-chain",
        ],
    );
    m.insert(
        "pricing-config.set-fallback-chain",
        vec![
            "pricing-config.set-stale-policy",
        ],
    );
    m.insert(
        "pricing-config.set-stale-policy",
        vec![
            "pricing-config.set-nav-threshold",
        ],
    );
    m.insert(
        "pricing-config.set-nav-threshold",
        vec![
            "pricing-config.validate-pricing-config",
        ],
    );

    // ==========================================================================
    // CORPORATE ACTION FLOW
    // ==========================================================================
    m.insert(
        "corporate-action.set-preferences",
        vec![
            "corporate-action.set-instruction-window",
            "corporate-action.link-ca-ssi",
        ],
    );
    m.insert(
        "corporate-action.set-instruction-window",
        vec![
            "corporate-action.link-ca-ssi",
        ],
    );
    m.insert(
        "corporate-action.link-ca-ssi",
        vec![
            "corporate-action.validate-ca-config",
        ],
    );
    m.insert(
        "corporate-action.validate-ca-config",
        vec![
            "corporate-action.derive-required-config",
        ],
    );
    m.insert(
        "corporate-action.derive-required-config",
        vec![
            "corporate-action.set-preferences",
        ],
    );

    // ==========================================================================
    // TAX FLOW
    // ==========================================================================
    m.insert(
        "tax-config.set-withholding-profile",
        vec![
            "tax-config.set-reclaim-preferences",
        ],
    );
    m.insert(
        "tax-config.set-reclaim-preferences",
        vec![
            "tax-config.link-tax-documentation",
        ],
    );
    m.insert(
        "tax-config.link-tax-documentation",
        vec![
            "tax-config.set-rate-override",
            "tax-config.validate-tax-config",
        ],
    );
    m.insert(
        "tax-config.set-rate-override",
        vec![
            "tax-config.validate-tax-config",
        ],
    );
    m.insert(
        "tax-config.validate-tax-config",
        vec![
            "tax-config.find-withholding-rate",
        ],
    );

    // ==========================================================================
    // TRADING PROFILE FLOW
    // ==========================================================================
    m.insert(
        "trading-profile.import",
        vec![
            "trading-profile.validate",
        ],
    );
    m.insert(
        "trading-profile.validate",
        vec![
            "trading-profile.activate",
        ],
    );
    m.insert(
        "trading-profile.activate",
        vec![
            "trading-profile.materialize",
        ],
    );
    m.insert(
        "trading-profile.materialize",
        vec![
            "trading-profile.validate-matrix-completeness",
        ],
    );
    m.insert(
        "trading-profile.validate-matrix-completeness",
        vec![
            "trading-profile.generate-gap-remediation-plan",
            "trading-profile.export-full-matrix",
        ],
    );
    m.insert(
        "trading-profile.generate-gap-remediation-plan",
        vec![
            "cbu-custody.create-ssi",
            "instruction-profile.assign-template",
            "trade-gateway.add-routing-rule",
            "corporate-action.set-preferences",
            "tax-config.set-withholding-profile",
        ],
    );

    // ==========================================================================
    // LIFECYCLE FLOW
    // ==========================================================================
    m.insert(
        "lifecycle.discover",
        vec![
            "lifecycle.provision",
        ],
    );
    m.insert(
        "lifecycle.provision",
        vec![
            "lifecycle.activate",
        ],
    );
    m.insert(
        "lifecycle.activate",
        vec![
            "lifecycle.analyze-gaps",
        ],
    );
    m.insert(
        "lifecycle.analyze-gaps",
        vec![
            "lifecycle.check-readiness",
            "lifecycle.generate-plan",
        ],
    );
    m.insert(
        "lifecycle.check-readiness",
        vec![
            "lifecycle.generate-plan",
        ],
    );
    m.insert(
        "lifecycle.generate-plan",
        vec![
            "lifecycle.provision",
        ],
    );

    // ==========================================================================
    // ISDA FLOW
    // ==========================================================================
    m.insert(
        "isda.create",
        vec![
            "isda.add-coverage",
        ],
    );
    m.insert(
        "isda.add-coverage",
        vec![
            "isda.add-csa",
        ],
    );
    m.insert(
        "isda.add-csa",
        vec![
            "trading-profile.validate-matrix-completeness",
        ],
    );

    // ==========================================================================
    // MATRIX OVERLAY FLOW
    // ==========================================================================
    m.insert(
        "matrix-overlay.subscribe",
        vec![
            "matrix-overlay.add",
        ],
    );
    m.insert(
        "matrix-overlay.add",
        vec![
            "matrix-overlay.effective-matrix",
        ],
    );
    m.insert(
        "matrix-overlay.effective-matrix",
        vec![
            "matrix-overlay.unified-gaps",
        ],
    );
    m.insert(
        "matrix-overlay.unified-gaps",
        vec![
            "matrix-overlay.compare-products",
        ],
    );

    // ==========================================================================
    // TEAM FLOW
    // ==========================================================================
    m.insert(
        "team.create",
        vec![
            "team.add-member",
            "team.add-cbu-access",
        ],
    );
    m.insert(
        "team.add-member",
        vec![
            "team.grant-service",
            "team.add-cbu-access",
        ],
    );
    m.insert(
        "team.add-cbu-access",
        vec![
            "team.grant-service",
        ],
    );

    // ==========================================================================
    // USER FLOW
    // ==========================================================================
    m.insert(
        "user.create",
        vec![
            "team.add-member",
        ],
    );
    m.insert(
        "user.offboard",
        vec![
            "team.remove-member",
        ],
    );

    // ==========================================================================
    // SLA FLOW
    // ==========================================================================
    m.insert(
        "sla.commit",
        vec![
            "sla.bind-to-profile",
            "sla.bind-to-service",
        ],
    );
    m.insert(
        "sla.report-breach",
        vec![
            "sla.update-remediation",
        ],
    );
    m.insert(
        "sla.update-remediation",
        vec![
            "sla.resolve-breach",
            "sla.escalate-breach",
        ],
    );

    // ==========================================================================
    // REGULATORY FLOW
    // ==========================================================================
    m.insert(
        "regulatory.registration.add",
        vec![
            "regulatory.registration.verify",
        ],
    );
    m.insert(
        "regulatory.registration.verify",
        vec![
            "regulatory.status.check",
        ],
    );

    // ==========================================================================
    // CASH SWEEP FLOW
    // ==========================================================================
    m.insert(
        "cash-sweep.configure",
        vec![
            "cash-sweep.link-resource",
        ],
    );

    // ==========================================================================
    // INVESTMENT MANAGER FLOW
    // ==========================================================================
    m.insert(
        "investment-manager.assign",
        vec![
            "investment-manager.set-scope",
        ],
    );
    m.insert(
        "investment-manager.set-scope",
        vec![
            "investment-manager.link-connectivity",
        ],
    );

    // ==========================================================================
    // DELEGATION FLOW
    // ==========================================================================
    m.insert(
        "delegation.add",
        vec![
            "cbu.role:assign-fund-role",
        ],
    );

    // ==========================================================================
    // SERVICE RESOURCE FLOW
    // ==========================================================================
    m.insert(
        "service-resource.provision",
        vec![
            "service-resource.set-attr",
        ],
    );
    m.insert(
        "service-resource.set-attr",
        vec![
            "service-resource.validate-attrs",
        ],
    );
    m.insert(
        "service-resource.validate-attrs",
        vec![
            "service-resource.activate",
        ],
    );

    // ==========================================================================
    // CLIENT PORTAL FLOW
    // ==========================================================================
    m.insert(
        "client.get-outstanding",
        vec![
            "client.get-request-detail",
            "client.submit-document",
            "client.provide-info",
        ],
    );
    m.insert(
        "client.start-collection",
        vec![
            "client.collection-response",
        ],
    );
    m.insert(
        "client.collection-response",
        vec![
            "client.collection-confirm",
        ],
    );
    m.insert(
        "client.collection-confirm",
        vec![
            "client.get-outstanding",
        ],
    );

    // ==========================================================================
    // FUND INVESTOR FLOW
    // ==========================================================================
    m.insert(
        "fund-investor.create",
        vec![
            "holding.create",
            "fund-investor.update-kyc-status",
        ],
    );

    // ==========================================================================
    // HOLDING FLOW
    // ==========================================================================
    m.insert(
        "holding.create",
        vec![
            "movement.subscribe",
        ],
    );
    m.insert(
        "movement.subscribe",
        vec![
            "movement.confirm",
        ],
    );
    m.insert(
        "movement.redeem",
        vec![
            "movement.confirm",
        ],
    );
    m.insert(
        "movement.confirm",
        vec![
            "movement.settle",
        ],
    );
    m.insert(
        "movement.settle",
        vec![
            "holding.update-units",
        ],
    );

    // ==========================================================================
    // ONBOARDING AUTOMATION
    // ==========================================================================
    m.insert(
        "onboarding.auto-complete",
        vec![
            "kyc-case.create",
            "trading-profile.validate-matrix-completeness",
        ],
    );

    // ==========================================================================
    // SEMANTIC FLOW
    // ==========================================================================
    m.insert(
        "semantic.get-state",
        vec![
            "semantic.next-actions",
            "semantic.missing-entities",
        ],
    );
    m.insert(
        "semantic.next-actions",
        vec![
            "semantic.missing-entities",
        ],
    );

    m
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Get all verbs that match a workflow phase
pub fn verbs_for_phase(phase: &str) -> Vec<&'static str> {
    let phases = get_workflow_phases();
    phases
        .iter()
        .filter_map(|(verb, p)| if *p == phase { Some(*verb) } else { None })
        .collect()
}

/// Get all verbs that are relevant in a graph context
pub fn verbs_for_graph_context(context: &str) -> Vec<&'static str> {
    let contexts = get_graph_contexts();
    contexts.get(context).cloned().unwrap_or_default()
}

/// Find verbs that match an intent pattern (substring match)
pub fn find_verbs_by_intent(query: &str) -> Vec<(&'static str, &'static str)> {
    let patterns = get_intent_patterns();
    let query_lower = query.to_lowercase();
    let mut matches = Vec::new();

    for (verb, intents) in patterns.iter() {
        for intent in intents {
            if intent.contains(&query_lower) || query_lower.contains(intent) {
                matches.push((*verb, *intent));
                break;
            }
        }
    }

    matches
}

/// Get suggested next verbs after executing a verb
pub fn suggest_next(verb: &str) -> Vec<&'static str> {
    let typical_next = get_typical_next();
    typical_next.get(verb).cloned().unwrap_or_default()
}

/// Get all workflow phases
pub fn list_workflow_phases() -> Vec<&'static str> {
    let phases = get_workflow_phases();
    let mut unique_phases: Vec<&str> = phases.values().copied().collect();
    unique_phases.sort();
    unique_phases.dedup();
    unique_phases
}

/// Get all graph contexts
pub fn list_graph_contexts() -> Vec<&'static str> {
    let contexts = get_graph_contexts();
    let mut keys: Vec<&str> = contexts.keys().copied().collect();
    keys.sort();
    keys
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_intent_patterns_populated() {
        let patterns = get_intent_patterns();
        assert!(!patterns.is_empty());
        assert!(patterns.contains_key("cbu.create"));
        assert!(patterns.contains_key("kyc-case.create"));
        assert!(patterns.contains_key("trading-profile.validate-matrix-completeness"));
    }

    #[test]
    fn test_workflow_phases_populated() {
        let phases = get_workflow_phases();
        assert!(!phases.is_empty());
        assert_eq!(phases.get("cbu.create"), Some(&"cbu_setup"));
        assert_eq!(phases.get("kyc-case.create"), Some(&"kyc_case"));
    }

    #[test]
    fn test_graph_contexts_populated() {
        let contexts = get_graph_contexts();
        assert!(!contexts.is_empty());
        assert!(contexts.contains_key("layer_ownership"));
        assert!(contexts.contains_key("layer_kyc_case"));
        assert!(contexts.contains_key("layer_trading_matrix"));
    }

    #[test]
    fn test_typical_next_populated() {
        let next = get_typical_next();
        assert!(!next.is_empty());
        assert!(next.contains_key("cbu.create"));
        assert!(next.get("cbu.create").unwrap().contains(&"entity.create-limited-company"));
    }

    #[test]
    fn test_find_verbs_by_intent() {
        let matches = find_verbs_by_intent("create company");
        assert!(!matches.is_empty());
        let verbs: Vec<&str> = matches.iter().map(|(v, _)| *v).collect();
        assert!(verbs.contains(&"entity.create-limited-company"));
    }

    #[test]
    fn test_suggest_next() {
        let suggestions = suggest_next("cbu.create");
        assert!(!suggestions.is_empty());
        assert!(suggestions.contains(&"entity.create-limited-company"));
    }

    #[test]
    fn test_verbs_for_phase() {
        let verbs = verbs_for_phase("kyc_case");
        assert!(!verbs.is_empty());
        assert!(verbs.contains(&"kyc-case.create"));
    }

    #[test]
    fn test_verbs_for_graph_context() {
        let verbs = verbs_for_graph_context("layer_ownership");
        assert!(!verbs.is_empty());
        assert!(verbs.contains(&"ubo.add-ownership"));
    }
}
