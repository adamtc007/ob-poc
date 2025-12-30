//! Verb RAG Metadata
//!
//! Provides semantic metadata for verb discovery:
//! - intent_patterns: Natural language patterns that map to verbs
//! - workflow_phases: KYC lifecycle phases where verb is applicable
//! - graph_contexts: Graph UI contexts where verb is relevant
//! - typical_next: Common follow-up verbs in workflows

use std::collections::HashMap;

/// Intent patterns for natural language -> verb matching
pub fn get_intent_patterns() -> HashMap<&'static str, Vec<&'static str>> {
    let mut m = HashMap::new();

    // === CBU VERBS ===
    m.insert(
        "cbu.create",
        vec![
            "create cbu",
            "new cbu",
            "add client",
            "onboard client",
            "create client business unit",
            "new client",
        ],
    );
    m.insert(
        "cbu.ensure",
        vec!["ensure cbu exists", "upsert cbu", "create or update cbu"],
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
        ],
    );
    m.insert(
        "cbu.remove-role",
        vec![
            "remove role",
            "revoke role",
            "unassign role",
            "take away role",
        ],
    );
    m.insert(
        "cbu.show",
        vec!["show cbu", "display cbu", "view cbu", "cbu details"],
    );
    m.insert(
        "cbu.parties",
        vec![
            "list parties",
            "show parties",
            "who is involved",
            "all entities",
        ],
    );

    // === ENTITY VERBS ===
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
        ],
    );
    m.insert(
        "entity.ensure-limited-company",
        vec![
            "ensure company",
            "upsert company",
            "create or update company",
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
        ],
    );
    m.insert(
        "entity.ensure-proper-person",
        vec!["ensure person", "upsert person", "create or update person"],
    );
    m.insert(
        "entity.create-trust-discretionary",
        vec![
            "create trust",
            "new trust",
            "add trust",
            "discretionary trust",
        ],
    );
    m.insert(
        "entity.create-partnership-limited",
        vec![
            "create partnership",
            "new lp",
            "add limited partnership",
            "create lp",
        ],
    );

    // === FUND VERBS ===
    m.insert(
        "fund.create-umbrella",
        vec![
            "create umbrella",
            "new sicav",
            "create sicav",
            "new icav",
            "create fund umbrella",
            "umbrella fund",
        ],
    );
    m.insert(
        "fund.ensure-umbrella",
        vec!["ensure umbrella", "upsert umbrella", "ensure sicav exists"],
    );
    m.insert(
        "fund.create-subfund",
        vec![
            "create subfund",
            "new subfund",
            "add compartment",
            "create compartment",
            "new sub-fund",
        ],
    );
    m.insert(
        "fund.ensure-subfund",
        vec!["ensure subfund", "upsert subfund", "ensure compartment"],
    );
    m.insert(
        "fund.create-share-class",
        vec![
            "create share class",
            "new share class",
            "add share class",
            "create isin",
            "new isin",
        ],
    );
    m.insert(
        "fund.ensure-share-class",
        vec!["ensure share class", "upsert share class"],
    );
    m.insert(
        "fund.link-feeder",
        vec![
            "link feeder",
            "connect feeder to master",
            "feeder master relationship",
        ],
    );
    m.insert(
        "fund.list-subfunds",
        vec![
            "list subfunds",
            "show compartments",
            "subfunds under umbrella",
        ],
    );
    m.insert(
        "fund.list-share-classes",
        vec!["list share classes", "show share classes", "isins for fund"],
    );

    // === UBO/OWNERSHIP VERBS ===
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
        ],
    );
    m.insert(
        "ubo.update-ownership",
        vec!["update ownership", "change percentage", "modify stake"],
    );
    m.insert(
        "ubo.end-ownership",
        vec!["end ownership", "remove owner", "sold stake", "divested"],
    );
    m.insert(
        "ubo.list-owners",
        vec![
            "list owners",
            "who owns",
            "shareholders",
            "ownership chain up",
        ],
    );
    m.insert(
        "ubo.list-owned",
        vec![
            "list owned",
            "subsidiaries",
            "what do they own",
            "ownership chain down",
        ],
    );
    m.insert(
        "ubo.register-ubo",
        vec!["register ubo", "add beneficial owner", "ubo registration"],
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
        ],
    );
    m.insert(
        "ubo.trace-chains",
        vec![
            "trace chains",
            "trace ownership",
            "follow ownership",
            "ownership path",
        ],
    );

    // === CONTROL VERBS ===
    m.insert(
        "control.add",
        vec![
            "add control",
            "controls",
            "controlling person",
            "significant control",
        ],
    );
    m.insert(
        "control.list-controllers",
        vec!["list controllers", "who controls", "controlling parties"],
    );

    // === ROLE ASSIGNMENT (V2) ===
    m.insert(
        "cbu.role:assign",
        vec!["assign role", "add role to cbu", "entity role"],
    );
    m.insert(
        "cbu.role:assign-ownership",
        vec!["assign ownership role", "shareholder role", "owner role"],
    );
    m.insert(
        "cbu.role:assign-control",
        vec!["assign control role", "director role", "officer role"],
    );
    m.insert(
        "cbu.role:assign-trust-role",
        vec![
            "assign trust role",
            "trustee",
            "settlor",
            "beneficiary",
            "protector",
        ],
    );
    m.insert(
        "cbu.role:assign-fund-role",
        vec![
            "assign fund role",
            "management company",
            "manco",
            "investment manager",
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
        ],
    );
    m.insert(
        "cbu.role:assign-signatory",
        vec![
            "assign signatory",
            "authorized signatory",
            "authorized trader",
            "power of attorney",
        ],
    );

    // === GRAPH/NAVIGATION VERBS ===
    m.insert(
        "graph.view",
        vec!["view graph", "show graph", "visualize", "display structure"],
    );
    m.insert("graph.focus", vec!["focus on", "zoom to", "center on"]);
    m.insert(
        "graph.ancestors",
        vec!["show ancestors", "ownership chain up", "who owns this"],
    );
    m.insert(
        "graph.descendants",
        vec![
            "show descendants",
            "ownership chain down",
            "what do they own",
        ],
    );
    m.insert(
        "graph.path",
        vec!["path between", "connection between", "how are they related"],
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
        ],
    );
    m.insert(
        "graph.group-by",
        vec![
            "group by",
            "cluster by",
            "organize by",
            "group by jurisdiction",
        ],
    );

    // === KYC VERBS ===
    m.insert(
        "kyc-case.create",
        vec![
            "create kyc case",
            "new kyc case",
            "start kyc",
            "open case",
            "initiate kyc",
        ],
    );
    m.insert(
        "kyc-case.update-status",
        vec!["update case status", "change case status", "case progress"],
    );
    m.insert(
        "kyc-case.close",
        vec!["close case", "complete case", "finalize case"],
    );
    m.insert(
        "case-screening.run",
        vec![
            "run screening",
            "screen entity",
            "sanctions check",
            "pep check",
            "aml screening",
        ],
    );

    // === DOCUMENT VERBS ===
    m.insert(
        "document.catalog",
        vec![
            "catalog document",
            "upload document",
            "add document",
            "attach file",
            "store document",
        ],
    );
    m.insert(
        "document.extract",
        vec![
            "extract from document",
            "parse document",
            "read document",
            "document extraction",
        ],
    );
    m.insert(
        "doc-request.create",
        vec![
            "request document",
            "ask for document",
            "doc request",
            "require document",
        ],
    );

    // === SERVICE/PRODUCT VERBS ===
    m.insert(
        "service.list",
        vec!["list services", "available services", "what services"],
    );
    m.insert(
        "product.list",
        vec!["list products", "available products", "what products"],
    );
    m.insert(
        "cbu.add-product",
        vec![
            "add product",
            "assign product",
            "enable product",
            "activate product",
        ],
    );

    // === CUSTODY VERBS ===
    m.insert(
        "cbu-custody.add-universe",
        vec![
            "add to universe",
            "trading universe",
            "can trade",
            "allowed instruments",
        ],
    );
    m.insert(
        "cbu-custody.create-ssi",
        vec![
            "create ssi",
            "standing settlement instruction",
            "settlement instruction",
        ],
    );
    m.insert(
        "cbu-custody.add-booking-rule",
        vec!["add booking rule", "routing rule", "settlement routing"],
    );

    m
}

