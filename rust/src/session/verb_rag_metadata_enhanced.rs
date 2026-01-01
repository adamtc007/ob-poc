//! Verb RAG Metadata - Enhanced Edition
//!
//! Provides semantic metadata for verb discovery:
//! - intent_patterns: Natural language patterns that map to verbs
//! - workflow_phases: KYC lifecycle phases where verb is applicable
//! - graph_contexts: Graph UI contexts where verb is relevant
//! - typical_next: Common follow-up verbs in workflows
//!
//! COMPREHENSIVE UPDATE: 2024-12-31
//! - Enhanced ALL verb domains with rich intent patterns
//! - Added corner cases, abbreviations, question forms
//! - Added UK/US terminology variations
//! - Added error recovery patterns
//! - Added colloquial expressions domain experts use
//! - Added compound query patterns

use std::collections::HashMap;

/// Intent patterns for natural language -> verb matching
pub fn get_intent_patterns() -> HashMap<&'static str, Vec<&'static str>> {
    let mut m = HashMap::new();

    // ==========================================================================
    // CBU VERBS - Client Business Unit Management
    // ==========================================================================
    m.insert(
        "cbu.create",
        vec![
            // Core patterns
            "create cbu",
            "new cbu",
            "add client",
            "onboard client",
            "create client business unit",
            "new client",
            // Question forms
            "how do i create a cbu",
            "how to add a client",
            "how to onboard a new client",
            "can i create a new cbu",
            // Abbreviations and shorthand
            "new cbu pls",
            "add cbu",
            "cbu setup",
            "setup cbu",
            // Account-based terminology
            "start onboarding",
            "new account",
            "open account",
            "open new account",
            "account opening",
            "new account setup",
            // Customer terminology
            "register client",
            "new customer",
            "client setup",
            "client registration",
            "register new client",
            "add new customer",
            // Regional variations
            "onboard customer",
            "customer onboarding",
            "client take-on",
            "take on client",
            // Process terminology
            "initiate onboarding",
            "begin onboarding",
            "start client onboarding",
            "kick off onboarding",
            "commence onboarding",
            // Relationship terminology
            "new relationship",
            "establish relationship",
            "new client relationship",
            // Error recovery
            "need to create cbu",
            "should create cbu",
            "want to add client",
        ],
    );
    m.insert(
        "cbu.ensure",
        vec![
            // Core patterns
            "ensure cbu exists",
            "upsert cbu",
            "create or update cbu",
            "idempotent cbu",
            "find or create cbu",
            "cbu if not exists",
            // Question forms
            "does cbu exist",
            "check if cbu exists",
            "is there a cbu for",
            "cbu already exists",
            // Safe creation patterns
            "safe create cbu",
            "create cbu if needed",
            "cbu when not exists",
            "ensure client exists",
            // Deduplication patterns
            "avoid duplicate cbu",
            "no duplicate cbu",
            "dedupe cbu",
            "check before create cbu",
            // Merge/match patterns
            "match existing cbu",
            "link to existing cbu",
            "use existing cbu if present",
        ],
    );
    m.insert(
        "cbu.assign-role",
        vec![
            // Core patterns
            "assign role",
            "add role",
            "give role",
            "set role",
            "make them",
            "appoint as",
            "designate as",
            "role assignment",
            "link entity to cbu",
            // Question forms
            "how do i assign a role",
            "how to add someone to cbu",
            "can i assign a role",
            "what roles can i assign",
            // Specific role terminology
            "make director",
            "make shareholder",
            "make signatory",
            "add as owner",
            "add as controller",
            // Relationship patterns
            "connect entity to cbu",
            "associate entity with cbu",
            "attach entity to cbu",
            "add party to cbu",
            "add stakeholder",
            // Corporate governance
            "appoint officer",
            "appoint board member",
            "nominate director",
            // Error recovery
            "change role",
            "update role",
            "fix role assignment",
            "correct role",
        ],
    );
    m.insert(
        "cbu.remove-role",
        vec![
            // Core patterns
            "remove role",
            "revoke role",
            "unassign role",
            "take away role",
            "end role",
            "terminate role",
            "delete role assignment",
            // Question forms
            "how do i remove a role",
            "how to unassign role",
            "can i revoke a role",
            // Resignation/departure patterns
            "resigned from role",
            "stepped down",
            "left position",
            "no longer holds role",
            "role ended",
            // Governance patterns
            "remove from board",
            "remove director",
            "remove signatory",
            "remove authorised signatory",
            "remove authorized signatory",
            // Error recovery
            "undo role assignment",
            "reverse role assignment",
            "wrong role assigned",
        ],
    );
    m.insert(
        "cbu.show",
        vec![
            // Core patterns
            "show cbu",
            "display cbu",
            "view cbu",
            "cbu details",
            "cbu info",
            "client details",
            "get cbu",
            "read cbu",
            // Question forms
            "what is the cbu",
            "show me the cbu",
            "what does the cbu look like",
            "tell me about cbu",
            // Summary patterns
            "cbu summary",
            "cbu overview",
            "cbu snapshot",
            "client overview",
            // Status patterns
            "cbu status",
            "where is cbu",
            "cbu state",
            // Lookup patterns
            "find cbu",
            "lookup cbu",
            "search cbu",
            "retrieve cbu",
        ],
    );
    m.insert(
        "cbu.parties",
        vec![
            // Core patterns
            "list parties",
            "show parties",
            "who is involved",
            "all entities",
            "related parties",
            "cbu participants",
            "stakeholders",
            "cbu entities",
            "who are the parties",
            // Question forms
            "who is in this cbu",
            "who are the stakeholders",
            "what entities are linked",
            "show me all parties",
            "list all stakeholders",
            // Role-based queries
            "who has roles",
            "all role holders",
            "all directors",
            "all signatories",
            "all shareholders",
            // Relationship queries
            "all related entities",
            "connected entities",
            "linked parties",
            "associated parties",
            // Summary patterns
            "party list",
            "party summary",
            "stakeholder list",
            "entity list for cbu",
        ],
    );
    m.insert(
        "cbu.add-product",
        vec![
            // Core patterns
            "add product",
            "assign product",
            "enable product",
            "activate product",
            "subscribe to product",
            "product subscription",
            "enroll in product",
            // Question forms
            "how do i add a product",
            "what products can i add",
            "can i subscribe to product",
            // Service patterns
            "add service",
            "enable service",
            "activate service",
            "subscribe to service",
            // Custody-specific patterns
            "add custody product",
            "add fund admin product",
            "add ta product",
            "add transfer agency",
            // Trading patterns
            "enable trading",
            "activate trading",
            "trading product",
            "global custody",
            // Enrollment patterns
            "enroll client",
            "product enrollment",
            "service enrollment",
            // Error recovery
            "change product",
            "update product subscription",
        ],
    );
    m.insert(
        "cbu.decide",
        vec![
            // Core patterns
            "make decision",
            "approve cbu",
            "reject cbu",
            "decide on client",
            "final decision",
            "onboarding decision",
            "cbu approval",
            // Question forms
            "how do i approve",
            "can i approve this cbu",
            "ready to approve",
            "should i approve",
            // Approval patterns
            "approve client",
            "approve onboarding",
            "give approval",
            "grant approval",
            "sign off",
            "sign-off",
            // Rejection patterns
            "reject client",
            "decline client",
            "refuse onboarding",
            "decline onboarding",
            // Conditional patterns
            "approve with conditions",
            "conditional approval",
            "approve pending",
            // Workflow patterns
            "complete onboarding",
            "finish onboarding",
            "close onboarding",
            "finalize client",
        ],
    );

    // ==========================================================================
    // ENTITY VERBS - Legal Entity Management
    // ==========================================================================
    m.insert(
        "entity.create-limited-company",
        vec![
            // Core patterns
            "create company",
            "new company",
            "add company",
            "create entity",
            "create ltd",
            "create limited company",
            "new legal entity",
            // Question forms
            "how do i add a company",
            "how to create a company entity",
            "can i add a new company",
            // Incorporation patterns
            "incorporate company",
            "add corporation",
            "register company",
            "company registration",
            "company incorporation",
            // Regional company types - UK
            "create plc",
            "create private limited",
            "create public limited",
            "ltd company",
            // Regional company types - German
            "create gmbh",
            "neue gmbh",
            "gmbh anlegen",
            "create ag",
            "create kg",
            // Regional company types - French
            "create sarl",
            "create sas",
            "create sa",
            // Regional company types - Dutch/Belgian
            "create bv",
            "create nv",
            // Regional company types - Irish
            "create dac",
            "create clg",
            // Regional company types - US
            "create llc",
            "create inc",
            "create corp",
            "create c-corp",
            "create s-corp",
            // Regional company types - Offshore
            "create spv",
            "create offshore company",
            "create holding company",
            "cayman company",
            "bvi company",
            "jersey company",
            "guernsey company",
            "luxembourg company",
            // Parent/subsidiary patterns
            "add subsidiary",
            "create subsidiary",
            "add holding company",
            "create parent company",
            // Error recovery
            "fix company details",
            "correct company",
        ],
    );
    m.insert(
        "entity.ensure-limited-company",
        vec![
            // Core patterns
            "ensure company",
            "upsert company",
            "create or update company",
            "find or create company",
            "idempotent company",
            "company if not exists",
            // Question forms
            "does company exist",
            "is company already added",
            "check company exists",
            // Safe creation patterns
            "safe create company",
            "create company if needed",
            "company when not exists",
            // Deduplication patterns
            "avoid duplicate company",
            "dedupe company",
            "check before create company",
            "match existing company",
        ],
    );
    m.insert(
        "entity.create-proper-person",
        vec![
            // Core patterns
            "create person",
            "add person",
            "new individual",
            "add individual",
            "create natural person",
            "add human",
            // Question forms
            "how do i add a person",
            "how to create individual",
            "can i add a person",
            // Record patterns
            "new person record",
            "create director",
            "add shareholder person",
            "register individual",
            "new natural person",
            // Role-specific patterns
            "add individual director",
            "add individual shareholder",
            "add individual signatory",
            "add beneficial owner",
            "add ubo person",
            // Relationship patterns
            "add related person",
            "add connected person",
            "add controller person",
            // Error recovery
            "fix person details",
            "correct person",
            "update person",
        ],
    );
    m.insert(
        "entity.ensure-proper-person",
        vec![
            // Core patterns
            "ensure person",
            "upsert person",
            "create or update person",
            "find or create person",
            "idempotent person",
            "person if not exists",
            // Question forms
            "does person exist",
            "is person already added",
            "check person exists",
            // Safe creation patterns
            "safe create person",
            "create person if needed",
            "person when not exists",
            // Deduplication patterns
            "avoid duplicate person",
            "dedupe person",
            "match existing person",
        ],
    );
    m.insert(
        "entity.create-trust-discretionary",
        vec![
            // Core patterns
            "create trust",
            "new trust",
            "add trust",
            "discretionary trust",
            "family trust",
            "create settlement",
            "establish trust",
            // Question forms
            "how do i add a trust",
            "how to create trust entity",
            "can i add a trust",
            // Trust types
            "unit trust",
            "bare trust",
            "charitable trust",
            "private trust",
            "purpose trust",
            "create trust structure",
            "fixed trust",
            "revocable trust",
            "irrevocable trust",
            // Regional patterns
            "jersey trust",
            "cayman trust",
            "guernsey trust",
            "isle of man trust",
            // Foundation patterns
            "create foundation",
            "private foundation",
            "stiftung",
            "anstalt",
        ],
    );
    m.insert(
        "entity.create-partnership-limited",
        vec![
            // Core patterns
            "create partnership",
            "new lp",
            "add limited partnership",
            "create lp",
            "new partnership",
            // Question forms
            "how do i add a partnership",
            "how to create lp",
            "can i add a partnership",
            // Partnership types
            "create gp",
            "general partner",
            "limited partner",
            "create llp",
            "create slp",
            "scottish partnership",
            "scottish lp",
            // Regional patterns
            "cayman lp",
            "delaware lp",
            "luxembourg lp",
            "english lp",
            // PE/VC patterns
            "create fund vehicle",
            "create investment partnership",
            "create co-invest vehicle",
        ],
    );
    m.insert(
        "entity.update",
        vec![
            // Core patterns
            "update entity",
            "modify entity",
            "change entity details",
            "edit entity",
            "correct entity",
            "amend entity",
            // Question forms
            "how do i update an entity",
            "can i change entity details",
            "how to edit entity",
            // Specific updates
            "change entity name",
            "update entity address",
            "change jurisdiction",
            "update registration number",
            "change lei",
            "update incorporation date",
            // Error recovery
            "fix entity",
            "correct entity mistake",
            "entity was wrong",
        ],
    );


    // ==========================================================================
    // FUND VERBS - Fund Structure Management
    // ==========================================================================
    m.insert(
        "fund.create-umbrella",
        vec![
            // Core patterns
            "create umbrella",
            "new sicav",
            "create sicav",
            "new icav",
            "create fund umbrella",
            "umbrella fund",
            "create master fund",
            "new fund structure",
            // Question forms
            "how do i create an umbrella",
            "how to set up sicav",
            "can i create a fund",
            // Establishment patterns
            "establish fund",
            "establish umbrella",
            "launch fund",
            "set up fund",
            // Regional fund types - Irish
            "create vcic",
            "create icav",
            "create plc fund",
            "irish fund",
            "ucits fund",
            // Regional fund types - UK
            "create oeic",
            "create aic",
            "uk fund",
            // Regional fund types - Luxembourg
            "luxembourg sicav",
            "fcp",
            "sif",
            "raif",
            "part ii fund",
            // Regional fund types - Cayman
            "cayman fund",
            "spc",
            "segregated portfolio company",
            // Fund strategy patterns
            "create hedge fund",
            "create pe fund",
            "create vc fund",
            "create private equity fund",
            "create venture fund",
            "create real estate fund",
            // Registration patterns
            "register fund",
            "fund registration",
            "new umbrella structure",
        ],
    );
    m.insert(
        "fund.ensure-umbrella",
        vec![
            // Core patterns
            "ensure umbrella",
            "upsert umbrella",
            "ensure sicav exists",
            "find or create umbrella",
            "umbrella if not exists",
            // Question forms
            "does umbrella exist",
            "is fund already created",
            "check umbrella exists",
            // Safe creation patterns
            "safe create umbrella",
            "create umbrella if needed",
            "umbrella when not exists",
            // Deduplication patterns
            "avoid duplicate umbrella",
            "match existing fund",
        ],
    );
    m.insert(
        "fund.create-subfund",
        vec![
            // Core patterns
            "create subfund",
            "new subfund",
            "add compartment",
            "create compartment",
            "new sub-fund",
            "add portfolio",
            "create sleeve",
            "new fund compartment",
            // Question forms
            "how do i add a subfund",
            "how to create compartment",
            "can i add a subfund",
            // Structure patterns
            "add sub-fund",
            "create cell",
            "create segregated portfolio",
            "add strategy",
            "new strategy",
            // Naming patterns
            "create fund class",
            "add fund series",
            "new fund sleeve",
            // Error recovery
            "fix subfund",
            "correct compartment",
        ],
    );
    m.insert(
        "fund.ensure-subfund",
        vec![
            // Core patterns
            "ensure subfund",
            "upsert subfund",
            "ensure compartment",
            "find or create subfund",
            "subfund if not exists",
            // Question forms
            "does subfund exist",
            "is compartment already created",
            // Safe creation patterns
            "safe create subfund",
            "create subfund if needed",
        ],
    );
    m.insert(
        "fund.create-share-class",
        vec![
            // Core patterns
            "create share class",
            "new share class",
            "add share class",
            "create isin",
            "new isin",
            "add class",
            // Question forms
            "how do i create a share class",
            "how to add isin",
            "can i create new class",
            // Class types - distribution
            "institutional class",
            "retail class",
            "accumulating class",
            "distributing class",
            "income class",
            "accumulation class",
            // Class types - hedging
            "hedged class",
            "unhedged class",
            "currency hedged",
            "fx hedged class",
            // Class types - fee
            "founder class",
            "seed class",
            "clean class",
            "performance fee class",
            "management fee class",
            // Regional patterns
            "create sedol",
            "add sedol",
            "create cusip",
            "add cusip",
            // Launch patterns
            "launch share class",
            "new class launch",
            "soft close class",
            "hard close class",
        ],
    );
    m.insert(
        "fund.ensure-share-class",
        vec![
            // Core patterns
            "ensure share class",
            "upsert share class",
            "find or create share class",
            "share class if not exists",
            // Question forms
            "does share class exist",
            "is isin already created",
            // Safe creation patterns
            "safe create share class",
            "create class if needed",
        ],
    );
    m.insert(
        "fund.link-feeder",
        vec![
            // Core patterns
            "link feeder",
            "connect feeder to master",
            "feeder master relationship",
            "master feeder",
            "feeder fund",
            "link to master",
            "feeder structure",
            // Question forms
            "how do i link feeder to master",
            "how to connect feeder fund",
            "can i link feeder",
            // Structure patterns
            "establish feeder relationship",
            "create master-feeder",
            "master feeder structure",
            "feeder fund setup",
            // Investment patterns
            "feeder invests in master",
            "feeder allocation",
        ],
    );
    m.insert(
        "fund.list-subfunds",
        vec![
            // Core patterns
            "list subfunds",
            "show compartments",
            "subfunds under umbrella",
            "all compartments",
            "fund hierarchy",
            "umbrella compartments",
            // Question forms
            "what subfunds exist",
            "how many compartments",
            "show me all subfunds",
            // Query patterns
            "get subfund list",
            "subfund overview",
            "compartment summary",
        ],
    );
    m.insert(
        "fund.list-share-classes",
        vec![
            // Core patterns
            "list share classes",
            "show share classes",
            "isins for fund",
            "all classes",
            "fund isins",
            "share class list",
            // Question forms
            "what share classes exist",
            "how many classes",
            "show me all isins",
            // Query patterns
            "get class list",
            "class overview",
            "isin summary",
            "available classes",
        ],
    );

    // ==========================================================================
    // UBO/OWNERSHIP VERBS - Beneficial Ownership Management
    // ==========================================================================
    m.insert(
        "ubo.add-ownership",
        vec![
            // Core patterns
            "add owner",
            "add ownership",
            "owns",
            "shareholder of",
            "add shareholder",
            "ownership stake",
            "equity stake",
            "parent company",
            "holding company",
            // Question forms
            "how do i add ownership",
            "how to link owner",
            "can i add shareholder",
            "who owns this",
            // UBO terminology
            "beneficial owner",
            "percentage holding",
            "ownership link",
            "owns percent",
            "shareholding",
            "equity holder",
            "ultimate owner",
            // Directness patterns
            "direct ownership",
            "indirect ownership",
            "direct stake",
            "indirect stake",
            // Percentage patterns
            "25% ownership",
            "majority owner",
            "minority stake",
            "controlling interest",
            "owns 100%",
            "wholly owned",
            // Chain patterns
            "ownership chain",
            "ownership structure",
            "add to chain",
            "intermediate owner",
            // Error recovery
            "change ownership percentage",
            "fix ownership",
            "correct percentage",
        ],
    );
    m.insert(
        "ubo.update-ownership",
        vec![
            // Core patterns
            "update ownership",
            "change percentage",
            "modify stake",
            "adjust ownership",
            "correct percentage",
            "ownership changed",
            // Question forms
            "how do i change ownership",
            "can i update percentage",
            "how to modify stake",
            // Specific changes
            "increase stake",
            "decrease stake",
            "dilution",
            "ownership diluted",
            "stake increased",
            "stake decreased",
            // Transaction patterns
            "partial sale",
            "acquired more shares",
            "bought more",
            "sold some shares",
            // Error recovery
            "fix ownership mistake",
            "wrong percentage",
            "percentage was wrong",
        ],
    );
    m.insert(
        "ubo.end-ownership",
        vec![
            // Core patterns
            "end ownership",
            "remove owner",
            "sold stake",
            "divested",
            "ownership ended",
            "no longer owns",
            "exit ownership",
            "disposed shares",
            // Question forms
            "how do i remove owner",
            "how to end ownership",
            "can i remove shareholder",
            // Transaction patterns
            "sold out",
            "full exit",
            "complete divestment",
            "ownership transfer",
            "shares transferred",
            // Timing patterns
            "ownership ceased",
            "no longer shareholder",
            "left ownership",
            "exited position",
        ],
    );
    m.insert(
        "ubo.list-owners",
        vec![
            // Core patterns
            "list owners",
            "who owns",
            "shareholders",
            "ownership chain up",
            "direct owners",
            "immediate shareholders",
            "show owners",
            "parent entities",
            // Question forms
            "who owns this company",
            "who are the shareholders",
            "show me the owners",
            "what entities own this",
            // Chain patterns
            "ownership above",
            "upstream owners",
            "parent chain",
            "who is above",
            // Percentage patterns
            "owners above 25%",
            "significant shareholders",
            "majority owners",
        ],
    );
    m.insert(
        "ubo.list-owned",
        vec![
            // Core patterns
            "list owned",
            "subsidiaries",
            "what do they own",
            "ownership chain down",
            "investments",
            "holdings",
            "show subsidiaries",
            "child entities",
            // Question forms
            "what does this own",
            "show subsidiaries",
            "what are the holdings",
            "downstream ownership",
            // Structure patterns
            "portfolio companies",
            "controlled entities",
            "wholly owned subs",
            "downstream investments",
        ],
    );
    m.insert(
        "ubo.register-ubo",
        vec![
            // Core patterns
            "register ubo",
            "add beneficial owner",
            "ubo registration",
            "record ubo",
            "beneficial owner declaration",
            "ultimate beneficial owner",
            "declare ubo",
            "ubo identified",
            // Question forms
            "how do i register a ubo",
            "how to declare ubo",
            "can i add ubo",
            // Regulatory patterns
            "ubo declaration",
            "ubo disclosure",
            "bo register entry",
            "beneficial owner register",
            // Threshold patterns
            "ubo above 25%",
            "qualifying ubo",
            "registrable ubo",
        ],
    );
    m.insert(
        "ubo.mark-terminus",
        vec![
            // Core patterns
            "mark terminus",
            "end of chain",
            "public company",
            "no known person",
            "ubo terminus",
            "dispersed ownership",
            "listed company",
            "widely held",
            // Question forms
            "how do i end the chain",
            "where does chain stop",
            "can i mark as terminus",
            // Terminus types
            "regulated entity",
            "government owned",
            "natural person terminus",
            "chain termination",
            "ownership stops here",
            "listed parent",
            "sovereign entity",
            "state owned",
            // Documentation patterns
            "exempt entity",
            "simplified dd",
            "no further owners",
        ],
    );
    m.insert(
        "ubo.calculate",
        vec![
            // Core patterns
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
            // Question forms
            "who are the ubos",
            "calculate who owns",
            "work out ubos",
            "determine ubos",
            // Threshold patterns
            "above 25%",
            "threshold analysis",
            "qualifying owners",
            "significant ownership",
            // Chain calculation
            "indirect ownership calc",
            "chain multiplication",
            "effective ownership",
            "ultimate ownership percentage",
        ],
    );
    m.insert(
        "ubo.trace-chains",
        vec![
            // Core patterns
            "trace chains",
            "trace ownership",
            "follow ownership",
            "ownership path",
            "ownership tree",
            "chain analysis",
            "ownership structure",
            "walk ownership",
            "ownership diagram",
            // Question forms
            "trace the ownership chain",
            "show ownership path",
            "how does ownership flow",
            // Visualization patterns
            "ownership chart",
            "ownership graph",
            "visualize ownership",
            "draw ownership",
            // Analysis patterns
            "chain depth",
            "ownership layers",
            "intermediate entities",
        ],
    );

    // ==========================================================================
    // CONTROL VERBS - Significant Control Management
    // ==========================================================================
    m.insert(
        "control.add",
        vec![
            // Core patterns
            "add control",
            "controls",
            "controlling person",
            "significant control",
            "psc",
            "person of significant control",
            "control relationship",
            // Question forms
            "how do i add control",
            "who controls this",
            "can i add controller",
            // Control types
            "voting control",
            "board control",
            "significant influence",
            "de facto control",
            "management control",
            "senior managing official",
            // PSC patterns (UK specific)
            "registrable person",
            "psc register",
            "add to psc register",
            // Percentage patterns
            "controls 25%",
            "majority control",
            "controls board",
            "appoints directors",
        ],
    );
    m.insert(
        "control.list-controllers",
        vec![
            // Core patterns
            "list controllers",
            "who controls",
            "controlling parties",
            "show control",
            "control chain",
            "persons with control",
            // Question forms
            "who controls this company",
            "show me controllers",
            "list all pscs",
            // Query patterns
            "control summary",
            "psc list",
            "significant controllers",
        ],
    );


    // ==========================================================================
    // ROLE ASSIGNMENT (V2) - Enhanced Role Management
    // ==========================================================================
    m.insert(
        "cbu.role:assign",
        vec![
            // Core patterns
            "assign role",
            "add role to cbu",
            "entity role",
            "role assignment",
            "link with role",
            // Question forms
            "how do i assign role",
            "what role to use",
            "can i assign role",
        ],
    );
    m.insert(
        "cbu.role:assign-ownership",
        vec![
            // Core patterns
            "assign ownership role",
            "shareholder role",
            "owner role",
            "beneficial ownership role",
            "equity holder",
            "investor role",
            // Question forms
            "how do i add as owner",
            "make them shareholder",
            "assign as investor",
            // Specific patterns
            "minority shareholder",
            "majority shareholder",
            "founding shareholder",
            "preference shareholder",
            "ordinary shareholder",
        ],
    );
    m.insert(
        "cbu.role:assign-control",
        vec![
            // Core patterns
            "assign control role",
            "director role",
            "officer role",
            "board member",
            "executive role",
            // Question forms
            "how do i add as director",
            "make them board member",
            "assign as officer",
            // Executive roles
            "ceo",
            "cfo",
            "coo",
            "cio",
            "cto",
            "chairman",
            "chairwoman",
            "chair",
            "managing director",
            "md",
            "company secretary",
            "cosec",
            // Board roles
            "executive director",
            "non-executive director",
            "ned",
            "independent director",
            // Committee roles
            "audit committee",
            "remuneration committee",
            "risk committee",
        ],
    );
    m.insert(
        "cbu.role:assign-trust-role",
        vec![
            // Core patterns
            "assign trust role",
            "trustee",
            "settlor",
            "beneficiary",
            "protector",
            "enforcer",
            "trust role",
            // Question forms
            "how do i add as trustee",
            "make them beneficiary",
            "assign as protector",
            // Specific patterns
            "trust beneficiary",
            "trust settlor",
            "named beneficiary",
            "discretionary beneficiary",
            "fixed beneficiary",
            "remainderman",
            "life tenant",
            // Professional trustee
            "corporate trustee",
            "professional trustee",
            "trust company",
        ],
    );
    m.insert(
        "cbu.role:assign-fund-role",
        vec![
            // Core patterns
            "assign fund role",
            "management company",
            "manco",
            "investment manager",
            "aifm",
            "fund admin",
            "portfolio manager",
            "sub-advisor",
            "investment advisor",
            // Question forms
            "how do i assign manco",
            "add investment manager",
            "assign aifm",
            // Specific roles
            "ucits management company",
            "super manco",
            "third party manco",
            "discretionary manager",
            "non-discretionary manager",
            // Administrator roles
            "fund administrator",
            "central administration",
            "naf",
            "nav calculator",
        ],
    );
    m.insert(
        "cbu.role:assign-service-provider",
        vec![
            // Core patterns
            "assign service provider",
            "depositary",
            "custodian",
            "auditor",
            "administrator",
            "transfer agent",
            // Question forms
            "how do i add service provider",
            "assign custodian",
            "add depositary",
            // Professional services
            "prime broker",
            "pb",
            "legal counsel",
            "tax advisor",
            "registrar",
            "fund accountant",
            "valuation agent",
            "paying agent",
            // Specific patterns
            "external auditor",
            "internal auditor",
            "legal advisor",
            "compliance consultant",
        ],
    );
    m.insert(
        "cbu.role:assign-signatory",
        vec![
            // Core patterns
            "assign signatory",
            "authorized signatory",
            "authorized trader",
            "power of attorney",
            "signing authority",
            "poa",
            "mandate holder",
            "signing rights",
            // Question forms
            "how do i add signatory",
            "give signing authority",
            "authorize to sign",
            // Authority patterns
            "bank signatory",
            "trading signatory",
            "joint signatory",
            "sole signatory",
            "a+b signatory",
            // Mandate patterns
            "trading mandate",
            "custody mandate",
            "banking mandate",
            "investment mandate",
        ],
    );

    // ==========================================================================
    // GRAPH/NAVIGATION VERBS - Graph UI Navigation
    // ==========================================================================
    m.insert(
        "graph.view",
        vec![
            // Core patterns
            "view graph",
            "show graph",
            "visualize",
            "display structure",
            "entity graph",
            "ownership graph",
            "structure visualization",
            "show structure",
            // Question forms
            "can i see the graph",
            "show me the structure",
            "visualize ownership",
            // Display patterns
            "render graph",
            "draw structure",
            "graph view",
            "structure view",
            "open graph",
        ],
    );
    m.insert(
        "graph.focus",
        vec![
            // Core patterns
            "focus on",
            "zoom to",
            "center on",
            "highlight entity",
            "select node",
            "navigate to",
            // Question forms
            "can you focus on",
            "show me this entity",
            "zoom into",
            // Navigation patterns
            "go to entity",
            "jump to",
            "find in graph",
            "locate node",
        ],
    );
    m.insert(
        "graph.ancestors",
        vec![
            // Core patterns
            "show ancestors",
            "ownership chain up",
            "who owns this",
            "parent chain",
            "upstream owners",
            "trace up",
            // Question forms
            "who are the parents",
            "show me the owners",
            "trace up the chain",
            // Direction patterns
            "go up chain",
            "owners above",
            "parent entities",
        ],
    );
    m.insert(
        "graph.descendants",
        vec![
            // Core patterns
            "show descendants",
            "ownership chain down",
            "what do they own",
            "child entities",
            "downstream holdings",
            "trace down",
            // Question forms
            "what are the subsidiaries",
            "show me what they own",
            "trace down the chain",
            // Direction patterns
            "go down chain",
            "holdings below",
            "subsidiaries",
        ],
    );
    m.insert(
        "graph.path",
        vec![
            // Core patterns
            "path between",
            "connection between",
            "how are they related",
            "relationship path",
            "find route",
            "link between",
            // Question forms
            "how are these connected",
            "what is the relationship",
            "show the path between",
            // Analysis patterns
            "shortest path",
            "all paths",
            "connection analysis",
        ],
    );
    m.insert(
        "graph.filter",
        vec![
            // Core patterns
            "filter graph",
            "show only",
            "hide",
            "filter by type",
            "show funds only",
            "show persons only",
            "filter entities",
            // Question forms
            "can you filter to",
            "show me only",
            "hide everything except",
            // Specific filters
            "filter by jurisdiction",
            "filter by role",
            "filter by entity type",
            "filter by status",
        ],
    );
    m.insert(
        "graph.group-by",
        vec![
            // Core patterns
            "group by",
            "cluster by",
            "organize by",
            "group by jurisdiction",
            "group by type",
            // Question forms
            "can you group by",
            "organize the graph by",
            "cluster by country",
            // Grouping patterns
            "group by entity type",
            "group by role",
            "group by status",
        ],
    );

    // ==========================================================================
    // KYC CASE VERBS - KYC Case Management
    // ==========================================================================
    m.insert(
        "kyc-case.create",
        vec![
            // Core patterns
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
            // Question forms
            "how do i start kyc",
            "can i create a case",
            "how to begin kyc",
            // Process patterns
            "initiate due diligence",
            "start dd",
            "begin cdd",
            "start customer due diligence",
            // Event-driven patterns
            "periodic review case",
            "event driven review",
            "trigger event case",
            "remediation case",
        ],
    );
    m.insert(
        "kyc-case.update-status",
        vec![
            // Core patterns
            "update case status",
            "change case status",
            "case progress",
            "move case forward",
            "advance case",
            "progress case",
            "case status change",
            // Question forms
            "how do i update case",
            "can i change status",
            "how to progress case",
            // Specific status changes
            "move to review",
            "move to approval",
            "send for approval",
            "mark as pending",
            "mark as complete",
        ],
    );
    m.insert(
        "kyc-case.escalate",
        vec![
            // Core patterns
            "escalate case",
            "case escalation",
            "raise to senior",
            "escalate kyc",
            "send to compliance",
            "bump up case",
            // Question forms
            "how do i escalate",
            "can i escalate case",
            "need senior review",
            // Escalation targets
            "escalate to mlro",
            "send to compliance officer",
            "raise to head of kyc",
            "send to legal",
            // Reason patterns
            "escalate due to risk",
            "pep escalation",
            "high risk escalation",
        ],
    );
    m.insert(
        "kyc-case.assign",
        vec![
            // Core patterns
            "assign case",
            "assign analyst",
            "assign reviewer",
            "case assignment",
            "allocate case",
            "who works on case",
            // Question forms
            "how do i assign case",
            "can i assign analyst",
            "who should work on this",
            // Assignment patterns
            "reassign case",
            "transfer case",
            "allocate to team",
            "assign to queue",
            // Workflow patterns
            "assign for review",
            "assign for approval",
            "assign for dd",
        ],
    );
    m.insert(
        "kyc-case.set-risk-rating",
        vec![
            // Core patterns
            "set risk rating",
            "risk rate case",
            "case risk",
            "rate risk",
            "assign risk",
            "risk assessment",
            // Risk levels
            "high risk",
            "low risk",
            "medium risk",
            "standard risk",
            "enhanced risk",
            "prohibited",
            // Question forms
            "what risk rating",
            "how to set risk",
            "can i change risk rating",
            // Assessment patterns
            "risk score",
            "overall risk",
            "inherent risk",
            "residual risk",
        ],
    );
    m.insert(
        "kyc-case.close",
        vec![
            // Core patterns
            "close case",
            "complete case",
            "finalize case",
            "end case",
            "case completion",
            "finish kyc",
            "case done",
            // Question forms
            "how do i close case",
            "can i complete case",
            "ready to close",
            // Closure patterns
            "mark complete",
            "case approved",
            "case rejected",
            "archive case",
            // Documentation patterns
            "sign off case",
            "final approval",
            "case closure",
        ],
    );
    m.insert(
        "kyc-case.read",
        vec![
            // Core patterns
            "read case",
            "get case",
            "case details",
            "show case",
            "view case",
            "case info",
            // Question forms
            "show me the case",
            "what is the case status",
            "tell me about case",
            // Detail patterns
            "case summary",
            "case overview",
            "full case details",
        ],
    );
    m.insert(
        "kyc-case.list-by-cbu",
        vec![
            // Core patterns
            "list cases",
            "cases for cbu",
            "cbu cases",
            "all cases",
            "case history",
            // Question forms
            "what cases exist",
            "show all cases",
            "how many cases",
            // Query patterns
            "case list",
            "historical cases",
            "past reviews",
        ],
    );
    m.insert(
        "kyc-case.reopen",
        vec![
            // Core patterns
            "reopen case",
            "case reopened",
            "remediation case",
            "review again",
            "periodic review",
            "event driven review",
            // Question forms
            "how do i reopen case",
            "can i reopen",
            "need to review again",
            // Trigger patterns
            "refresh review",
            "trigger review",
            "material change review",
            "anniversary review",
        ],
    );
    m.insert(
        "kyc-case.state",
        vec![
            // Core patterns
            "case state",
            "full case status",
            "case with workstreams",
            "case summary",
            "case overview",
            // Question forms
            "what is case state",
            "show full status",
            "where is case at",
            // Status patterns
            "complete picture",
            "all workstreams",
            "case snapshot",
        ],
    );


    // ==========================================================================
    // ENTITY WORKSTREAM VERBS - Due Diligence Workstreams
    // ==========================================================================
    m.insert(
        "entity-workstream.create",
        vec![
            // Core patterns
            "create workstream",
            "entity workstream",
            "kyc workstream",
            "due diligence workstream",
            "new workstream",
            "add entity to case",
            "start entity review",
            // Question forms
            "how do i create workstream",
            "can i add entity to case",
            "how to start entity dd",
            // Process patterns
            "initiate dd for entity",
            "begin entity review",
            "entity kyc",
            "dd workstream",
            "cdd for entity",
        ],
    );
    m.insert(
        "entity-workstream.update-status",
        vec![
            // Core patterns
            "update workstream",
            "workstream progress",
            "change workstream status",
            "advance workstream",
            // Question forms
            "how do i update workstream",
            "can i progress workstream",
            // Progress patterns
            "move workstream forward",
            "workstream status change",
            "mark workstream progress",
        ],
    );
    m.insert(
        "entity-workstream.block",
        vec![
            // Core patterns
            "block workstream",
            "workstream blocked",
            "pause workstream",
            "stop workstream",
            // Question forms
            "how do i block workstream",
            "need to pause workstream",
            // Blocking reasons
            "waiting for docs",
            "pending information",
            "blocked on client",
            "external dependency",
        ],
    );
    m.insert(
        "entity-workstream.complete",
        vec![
            // Core patterns
            "complete workstream",
            "workstream done",
            "finish workstream",
            "workstream complete",
            // Question forms
            "how do i complete workstream",
            "can i finish workstream",
            // Completion patterns
            "mark workstream complete",
            "workstream finished",
            "entity dd complete",
        ],
    );
    m.insert(
        "entity-workstream.set-enhanced-dd",
        vec![
            // Core patterns
            "enhanced dd",
            "enhanced due diligence",
            "edd required",
            "heightened dd",
            "extra scrutiny",
            // Question forms
            "how do i set edd",
            "need enhanced dd",
            "require edd",
            // Trigger patterns
            "pep edd",
            "high risk edd",
            "adverse media edd",
            "sanctions edd",
        ],
    );
    m.insert(
        "entity-workstream.set-ubo",
        vec![
            // Core patterns
            "mark as ubo",
            "workstream ubo",
            "ubo workstream",
            "identify ubo",
            // Question forms
            "is this a ubo",
            "mark entity as ubo",
            // UBO patterns
            "beneficial owner workstream",
            "ubo dd",
            "ubo due diligence",
        ],
    );
    m.insert(
        "entity-workstream.list-by-case",
        vec![
            // Core patterns
            "list workstreams",
            "case workstreams",
            "all workstreams",
            "entities in case",
            // Question forms
            "what workstreams exist",
            "show all workstreams",
            "how many entities in case",
            // Query patterns
            "workstream list",
            "dd workstreams",
            "outstanding workstreams",
        ],
    );
    m.insert(
        "entity-workstream.state",
        vec![
            // Core patterns
            "workstream state",
            "workstream details",
            "workstream with requests",
            // Question forms
            "what is workstream status",
            "show workstream details",
            // Detail patterns
            "full workstream info",
            "workstream overview",
        ],
    );

    // ==========================================================================
    // DOCUMENT REQUEST VERBS - Document Collection
    // ==========================================================================
    m.insert(
        "doc-request.create",
        vec![
            // Core patterns
            "request document",
            "ask for document",
            "doc request",
            "require document",
            "document requirement",
            "outstanding document",
            "need document",
            "document needed",
            // Question forms
            "how do i request a document",
            "can i ask for document",
            "what documents to request",
            // Document types
            "request id",
            "request passport",
            "request utility bill",
            "request bank statement",
            "request certificate of incorporation",
            "request cert of inc",
            "request register of members",
            "request rom",
            "request articles",
            "request moa aoa",
            "request constitutional docs",
            // Compliance docs
            "request kyc docs",
            "request aml docs",
            "request source of wealth",
            "request source of funds",
            "request sow",
            "request sof",
        ],
    );
    m.insert(
        "doc-request.mark-requested",
        vec![
            // Core patterns
            "mark requested",
            "formally request",
            "send request",
            "document requested",
            // Question forms
            "how do i mark as requested",
            "document has been requested",
            // Process patterns
            "send document request",
            "email request",
            "request sent",
        ],
    );
    m.insert(
        "doc-request.receive",
        vec![
            // Core patterns
            "receive document",
            "document received",
            "fulfilled request",
            "got document",
            "doc uploaded",
            "document submitted",
            // Question forms
            "how do i mark as received",
            "document has arrived",
            // Upload patterns
            "client uploaded",
            "document attached",
            "file received",
            "doc incoming",
        ],
    );
    m.insert(
        "doc-request.verify",
        vec![
            // Core patterns
            "verify document",
            "validate document",
            "check document",
            "document verification",
            "doc verified",
            "approve document",
            // Question forms
            "how do i verify document",
            "is document valid",
            "can i approve document",
            // Verification patterns
            "document approved",
            "doc accepted",
            "document passed",
            "verification complete",
        ],
    );
    m.insert(
        "doc-request.reject",
        vec![
            // Core patterns
            "reject document",
            "document rejected",
            "invalid document",
            "doc not acceptable",
            // Question forms
            "how do i reject document",
            "document is wrong",
            // Rejection reasons
            "document expired",
            "poor quality",
            "unreadable",
            "wrong document type",
            "incomplete document",
            "not certified",
            "not notarized",
        ],
    );
    m.insert(
        "doc-request.waive",
        vec![
            // Core patterns
            "waive document",
            "document waived",
            "skip document",
            "not required",
            "waive requirement",
            // Question forms
            "how do i waive document",
            "can i skip this document",
            // Waiver patterns
            "document not needed",
            "alternative provided",
            "exception granted",
            "waiver approved",
        ],
    );
    m.insert(
        "doc-request.list-by-workstream",
        vec![
            // Core patterns
            "list doc requests",
            "outstanding documents",
            "pending documents",
            "what documents needed",
            // Question forms
            "what docs are pending",
            "show outstanding requests",
            // Query patterns
            "document checklist",
            "doc status",
            "missing documents",
        ],
    );

    // ==========================================================================
    // CASE SCREENING VERBS - AML/KYC Screening
    // ==========================================================================
    m.insert(
        "case-screening.run",
        vec![
            // Core patterns
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
            // Question forms
            "how do i run screening",
            "can i screen entity",
            "need to run checks",
            // Provider patterns
            "world check",
            "world-check",
            "dowjones",
            "dow jones",
            "lexisnexis",
            "refinitiv screening",
            // Scope patterns
            "full screening",
            "initial screening",
            "rescreening",
            "rescreen",
        ],
    );
    m.insert(
        "case-screening.complete",
        vec![
            // Core patterns
            "complete screening",
            "screening done",
            "screening finished",
            "screening result",
            // Question forms
            "is screening done",
            "screening status",
            // Result patterns
            "screening passed",
            "no hits",
            "clear screening",
            "hits found",
        ],
    );
    m.insert(
        "case-screening.review-hit",
        vec![
            // Core patterns
            "review hit",
            "screening hit",
            "hit review",
            "potential match",
            "review match",
            "hit confirmed",
            "hit dismissed",
            "false positive",
            // Question forms
            "how do i review hit",
            "is this a true match",
            // Disposition patterns
            "confirm hit",
            "dismiss hit",
            "true match",
            "not a match",
            "fp",
            "true positive",
            "tp",
        ],
    );
    m.insert(
        "case-screening.list-by-workstream",
        vec![
            // Core patterns
            "list screenings",
            "screening history",
            "all screenings",
            "screening results",
            // Question forms
            "what screenings have been run",
            "show screening history",
            // Query patterns
            "screening summary",
            "hit summary",
            "screening status",
        ],
    );

    // ==========================================================================
    // RED FLAG VERBS - Risk Flag Management
    // ==========================================================================
    m.insert(
        "red-flag.raise",
        vec![
            // Core patterns
            "raise red flag",
            "flag issue",
            "compliance concern",
            "escalate issue",
            "alert",
            "raise concern",
            "report issue",
            "flag problem",
            "red flag identified",
            // Question forms
            "how do i raise a flag",
            "need to flag issue",
            "should i raise concern",
            // Specific flags
            "pep flag",
            "sanctions flag",
            "adverse media flag",
            "source of wealth concern",
            "suspicious activity",
            "unusual pattern",
            "shell company flag",
            "opacity flag",
            "circular ownership flag",
        ],
    );
    m.insert(
        "red-flag.mitigate",
        vec![
            // Core patterns
            "mitigate red flag",
            "resolve flag",
            "address concern",
            "close red flag",
            "flag mitigated",
            "issue resolved",
            // Question forms
            "how do i mitigate flag",
            "can i resolve this flag",
            // Mitigation patterns
            "document mitigation",
            "explain flag",
            "flag addressed",
            "concern addressed",
            "comfort obtained",
        ],
    );
    m.insert(
        "red-flag.waive",
        vec![
            // Core patterns
            "waive red flag",
            "flag waived",
            "approve despite flag",
            "accept risk",
            // Question forms
            "how do i waive flag",
            "can i proceed with flag",
            // Approval patterns
            "senior waiver",
            "mlro waiver",
            "risk accepted",
            "exception approved",
        ],
    );
    m.insert(
        "red-flag.dismiss",
        vec![
            // Core patterns
            "dismiss flag",
            "false positive flag",
            "flag dismissed",
            "not a concern",
            // Question forms
            "how do i dismiss flag",
            "flag is wrong",
            // Dismissal patterns
            "flag irrelevant",
            "not applicable",
            "false alarm",
            "erroneous flag",
        ],
    );
    m.insert(
        "red-flag.set-blocking",
        vec![
            // Core patterns
            "blocking flag",
            "flag blocks case",
            "hard stop",
            "case blocked",
            // Question forms
            "is this a blocker",
            "does flag block",
            // Blocking patterns
            "mandatory block",
            "cannot proceed",
            "must resolve",
            "showstopper",
        ],
    );
    m.insert(
        "red-flag.list-by-case",
        vec![
            // Core patterns
            "list red flags",
            "case flags",
            "all flags",
            "open flags",
            // Question forms
            "what flags exist",
            "show all flags",
            // Query patterns
            "flag summary",
            "outstanding flags",
            "blocking flags",
        ],
    );

    // ==========================================================================
    // SCREENING VERBS (PEP, Sanctions, Adverse Media)
    // ==========================================================================
    m.insert(
        "screening.pep",
        vec![
            // Core patterns
            "pep screening",
            "politically exposed",
            "pep check",
            "check for pep",
            "political exposure",
            "government official check",
            // Question forms
            "is this person a pep",
            "check pep status",
            "any political exposure",
            // PEP types
            "domestic pep",
            "foreign pep",
            "international organization pep",
            "senior government official",
            "head of state",
            "rca",
            "relative close associate",
            "pep family member",
        ],
    );
    m.insert(
        "screening.sanctions",
        vec![
            // Core patterns
            "sanctions screening",
            "sanctions check",
            "ofac check",
            "sanctions list",
            "restricted party",
            "blocked persons",
            "sdn list",
            // Question forms
            "is entity sanctioned",
            "check sanctions",
            "any sanctions hits",
            // Specific lists
            "ofac",
            "un sanctions",
            "eu sanctions",
            "uk sanctions",
            "hm treasury",
            "consolidated list",
            "specially designated nationals",
            "sectoral sanctions",
        ],
    );
    m.insert(
        "screening.adverse-media",
        vec![
            // Core patterns
            "adverse media",
            "negative news",
            "media screening",
            "news check",
            "reputation check",
            "bad press",
            // Question forms
            "any negative news",
            "check media coverage",
            "adverse media hits",
            // Coverage patterns
            "financial crime news",
            "fraud allegations",
            "bribery news",
            "corruption news",
            "money laundering news",
            "reputational risk",
        ],
    );

    // ==========================================================================
    // DOCUMENT VERBS - Document Management
    // ==========================================================================
    m.insert(
        "document.catalog",
        vec![
            // Core patterns
            "catalog document",
            "upload document",
            "add document",
            "attach file",
            "store document",
            "register document",
            "save document",
            "document uploaded",
            // Question forms
            "how do i upload document",
            "can i add a file",
            // Process patterns
            "file attachment",
            "document storage",
            "add to repository",
            "store in dms",
        ],
    );
    m.insert(
        "document.extract",
        vec![
            // Core patterns
            "extract from document",
            "parse document",
            "read document",
            "document extraction",
            "ocr document",
            "extract data",
            "pull from document",
            // Question forms
            "how do i extract data",
            "can you read document",
            // Extraction patterns
            "document processing",
            "data extraction",
            "idp",
            "intelligent document processing",
            "document ocr",
            "document ai",
        ],
    );


    // ==========================================================================
    // SERVICE/PRODUCT VERBS - Product Catalog
    // ==========================================================================
    m.insert(
        "service.list",
        vec![
            // Core patterns
            "list services",
            "available services",
            "what services",
            "service catalog",
            "show services",
            // Question forms
            "what services are available",
            "show me services",
            // Catalog patterns
            "service menu",
            "service offering",
            "all services",
        ],
    );
    m.insert(
        "product.list",
        vec![
            // Core patterns
            "list products",
            "available products",
            "what products",
            "product catalog",
            "show products",
            // Question forms
            "what products are available",
            "show me products",
            // Catalog patterns
            "product menu",
            "product offering",
            "all products",
        ],
    );
    m.insert(
        "product.subscribe",
        vec![
            // Core patterns
            "subscribe to product",
            "enable product",
            "activate product",
            "product subscription",
            "add product",
            // Question forms
            "how do i subscribe",
            "can i enable product",
            // Enrollment patterns
            "enroll in product",
            "sign up for product",
            "turn on product",
        ],
    );
    m.insert(
        "product.unsubscribe",
        vec![
            // Core patterns
            "unsubscribe product",
            "disable product",
            "deactivate product",
            "cancel subscription",
            "remove product",
            // Question forms
            "how do i unsubscribe",
            "can i disable product",
            // Removal patterns
            "turn off product",
            "stop product",
            "end subscription",
        ],
    );

    // ==========================================================================
    // CUSTODY VERBS - UNIVERSE Management
    // ==========================================================================
    m.insert(
        "cbu-custody.add-universe",
        vec![
            // Core patterns
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
            // Question forms
            "how do i add to universe",
            "what can client trade",
            "can i enable market",
            // Market patterns
            "enable us equities",
            "add european bonds",
            "allow derivatives",
            "add fixed income",
            "enable etfs",
            "add listed options",
            // Instrument class patterns
            "add asset class",
            "enable security type",
            "add to permitted list",
        ],
    );
    m.insert(
        "cbu-custody.list-universe",
        vec![
            // Core patterns
            "list universe",
            "show universe",
            "trading permissions",
            "what can cbu trade",
            "universe entries",
            "permitted instruments",
            // Question forms
            "what can they trade",
            "show trading permissions",
            // Query patterns
            "universe summary",
            "trading scope",
            "permitted markets",
        ],
    );
    m.insert(
        "cbu-custody.remove-universe",
        vec![
            // Core patterns
            "remove from universe",
            "disable trading",
            "remove instrument class",
            "stop trading",
            "restrict universe",
            // Question forms
            "how do i remove from universe",
            "can i disable market",
            // Restriction patterns
            "block market",
            "remove market",
            "restrict trading",
        ],
    );

    // ==========================================================================
    // CUSTODY VERBS - SSI Management
    // ==========================================================================
    m.insert(
        "cbu-custody.create-ssi",
        vec![
            // Core patterns
            "create ssi",
            "standing settlement instruction",
            "settlement instruction",
            "new ssi",
            "add settlement details",
            "settlement account",
            "safekeeping account",
            "setup ssi",
            // Question forms
            "how do i create ssi",
            "what ssi do i need",
            "can i add settlement instruction",
            // Account patterns
            "add custody account",
            "add settlement account",
            "add agent account",
            // SWIFT patterns
            "add swift details",
            "add bic",
            "add safekeeping account number",
            "add sac",
            // Network patterns
            "dtc settlement",
            "euroclear account",
            "clearstream account",
            "fed book entry",
        ],
    );
    m.insert(
        "cbu-custody.ensure-ssi",
        vec![
            // Core patterns
            "ensure ssi",
            "upsert ssi",
            "find or create ssi",
            "idempotent ssi",
            "ssi if not exists",
            // Question forms
            "does ssi exist",
            "check ssi exists",
            // Safe patterns
            "safe create ssi",
            "create ssi if needed",
        ],
    );
    m.insert(
        "cbu-custody.activate-ssi",
        vec![
            // Core patterns
            "activate ssi",
            "enable ssi",
            "ssi active",
            "go live ssi",
            "ssi ready",
            // Question forms
            "how do i activate ssi",
            "can i enable ssi",
            // Activation patterns
            "make ssi live",
            "ssi effective",
            "ssi approved",
        ],
    );
    m.insert(
        "cbu-custody.suspend-ssi",
        vec![
            // Core patterns
            "suspend ssi",
            "disable ssi",
            "pause ssi",
            "ssi inactive",
            "deactivate ssi",
            // Question forms
            "how do i suspend ssi",
            "can i disable ssi",
            // Suspension patterns
            "ssi on hold",
            "stop using ssi",
            "ssi suspended",
        ],
    );
    m.insert(
        "cbu-custody.list-ssis",
        vec![
            // Core patterns
            "list ssis",
            "show settlement instructions",
            "all ssis",
            "settlement accounts",
            "ssi list",
            // Question forms
            "what ssis exist",
            "show all ssis",
            // Query patterns
            "ssi summary",
            "ssi overview",
            "active ssis",
        ],
    );
    m.insert(
        "cbu-custody.setup-ssi",
        vec![
            // Core patterns
            "setup ssi",
            "bulk ssi import",
            "import settlement instructions",
            "load ssis",
            "ssi migration",
            // Question forms
            "how do i bulk import ssis",
            "can i import ssis",
            // Import patterns
            "ssi upload",
            "batch ssi",
            "ssi file",
            "ssi spreadsheet",
        ],
    );
    m.insert(
        "cbu-custody.lookup-ssi",
        vec![
            // Core patterns
            "lookup ssi",
            "find ssi",
            "resolve ssi",
            "which ssi",
            "ssi for trade",
            "ssi lookup",
            // Question forms
            "which ssi to use",
            "what ssi for this trade",
            // Resolution patterns
            "ssi selection",
            "ssi matching",
            "best ssi",
            "default ssi",
        ],
    );

    // ==========================================================================
    // CUSTODY VERBS - BOOKING RULES
    // ==========================================================================
    m.insert(
        "cbu-custody.add-booking-rule",
        vec![
            // Core patterns
            "add booking rule",
            "routing rule",
            "settlement routing",
            "booking configuration",
            "trade routing",
            "alert rule",
            "ssi selection rule",
            // Question forms
            "how do i add booking rule",
            "what routing rule to use",
            // Rule patterns
            "add routing logic",
            "settlement rule",
            "booking logic",
            "ssi rule",
        ],
    );
    m.insert(
        "cbu-custody.ensure-booking-rule",
        vec![
            // Core patterns
            "ensure booking rule",
            "upsert booking rule",
            "idempotent booking rule",
            // Question forms
            "does rule exist",
            // Safe patterns
            "safe create rule",
        ],
    );
    m.insert(
        "cbu-custody.list-booking-rules",
        vec![
            // Core patterns
            "list booking rules",
            "show routing rules",
            "all booking rules",
            "routing configuration",
            // Question forms
            "what rules exist",
            "show all rules",
            // Query patterns
            "rule summary",
            "routing overview",
        ],
    );
    m.insert(
        "cbu-custody.update-rule-priority",
        vec![
            // Core patterns
            "update rule priority",
            "change rule order",
            "reorder rules",
            "rule precedence",
            // Question forms
            "how do i change priority",
            "can i reorder rules",
            // Priority patterns
            "rule ranking",
            "move rule up",
            "move rule down",
        ],
    );
    m.insert(
        "cbu-custody.deactivate-rule",
        vec![
            // Core patterns
            "deactivate rule",
            "disable booking rule",
            "remove routing rule",
            // Question forms
            "how do i deactivate rule",
            "can i remove rule",
            // Removal patterns
            "turn off rule",
            "suspend rule",
        ],
    );

    // ==========================================================================
    // CUSTODY VERBS - AGENT OVERRIDES
    // ==========================================================================
    m.insert(
        "cbu-custody.add-agent-override",
        vec![
            // Core patterns
            "add agent override",
            "settlement chain override",
            "reag override",
            "deag override",
            "intermediary override",
            "agent chain",
            // Question forms
            "how do i add agent override",
            "can i override agent",
            // Override patterns
            "receiving agent override",
            "delivering agent override",
            "intermediary agent",
            "correspondent override",
            "place of settlement override",
        ],
    );
    m.insert(
        "cbu-custody.list-agent-overrides",
        vec![
            // Core patterns
            "list agent overrides",
            "show overrides",
            "settlement chain overrides",
            // Question forms
            "what overrides exist",
            "show agent overrides",
            // Query patterns
            "override summary",
            "agent override list",
        ],
    );
    m.insert(
        "cbu-custody.remove-agent-override",
        vec![
            // Core patterns
            "remove agent override",
            "delete override",
            "clear override",
            // Question forms
            "how do i remove override",
            "can i delete override",
        ],
    );

    // ==========================================================================
    // CUSTODY VERBS - ANALYSIS
    // ==========================================================================
    m.insert(
        "cbu-custody.derive-required-coverage",
        vec![
            // Core patterns
            "derive required coverage",
            "what ssis needed",
            "coverage analysis",
            "ssi gap analysis",
            "what do we need",
            // Question forms
            "what ssis do we need",
            "where are the gaps",
            // Analysis patterns
            "coverage gaps",
            "missing ssis",
            "required accounts",
            "gap report",
        ],
    );
    m.insert(
        "cbu-custody.validate-booking-coverage",
        vec![
            // Core patterns
            "validate booking coverage",
            "check routing completeness",
            "booking gaps",
            "routing validation",
            "is routing complete",
            // Question forms
            "is routing set up correctly",
            "are there gaps",
            // Validation patterns
            "coverage check",
            "completeness check",
            "routing readiness",
        ],
    );

    // ==========================================================================
    // CUSTODY VERBS - SETTLEMENT EXTENSIONS
    // ==========================================================================
    m.insert(
        "cbu-custody.define-settlement-chain",
        vec![
            // Core patterns
            "define settlement chain",
            "settlement chain",
            "multi-hop settlement",
            "chain definition",
            "settlement path",
            "cross-border settlement",
            // Question forms
            "how do i define chain",
            "what is the settlement path",
            // Chain patterns
            "settlement network",
            "correspondent chain",
            "agent chain",
            "settlement route",
        ],
    );
    m.insert(
        "cbu-custody.list-settlement-chains",
        vec![
            // Core patterns
            "list settlement chains",
            "show chains",
            "settlement paths",
            // Question forms
            "what chains exist",
            "show settlement routes",
        ],
    );
    m.insert(
        "cbu-custody.set-fop-rules",
        vec![
            // Core patterns
            "set fop rules",
            "free of payment rules",
            "fop allowed",
            "fop threshold",
            "dvp vs fop",
            "fop configuration",
            // Question forms
            "how do i configure fop",
            "when is fop allowed",
            // Settlement type patterns
            "dvp only",
            "fop permitted",
            "fop threshold amount",
            "max fop value",
        ],
    );
    m.insert(
        "cbu-custody.list-fop-rules",
        vec![
            // Core patterns
            "list fop rules",
            "fop configuration",
            "show fop rules",
            // Question forms
            "what fop rules exist",
        ],
    );
    m.insert(
        "cbu-custody.set-csd-preference",
        vec![
            // Core patterns
            "set csd preference",
            "preferred csd",
            "euroclear preference",
            "clearstream preference",
            "dtcc preference",
            "icsd preference",
            // Question forms
            "which csd to use",
            "what is preferred csd",
            // CSD patterns
            "settle via euroclear",
            "settle via clearstream",
            "crest settlement",
            "dtc settlement",
        ],
    );
    m.insert(
        "cbu-custody.list-csd-preferences",
        vec![
            // Core patterns
            "list csd preferences",
            "csd configuration",
            "show csd preferences",
            // Question forms
            "what csd preferences",
        ],
    );
    m.insert(
        "cbu-custody.set-settlement-cycle",
        vec![
            // Core patterns
            "set settlement cycle",
            "settlement cycle override",
            "t+1",
            "t+2",
            "t+3",
            "settlement timing",
            // Question forms
            "what settlement cycle",
            "when does it settle",
            // Cycle patterns
            "value date offset",
            "same day settlement",
            "next day settlement",
            "standard settlement",
        ],
    );
    m.insert(
        "cbu-custody.list-settlement-cycle-overrides",
        vec![
            // Core patterns
            "list settlement cycles",
            "cycle overrides",
            "settlement timing config",
            // Question forms
            "what cycle overrides exist",
        ],
    );

    // ==========================================================================
    // ENTITY SETTLEMENT VERBS
    // ==========================================================================
    m.insert(
        "entity-settlement.set-identity",
        vec![
            // Core patterns
            "set settlement identity",
            "counterparty identity",
            "settlement bic",
            "alert participant",
            "ctm participant",
            "counterparty setup",
            // Question forms
            "how do i set identity",
            "what is their settlement id",
            // Identity patterns
            "broker identity",
            "dealer identity",
            "counterparty bic",
            "participant code",
        ],
    );
    m.insert(
        "entity-settlement.add-ssi",
        vec![
            // Core patterns
            "add counterparty ssi",
            "counterparty settlement",
            "their ssi",
            "broker ssi",
            "dealer ssi",
            // Question forms
            "what is their ssi",
            "add broker settlement",
            // SSI patterns
            "counterparty account",
            "trading counterparty ssi",
        ],
    );
    m.insert(
        "entity-settlement.remove-ssi",
        vec![
            // Core patterns
            "remove counterparty ssi",
            "delete their ssi",
            // Question forms
            "how do i remove ssi",
        ],
    );


    // ==========================================================================
    // PRICING CONFIG VERBS - Pricing & Valuation
    // ==========================================================================
    m.insert(
        "pricing-config.set",
        vec![
            // Core patterns
            "set pricing source",
            "pricing configuration",
            "valuation source",
            "bloomberg pricing",
            "reuters pricing",
            "price feed",
            "how to price",
            "pricing setup",
            // Question forms
            "what pricing source to use",
            "how do i configure pricing",
            "which price feed",
            // Provider patterns
            "bloomberg terminal",
            "refinitiv",
            "ice pricing",
            "markit pricing",
            "six telekurs",
            "data vendor",
            // Instrument patterns
            "price equities",
            "price bonds",
            "price derivatives",
            "fund nav source",
        ],
    );
    m.insert(
        "pricing-config.list",
        vec![
            // Core patterns
            "list pricing config",
            "show pricing sources",
            "pricing setup",
            // Question forms
            "what pricing sources exist",
            "show pricing configuration",
            // Query patterns
            "pricing summary",
            "price feed list",
        ],
    );
    m.insert(
        "pricing-config.remove",
        vec![
            // Core patterns
            "remove pricing config",
            "delete pricing source",
            // Question forms
            "how do i remove pricing",
            // Removal patterns
            "disable price feed",
            "stop pricing source",
        ],
    );
    m.insert(
        "pricing-config.find-for-instrument",
        vec![
            // Core patterns
            "find pricing source",
            "which pricing",
            "pricing for instrument",
            "resolve pricing",
            // Question forms
            "what pricing for this security",
            "which source for bond",
            // Resolution patterns
            "pricing lookup",
            "price source resolution",
            "applicable pricing",
        ],
    );
    m.insert(
        "pricing-config.link-resource",
        vec![
            // Core patterns
            "link pricing resource",
            "connect price feed",
            "pricing resource",
            // Question forms
            "how do i link pricing",
            // Connection patterns
            "pricing feed connection",
            "data vendor link",
        ],
    );
    m.insert(
        "pricing-config.set-valuation-schedule",
        vec![
            // Core patterns
            "set valuation schedule",
            "valuation frequency",
            "when to price",
            "eod pricing",
            "intraday pricing",
            "nav timing",
            // Question forms
            "when should we price",
            "what is valuation schedule",
            // Schedule patterns
            "daily pricing",
            "weekly pricing",
            "monthly nav",
            "valuation point",
            "cut off time",
        ],
    );
    m.insert(
        "pricing-config.list-valuation-schedules",
        vec![
            // Core patterns
            "list valuation schedules",
            "show pricing schedules",
            "valuation timing",
            // Question forms
            "what schedules exist",
        ],
    );
    m.insert(
        "pricing-config.set-fallback-chain",
        vec![
            // Core patterns
            "set fallback chain",
            "pricing fallback",
            "backup pricing source",
            "secondary pricing",
            "price fallback",
            // Question forms
            "what if primary fails",
            "backup pricing source",
            // Fallback patterns
            "fallback order",
            "price source priority",
            "secondary source",
        ],
    );
    m.insert(
        "pricing-config.set-stale-policy",
        vec![
            // Core patterns
            "set stale policy",
            "stale price handling",
            "old price policy",
            "price staleness",
            "max price age",
            // Question forms
            "how old can price be",
            "what is stale policy",
            // Policy patterns
            "stale price threshold",
            "price age limit",
            "escalate stale",
        ],
    );
    m.insert(
        "pricing-config.set-nav-threshold",
        vec![
            // Core patterns
            "set nav threshold",
            "nav impact alert",
            "price movement alert",
            "nav tolerance",
            // Question forms
            "what nav movement triggers alert",
            // Threshold patterns
            "nav change threshold",
            "price swing alert",
            "swing pricing trigger",
        ],
    );
    m.insert(
        "pricing-config.validate-pricing-config",
        vec![
            // Core patterns
            "validate pricing config",
            "pricing gaps",
            "check pricing setup",
            "pricing completeness",
            // Question forms
            "is pricing complete",
            "any pricing gaps",
            // Validation patterns
            "pricing readiness",
            "price config check",
        ],
    );

    // ==========================================================================
    // INSTRUCTION PROFILE VERBS - Message Templates
    // ==========================================================================
    m.insert(
        "instruction-profile.define-message-type",
        vec![
            // Core patterns
            "define message type",
            "new message type",
            "mt message",
            "mx message",
            "swift message type",
            "fix message",
            "instruction type",
            // Question forms
            "what message types exist",
            "how do i define message type",
            // Message standards
            "mt540",
            "mt541",
            "mt542",
            "mt543",
            "mt544",
            "mt545",
            "mt546",
            "mt547",
            "mt548",
            "sese.023",
            "sese.024",
            // Protocol patterns
            "iso 15022",
            "iso 20022",
            "fix protocol",
        ],
    );
    m.insert(
        "instruction-profile.list-message-types",
        vec![
            // Core patterns
            "list message types",
            "available messages",
            "swift messages",
            "message catalog",
            // Question forms
            "what message types available",
            "show message types",
        ],
    );
    m.insert(
        "instruction-profile.create-template",
        vec![
            // Core patterns
            "create instruction template",
            "new template",
            "message template",
            "swift template",
            "instruction format",
            // Question forms
            "how do i create template",
            "need new template",
            // Template patterns
            "template definition",
            "message format",
            "instruction pattern",
        ],
    );
    m.insert(
        "instruction-profile.read-template",
        vec![
            // Core patterns
            "read template",
            "show template",
            "template details",
            // Question forms
            "what is in template",
            "show template content",
        ],
    );
    m.insert(
        "instruction-profile.list-templates",
        vec![
            // Core patterns
            "list templates",
            "available templates",
            "message templates",
            // Question forms
            "what templates exist",
            "show all templates",
        ],
    );
    m.insert(
        "instruction-profile.assign-template",
        vec![
            // Core patterns
            "assign template",
            "map template",
            "which template",
            "template for instrument",
            "template assignment",
            "how to instruct",
            // Question forms
            "which template to use",
            "what template for this trade",
            // Assignment patterns
            "template mapping",
            "instruction mapping",
            "message selection",
        ],
    );
    m.insert(
        "instruction-profile.list-assignments",
        vec![
            // Core patterns
            "list template assignments",
            "show assignments",
            "template mappings",
            // Question forms
            "what assignments exist",
            "show template usage",
        ],
    );
    m.insert(
        "instruction-profile.remove-assignment",
        vec![
            // Core patterns
            "remove assignment",
            "unassign template",
            "delete assignment",
            // Question forms
            "how do i remove assignment",
        ],
    );
    m.insert(
        "instruction-profile.add-field-override",
        vec![
            // Core patterns
            "add field override",
            "override field",
            "custom field value",
            "field customization",
            "swift field override",
            "message field override",
            // Question forms
            "how do i override field",
            "can i customize field",
            // Override patterns
            "field 20c override",
            "sequence override",
            "block override",
        ],
    );
    m.insert(
        "instruction-profile.list-field-overrides",
        vec![
            // Core patterns
            "list field overrides",
            "show overrides",
            "field customizations",
            // Question forms
            "what overrides exist",
        ],
    );
    m.insert(
        "instruction-profile.remove-field-override",
        vec![
            // Core patterns
            "remove field override",
            "delete override",
            "clear override",
            // Question forms
            "how do i remove override",
        ],
    );
    m.insert(
        "instruction-profile.find-template",
        vec![
            // Core patterns
            "find template",
            "which template for trade",
            "resolve template",
            "template lookup",
            // Question forms
            "what template to use",
            "template for this",
            // Resolution patterns
            "template resolution",
            "template matching",
        ],
    );
    m.insert(
        "instruction-profile.validate-profile",
        vec![
            // Core patterns
            "validate instruction profile",
            "instruction gaps",
            "template coverage",
            "instruction completeness",
            // Question forms
            "is instruction profile complete",
            "any template gaps",
            // Validation patterns
            "profile readiness",
            "instruction check",
        ],
    );
    m.insert(
        "instruction-profile.derive-required-templates",
        vec![
            // Core patterns
            "derive required templates",
            "what templates needed",
            "template gap analysis",
            // Question forms
            "what templates do we need",
            "where are template gaps",
            // Analysis patterns
            "required templates",
            "missing templates",
        ],
    );

    // ==========================================================================
    // TRADE GATEWAY VERBS - Gateway Management
    // ==========================================================================
    m.insert(
        "trade-gateway.define-gateway",
        vec![
            // Core patterns
            "define gateway",
            "new gateway",
            "add gateway",
            "create gateway",
            "trade gateway",
            "swift gateway",
            "fix gateway",
            // Question forms
            "how do i add gateway",
            "what gateways available",
            // Gateway types
            "swift network",
            "fix connection",
            "alliance lite",
            "swift alliance",
        ],
    );
    m.insert(
        "trade-gateway.read-gateway",
        vec![
            // Core patterns
            "read gateway",
            "gateway details",
            "show gateway",
            // Question forms
            "what is gateway config",
        ],
    );
    m.insert(
        "trade-gateway.list-gateways",
        vec![
            // Core patterns
            "list gateways",
            "available gateways",
            "all gateways",
            "gateway catalog",
            // Question forms
            "what gateways exist",
            "show all gateways",
        ],
    );
    m.insert(
        "trade-gateway.enable-gateway",
        vec![
            // Core patterns
            "enable gateway",
            "connect gateway",
            "gateway connectivity",
            "activate gateway connection",
            "setup gateway",
            // Question forms
            "how do i enable gateway",
            "can i connect gateway",
        ],
    );
    m.insert(
        "trade-gateway.activate-gateway",
        vec![
            // Core patterns
            "activate gateway",
            "go live gateway",
            "gateway active",
            // Question forms
            "how do i activate",
            "make gateway live",
        ],
    );
    m.insert(
        "trade-gateway.suspend-gateway",
        vec![
            // Core patterns
            "suspend gateway",
            "disable gateway",
            "pause gateway",
            "gateway inactive",
            // Question forms
            "how do i suspend gateway",
        ],
    );
    m.insert(
        "trade-gateway.list-cbu-gateways",
        vec![
            // Core patterns
            "list cbu gateways",
            "cbu connectivity",
            "connected gateways",
            "gateway status",
            // Question forms
            "what gateways is cbu connected to",
        ],
    );
    m.insert(
        "trade-gateway.add-routing-rule",
        vec![
            // Core patterns
            "add gateway routing",
            "route to gateway",
            "gateway rule",
            "which gateway",
            "trade routing",
            "instruction routing",
            // Question forms
            "how do i route trades",
            "which gateway for this",
            // Routing patterns
            "gateway selection",
            "route by market",
            "route by instrument",
        ],
    );
    m.insert(
        "trade-gateway.list-routing-rules",
        vec![
            // Core patterns
            "list routing rules",
            "gateway routing",
            "show routing",
            // Question forms
            "what routing rules exist",
        ],
    );
    m.insert(
        "trade-gateway.remove-routing-rule",
        vec![
            // Core patterns
            "remove routing rule",
            "delete gateway route",
            // Question forms
            "how do i remove route",
        ],
    );
    m.insert(
        "trade-gateway.set-fallback",
        vec![
            // Core patterns
            "set gateway fallback",
            "fallback gateway",
            "backup gateway",
            "gateway failover",
            // Question forms
            "what if gateway fails",
            "backup gateway for",
            // Fallback patterns
            "failover configuration",
            "secondary gateway",
        ],
    );
    m.insert(
        "trade-gateway.list-fallbacks",
        vec![
            // Core patterns
            "list fallbacks",
            "gateway fallbacks",
            "failover config",
            // Question forms
            "what fallbacks exist",
        ],
    );
    m.insert(
        "trade-gateway.find-gateway",
        vec![
            // Core patterns
            "find gateway",
            "which gateway for trade",
            "resolve gateway",
            "gateway lookup",
            // Question forms
            "what gateway to use",
            "gateway for this trade",
        ],
    );
    m.insert(
        "trade-gateway.validate-routing",
        vec![
            // Core patterns
            "validate gateway routing",
            "routing gaps",
            "gateway coverage",
            "routing completeness",
            // Question forms
            "is routing complete",
            "any routing gaps",
        ],
    );
    m.insert(
        "trade-gateway.derive-required-routes",
        vec![
            // Core patterns
            "derive required routes",
            "what routes needed",
            "routing gap analysis",
            // Question forms
            "what routes do we need",
            "where are routing gaps",
        ],
    );

    // ==========================================================================
    // CORPORATE ACTION VERBS - CA Configuration
    // ==========================================================================
    m.insert(
        "corporate-action.define-event-type",
        vec![
            // Core patterns
            "define ca event",
            "corporate action type",
            "new event type",
            "ca event definition",
            // Question forms
            "what ca types exist",
            // Event types
            "dividend event",
            "stock split",
            "rights issue",
            "merger",
            "tender offer",
            "spin off",
            "bonus issue",
        ],
    );
    m.insert(
        "corporate-action.list-event-types",
        vec![
            // Core patterns
            "list ca events",
            "corporate action types",
            "event catalog",
            "ca types",
            // Question forms
            "what event types exist",
        ],
    );
    m.insert(
        "corporate-action.set-preferences",
        vec![
            // Core patterns
            "set ca preferences",
            "corporate action preferences",
            "ca processing mode",
            "how to handle ca",
            "dividend preferences",
            "auto instruct ca",
            "manual ca",
            // Question forms
            "how should we handle ca",
            "ca processing preference",
            // Preference patterns
            "reinvest dividends",
            "cash dividends",
            "drip",
            "standing instruction",
        ],
    );
    m.insert(
        "corporate-action.list-preferences",
        vec![
            // Core patterns
            "list ca preferences",
            "show ca config",
            "ca setup",
            // Question forms
            "what preferences exist",
        ],
    );
    m.insert(
        "corporate-action.remove-preference",
        vec![
            // Core patterns
            "remove ca preference",
            "delete ca config",
            // Question forms
            "how do i remove preference",
        ],
    );
    m.insert(
        "corporate-action.set-instruction-window",
        vec![
            // Core patterns
            "set instruction window",
            "ca deadline",
            "response window",
            "ca cutoff",
            "instruction deadline",
            "ca timing",
            // Question forms
            "when is ca deadline",
            "how long to respond",
            // Window patterns
            "early deadline",
            "late deadline",
            "protected deadline",
            "market deadline",
        ],
    );
    m.insert(
        "corporate-action.list-instruction-windows",
        vec![
            // Core patterns
            "list instruction windows",
            "ca deadlines",
            "response deadlines",
            // Question forms
            "what deadlines exist",
        ],
    );
    m.insert(
        "corporate-action.link-ca-ssi",
        vec![
            // Core patterns
            "link ca ssi",
            "ca payment ssi",
            "dividend ssi",
            "ca proceeds account",
            "where to receive ca",
            // Question forms
            "where should ca proceeds go",
            "dividend payment account",
            // SSI patterns
            "ca payment routing",
            "dividend routing",
            "proceeds account",
        ],
    );
    m.insert(
        "corporate-action.list-ca-ssi-mappings",
        vec![
            // Core patterns
            "list ca ssi mappings",
            "ca payment accounts",
            "dividend accounts",
            // Question forms
            "what ca accounts exist",
        ],
    );
    m.insert(
        "corporate-action.validate-ca-config",
        vec![
            // Core patterns
            "validate ca config",
            "ca gaps",
            "corporate action completeness",
            "ca readiness",
            // Question forms
            "is ca config complete",
            "any ca gaps",
        ],
    );
    m.insert(
        "corporate-action.derive-required-config",
        vec![
            // Core patterns
            "derive ca config",
            "what ca config needed",
            "ca gap analysis",
            // Question forms
            "what ca config do we need",
        ],
    );


    // ==========================================================================
    // TAX CONFIG VERBS - Tax & Withholding
    // ==========================================================================
    m.insert(
        "tax-config.set-withholding-profile",
        vec![
            // Core patterns
            "set withholding profile",
            "withholding tax",
            "tax rate",
            "treaty rate",
            "statutory rate",
            "qi status",
            "nqi status",
            "tax setup",
            // Question forms
            "what tax rate applies",
            "how do i set withholding",
            // Tax patterns
            "dividend withholding",
            "interest withholding",
            "royalty withholding",
            "us withholding",
            "eu withholding",
        ],
    );
    m.insert(
        "tax-config.list-withholding-profiles",
        vec![
            // Core patterns
            "list withholding profiles",
            "tax configuration",
            "withholding rates",
            // Question forms
            "what profiles exist",
            "show tax setup",
        ],
    );
    m.insert(
        "tax-config.set-reclaim-preferences",
        vec![
            // Core patterns
            "set reclaim preferences",
            "tax reclaim",
            "reclaim method",
            "quick refund",
            "standard reclaim",
            "tax recovery",
            // Question forms
            "how to reclaim tax",
            "what reclaim method",
            // Reclaim patterns
            "relief at source",
            "ras",
            "long form reclaim",
            "short form reclaim",
            "dtr",
            "double tax relief",
        ],
    );
    m.insert(
        "tax-config.list-reclaim-preferences",
        vec![
            // Core patterns
            "list reclaim preferences",
            "reclaim configuration",
            "tax recovery setup",
            // Question forms
            "what reclaim preferences exist",
        ],
    );
    m.insert(
        "tax-config.link-tax-documentation",
        vec![
            // Core patterns
            "link tax documentation",
            "tax docs",
            "w8 form",
            "w9 form",
            "crs form",
            "fatca form",
            "tax residency certificate",
            "beneficial owner certificate",
            // Question forms
            "what tax docs needed",
            "how do i add tax docs",
            // Document patterns
            "w8ben",
            "w8bene",
            "w8imy",
            "w9",
            "self certification",
            "tax reclaim form",
            "power of attorney",
        ],
    );
    m.insert(
        "tax-config.list-tax-documentation",
        vec![
            // Core patterns
            "list tax documentation",
            "tax docs status",
            "expiring tax docs",
            // Question forms
            "what tax docs exist",
            "show tax documentation",
        ],
    );
    m.insert(
        "tax-config.set-rate-override",
        vec![
            // Core patterns
            "set tax rate override",
            "override withholding",
            "custom tax rate",
            "tax exception",
            // Question forms
            "how do i override rate",
            "can i change tax rate",
            // Override patterns
            "reduced rate",
            "exempt rate",
            "treaty override",
            "special rate",
        ],
    );
    m.insert(
        "tax-config.list-rate-overrides",
        vec![
            // Core patterns
            "list rate overrides",
            "tax exceptions",
            "custom rates",
            // Question forms
            "what overrides exist",
        ],
    );
    m.insert(
        "tax-config.validate-tax-config",
        vec![
            // Core patterns
            "validate tax config",
            "tax gaps",
            "tax completeness",
            "tax readiness",
            "expiring documentation",
            // Question forms
            "is tax config complete",
            "any tax gaps",
        ],
    );
    m.insert(
        "tax-config.find-withholding-rate",
        vec![
            // Core patterns
            "find withholding rate",
            "which tax rate",
            "applicable rate",
            "tax rate lookup",
            // Question forms
            "what rate applies",
            "rate for this security",
        ],
    );

    // ==========================================================================
    // TRADING PROFILE VERBS - Profile Management
    // ==========================================================================
    m.insert(
        "trading-profile.import",
        vec![
            // Core patterns
            "import trading profile",
            "load trading matrix",
            "upload trading config",
            "trading profile yaml",
            // Question forms
            "how do i import profile",
            "can i upload trading config",
            // Import patterns
            "profile upload",
            "matrix import",
            "config file",
            "yaml upload",
        ],
    );
    m.insert(
        "trading-profile.read",
        vec![
            // Core patterns
            "read trading profile",
            "show trading matrix",
            "trading configuration",
            // Question forms
            "what is the profile",
            "show me trading config",
        ],
    );
    m.insert(
        "trading-profile.get-active",
        vec![
            // Core patterns
            "get active profile",
            "current trading profile",
            "active trading matrix",
            // Question forms
            "what profile is active",
            "current config",
        ],
    );
    m.insert(
        "trading-profile.list-versions",
        vec![
            // Core patterns
            "list profile versions",
            "trading profile history",
            "version history",
            // Question forms
            "what versions exist",
            "show version history",
        ],
    );
    m.insert(
        "trading-profile.activate",
        vec![
            // Core patterns
            "activate profile",
            "enable trading profile",
            "make profile active",
            // Question forms
            "how do i activate profile",
            "can i enable profile",
        ],
    );
    m.insert(
        "trading-profile.materialize",
        vec![
            // Core patterns
            "materialize profile",
            "sync profile to tables",
            "apply trading profile",
            "deploy profile",
            // Question forms
            "how do i apply profile",
            "sync profile now",
        ],
    );
    m.insert(
        "trading-profile.validate",
        vec![
            // Core patterns
            "validate profile",
            "check trading profile",
            "profile validation",
            // Question forms
            "is profile valid",
            "any profile errors",
        ],
    );
    m.insert(
        "trading-profile.diff",
        vec![
            // Core patterns
            "diff profiles",
            "compare profiles",
            "profile changes",
            "what changed",
            // Question forms
            "what is different",
            "compare versions",
        ],
    );
    m.insert(
        "trading-profile.export",
        vec![
            // Core patterns
            "export profile",
            "download trading matrix",
            "profile yaml",
            // Question forms
            "how do i export profile",
            "download config",
        ],
    );
    m.insert(
        "trading-profile.export-full-matrix",
        vec![
            // Core patterns
            "export full matrix",
            "complete trading matrix",
            "full trading document",
            "comprehensive matrix export",
            "matrix document",
            // Question forms
            "how do i get full matrix",
            "export everything",
            // Export patterns
            "full config export",
            "complete export",
        ],
    );
    m.insert(
        "trading-profile.validate-matrix-completeness",
        vec![
            // Core patterns
            "validate matrix completeness",
            "matrix ready",
            "go live check",
            "trading readiness",
            "matrix gaps",
            // Question forms
            "is matrix complete",
            "ready for go live",
            "any gaps",
        ],
    );
    m.insert(
        "trading-profile.generate-gap-remediation-plan",
        vec![
            // Core patterns
            "generate remediation plan",
            "fix matrix gaps",
            "gap remediation",
            "what to fix",
            // Question forms
            "how do i fix gaps",
            "what needs to be done",
            // Remediation patterns
            "action plan",
            "gap resolution",
            "fix list",
        ],
    );

    // ==========================================================================
    // LIFECYCLE VERBS - Lifecycle Management
    // ==========================================================================
    m.insert(
        "lifecycle.read",
        vec![
            // Core patterns
            "read lifecycle",
            "lifecycle definition",
            "lifecycle details",
            // Question forms
            "what is the lifecycle",
            "show lifecycle",
        ],
    );
    m.insert(
        "lifecycle.list",
        vec![
            // Core patterns
            "list lifecycles",
            "available lifecycles",
            "all lifecycles",
            // Question forms
            "what lifecycles exist",
        ],
    );
    m.insert(
        "lifecycle.list-by-instrument",
        vec![
            // Core patterns
            "lifecycles for instrument",
            "instrument lifecycles",
            "what lifecycles apply",
            // Question forms
            "what lifecycles for this security",
        ],
    );
    m.insert(
        "lifecycle.discover",
        vec![
            // Core patterns
            "discover lifecycle",
            "lifecycle discovery",
            "find lifecycles",
            "applicable lifecycles",
            // Question forms
            "what lifecycles should we have",
        ],
    );
    m.insert(
        "lifecycle.provision",
        vec![
            // Core patterns
            "provision lifecycle resource",
            "setup lifecycle",
            "enable lifecycle",
            "lifecycle provisioning",
            // Question forms
            "how do i provision lifecycle",
        ],
    );
    m.insert(
        "lifecycle.activate",
        vec![
            // Core patterns
            "activate lifecycle",
            "go live lifecycle",
            "lifecycle active",
            // Question forms
            "how do i activate lifecycle",
        ],
    );
    m.insert(
        "lifecycle.suspend",
        vec![
            // Core patterns
            "suspend lifecycle",
            "pause lifecycle",
            "lifecycle inactive",
            // Question forms
            "how do i suspend lifecycle",
        ],
    );
    m.insert(
        "lifecycle.analyze-gaps",
        vec![
            // Core patterns
            "analyze lifecycle gaps",
            "lifecycle gap analysis",
            "missing lifecycles",
            "lifecycle readiness",
            // Question forms
            "what lifecycles are missing",
            "any lifecycle gaps",
        ],
    );
    m.insert(
        "lifecycle.check-readiness",
        vec![
            // Core patterns
            "check lifecycle readiness",
            "lifecycle ready",
            "can go live",
            // Question forms
            "is lifecycle ready",
            "ready for go live",
        ],
    );
    m.insert(
        "lifecycle.generate-plan",
        vec![
            // Core patterns
            "generate lifecycle plan",
            "lifecycle remediation",
            "fix lifecycle gaps",
            // Question forms
            "how do i fix lifecycle gaps",
        ],
    );
    m.insert(
        "lifecycle.visualize-coverage",
        vec![
            // Core patterns
            "visualize lifecycle coverage",
            "lifecycle diagram",
            "lifecycle coverage chart",
            // Question forms
            "show me lifecycle coverage",
        ],
    );

    // ==========================================================================
    // ISDA VERBS - Derivatives Agreements
    // ==========================================================================
    m.insert(
        "isda.create",
        vec![
            // Core patterns
            "create isda",
            "new isda agreement",
            "isda master",
            "derivatives agreement",
            "otc agreement",
            // Question forms
            "how do i add isda",
            "need isda agreement",
            // Agreement patterns
            "isda 2002",
            "isda master agreement",
            "otc derivatives",
            "swap agreement",
        ],
    );
    m.insert(
        "isda.add-coverage",
        vec![
            // Core patterns
            "add isda coverage",
            "covered instruments",
            "isda instrument class",
            "derivatives coverage",
            // Question forms
            "what instruments covered",
            "add coverage for",
            // Coverage patterns
            "irs coverage",
            "cds coverage",
            "fx coverage",
            "equity derivatives",
        ],
    );
    m.insert(
        "isda.remove-coverage",
        vec![
            // Core patterns
            "remove isda coverage",
            "uncovered instruments",
            // Question forms
            "how do i remove coverage",
        ],
    );
    m.insert(
        "isda.add-csa",
        vec![
            // Core patterns
            "add csa",
            "credit support annex",
            "collateral agreement",
            "margin agreement",
            "vm csa",
            "im csa",
            // Question forms
            "how do i add csa",
            "need csa",
            // CSA patterns
            "variation margin csa",
            "initial margin csa",
            "collateral terms",
            "margin call",
        ],
    );
    m.insert("isda.remove-csa", vec!["remove csa", "delete csa"]);
    m.insert(
        "isda.list",
        vec![
            // Core patterns
            "list isda",
            "show isda agreements",
            "all isda",
            "derivatives agreements",
            // Question forms
            "what isda agreements exist",
        ],
    );

    // ==========================================================================
    // MATRIX OVERLAY VERBS - Product Overlays
    // ==========================================================================
    m.insert(
        "matrix-overlay.subscribe",
        vec![
            // Core patterns
            "subscribe to overlay",
            "product overlay",
            "add product to matrix",
            "enable product overlay",
            // Question forms
            "how do i add overlay",
        ],
    );
    m.insert(
        "matrix-overlay.unsubscribe",
        vec![
            // Core patterns
            "unsubscribe overlay",
            "remove product overlay",
            "disable overlay",
            // Question forms
            "how do i remove overlay",
        ],
    );
    m.insert(
        "matrix-overlay.add",
        vec![
            // Core patterns
            "add overlay",
            "create overlay",
            "new matrix overlay",
            // Question forms
            "how do i create overlay",
        ],
    );
    m.insert(
        "matrix-overlay.list",
        vec![
            // Core patterns
            "list overlays",
            "show overlays",
            "product overlays",
            // Question forms
            "what overlays exist",
        ],
    );
    m.insert(
        "matrix-overlay.effective-matrix",
        vec![
            // Core patterns
            "effective matrix",
            "computed matrix",
            "merged matrix",
            "final matrix",
            // Question forms
            "what is effective matrix",
        ],
    );
    m.insert(
        "matrix-overlay.unified-gaps",
        vec![
            // Core patterns
            "unified gaps",
            "all gaps",
            "combined gap analysis",
            "matrix gaps",
            // Question forms
            "what are all gaps",
        ],
    );
    m.insert(
        "matrix-overlay.compare-products",
        vec![
            // Core patterns
            "compare products",
            "product comparison",
            "product differences",
            // Question forms
            "how do products differ",
        ],
    );

    // ==========================================================================
    // TEMPORAL VERBS - Point-in-Time Queries
    // ==========================================================================
    m.insert(
        "temporal.ownership-as-of",
        vec![
            // Core patterns
            "ownership as of",
            "historical ownership",
            "ownership on date",
            "past ownership",
            "point in time ownership",
            // Question forms
            "who owned on date",
            "ownership at time",
            // Historical patterns
            "ownership snapshot",
            "ownership history",
            "backdated ownership",
        ],
    );
    m.insert(
        "temporal.ubo-chain-as-of",
        vec![
            // Core patterns
            "ubo chain as of",
            "historical ubo",
            "ubo on date",
            "past ubo structure",
            // Question forms
            "who were ubos on date",
        ],
    );
    m.insert(
        "temporal.cbu-relationships-as-of",
        vec![
            // Core patterns
            "cbu relationships as of",
            "historical relationships",
            "relationships on date",
            // Question forms
            "who was related on date",
        ],
    );
    m.insert(
        "temporal.cbu-roles-as-of",
        vec![
            // Core patterns
            "roles as of",
            "historical roles",
            "roles on date",
            // Question forms
            "who held role on date",
        ],
    );
    m.insert(
        "temporal.cbu-state-at-approval",
        vec![
            // Core patterns
            "state at approval",
            "snapshot at approval",
            "what was approved",
            // Question forms
            "what did we approve",
        ],
    );
    m.insert(
        "temporal.relationship-history",
        vec![
            // Core patterns
            "relationship history",
            "audit trail",
            "change history",
            // Question forms
            "what changed",
            "show history",
        ],
    );
    m.insert(
        "temporal.entity-history",
        vec![
            // Core patterns
            "entity history",
            "entity changes",
            "entity audit",
            // Question forms
            "what changed on entity",
        ],
    );
    m.insert(
        "temporal.compare-ownership",
        vec![
            // Core patterns
            "compare ownership",
            "ownership diff",
            "what changed between dates",
            // Question forms
            "how did ownership change",
        ],
    );


    // ==========================================================================
    // TEAM VERBS - Team Management
    // ==========================================================================
    m.insert(
        "team.create",
        vec![
            // Core patterns
            "create team",
            "new team",
            "add team",
            "setup team",
            // Question forms
            "how do i create team",
            "can i add team",
            // Team patterns
            "establish team",
            "form team",
        ],
    );
    m.insert("team.read", vec!["read team", "team details", "show team"]);
    m.insert(
        "team.archive",
        vec![
            // Core patterns
            "archive team",
            "deactivate team",
            "remove team",
            // Question forms
            "how do i remove team",
        ],
    );
    m.insert(
        "team.add-member",
        vec![
            // Core patterns
            "add team member",
            "add user to team",
            "join team",
            "team membership",
            // Question forms
            "how do i add member",
            "can i add to team",
        ],
    );
    m.insert(
        "team.remove-member",
        vec![
            // Core patterns
            "remove team member",
            "leave team",
            "remove from team",
            // Question forms
            "how do i remove member",
        ],
    );
    m.insert(
        "team.update-member",
        vec![
            // Core patterns
            "update member",
            "change member role",
            "modify membership",
            // Question forms
            "how do i change member",
        ],
    );
    m.insert(
        "team.transfer-member",
        vec![
            // Core patterns
            "transfer member",
            "move to team",
            "reassign to team",
            // Question forms
            "how do i transfer member",
        ],
    );
    m.insert(
        "team.add-governance-member",
        vec![
            // Core patterns
            "add governance member",
            "board member",
            "committee member",
            // Question forms
            "how do i add governance",
        ],
    );
    m.insert(
        "team.verify-governance-access",
        vec![
            // Core patterns
            "verify governance access",
            "audit governance",
            "governance check",
            // Question forms
            "who has governance access",
        ],
    );
    m.insert(
        "team.add-cbu-access",
        vec![
            // Core patterns
            "add cbu access",
            "team cbu access",
            "grant access",
            // Question forms
            "how do i grant access",
        ],
    );
    m.insert(
        "team.remove-cbu-access",
        vec![
            // Core patterns
            "remove cbu access",
            "revoke access",
            // Question forms
            "how do i revoke access",
        ],
    );
    m.insert(
        "team.grant-service",
        vec![
            // Core patterns
            "grant service",
            "team entitlement",
            "enable service",
            // Question forms
            "how do i grant service",
        ],
    );
    m.insert(
        "team.revoke-service",
        vec![
            // Core patterns
            "revoke service",
            "remove entitlement",
            "disable service",
            // Question forms
            "how do i revoke service",
        ],
    );
    m.insert(
        "team.list-members",
        vec![
            // Core patterns
            "list team members",
            "team roster",
            "who is on team",
            // Question forms
            "who is on this team",
        ],
    );
    m.insert(
        "team.list-cbus",
        vec![
            // Core patterns
            "list team cbus",
            "team access",
            "cbus for team",
            // Question forms
            "what cbus can team access",
        ],
    );

    // ==========================================================================
    // USER VERBS - User Management
    // ==========================================================================
    m.insert(
        "user.create",
        vec![
            // Core patterns
            "create user",
            "new user",
            "add user",
            "register user",
            // Question forms
            "how do i add user",
            "can i create user",
        ],
    );
    m.insert(
        "user.suspend",
        vec![
            // Core patterns
            "suspend user",
            "disable user",
            "deactivate user",
            // Question forms
            "how do i suspend user",
        ],
    );
    m.insert(
        "user.reactivate",
        vec![
            // Core patterns
            "reactivate user",
            "enable user",
            "activate user",
            // Question forms
            "how do i reactivate user",
        ],
    );
    m.insert(
        "user.offboard",
        vec![
            // Core patterns
            "offboard user",
            "terminate user",
            "user left company",
            // Question forms
            "how do i offboard user",
            // Offboarding patterns
            "user leaving",
            "remove access",
            "user departed",
        ],
    );
    m.insert(
        "user.list-teams",
        vec![
            // Core patterns
            "user teams",
            "which teams",
            "team membership",
            // Question forms
            "what teams is user on",
        ],
    );
    m.insert(
        "user.list-cbus",
        vec![
            // Core patterns
            "user cbus",
            "user access",
            "what can user access",
            // Question forms
            "what cbus can user see",
        ],
    );
    m.insert(
        "user.check-access",
        vec![
            // Core patterns
            "check user access",
            "can user access",
            "access check",
            // Question forms
            "does user have access",
        ],
    );

    // ==========================================================================
    // SLA VERBS - Service Level Agreement Management
    // ==========================================================================
    m.insert(
        "sla.list-templates",
        vec![
            // Core patterns
            "list sla templates",
            "sla catalog",
            "available slas",
            // Question forms
            "what sla templates exist",
        ],
    );
    m.insert(
        "sla.read-template",
        vec![
            // Core patterns
            "read sla template",
            "sla details",
            "template details",
            // Question forms
            "what is in sla",
        ],
    );
    m.insert(
        "sla.commit",
        vec![
            // Core patterns
            "commit sla",
            "sla commitment",
            "agree to sla",
            "sla agreement",
            // Question forms
            "how do i commit to sla",
        ],
    );
    m.insert(
        "sla.bind-to-profile",
        vec![
            // Core patterns
            "bind sla to profile",
            "sla for profile",
            "link sla",
            // Question forms
            "how do i bind sla",
        ],
    );
    m.insert(
        "sla.bind-to-service",
        vec![
            // Core patterns
            "bind sla to service",
            "service sla",
            // Question forms
            "how do i add sla to service",
        ],
    );
    m.insert(
        "sla.bind-to-resource",
        vec![
            // Core patterns
            "bind sla to resource",
            "resource sla",
            // Question forms
            "how do i add sla to resource",
        ],
    );
    m.insert("sla.bind-to-isda", vec!["bind sla to isda", "isda sla"]);
    m.insert("sla.bind-to-csa", vec!["bind sla to csa", "csa sla"]);
    m.insert(
        "sla.list-commitments",
        vec![
            // Core patterns
            "list sla commitments",
            "cbu slas",
            "all commitments",
            // Question forms
            "what slas are committed",
        ],
    );
    m.insert(
        "sla.suspend-commitment",
        vec![
            // Core patterns
            "suspend sla",
            "pause commitment",
            // Question forms
            "how do i suspend sla",
        ],
    );
    m.insert(
        "sla.record-measurement",
        vec![
            // Core patterns
            "record sla measurement",
            "sla metric",
            "measure sla",
            // Question forms
            "how do i record measurement",
        ],
    );
    m.insert(
        "sla.list-measurements",
        vec![
            // Core patterns
            "list sla measurements",
            "sla history",
            "measurement history",
            // Question forms
            "what measurements exist",
        ],
    );
    m.insert(
        "sla.report-breach",
        vec![
            // Core patterns
            "report sla breach",
            "sla violation",
            "sla failure",
            // Question forms
            "how do i report breach",
            // Breach patterns
            "sla missed",
            "sla not met",
        ],
    );
    m.insert(
        "sla.update-remediation",
        vec![
            // Core patterns
            "update remediation",
            "breach remediation",
            "fix sla breach",
            // Question forms
            "how do i update remediation",
        ],
    );
    m.insert(
        "sla.resolve-breach",
        vec![
            // Core patterns
            "resolve breach",
            "breach resolved",
            "sla fixed",
            // Question forms
            "how do i resolve breach",
        ],
    );
    m.insert(
        "sla.escalate-breach",
        vec![
            // Core patterns
            "escalate breach",
            "sla escalation",
            // Question forms
            "how do i escalate breach",
        ],
    );
    m.insert(
        "sla.list-open-breaches",
        vec![
            // Core patterns
            "list open breaches",
            "active breaches",
            "unresolved slas",
            // Question forms
            "what breaches are open",
        ],
    );

    // ==========================================================================
    // REGULATORY VERBS - Regulatory Registration
    // ==========================================================================
    m.insert(
        "regulatory.registration.add",
        vec![
            // Core patterns
            "add regulatory registration",
            "register with regulator",
            "fca registration",
            "sec registration",
            "regulatory license",
            // Question forms
            "how do i add registration",
            // Regulators
            "fca regulated",
            "sec regulated",
            "cssf regulated",
            "cbi regulated",
            "finma regulated",
            "bafin regulated",
            "amf regulated",
        ],
    );
    m.insert(
        "regulatory.registration.list",
        vec![
            // Core patterns
            "list registrations",
            "regulatory status",
            "all registrations",
            // Question forms
            "what registrations exist",
        ],
    );
    m.insert(
        "regulatory.registration.verify",
        vec![
            // Core patterns
            "verify registration",
            "check registration",
            "registration verification",
            // Question forms
            "is registration valid",
        ],
    );
    m.insert(
        "regulatory.registration.remove",
        vec![
            // Core patterns
            "remove registration",
            "withdraw registration",
            "deregister",
            // Question forms
            "how do i remove registration",
        ],
    );
    m.insert(
        "regulatory.status.check",
        vec![
            // Core patterns
            "check regulatory status",
            "is regulated",
            "regulatory check",
            // Question forms
            "is entity regulated",
        ],
    );

    // ==========================================================================
    // SEMANTIC VERBS - Stage & Progress Tracking
    // ==========================================================================
    m.insert(
        "semantic.get-state",
        vec![
            // Core patterns
            "get semantic state",
            "onboarding progress",
            "where are we",
            "stage progress",
            // Question forms
            "what stage are we at",
            "how far along",
        ],
    );
    m.insert(
        "semantic.list-stages",
        vec![
            // Core patterns
            "list stages",
            "all stages",
            "stage definitions",
            // Question forms
            "what stages exist",
        ],
    );
    m.insert(
        "semantic.stages-for-product",
        vec![
            // Core patterns
            "stages for product",
            "product stages",
            "required stages",
            // Question forms
            "what stages for this product",
        ],
    );
    m.insert(
        "semantic.next-actions",
        vec![
            // Core patterns
            "next actions",
            "what to do next",
            "suggested actions",
            "actionable stages",
            // Question forms
            "what should we do next",
            "what is recommended",
        ],
    );
    m.insert(
        "semantic.missing-entities",
        vec![
            // Core patterns
            "missing entities",
            "what is missing",
            "gaps in structure",
            // Question forms
            "what entities are missing",
        ],
    );
    m.insert(
        "semantic.prompt-context",
        vec![
            // Core patterns
            "prompt context",
            "agent context",
            "session context",
            // Question forms
            "what is the context",
        ],
    );

    // ==========================================================================
    // CASH SWEEP VERBS - Cash Management
    // ==========================================================================
    m.insert(
        "cash-sweep.configure",
        vec![
            // Core patterns
            "configure cash sweep",
            "setup sweep",
            "stif configuration",
            "cash management",
            // Question forms
            "how do i configure sweep",
            // Sweep patterns
            "money market sweep",
            "overnight sweep",
            "target balance sweep",
        ],
    );
    m.insert(
        "cash-sweep.link-resource",
        vec![
            // Core patterns
            "link sweep resource",
            "sweep account",
            // Question forms
            "how do i link sweep",
        ],
    );
    m.insert(
        "cash-sweep.list",
        vec![
            // Core patterns
            "list cash sweeps",
            "sweep configuration",
            "all sweeps",
            // Question forms
            "what sweeps exist",
        ],
    );
    m.insert(
        "cash-sweep.update-threshold",
        vec![
            // Core patterns
            "update sweep threshold",
            "change threshold",
            // Question forms
            "how do i change threshold",
        ],
    );
    m.insert(
        "cash-sweep.update-timing",
        vec![
            // Core patterns
            "update sweep timing",
            "change sweep time",
            // Question forms
            "when does sweep run",
        ],
    );
    m.insert(
        "cash-sweep.change-vehicle",
        vec![
            // Core patterns
            "change sweep vehicle",
            "different stif",
            "change mmf",
            // Question forms
            "how do i change vehicle",
        ],
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
    // INVESTMENT MANAGER VERBS - IM Management
    // ==========================================================================
    m.insert(
        "investment-manager.assign",
        vec![
            // Core patterns
            "assign investment manager",
            "add im",
            "investment manager setup",
            "appoint im",
            // Question forms
            "how do i add im",
        ],
    );
    m.insert(
        "investment-manager.set-scope",
        vec![
            // Core patterns
            "set im scope",
            "im trading scope",
            "im permissions",
            // Question forms
            "what can im trade",
        ],
    );
    m.insert(
        "investment-manager.link-connectivity",
        vec![
            // Core patterns
            "link im connectivity",
            "im instruction method",
            "how im sends trades",
            // Question forms
            "how does im connect",
        ],
    );
    m.insert(
        "investment-manager.list",
        vec![
            // Core patterns
            "list investment managers",
            "all ims",
            "im assignments",
            // Question forms
            "what ims exist",
        ],
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
    // FUND INVESTOR VERBS - Investor Management
    // ==========================================================================
    m.insert(
        "fund-investor.create",
        vec![
            // Core patterns
            "create fund investor",
            "register investor",
            "new investor",
            "add investor to fund",
            // Question forms
            "how do i add investor",
        ],
    );
    m.insert(
        "fund-investor.list",
        vec![
            // Core patterns
            "list fund investors",
            "all investors",
            "investor list",
            // Question forms
            "who are the investors",
        ],
    );
    m.insert(
        "fund-investor.update-kyc-status",
        vec!["update investor kyc", "investor kyc status"],
    );
    m.insert("fund-investor.get", vec!["get investor", "investor details"]);

    // ==========================================================================
    // DELEGATION VERBS - Delegation Management
    // ==========================================================================
    m.insert(
        "delegation.add",
        vec![
            // Core patterns
            "add delegation",
            "delegate to",
            "sub-advisor",
            "outsource to",
            "delegation chain",
            // Question forms
            "how do i delegate",
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
    // DELIVERY VERBS - Service Delivery Tracking
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
    // SERVICE RESOURCE VERBS - Resource Provisioning
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
    // CLIENT PORTAL VERBS - Client Self-Service
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
    // BATCH VERBS - Batch Processing
    // ==========================================================================
    m.insert("batch.pause", vec!["pause batch", "stop batch", "batch pause"]);
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
    m.insert("batch.abort", vec!["abort batch", "cancel batch", "stop all"]);
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
    // KYC AGREEMENT VERBS - Agreement Management
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
    // KYC SCOPE VERBS - Scope Management
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
    // REQUEST VERBS - Request Management
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
    m.insert("request.remind", vec!["remind", "send reminder", "follow up"]);
    m.insert(
        "request.escalate",
        vec!["escalate request", "bump request", "urgent request"],
    );
    m.insert(
        "request.waive",
        vec!["waive request", "not needed", "skip request"],
    );

    // ==========================================================================
    // CASE EVENT VERBS - Case Audit Trail
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
    // OBSERVATION VERBS - Data Observation & Recording
    // ==========================================================================
    m.insert(
        "observation.record",
        vec![
            // Core patterns
            "record observation",
            "capture observation",
            "observe attribute",
            "attribute observation",
            // Question forms
            "how do i record observation",
            // Recording patterns
            "record fact",
            "capture data",
            "log observation",
        ],
    );
    m.insert(
        "observation.record-from-document",
        vec![
            // Core patterns
            "observation from document",
            "extract observation",
            "document observation",
            // Question forms
            "how do i extract from doc",
        ],
    );
    m.insert(
        "observation.supersede",
        vec![
            // Core patterns
            "supersede observation",
            "replace observation",
            "newer observation",
            // Question forms
            "how do i replace observation",
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
    // ALLEGATION VERBS - Client Claims & Verification
    // ==========================================================================
    m.insert(
        "allegation.record",
        vec![
            // Core patterns
            "record allegation",
            "client claim",
            "alleged value",
            "client says",
            // Question forms
            "how do i record claim",
        ],
    );
    m.insert(
        "allegation.verify",
        vec![
            "verify allegation",
            "confirm claim",
            "allegation verified",
        ],
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
    // DISCREPANCY VERBS - Data Conflict Management
    // ==========================================================================
    m.insert(
        "discrepancy.record",
        vec![
            // Core patterns
            "record discrepancy",
            "data conflict",
            "observation conflict",
            "mismatch",
            // Question forms
            "how do i record discrepancy",
            // Conflict patterns
            "data mismatch",
            "inconsistent data",
            "conflicting information",
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
    // VERIFICATION VERBS - Adversarial Verification
    // ==========================================================================
    m.insert(
        "verify.detect-patterns",
        vec![
            // Core patterns
            "detect patterns",
            "find red flags",
            "circular ownership",
            "layering detection",
            "nominee detection",
            "opacity detection",
            // Question forms
            "any suspicious patterns",
            // Pattern types
            "shell company detection",
            "complex structure",
            "unusual ownership",
        ],
    );
    m.insert(
        "verify.detect-evasion",
        vec![
            // Core patterns
            "detect evasion",
            "evasion signals",
            "suspicious behavior",
            "document delays",
            // Question forms
            "any evasion signals",
            // Evasion patterns
            "reluctance to provide",
            "changing story",
            "inconsistent information",
        ],
    );
    m.insert(
        "verify.challenge",
        vec![
            // Core patterns
            "raise challenge",
            "verification challenge",
            "question client",
            "formal challenge",
            // Question forms
            "how do i challenge",
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
            // Core patterns
            "verify against registry",
            "registry check",
            "gleif check",
            "companies house check",
            // Registry patterns
            "sec edgar check",
            "regulatory register check",
            "public register verification",
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
    // ONBOARDING VERBS - Automation
    // ==========================================================================
    m.insert(
        "onboarding.auto-complete",
        vec![
            // Core patterns
            "auto complete onboarding",
            "generate missing entities",
            "fill gaps automatically",
            "autopilot onboarding",
            // Question forms
            "can you auto complete",
            // Automation patterns
            "complete automatically",
            "generate structure",
            "fill in gaps",
        ],
    );

    // ==========================================================================
    // HOLDING VERBS - Registry Holding Management
    // ==========================================================================
    m.insert(
        "holding.create",
        vec![
            // Core patterns
            "create holding",
            "new investor holding",
            "register holding",
            // Question forms
            "how do i create holding",
        ],
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
    // MOVEMENT VERBS - Registry Transaction Management
    // ==========================================================================
    m.insert(
        "movement.subscribe",
        vec![
            // Core patterns
            "subscription",
            "buy units",
            "invest in fund",
            "new subscription",
            // Question forms
            "how do i subscribe",
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
    // REFERENCE DATA VERBS - Market & Instrument Data
    // ==========================================================================
    m.insert(
        "market.read",
        vec![
            // Core patterns
            "read market",
            "market details",
            "mic code",
            "exchange info",
            // Question forms
            "what is this market",
            // Market patterns
            "exchange details",
            "trading venue",
        ],
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
            // Core patterns
            "set holiday calendar",
            "market holidays",
            "trading calendar",
            "business days",
            // Question forms
            "when is market closed",
        ],
    );
    m.insert(
        "market.calculate-settlement-date",
        vec![
            // Core patterns
            "calculate settlement date",
            "when does it settle",
            "settlement date calc",
            "value date",
            // Question forms
            "when will this settle",
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