/// Workflow phases for lifecycle-aware suggestions
pub fn get_workflow_phases() -> HashMap<&'static str, Vec<&'static str>> {
    let mut m = HashMap::new();

    // Entity collection phase
    m.insert(
        "entity_collection",
        vec![
            "cbu.create",
            "cbu.ensure",
            "entity.create-limited-company",
            "entity.ensure-limited-company",
            "entity.create-proper-person",
            "entity.ensure-proper-person",
            "entity.create-trust-discretionary",
            "entity.create-partnership-limited",
            "fund.create-umbrella",
            "fund.ensure-umbrella",
            "fund.create-subfund",
            "fund.ensure-subfund",
            "fund.create-share-class",
            "fund.ensure-share-class",
        ],
    );

    // Structure building phase
    m.insert(
        "structure_building",
        vec![
            "cbu.assign-role",
            "cbu.role:assign",
            "cbu.role:assign-ownership",
            "cbu.role:assign-control",
            "cbu.role:assign-trust-role",
            "cbu.role:assign-fund-role",
            "cbu.role:assign-service-provider",
            "cbu.role:assign-signatory",
            "ubo.add-ownership",
            "control.add",
            "fund.link-feeder",
        ],
    );

    // UBO discovery phase
    m.insert(
        "ubo_discovery",
        vec![
            "ubo.add-ownership",
            "ubo.list-owners",
            "ubo.list-owned",
            "ubo.calculate",
            "ubo.register-ubo",
            "ubo.mark-terminus",
            "ubo.trace-chains",
            "graph.ancestors",
            "graph.descendants",
        ],
    );

    // Document collection phase
    m.insert(
        "document_collection",
        vec![
            "document.catalog",
            "document.extract",
            "doc-request.create",
            "doc-request.receive",
            "doc-request.verify",
        ],
    );

    // Screening phase
    m.insert(
        "screening",
        vec![
            "case-screening.run",
            "case-screening.complete",
            "case-screening.review-hit",
            "kyc-case.create",
        ],
    );

    // Review phase
    m.insert(
        "review",
        vec![
            "kyc-case.update-status",
            "kyc-case.close",
            "cbu.decide",
            "red-flag.raise",
            "red-flag.mitigate",
        ],
    );

    // Graph exploration (always available)
    m.insert(
        "exploration",
        vec![
            "graph.view",
            "graph.focus",
            "graph.ancestors",
            "graph.descendants",
            "graph.path",
            "graph.filter",
            "graph.group-by",
            "cbu.show",
            "cbu.parties",
        ],
    );

    m
}

/// Graph contexts for UI-aware suggestions
pub fn get_graph_contexts() -> HashMap<&'static str, Vec<&'static str>> {
    let mut m = HashMap::new();

    // When cursor is on a CBU node
    m.insert(
        "cursor_on_cbu",
        vec![
            "cbu.show",
            "cbu.parties",
            "cbu.assign-role",
            "cbu.add-product",
            "kyc-case.create",
            "graph.focus",
        ],
    );

    // When cursor is on an entity node
    m.insert(
        "cursor_on_entity",
        vec![
            "entity.update",
            "cbu.assign-role",
            "ubo.add-ownership",
            "ubo.list-owners",
            "ubo.list-owned",
            "control.add",
            "graph.ancestors",
            "graph.descendants",
            "graph.focus",
        ],
    );

    // When cursor is on a fund entity
    m.insert(
        "cursor_on_fund",
        vec![
            "fund.list-subfunds",
            "fund.list-share-classes",
            "fund.create-subfund",
            "fund.create-share-class",
            "cbu.role:assign-fund-role",
            "graph.descendants",
        ],
    );

    // When cursor is on a person
    m.insert(
        "cursor_on_person",
        vec![
            "ubo.register-ubo",
            "ubo.list-owned",
            "cbu.role:assign-control",
            "cbu.role:assign-signatory",
            "case-screening.run",
        ],
    );

    // When viewing UBO layer
    m.insert(
        "layer_ubo",
        vec![
            "ubo.add-ownership",
            "ubo.list-owners",
            "ubo.calculate",
            "ubo.register-ubo",
            "ubo.mark-terminus",
            "ubo.trace-chains",
            "graph.ancestors",
        ],
    );

    // When viewing trading layer
    m.insert(
        "layer_trading",
        vec![
            "cbu.role:assign-signatory",
            "cbu.role:assign-service-provider",
            "fund.list-share-classes",
            "cbu-custody.add-universe",
            "cbu-custody.create-ssi",
        ],
    );

    // When viewing control layer
    m.insert(
        "layer_control",
        vec![
            "control.add",
            "control.list-controllers",
            "cbu.role:assign-control",
        ],
    );

    // When viewing fund structure
    m.insert(
        "layer_fund_structure",
        vec![
            "fund.create-umbrella",
            "fund.create-subfund",
            "fund.create-share-class",
            "fund.link-feeder",
            "fund.list-subfunds",
        ],
    );

    m
}

/// Typical next verbs for workflow suggestions
pub fn get_typical_next() -> HashMap<&'static str, Vec<&'static str>> {
    let mut m = HashMap::new();

    // After creating CBU
    m.insert(
        "cbu.create",
        vec![
            "entity.create-limited-company",
            "cbu.assign-role",
            "fund.create-umbrella",
        ],
    );
    m.insert(
        "cbu.ensure",
        vec![
            "entity.ensure-limited-company",
            "cbu.assign-role",
            "fund.ensure-umbrella",
        ],
    );

    // After creating entity
    m.insert(
        "entity.create-limited-company",
        vec!["cbu.assign-role", "ubo.add-ownership"],
    );
    m.insert(
        "entity.create-proper-person",
        vec!["cbu.role:assign-control", "ubo.register-ubo"],
    );

    // After creating umbrella
    m.insert(
        "fund.create-umbrella",
        vec!["fund.create-subfund", "cbu.role:assign-fund-role"],
    );
    m.insert(
        "fund.ensure-umbrella",
        vec!["fund.ensure-subfund", "cbu.role:assign-fund-role"],
    );

    // After creating subfund
    m.insert("fund.create-subfund", vec!["fund.create-share-class"]);
    m.insert("fund.ensure-subfund", vec!["fund.ensure-share-class"]);

    // After adding ownership
    m.insert(
        "ubo.add-ownership",
        vec![
            "ubo.add-ownership", // chain continues
            "ubo.mark-terminus",
            "ubo.calculate",
        ],
    );

    // After assigning role
    m.insert(
        "cbu.assign-role",
        vec![
            "cbu.assign-role", // more roles
            "ubo.add-ownership",
            "document.catalog",
        ],
    );

    // After UBO calculation
    m.insert(
        "ubo.calculate",
        vec!["ubo.register-ubo", "case-screening.run"],
    );
    m.insert(
        "ubo.trace-chains",
        vec!["ubo.register-ubo", "ubo.mark-terminus"],
    );

    // After screening
    m.insert(
        "case-screening.run",
        vec!["kyc-case.create", "doc-request.create"],
    );

    // After KYC case creation
    m.insert(
        "kyc-case.create",
        vec![
            "entity-workstream.create",
            "case-screening.run",
            "doc-request.create",
        ],
    );

    // After document operations
    m.insert(
        "document.catalog",
        vec!["document.extract", "doc-request.verify"],
    );

    // Graph navigation
    m.insert(
        "graph.view",
        vec!["graph.focus", "graph.filter", "graph.ancestors"],
    );
    m.insert(
        "graph.focus",
        vec!["graph.ancestors", "graph.descendants", "ubo.add-ownership"],
    );

    m
}
