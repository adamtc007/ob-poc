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
    // CBU VERBS - Enhanced with corner cases
    // ==========================================================================
    m.insert(
        "cbu.create",
        vec![
            // Core terms
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
            // Corner cases - abbreviations and alternatives
            "setup cbu",
            "cbu onboarding",
            "begin client onboarding",
            "initiate onboarding",
            "kick off onboarding",
            "client intake",
            "prospect to client",
            "convert prospect",
            "new relationship",
            "new counterparty",
            "add counterparty",
            "create counterparty",
            "institutional client",
            "corporate client",
            "new institutional",
            "fund client",
            "asset owner",
            "asset manager client",
            "hedge fund client",
            "pension fund client",
            "sovereign wealth",
            "family office client",
            "new fof",
            // Question forms
            "how do i create a client",
            "how to add new client",
            "how to start onboarding",
            "how to onboard",
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
            // Corner cases
            "check and create cbu",
            "create cbu if missing",
            "safe cbu create",
            "cbu upsert",
            "get or create cbu",
            "lookup or create cbu",
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
            // Corner cases
            "attach role",
            "bind role",
            "entity role",
            "who plays role",
            "allocate role",
            "define role",
            "role for entity",
            "entity plays",
            "acts as",
            "is the",
            "serves as",
            "functions as",
            "role mapping",
            "associate role",
            "connect entity with role",
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
            // Corner cases
            "deassign role",
            "unbind role",
            "role termination",
            "role removal",
            "strip role",
            "cancel role",
            "detach role",
            "no longer acts as",
            "stop being",
            "ceased to be",
            "role ended",
            "historical role",
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
            // Corner cases
            "cbu summary",
            "cbu overview",
            "client overview",
            "client profile",
            "cbu profile",
            "describe cbu",
            "tell me about cbu",
            "what is this cbu",
            "cbu information",
            "client information",
            "look up cbu",
            "fetch cbu",
            "retrieve cbu",
            "cbu snapshot",
            "current cbu state",
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
            // Corner cases
            "involved entities",
            "participating entities",
            "cbu structure",
            "everyone involved",
            "all related",
            "connected entities",
            "associated entities",
            "party list",
            "entity roster",
            "who is who",
            "org chart",
            "structure diagram",
            "relationship map",
            "counterparties",
            "affiliated parties",
            "related persons",
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
            // Corner cases
            "link product",
            "connect product",
            "product entitlement",
            "entitle product",
            "grant product",
            "product access",
            "sign up for product",
            "product enrollment",
            "activate service",
            "enable service",
            "service subscription",
            "provision product",
            "onboard to product",
            "custody product",
            "fund accounting product",
            "transfer agency product",
            "ta product",
            "fa product",
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
            // Corner cases
            "client decision",
            "accept client",
            "decline client",
            "approve onboarding",
            "reject onboarding",
            "go live decision",
            "activation decision",
            "board approval",
            "committee decision",
            "sign off",
            "final approval",
            "commence services",
            "start trading",
            "green light",
            "proceed with client",
            "terminate onboarding",
            "exit onboarding",
            "abort onboarding",
        ],
    );

    // ==========================================================================
    // ENTITY VERBS - Enhanced with corner cases
    // ==========================================================================
    m.insert(
        "entity.create-limited-company",
        vec![
            // Core terms
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
            // Corner cases - jurisdictions and entity types
            "create sas",
            "create sprl",
            "create nv",
            "create ab",
            "create oy",
            "create as",
            "create kk",
            "create pty ltd",
            "create pte ltd",
            "create inc",
            "create corp",
            "create llc",
            "create aps",
            "create srl",
            "create spa",
            "create limited",
            "new corporation",
            "new limited",
            "establish company",
            "set up company",
            "form company",
            "company formation",
            "corporate entity",
            "legal person",
            "juridical person",
            "corporate person",
            "body corporate",
            "holding company",
            "subsidiary",
            "spv",
            "special purpose vehicle",
            "operating company",
            "opco",
            "holdco",
            "propco",
            "newco",
            "bidco",
            "topco",
            // Question forms
            "how to create company",
            "how to add entity",
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
            // Corner cases
            "get or create company",
            "lookup or create company",
            "safe company create",
            "company upsert",
            "entity if missing",
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
            // Corner cases - roles and contexts
            "add officer",
            "create officer",
            "add executive",
            "add board member",
            "add trustee",
            "add beneficiary",
            "add settlor",
            "add protector",
            "add authorized signatory",
            "add contact",
            "create contact person",
            "add nominee",
            "create nominee",
            "add representative",
            "legal representative",
            "add poa holder",
            "power of attorney holder",
            "create ubo person",
            "add ubo",
            "key person",
            "add key person",
            "principal",
            "add principal",
            "human being",
            "flesh and blood",
            "real person",
            "living person",
            // Question forms
            "how to add person",
            "how to create individual",
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
            // Corner cases
            "get or create person",
            "lookup or create person",
            "safe person create",
            "person upsert",
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
            // Corner cases - trust types
            "fixed trust",
            "bare trust",
            "nominee trust",
            "charitable trust",
            "purpose trust",
            "star trust",
            "vista trust",
            "private trust company",
            "ptc",
            "grantor trust",
            "revocable trust",
            "irrevocable trust",
            "living trust",
            "testamentary trust",
            "inter vivos trust",
            "asset protection trust",
            "apt",
            "dynasty trust",
            "generation skipping trust",
            "gst trust",
            "qualified personal residence trust",
            "qprt",
            "grantor retained annuity trust",
            "grat",
            "charitable remainder trust",
            "crt",
            "charitable lead trust",
            "clt",
            "spendthrift trust",
            "blind trust",
            "land trust",
            "deed of trust",
            "trust deed",
            "declaration of trust",
            "trust instrument",
            "trust agreement",
            "settlement deed",
            "trust indenture",
            // Question forms
            "how to create trust",
            "how to add trust",
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
            // Corner cases - partnership types
            "limited liability partnership",
            "general partnership",
            "special limited partnership",
            "slp",
            "exempt limited partnership",
            "elp",
            "master limited partnership",
            "mlp",
            "investment limited partnership",
            "ilp",
            "carried interest partnership",
            "carry vehicle",
            "co-invest vehicle",
            "feeder lp",
            "master lp",
            "parallel lp",
            "aggregator lp",
            "blocker",
            "blocker entity",
            "cv",
            "commanditaire vennootschap",
            "kg",
            "kommanditgesellschaft",
            "scs",
            "societe en commandite simple",
            "sca",
            "societe en commandite par actions",
            "partnership agreement",
            "lpa",
            "limited partnership agreement",
            // Question forms
            "how to create partnership",
            "how to add lp",
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
            // Corner cases
            "entity modification",
            "entity correction",
            "fix entity",
            "patch entity",
            "entity amendment",
            "change name",
            "change address",
            "update registration",
            "change jurisdiction",
            "redomicile",
            "entity migration",
            "change legal form",
            "conversion",
            "entity conversion",
            "change incorporation date",
            "correct incorporation",
            "update lei",
            "change lei",
            "update registration number",
        ],
    );

    // ==========================================================================
    // FUND VERBS - Enhanced with corner cases
    // ==========================================================================
    m.insert(
        "fund.create-umbrella",
        vec![
            // Core terms
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
            // Corner cases - fund types and jurisdictions
            "create icav",
            "create fcp",
            "fonds commun de placement",
            "create fcpe",
            "create sif",
            "specialized investment fund",
            "create raif",
            "reserved alternative investment fund",
            "create aif",
            "alternative investment fund",
            "create ucits",
            "ucits fund",
            "create qiaif",
            "create retail fund",
            "create professional fund",
            "create riaif",
            "create iput",
            "irish collective asset vehicle",
            "cayman spc",
            "segregated portfolio company",
            "protected cell company",
            "pcc",
            "vcc",
            "variable capital company",
            "eltif",
            "european long term investment fund",
            "euveca",
            "eusef",
            "money market fund",
            "mmf",
            "etf",
            "exchange traded fund",
            "index fund",
            "fund of funds",
            "fof",
            "fund of one",
            "managed account",
            "private fund",
            "public fund",
            "closed end fund",
            "open end fund",
            "evergreen fund",
            "vintage fund",
            "pe fund",
            "private equity fund",
            "vc fund",
            "venture capital fund",
            "real estate fund",
            "re fund",
            "infrastructure fund",
            "infra fund",
            "credit fund",
            "debt fund",
            "hedge fund",
            "absolute return fund",
            "long only fund",
            "long short fund",
            "quant fund",
            "systematic fund",
            // Question forms
            "how to create fund",
            "how to setup umbrella",
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
            // Corner cases
            "get or create umbrella",
            "lookup or create fund",
            "safe fund create",
            "fund upsert",
            "ensure fund exists",
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
            // Corner cases
            "new segregated portfolio",
            "create sp",
            "create pc",
            "protected cell",
            "sub fund",
            "fund sleeve",
            "investment sleeve",
            "strategy sleeve",
            "new strategy",
            "add strategy",
            "create strategy fund",
            "feeder subfund",
            "master subfund",
            "onshore subfund",
            "offshore subfund",
            "currency subfund",
            "regional subfund",
            "sector subfund",
            "thematic subfund",
            // Question forms
            "how to create subfund",
            "how to add compartment",
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
            // Corner cases
            "get or create subfund",
            "lookup or create compartment",
            "safe subfund create",
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
            // Corner cases - share class types
            "acc class",
            "dis class",
            "inc class",
            "income class",
            "capitalizing class",
            "clean class",
            "super clean class",
            "bundled class",
            "founder class",
            "seed class",
            "a class",
            "b class",
            "c class",
            "d class",
            "i class",
            "r class",
            "s class",
            "z class",
            "class a",
            "class b",
            "class i",
            "institutional share",
            "retail share",
            "advisor share",
            "eur class",
            "usd class",
            "gbp class",
            "chf class",
            "jpy class",
            "currency class",
            "pegged class",
            "unpegged class",
            "voting class",
            "non-voting class",
            "participating class",
            "non-participating class",
            "performance fee class",
            "no performance fee class",
            "management fee class",
            "low fee class",
            "high watermark class",
            "hard hurdle class",
            "soft hurdle class",
            "create sedol",
            "create cusip",
            "create ticker",
            // Question forms
            "how to create share class",
            "how to add isin",
        ],
    );
    m.insert(
        "fund.ensure-share-class",
        vec![
            "ensure share class",
            "upsert share class",
            "find or create share class",
            "share class if not exists",
            // Corner cases
            "get or create class",
            "lookup or create isin",
            "safe class create",
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
            // Corner cases
            "master feeder structure",
            "offshore feeder",
            "onshore feeder",
            "us feeder",
            "non-us feeder",
            "blocker feeder",
            "parallel feeder",
            "mini master",
            "hub and spoke",
            "feeder into master",
            "feed into",
            "invest through",
            "feeder investment",
            "master portfolio",
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
            // Corner cases
            "subfund list",
            "compartment list",
            "show subfunds",
            "umbrella structure",
            "fund tree",
            "fund breakdown",
            "all sleeves",
            "portfolio list",
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
            // Corner cases
            "class list",
            "isin list",
            "sedol list",
            "cusip list",
            "ticker list",
            "share class hierarchy",
            "available classes",
            "class structure",
            "class breakdown",
            "share types",
        ],
    );

    // ==========================================================================
    // UBO/OWNERSHIP VERBS - Enhanced with corner cases
    // ==========================================================================
    m.insert(
        "ubo.add-ownership",
        vec![
            // Core terms
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
            // Corner cases - ownership types
            "add equity",
            "equity interest",
            "capital interest",
            "voting interest",
            "economic interest",
            "profits interest",
            "carried interest",
            "membership interest",
            "partnership interest",
            "unit holder",
            "add unit holder",
            "limited partner interest",
            "general partner interest",
            "lp interest",
            "gp interest",
            "co-invest interest",
            "side pocket",
            "pari passu",
            "senior stake",
            "junior stake",
            "preferred stake",
            "common stake",
            "participating preferred",
            "non-participating preferred",
            "convertible preferred",
            "warrant",
            "option",
            "stock option",
            "phantom equity",
            "shadow equity",
            "synthetic equity",
            "equity derivative",
            "total return swap",
            "cfd",
            "contract for difference",
            "look through ownership",
            "attributed ownership",
            "constructive ownership",
            "aggregated ownership",
            "family ownership",
            "group ownership",
            "joint ownership",
            "tenants in common",
            "joint tenants",
            "community property",
            "nominee ownership",
            "bare ownership",
            "usufruct",
            "beneficial interest",
            // Question forms
            "how to add owner",
            "how to record ownership",
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
            // Corner cases
            "dilution",
            "equity dilution",
            "ownership dilution",
            "anti-dilution",
            "increase stake",
            "decrease stake",
            "top up",
            "follow on",
            "secondary purchase",
            "secondary sale",
            "partial exit",
            "partial sale",
            "down round",
            "up round",
            "recapitalization",
            "recap",
            "stock split",
            "reverse split",
            "share consolidation",
            "bonus issue",
            "rights issue",
            "scrip dividend",
            "ownership transfer",
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
            // Corner cases
            "full exit",
            "complete sale",
            "total divestment",
            "redemption",
            "buyback",
            "share buyback",
            "repurchase",
            "tender",
            "tender offer",
            "squeeze out",
            "drag along",
            "tag along",
            "put option exercise",
            "call option exercise",
            "forced sale",
            "liquidation",
            "wind up",
            "dissolution",
            "bankruptcy",
            "insolvency",
            "forfeiture",
            "ownership termination",
            "historical owner",
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
            // Corner cases
            "ownership chain",
            "equity holders",
            "stakeholders",
            "investor list",
            "cap table",
            "capitalization table",
            "share register",
            "shareholder register",
            "member register",
            "partner list",
            "unit holder list",
            "ownership structure up",
            "upstream owners",
            "ultimate parents",
            "controlling shareholders",
            "majority owners",
            "minority owners",
            "significant shareholders",
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
            // Corner cases
            "downstream ownership",
            "portfolio companies",
            "investee companies",
            "controlled entities",
            "affiliated entities",
            "group companies",
            "consolidated entities",
            "unconsolidated entities",
            "associate companies",
            "joint ventures",
            "jv participations",
            "minority investments",
            "strategic investments",
            "financial investments",
            "ownership tree down",
            "corporate group",
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
            // Corner cases
            "25% owner",
            "threshold owner",
            "significant owner",
            "controlling natural person",
            "natural person behind",
            "real owner",
            "true owner",
            "hidden owner",
            "shadow owner",
            "de facto owner",
            "economic owner",
            "ultimate economic beneficiary",
            "ueb",
            "person of significant control",
            "psc registration",
            "bo declaration",
            "beneficial ownership declaration",
            "ubo form",
            "bo form",
            "ubo filing",
            "bo register entry",
            "ubo certificate",
            // Question forms
            "how to register ubo",
            "how to declare beneficial owner",
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
            // Corner cases
            "chain break",
            "ownership ceiling",
            "no further ubo",
            "terminus entity",
            "publicly traded",
            "exchange listed",
            "stock exchange",
            "nyse listed",
            "nasdaq listed",
            "lse listed",
            "euronext listed",
            "ftse company",
            "sp500 company",
            "sovereign entity",
            "state owned",
            "municipal owned",
            "crown entity",
            "charitable organization",
            "non-profit",
            "ngo",
            "foundation terminus",
            "pension fund terminus",
            "regulated financial institution",
            "bank terminus",
            "insurance terminus",
            "diversified ownership",
            "no controlling person",
            "exempt entity",
            "excepted entity",
            "legal arrangement terminus",
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
            // Corner cases
            "multiplicative ownership",
            "chain multiplication",
            "look through calculation",
            "attributed ownership calc",
            "effective ownership",
            "combined ownership",
            "family aggregation",
            "group aggregation",
            "acting in concert",
            "concerted action",
            "ownership threshold",
            "threshold check",
            "significant influence test",
            "control test",
            "ownership waterfall",
            "cascade calculation",
            "indirect holding calculation",
            "layer calculation",
            "multi-layer ownership",
            // Question forms
            "how to calculate ubo",
            "who owns 25%",
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
            // Corner cases
            "ownership visualization",
            "structure diagram",
            "org chart",
            "corporate tree",
            "group structure",
            "holding structure",
            "investment structure",
            "fund structure",
            "layered structure",
            "opaque structure",
            "complex structure",
            "circular ownership",
            "cross ownership",
            "reciprocal ownership",
            "interlocking ownership",
            "ownership loop",
            "ownership cycle",
            "self-ownership",
            "treasury shares",
            "dormant chains",
            "broken chains",
            "incomplete chains",
            "partial chains",
            "ownership path finder",
            "ownership navigator",
        ],
    );

    // ==========================================================================
    // CONTROL VERBS - Enhanced with corner cases
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
            // Corner cases - control types
            "de facto control",
            "de jure control",
            "legal control",
            "effective control",
            "negative control",
            "veto rights",
            "blocking rights",
            "reserved matters",
            "consent rights",
            "approval rights",
            "board representation",
            "board majority",
            "board appointment",
            "director nomination",
            "shareholder agreement control",
            "sha control",
            "voting agreement",
            "proxy",
            "proxy holder",
            "irrevocable proxy",
            "power of attorney control",
            "management control",
            "operational control",
            "financial control",
            "strategic control",
            "shadow director",
            "de facto director",
            "controlling mind",
            "directing mind",
            "golden share",
            "special share",
            "founder control",
            "dual class control",
            "supervoting",
            "class b control",
            // Question forms
            "how to add control",
            "who controls this",
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
            // Corner cases
            "control structure",
            "governance structure",
            "control analysis",
            "control diagram",
            "control hierarchy",
            "ultimate controller",
            "controlling persons",
            "pscs",
            "persons of significant control",
            "board composition",
            "voting power distribution",
            "control percentage",
            "control test results",
        ],
    );

    // ==========================================================================
    // ROLE ASSIGNMENT (V2) - Enhanced with corner cases
    // ==========================================================================
    m.insert(
        "cbu.role:assign",
        vec![
            "assign role",
            "add role to cbu",
            "entity role",
            "role assignment",
            "link with role",
            // Corner cases
            "generic role",
            "custom role",
            "ad hoc role",
            "special role",
            "temporary role",
            "permanent role",
            "assign entity role",
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
            // Corner cases
            "investor",
            "equity investor",
            "strategic investor",
            "financial investor",
            "anchor investor",
            "cornerstone investor",
            "seed investor",
            "lead investor",
            "co-investor",
            "limited partner role",
            "general partner role",
            "lp",
            "gp",
            "unit holder role",
            "member role",
            "capital provider",
            "silent partner",
            "dormant partner",
            "sleeping partner",
            "active partner",
            "managing member",
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
            // Corner cases
            "coo",
            "cto",
            "cio",
            "cro",
            "clo",
            "general counsel",
            "chief compliance officer",
            "cco",
            "chief risk officer",
            "president",
            "vice president",
            "vp",
            "svp",
            "evp",
            "executive director",
            "non-executive director",
            "ned",
            "independent director",
            "audit committee",
            "remuneration committee",
            "nomination committee",
            "board chair",
            "lead director",
            "senior independent director",
            "sid",
            "alternate director",
            "shadow director",
            "de facto director",
            "nominee director",
            "representative director",
            "corporate director",
            "manager",
            "gerant",
            "administrateur",
            "bestuurder",
            "vorstand",
            "aufsichtsrat",
            "statutory auditor",
            "sindico",
            "commissaire",
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
            // Corner cases
            "grantor",
            "donor",
            "creator",
            "appointor",
            "investment advisor to trust",
            "distribution advisor",
            "trust committee",
            "trust council",
            "primary beneficiary",
            "contingent beneficiary",
            "remainder beneficiary",
            "income beneficiary",
            "capital beneficiary",
            "discretionary beneficiary",
            "named beneficiary",
            "class beneficiary",
            "future beneficiary",
            "potential beneficiary",
            "excluded person",
            "excluded class",
            "co-trustee",
            "successor trustee",
            "replacement trustee",
            "professional trustee",
            "corporate trustee",
            "lay trustee",
            "trust officer",
            "fiduciary",
            "trust administrator",
            "trust protector",
            "trust enforcer",
            "trust guardian",
            "investment committee member",
            "distribution committee member",
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
            // Corner cases
            "fund manager",
            "asset manager",
            "discretionary manager",
            "non-discretionary advisor",
            "ucits management company",
            "aifm appointed",
            "external aifm",
            "self-managed",
            "internally managed",
            "fund administrator",
            "fund accountant",
            "nav calculation agent",
            "valuation agent",
            "pricing agent",
            "transfer agent",
            "ta",
            "registrar",
            "shareholder services",
            "investor services",
            "middle office provider",
            "back office provider",
            "risk manager",
            "compliance manager",
            "independent valuation firm",
            "valuer",
            "fund secretary",
            "company secretary",
            "aifm delegate",
            "delegation recipient",
            "placement agent",
            "distributor",
            "global distributor",
            "local distributor",
            "introducing broker",
            "ib",
            "placement fee recipient",
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
            // Corner cases
            "depositary bank",
            "ucits depositary",
            "aifmd depositary",
            "depositary lite",
            "sub-custodian",
            "global custodian",
            "local custodian",
            "securities lending agent",
            "collateral agent",
            "paying agent",
            "fiscal agent",
            "calculation agent",
            "listing agent",
            "corporate services provider",
            "domiciliation agent",
            "registered agent",
            "resident agent",
            "company formation agent",
            "process agent",
            "service of process agent",
            "statutory auditor",
            "external auditor",
            "internal auditor",
            "big4 auditor",
            "audit firm",
            "tax consultant",
            "tax preparer",
            "tax reclaim agent",
            "legal advisor",
            "external counsel",
            "compliance consultant",
            "regulatory advisor",
            "aml service provider",
            "kyc service provider",
            "screening provider",
            "data provider",
            "benchmark administrator",
            "index provider",
            "rating agency",
            "credit rating agency",
            "proxy advisor",
            "proxy voting service",
            "esg rating provider",
            "sustainability advisor",
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
            // Corner cases
            "signature authority",
            "bank signatory",
            "account signatory",
            "joint signatory",
            "sole signatory",
            "class a signatory",
            "class b signatory",
            "dual signature",
            "multi signature",
            "threshold signatory",
            "authorized person",
            "authorized representative",
            "authorized officer",
            "trading authorization",
            "dealing authority",
            "instruction giver",
            "instruction authority",
            "mandate",
            "discretionary mandate",
            "limited mandate",
            "full mandate",
            "general poa",
            "specific poa",
            "limited poa",
            "durable poa",
            "springing poa",
            "procuration",
            "prokura",
            "procurador",
            "fondé de pouvoir",
            "bevollmächtigter",
            "delegee",
            "delegate signatory",
        ],
    );

    // ==========================================================================
    // GRAPH/NAVIGATION VERBS - Enhanced with corner cases
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
            // Corner cases
            "graph view",
            "open graph",
            "launch graph",
            "render graph",
            "draw graph",
            "network view",
            "network diagram",
            "relationship diagram",
            "entity diagram",
            "org structure",
            "organizational chart",
            "corporate structure",
            "holding structure view",
            "investment structure view",
            "ubo structure",
            "control structure",
            "visual representation",
            "graphical view",
            "node view",
            "edge view",
            "force directed",
            "hierarchical view",
            "tree view",
            "radial view",
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
            // Corner cases
            "zoom in",
            "zoom out",
            "fit to screen",
            "fit view",
            "center view",
            "pan to",
            "scroll to",
            "jump to",
            "go to",
            "locate",
            "find in graph",
            "highlight",
            "spotlight",
            "pin node",
            "unpin node",
            "lock position",
            "expand node",
            "collapse node",
            "select entity",
            "click on",
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
            // Corner cases
            "parents",
            "grandparents",
            "all parents",
            "recursive parents",
            "ownership up",
            "control up",
            "holding chain",
            "corporate chain up",
            "investor chain",
            "owner chain",
            "who is behind",
            "who is above",
            "superior entities",
            "upstream entities",
            "path to top",
            "chain to ultimate",
            "all levels up",
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
            // Corner cases
            "children",
            "grandchildren",
            "all children",
            "recursive children",
            "ownership down",
            "control down",
            "subsidiary chain",
            "corporate chain down",
            "investment chain",
            "portfolio chain",
            "what is below",
            "subordinate entities",
            "downstream entities",
            "all levels down",
            "complete subtree",
            "full tree below",
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
            // Corner cases
            "shortest path",
            "all paths",
            "relationship between",
            "chain between",
            "connected through",
            "degrees of separation",
            "hops between",
            "route between",
            "trace between",
            "connection path",
            "link path",
            "relationship chain",
            "indirect relationship",
            "direct relationship",
            "common ancestor",
            "common parent",
            "shared owner",
            "related through",
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
            // Corner cases
            "filter by jurisdiction",
            "filter by country",
            "filter by entity type",
            "filter by role",
            "filter by ownership",
            "filter by control",
            "filter by percentage",
            "threshold filter",
            "material filter",
            "significant filter",
            "hide inactive",
            "show active only",
            "hide historical",
            "show current only",
            "date filter",
            "as of date filter",
            "point in time filter",
            "exclude",
            "include",
            "visible entities",
            "hidden entities",
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
            // Corner cases
            "cluster",
            "grouping",
            "categorize",
            "segment by",
            "partition by",
            "organize",
            "arrange by",
            "sort by",
            "color by",
            "color code",
            "legend",
            "group by country",
            "group by entity type",
            "group by role",
            "group by relationship",
            "group by ownership level",
            "group by ubo status",
            "swim lanes",
            "lanes by type",
        ],
    );

    // ==========================================================================
    // KYC CASE VERBS - Enhanced with corner cases
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
            // Corner cases
            "create case",
            "new dd case",
            "due diligence case",
            "cdd case",
            "edd case",
            "enhanced due diligence case",
            "client due diligence",
            "initial review",
            "periodic review case",
            "event driven review",
            "trigger event case",
            "remediation case",
            "refresh case",
            "recertification case",
            "annual review case",
            "biennial review",
            "triennial review",
            "high risk review",
            "low risk review",
            "medium risk review",
            "sdd case",
            "simplified due diligence",
            "new customer case",
            "prospect case",
            "pre-onboarding case",
            // Question forms
            "how to start kyc",
            "how to create case",
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
            // Corner cases
            "transition case",
            "case state change",
            "workflow progress",
            "stage transition",
            "phase transition",
            "move to next stage",
            "complete stage",
            "case milestone",
            "status update",
            "case update",
            "mark progress",
            "record progress",
            "case moved",
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
            // Corner cases
            "escalate to mlro",
            "escalate to compliance officer",
            "escalate to committee",
            "escalate to board",
            "senior review required",
            "management escalation",
            "approval required",
            "override required",
            "exception escalation",
            "risk escalation",
            "flag for review",
            "refer for decision",
            "second line review",
            "third line review",
            "audit escalation",
            "regulatory escalation",
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
            // Corner cases
            "assign to team",
            "assign to user",
            "case owner",
            "primary analyst",
            "secondary analyst",
            "backup analyst",
            "reviewer assignment",
            "approver assignment",
            "workload assignment",
            "queue assignment",
            "reassign case",
            "transfer case",
            "take ownership",
            "claim case",
            "release case",
            "unassign case",
            "auto assignment",
            "round robin",
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
            // Corner cases
            "inherent risk",
            "residual risk",
            "net risk",
            "gross risk",
            "risk score",
            "risk level",
            "risk tier",
            "risk category",
            "risk band",
            "risk bucket",
            "pep risk",
            "sanctions risk",
            "geographic risk",
            "product risk",
            "channel risk",
            "transaction risk",
            "composite risk",
            "overall risk",
            "final risk rating",
            "recommended risk",
            "calculated risk",
            "override risk",
            "manual risk",
            "system risk",
            "model risk score",
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
            // Corner cases
            "case closed",
            "kyc complete",
            "dd complete",
            "due diligence complete",
            "approved and closed",
            "rejected and closed",
            "terminated case",
            "archive case",
            "wrap up case",
            "sign off case",
            "final approval",
            "case finalized",
            "no further action",
            "nfa",
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
            // Corner cases
            "case summary",
            "case overview",
            "case snapshot",
            "case status",
            "case information",
            "fetch case",
            "retrieve case",
            "open case details",
            "case record",
            "case file",
            "case dossier",
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
            // Corner cases
            "client cases",
            "customer cases",
            "case list",
            "case queue",
            "my cases",
            "team cases",
            "open cases",
            "closed cases",
            "pending cases",
            "in progress cases",
            "overdue cases",
            "case backlog",
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
            // Corner cases
            "trigger review",
            "refresh review",
            "recertification",
            "annual recertification",
            "scheduled review",
            "adverse news trigger",
            "pep trigger",
            "sanctions trigger",
            "material change trigger",
            "transaction trigger",
            "regulatory trigger",
            "mandate trigger",
            "new information",
            "supplementary review",
            "additional review",
            "follow up review",
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
            // Corner cases
            "case dashboard",
            "case metrics",
            "case analytics",
            "case statistics",
            "completion percentage",
            "progress percentage",
            "workstream status",
            "document status",
            "screening status",
            "verification status",
            "overall case status",
            "consolidated status",
            "rollup status",
        ],
    );

    // ==========================================================================
    // ENTITY WORKSTREAM VERBS - Enhanced with corner cases
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
            // Corner cases
            "dd workstream",
            "cdd workstream",
            "edd workstream",
            "entity dd",
            "entity cdd",
            "entity edd",
            "review entity",
            "scope entity",
            "include entity",
            "add to scope",
            "workstream for entity",
            "entity in scope",
            "ubo workstream",
            "controller workstream",
            "shareholder workstream",
            "director workstream",
            "signatory workstream",
            "service provider workstream",
            "related party workstream",
        ],
    );
    m.insert(
        "entity-workstream.update-status",
        vec![
            "update workstream",
            "workstream progress",
            "change workstream status",
            "advance workstream",
            // Corner cases
            "workstream status",
            "progress workstream",
            "move workstream",
            "workstream transition",
            "workstream stage",
            "workstream phase",
            "workstream milestone",
            "workstream update",
        ],
    );
    m.insert(
        "entity-workstream.block",
        vec![
            "block workstream",
            "workstream blocked",
            "pause workstream",
            "stop workstream",
            // Corner cases
            "hold workstream",
            "workstream on hold",
            "pending workstream",
            "waiting for information",
            "awaiting documents",
            "blocked by screening",
            "blocked by verification",
            "blocker identified",
            "impediment",
            "dependency block",
            "external dependency",
        ],
    );
    m.insert(
        "entity-workstream.complete",
        vec![
            "complete workstream",
            "workstream done",
            "finish workstream",
            "workstream complete",
            // Corner cases
            "workstream finished",
            "entity cleared",
            "entity approved",
            "entity verified",
            "dd complete for entity",
            "cdd complete for entity",
            "edd complete for entity",
            "no further action",
            "workstream closed",
            "workstream finalized",
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
            // Corner cases
            "upgrade to edd",
            "escalate to edd",
            "edd flag",
            "enhanced review",
            "deep dive required",
            "additional dd",
            "supplementary dd",
            "extended dd",
            "pep edd",
            "sanctions edd",
            "high risk edd",
            "complex structure edd",
            "adverse media edd",
            "regulatory edd",
        ],
    );
    m.insert(
        "entity-workstream.set-ubo",
        vec![
            "mark as ubo",
            "workstream ubo",
            "ubo workstream",
            "identify ubo",
            // Corner cases
            "ubo identified",
            "beneficial owner workstream",
            "ubo flag",
            "25% owner",
            "threshold owner",
            "ubo scope",
            "ultimate owner workstream",
            "true owner workstream",
        ],
    );
    m.insert(
        "entity-workstream.list-by-case",
        vec![
            "list workstreams",
            "case workstreams",
            "all workstreams",
            "entities in case",
            // Corner cases
            "workstream list",
            "entity list",
            "scope list",
            "in scope entities",
            "case scope",
            "review scope",
            "dd scope",
            "workstream queue",
            "pending workstreams",
            "complete workstreams",
            "blocked workstreams",
        ],
    );
    m.insert(
        "entity-workstream.state",
        vec![
            "workstream state",
            "workstream details",
            "workstream with requests",
            // Corner cases
            "workstream summary",
            "workstream overview",
            "workstream status details",
            "workstream dashboard",
            "workstream metrics",
            "entity dd status",
            "entity verification status",
            "entity screening status",
        ],
    );

    // ==========================================================================
    // DOCUMENT REQUEST VERBS - Enhanced with corner cases
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
            // Corner cases
            "create doc request",
            "new doc request",
            "add doc requirement",
            "documentation required",
            "evidence required",
            "proof required",
            "supporting document",
            "mandatory document",
            "optional document",
            "conditional document",
            "prerequisite document",
            "dependency document",
            "certified copy",
            "original required",
            "notarized copy",
            "apostille required",
            "legalized document",
            "translated document",
            "recent document",
            "current document",
            "dated within",
            "not older than",
        ],
    );
    m.insert(
        "doc-request.mark-requested",
        vec![
            "mark requested",
            "formally request",
            "send request",
            "document requested",
            // Corner cases
            "request sent",
            "notification sent",
            "email sent",
            "chase sent",
            "reminder sent",
            "follow up sent",
            "formal request",
            "official request",
            "request dated",
            "request issued",
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
            // Corner cases
            "document in",
            "doc arrived",
            "attachment received",
            "file uploaded",
            "submission received",
            "evidence received",
            "proof received",
            "copy received",
            "original received",
            "scanned document",
            "digital copy",
            "hard copy received",
            "courier received",
            "mail received",
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
            // Corner cases
            "authenticate document",
            "document authentication",
            "verify authenticity",
            "check validity",
            "verify date",
            "verify signature",
            "verify content",
            "verify completeness",
            "verify accuracy",
            "cross reference",
            "confirm document",
            "accept document",
            "document acceptable",
            "document passes",
            "document satisfies",
        ],
    );
    m.insert(
        "doc-request.reject",
        vec![
            "reject document",
            "document rejected",
            "invalid document",
            "doc not acceptable",
            // Corner cases
            "document unacceptable",
            "document fails",
            "document invalid",
            "poor quality",
            "illegible document",
            "incomplete document",
            "wrong document",
            "incorrect document",
            "expired document",
            "outdated document",
            "unsigned document",
            "missing signature",
            "missing certification",
            "missing notarization",
            "missing apostille",
            "missing translation",
            "request resubmission",
            "resubmit required",
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
            // Corner cases
            "exception granted",
            "waiver approved",
            "document optional",
            "alternative accepted",
            "substitute accepted",
            "equivalent accepted",
            "exemption",
            "dispensation",
            "not applicable",
            "n/a",
            "out of scope",
            "regulatory waiver",
            "compliance waiver",
            "risk accepted",
        ],
    );
    m.insert(
        "doc-request.list-by-workstream",
        vec![
            "list doc requests",
            "outstanding documents",
            "pending documents",
            "what documents needed",
            // Corner cases
            "document list",
            "doc list",
            "required documents",
            "missing documents",
            "received documents",
            "verified documents",
            "rejected documents",
            "waived documents",
            "document checklist",
            "documentation checklist",
            "evidence checklist",
            "document status",
            "doc status",
            "document tracker",
        ],
    );

    // ==========================================================================
    // CASE SCREENING VERBS - Enhanced with corner cases
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
            // Corner cases
            "execute screening",
            "perform screening",
            "batch screening",
            "bulk screening",
            "real time screening",
            "ongoing screening",
            "continuous screening",
            "periodic screening",
            "trigger screening",
            "event screening",
            "name screening",
            "entity screening",
            "transaction screening",
            "payment screening",
            "counterparty screening",
            "third party screening",
            "vendor screening",
            "supplier screening",
            "agent screening",
            "intermediary screening",
            "ofac screening",
            "eu sanctions",
            "un sanctions",
            "uk sanctions",
            "hmrc sanctions",
            "consolidated list",
            "global sanctions",
            "negative news",
            "adverse media check",
            "reputational screening",
            // Question forms
            "how to screen",
            "how to check sanctions",
        ],
    );
    m.insert(
        "case-screening.complete",
        vec![
            "complete screening",
            "screening done",
            "screening finished",
            "screening result",
            // Corner cases
            "screening complete",
            "screening closed",
            "screening finalized",
            "no hits",
            "clear screening",
            "passed screening",
            "screening passed",
            "negative result",
            "positive result",
            "hits found",
            "matches found",
            "potential matches",
            "fuzzy matches",
            "exact matches",
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
            // Corner cases
            "true positive",
            "true match",
            "confirmed match",
            "actual match",
            "valid hit",
            "invalid hit",
            "false hit",
            "false match",
            "fp disposition",
            "tp disposition",
            "hit disposition",
            "match disposition",
            "close hit",
            "near match",
            "partial match",
            "name variant",
            "alias match",
            "aka match",
            "dob match",
            "country match",
            "id match",
            "hit escalation",
            "hit investigation",
        ],
    );
    m.insert(
        "case-screening.list-by-workstream",
        vec![
            "list screenings",
            "screening history",
            "all screenings",
            "screening results",
            // Corner cases
            "screening log",
            "screening audit",
            "screening records",
            "historical screenings",
            "past screenings",
            "screening timeline",
            "screening summary",
            "hit summary",
            "match summary",
            "screening report",
        ],
    );

    // ==========================================================================
    // RED FLAG VERBS - Enhanced with corner cases
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
            // Corner cases
            "create red flag",
            "new red flag",
            "add red flag",
            "risk flag",
            "warning flag",
            "caution flag",
            "yellow flag",
            "amber flag",
            "risk indicator",
            "suspicious indicator",
            "typology match",
            "pattern match",
            "sar indicator",
            "suspicious activity",
            "unusual activity",
            "anomaly detected",
            "concern noted",
            "issue logged",
            "observation flag",
            "finding",
            "adverse finding",
            "negative finding",
            "material concern",
            "significant concern",
            "minor concern",
            "major concern",
            "critical flag",
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
            // Corner cases
            "flag resolved",
            "concern addressed",
            "mitigation applied",
            "control implemented",
            "risk mitigated",
            "issue cleared",
            "flag cleared",
            "satisfactory explanation",
            "reasonable explanation",
            "supporting evidence",
            "documentation provided",
            "additional information",
            "clarification received",
            "comfort obtained",
            "assurance received",
        ],
    );
    m.insert(
        "red-flag.waive",
        vec![
            "waive red flag",
            "flag waived",
            "approve despite flag",
            "accept risk",
            // Corner cases
            "flag exception",
            "exception granted",
            "override flag",
            "risk acceptance",
            "residual risk accepted",
            "commercial override",
            "business override",
            "management override",
            "senior override",
            "committee override",
            "documented waiver",
            "approved exception",
        ],
    );
    m.insert(
        "red-flag.dismiss",
        vec![
            "dismiss flag",
            "false positive flag",
            "flag dismissed",
            "not a concern",
            // Corner cases
            "flag invalid",
            "flag incorrect",
            "false alarm",
            "not material",
            "not relevant",
            "out of scope",
            "no longer applicable",
            "superseded",
            "duplicate flag",
            "already addressed",
            "previously resolved",
            "historical flag",
            "stale flag",
        ],
    );
    m.insert(
        "red-flag.set-blocking",
        vec![
            "blocking flag",
            "flag blocks case",
            "hard stop",
            "case blocked",
            // Corner cases
            "blocker flag",
            "stop flag",
            "halt flag",
            "prevents approval",
            "prevents onboarding",
            "mandatory resolution",
            "must resolve",
            "cannot proceed",
            "showstopper",
            "deal breaker",
            "fatal flag",
            "non-waivable",
            "policy violation",
            "regulatory violation",
            "prohibition",
        ],
    );
    m.insert(
        "red-flag.list-by-case",
        vec![
            "list red flags",
            "case flags",
            "all flags",
            "open flags",
            // Corner cases
            "flag list",
            "flag summary",
            "flag report",
            "active flags",
            "resolved flags",
            "waived flags",
            "dismissed flags",
            "blocking flags",
            "non-blocking flags",
            "flag history",
            "flag timeline",
        ],
    );

    // ==========================================================================
    // SCREENING VERBS (PEP, Sanctions, Adverse Media) - Enhanced
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
            // Corner cases
            "pep status",
            "pep category",
            "pep tier",
            "pep level",
            "domestic pep",
            "foreign pep",
            "international pep",
            "head of state",
            "government minister",
            "senior government official",
            "judicial official",
            "senior military",
            "state owned enterprise",
            "soe executive",
            "political party official",
            "senior party member",
            "rca",
            "relative close associate",
            "family member of pep",
            "business associate of pep",
            "pep by association",
            "former pep",
            "ex-pep",
            "pep cooling off",
            "de-pep",
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
            // Corner cases
            "sanctions status",
            "sanctioned entity",
            "designated entity",
            "blocked entity",
            "prohibited entity",
            "restricted entity",
            "us sanctions",
            "ofac sdn",
            "ofac non-sdn",
            "ofac 50% rule",
            "ownership based sanctions",
            "eu sanctions",
            "uk sanctions",
            "un sanctions",
            "unsc sanctions",
            "security council",
            "consolidated sanctions",
            "targeted sanctions",
            "sectoral sanctions",
            "comprehensive sanctions",
            "secondary sanctions",
            "extraterritorial sanctions",
            "russia sanctions",
            "iran sanctions",
            "north korea sanctions",
            "cuba sanctions",
            "syria sanctions",
            "venezuela sanctions",
            "belarus sanctions",
            "crimea sanctions",
            "donetsk sanctions",
            "luhansk sanctions",
            "specially designated",
            "specially designated national",
            "blocked property",
            "frozen assets",
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
            // Corner cases
            "reputational risk",
            "media risk",
            "news risk",
            "public information",
            "open source intelligence",
            "osint",
            "negative publicity",
            "damaging news",
            "criminal news",
            "fraud news",
            "money laundering news",
            "bribery news",
            "corruption news",
            "tax evasion news",
            "financial crime news",
            "regulatory action news",
            "enforcement action news",
            "lawsuit news",
            "litigation news",
            "bankruptcy news",
            "insolvency news",
            "environmental news",
            "human rights news",
            "labor violation news",
            "safety violation news",
            "product recall news",
            "data breach news",
            "cyber incident news",
            "management issues news",
            "governance issues news",
        ],
    );

    // ==========================================================================
    // DOCUMENT VERBS - Enhanced with corner cases
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
            // Corner cases
            "file upload",
            "attachment",
            "add attachment",
            "save file",
            "store file",
            "archive document",
            "document repository",
            "document library",
            "dms upload",
            "document management",
            "file storage",
            "document storage",
            "evidence upload",
            "proof upload",
            "supporting doc upload",
            "scan upload",
            "pdf upload",
            "image upload",
            "bulk upload",
            "batch upload",
            "drag and drop",
            "browse and upload",
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
            // Corner cases
            "document parsing",
            "text extraction",
            "data extraction",
            "field extraction",
            "entity extraction",
            "name extraction",
            "date extraction",
            "address extraction",
            "number extraction",
            "table extraction",
            "structured extraction",
            "intelligent extraction",
            "ai extraction",
            "ml extraction",
            "nlp extraction",
            "document ai",
            "document intelligence",
            "form recognition",
            "invoice extraction",
            "id extraction",
            "passport extraction",
            "certificate extraction",
            "articles extraction",
            "register extract",
            "registry extract",
        ],
    );

    // ==========================================================================
    // SERVICE/PRODUCT VERBS - Enhanced with corner cases
    // ==========================================================================
    m.insert(
        "service.list",
        vec![
            "list services",
            "available services",
            "what services",
            "service catalog",
            "show services",
            // Corner cases
            "service list",
            "service menu",
            "service offering",
            "service portfolio",
            "offered services",
            "supported services",
            "enabled services",
            "active services",
            "service types",
            "service categories",
            "custody services",
            "fund services",
            "trading services",
            "reporting services",
            "tax services",
            "corporate action services",
            "securities lending",
            "collateral management",
            "cash management",
            "fx services",
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
            // Corner cases
            "product list",
            "product menu",
            "product offering",
            "product portfolio",
            "offered products",
            "supported products",
            "enabled products",
            "active products",
            "product types",
            "product categories",
            "custody product",
            "fund accounting product",
            "transfer agency product",
            "middle office product",
            "back office product",
            "front office product",
            "reporting product",
            "data product",
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
            // Corner cases
            "enroll product",
            "sign up product",
            "onboard to product",
            "start product",
            "go live product",
            "activate service",
            "enable service",
            "provision product",
            "setup product",
            "configure product",
            "product entitlement",
            "grant product access",
            "product activation",
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
            // Corner cases
            "end product",
            "terminate product",
            "off-board product",
            "decommission product",
            "sunset product",
            "product termination",
            "revoke product access",
            "product deactivation",
            "stop product",
            "product cancellation",
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
    // CUSTODY VERBS - UNIVERSE - Enhanced with corner cases
    // ==========================================================================
    m.insert(
        "cbu-custody.add-universe",
        vec![
            // Core terms
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
            // Corner cases - asset types
            "allow equities",
            "allow bonds",
            "allow fixed income",
            "allow derivatives",
            "allow futures",
            "allow options",
            "allow swaps",
            "allow fx",
            "allow commodities",
            "allow etfs",
            "allow funds",
            "allow structured products",
            "allow money market",
            "allow repos",
            "allow securities lending",
            // Corner cases - market access
            "enable xetra",
            "enable nyse",
            "enable lse",
            "enable euronext",
            "enable nasdaq",
            "allow us markets",
            "allow uk markets",
            "allow eu markets",
            "allow apac markets",
            "cross border trading",
            "international trading",
            "domestic trading",
            "otc trading",
            "exchange traded",
            // Corner cases - permissions
            "trading authorization",
            "investment mandate",
            "portfolio mandate",
            "eligible instruments",
            "permissible securities",
            "approved instruments",
            "whitelisted securities",
            "sanctioned instruments",
            "client mandate",
            "investment policy",
            // Question forms
            "how do i add trading permissions",
            "how to expand trading universe",
            "what instruments can i add",
            "can they trade equities",
        ],
    );
    m.insert(
        "cbu-custody.list-universe",
        vec![
            // Core terms
            "list universe",
            "show universe",
            "trading permissions",
            "what can cbu trade",
            "universe entries",
            "permitted instruments",
            // Corner cases - views
            "trading scope",
            "investment scope",
            "mandate scope",
            "authorized instruments",
            "enabled markets",
            "active markets",
            "trading capacity",
            "full universe",
            "complete universe",
            "universe summary",
            // Corner cases - filters
            "universe by asset class",
            "universe by market",
            "universe by currency",
            "universe by region",
            "universe for fund",
            "universe for subfund",
            "effective universe",
            "inherited universe",
            // Question forms
            "what instruments can they trade",
            "show me trading permissions",
            "what markets are enabled",
            "what can this client trade",
        ],
    );
    m.insert(
        "cbu-custody.remove-universe",
        vec![
            // Core terms
            "remove from universe",
            "disable trading",
            "remove instrument class",
            "stop trading",
            "restrict universe",
            // Corner cases - actions
            "revoke trading permission",
            "withdraw market access",
            "block instrument class",
            "blacklist instruments",
            "exclude from universe",
            "suspend trading",
            "terminate market access",
            "reduce universe",
            "narrow universe",
            "limit trading",
            // Corner cases - reasons
            "compliance restriction",
            "regulatory restriction",
            "risk restriction",
            "mandate breach",
            "sanction restriction",
            // Question forms
            "how do i restrict trading",
            "how to remove trading permission",
        ],
    );

    // ==========================================================================
    // CUSTODY VERBS - SSI - Enhanced with corner cases
    // ==========================================================================
    m.insert(
        "cbu-custody.create-ssi",
        vec![
            // Core terms
            "create ssi",
            "standing settlement instruction",
            "settlement instruction",
            "new ssi",
            "add settlement details",
            "settlement account",
            "safekeeping account",
            "setup ssi",
            // Corner cases - SSI types
            "delivery ssi",
            "receive ssi",
            "payment ssi",
            "securities ssi",
            "dvp ssi",
            "fop ssi",
            "rvp ssi",
            "dfp ssi",
            "cash ssi",
            "nostro account",
            "vostro account",
            "depo account",
            // Corner cases - components
            "bic code",
            "swift code",
            "account number",
            "safekeeping account number",
            "participant id",
            "dda account",
            "settlement chain",
            "correspondent bank",
            "intermediary bank",
            "agent bank",
            "custodian account",
            "sub-custodian account",
            "csd account",
            "icsd account",
            "euroclear account",
            "clearstream account",
            "dtc account",
            // Corner cases - markets
            "domestic ssi",
            "international ssi",
            "cross border ssi",
            "local market ssi",
            "global ssi",
            "market specific ssi",
            // Question forms
            "how do i create settlement instruction",
            "how to add ssi",
            "how to setup settlement account",
            "what details do i need for ssi",
        ],
    );
    m.insert(
        "cbu-custody.ensure-ssi",
        vec![
            // Core terms
            "ensure ssi",
            "upsert ssi",
            "find or create ssi",
            "idempotent ssi",
            "ssi if not exists",
            // Corner cases
            "get or create ssi",
            "lookup or create ssi",
            "safe ssi create",
            "ssi upsert",
            "check and create ssi",
            "create ssi if missing",
            "verify or create ssi",
            "match or create ssi",
        ],
    );
    m.insert(
        "cbu-custody.activate-ssi",
        vec![
            // Core terms
            "activate ssi",
            "enable ssi",
            "ssi active",
            "go live ssi",
            "ssi ready",
            // Corner cases
            "switch on ssi",
            "start using ssi",
            "ssi live",
            "production ssi",
            "approved ssi",
            "verified ssi",
            "confirmed ssi",
            "operational ssi",
            "effective ssi",
            // Question forms
            "how do i activate settlement instruction",
            "how to enable ssi",
            "how to go live with ssi",
        ],
    );
    m.insert(
        "cbu-custody.suspend-ssi",
        vec![
            // Core terms
            "suspend ssi",
            "disable ssi",
            "pause ssi",
            "ssi inactive",
            "deactivate ssi",
            // Corner cases
            "hold ssi",
            "freeze ssi",
            "stop ssi",
            "ssi on hold",
            "ssi blocked",
            "ssi dormant",
            "temporarily disable ssi",
            "ssi maintenance",
            "ssi under review",
            // Question forms
            "how do i suspend settlement instruction",
            "how to disable ssi",
        ],
    );
    m.insert(
        "cbu-custody.list-ssis",
        vec![
            // Core terms
            "list ssis",
            "show settlement instructions",
            "all ssis",
            "settlement accounts",
            "ssi list",
            // Corner cases - views
            "ssi inventory",
            "ssi catalog",
            "ssi directory",
            "complete ssi list",
            "full ssi details",
            "ssi summary",
            "ssi overview",
            // Corner cases - filters
            "active ssis",
            "inactive ssis",
            "suspended ssis",
            "ssis by market",
            "ssis by currency",
            "ssis by asset class",
            "ssis by custodian",
            "ssis for cbu",
            "ssis for entity",
            // Question forms
            "show me all settlement instructions",
            "what ssis do we have",
            "where do we settle",
        ],
    );
    m.insert(
        "cbu-custody.setup-ssi",
        vec![
            // Core terms
            "setup ssi",
            "bulk ssi import",
            "import settlement instructions",
            "load ssis",
            "ssi migration",
            // Corner cases - bulk operations
            "batch ssi upload",
            "mass ssi creation",
            "ssi file upload",
            "ssi excel import",
            "ssi csv import",
            "ssi template upload",
            "ssi onboarding",
            "ssi provisioning",
            "ssi seeding",
            "initial ssi setup",
            "standard ssi setup",
            "default ssi configuration",
            // Question forms
            "how do i bulk upload ssis",
            "how to import settlement instructions",
            "how to migrate ssis",
        ],
    );
    m.insert(
        "cbu-custody.lookup-ssi",
        vec![
            // Core terms
            "lookup ssi",
            "find ssi",
            "resolve ssi",
            "which ssi",
            "ssi for trade",
            "ssi lookup",
            // Corner cases - search criteria
            "ssi for market",
            "ssi for currency",
            "ssi for instrument",
            "ssi for asset class",
            "ssi for counterparty",
            "ssi for settlement location",
            "ssi for csd",
            "ssi for custodian",
            "best ssi",
            "preferred ssi",
            "default ssi",
            "applicable ssi",
            "matching ssi",
            // Corner cases - trade context
            "settlement instruction for dvp",
            "settlement instruction for fop",
            "settlement instruction for buy",
            "settlement instruction for sell",
            "settlement instruction for delivery",
            "settlement instruction for receive",
            // Question forms
            "which ssi should i use",
            "what ssi applies",
            "how do i find the right ssi",
            "where does this trade settle",
        ],
    );

    // ==========================================================================
    // CUSTODY VERBS - BOOKING RULES - Enhanced with corner cases
    // ==========================================================================
    m.insert(
        "cbu-custody.add-booking-rule",
        vec![
            // Core terms
            "add booking rule",
            "routing rule",
            "settlement routing",
            "booking configuration",
            "trade routing",
            "alert rule",
            "ssi selection rule",
            // Corner cases - rule types
            "ssi routing rule",
            "trade booking rule",
            "instruction routing",
            "settlement rule",
            "allocation rule",
            "confirmation rule",
            "matching rule",
            "exception rule",
            "default routing",
            "fallback routing",
            "override rule",
            // Corner cases - criteria
            "rule by market",
            "rule by currency",
            "rule by asset class",
            "rule by instrument type",
            "rule by counterparty",
            "rule by trade type",
            "rule by settlement type",
            "rule by trade size",
            "rule by value",
            "rule by broker",
            // Corner cases - actions
            "route to ssi",
            "assign ssi",
            "select ssi",
            "determine settlement",
            "apply settlement logic",
            // Question forms
            "how do i add routing rule",
            "how to configure settlement routing",
            "how to setup booking rules",
        ],
    );
    m.insert(
        "cbu-custody.ensure-booking-rule",
        vec![
            // Core terms
            "ensure booking rule",
            "upsert booking rule",
            "idempotent booking rule",
            // Corner cases
            "get or create rule",
            "find or create rule",
            "rule if not exists",
            "safe rule create",
            "check and create rule",
        ],
    );
    m.insert(
        "cbu-custody.list-booking-rules",
        vec![
            // Core terms
            "list booking rules",
            "show routing rules",
            "all booking rules",
            "routing configuration",
            // Corner cases - views
            "rule inventory",
            "complete rule list",
            "active rules",
            "effective rules",
            "rule summary",
            "rule matrix",
            "rule catalog",
            // Corner cases - filters
            "rules by market",
            "rules by priority",
            "rules by status",
            "rules for cbu",
            "rules for entity",
            "rules for product",
            // Question forms
            "show me all routing rules",
            "what rules are configured",
            "how is settlement routing configured",
        ],
    );
    m.insert(
        "cbu-custody.update-rule-priority",
        vec![
            // Core terms
            "update rule priority",
            "change rule order",
            "reorder rules",
            "rule precedence",
            // Corner cases
            "adjust priority",
            "move rule up",
            "move rule down",
            "set rule rank",
            "rule sequence",
            "rule ordering",
            "priority adjustment",
            "execution order",
            "evaluation order",
            // Question forms
            "how do i change rule priority",
            "how to reorder rules",
        ],
    );
    m.insert(
        "cbu-custody.deactivate-rule",
        vec![
            // Core terms
            "deactivate rule",
            "disable booking rule",
            "remove routing rule",
            // Corner cases
            "turn off rule",
            "suspend rule",
            "pause rule",
            "retire rule",
            "archive rule",
            "delete rule",
            "rule inactive",
            "stop rule",
            // Question forms
            "how do i disable a rule",
            "how to remove routing rule",
        ],
    );

    // ==========================================================================
    // CUSTODY VERBS - AGENT OVERRIDES - Enhanced with corner cases
    // ==========================================================================
    m.insert(
        "cbu-custody.add-agent-override",
        vec![
            // Core terms
            "add agent override",
            "settlement chain override",
            "reag override",
            "deag override",
            "intermediary override",
            "agent chain",
            // Corner cases - override types
            "place of settlement override",
            "pset override",
            "safe override",
            "buyr override",
            "sell override",
            "receiving agent override",
            "delivering agent override",
            "custodian override",
            "sub-custodian override",
            "correspondent override",
            "beneficiary override",
            "settlement location override",
            // Corner cases - scenarios
            "cross border override",
            "market specific override",
            "counterparty specific override",
            "instrument specific override",
            "exception handling",
            "non-standard settlement",
            // Question forms
            "how do i add agent override",
            "how to override settlement chain",
            "how to specify different agent",
        ],
    );
    m.insert(
        "cbu-custody.list-agent-overrides",
        vec![
            // Core terms
            "list agent overrides",
            "show overrides",
            "settlement chain overrides",
            // Corner cases
            "all overrides",
            "active overrides",
            "override inventory",
            "override summary",
            "override by market",
            "override by counterparty",
            "configured overrides",
            // Question forms
            "what overrides are configured",
            "show me settlement overrides",
        ],
    );
    m.insert(
        "cbu-custody.remove-agent-override",
        vec![
            // Core terms
            "remove agent override",
            "delete override",
            "clear override",
            // Corner cases
            "revoke override",
            "cancel override",
            "end override",
            "disable override",
            "restore default agent",
            "remove exception",
            // Question forms
            "how do i remove override",
            "how to clear agent override",
        ],
    );

    // ==========================================================================
    // CUSTODY VERBS - ANALYSIS - Enhanced with corner cases
    // ==========================================================================
    m.insert(
        "cbu-custody.derive-required-coverage",
        vec![
            // Core terms
            "derive required coverage",
            "what ssis needed",
            "coverage analysis",
            "ssi gap analysis",
            "what do we need",
            // Corner cases - analysis types
            "calculate required ssis",
            "determine ssi needs",
            "ssi requirements",
            "settlement requirements",
            "coverage requirements",
            "gap assessment",
            "readiness assessment",
            "completeness check",
            "missing ssis",
            "required settlements",
            "settlement gaps",
            // Corner cases - scope
            "coverage for universe",
            "coverage for market",
            "coverage for asset class",
            "coverage for product",
            "coverage for mandate",
            "trading readiness",
            "go live requirements",
            // Question forms
            "what ssis do i need",
            "what settlement coverage is missing",
            "what do i need to trade",
            "how complete is settlement setup",
        ],
    );
    m.insert(
        "cbu-custody.validate-booking-coverage",
        vec![
            // Core terms
            "validate booking coverage",
            "check routing completeness",
            "booking gaps",
            "routing validation",
            "is routing complete",
            // Corner cases - validation types
            "coverage validation",
            "settlement coverage check",
            "routing coverage check",
            "complete coverage check",
            "full coverage validation",
            "ssi coverage validation",
            "booking rule validation",
            "rule completeness",
            "routing completeness",
            // Corner cases - results
            "coverage gaps",
            "missing coverage",
            "incomplete setup",
            "validation errors",
            "validation warnings",
            "coverage score",
            "coverage percentage",
            "readiness score",
            // Question forms
            "is settlement routing complete",
            "are all ssis covered",
            "is booking fully configured",
            "what coverage gaps exist",
        ],
    );

    // ==========================================================================
    // CUSTODY VERBS - SETTLEMENT EXTENSIONS - Enhanced with corner cases
    // ==========================================================================
    m.insert(
        "cbu-custody.define-settlement-chain",
        vec![
            // Core terms
            "define settlement chain",
            "settlement chain",
            "multi-hop settlement",
            "chain definition",
            "settlement path",
            "cross-border settlement",
            // Corner cases - chain types
            "correspondent chain",
            "custodian chain",
            "agent chain",
            "sub-custodian chain",
            "delivery chain",
            "receipt chain",
            "payment chain",
            "securities chain",
            // Corner cases - components
            "place of safekeeping",
            "place of settlement",
            "intermediary chain",
            "settlement location",
            "csd chain",
            "icsd chain",
            "direct membership",
            "indirect membership",
            "agent relationship",
            // Corner cases - scenarios
            "tri-party settlement",
            "bilateral settlement",
            "central clearing",
            "ccp settlement",
            "bridge settlement",
            "link settlement",
            // Question forms
            "how do i define settlement chain",
            "how to setup multi-hop settlement",
            "how does cross-border settlement work",
        ],
    );
    m.insert(
        "cbu-custody.list-settlement-chains",
        vec![
            // Core terms
            "list settlement chains",
            "show chains",
            "settlement paths",
            // Corner cases
            "all chains",
            "active chains",
            "chain inventory",
            "chain summary",
            "chains by market",
            "chains by csd",
            "chain configuration",
            "chain overview",
            // Question forms
            "what settlement chains exist",
            "show me settlement paths",
        ],
    );
    m.insert(
        "cbu-custody.set-fop-rules",
        vec![
            // Core terms
            "set fop rules",
            "free of payment rules",
            "fop allowed",
            "fop threshold",
            "dvp vs fop",
            "fop configuration",
            // Corner cases - rule types
            "fop permitted",
            "fop limit",
            "fop maximum",
            "fop exception",
            "conditional fop",
            "fop by counterparty",
            "fop by market",
            "fop by instrument",
            "fop by value",
            "fop by currency",
            // Corner cases - related terms
            "free delivery",
            "free receipt",
            "delivery free of payment",
            "receipt free of payment",
            "unilateral delivery",
            "risk exposure",
            "counterparty risk",
            // Question forms
            "how do i configure fop",
            "when can we settle free of payment",
            "what are fop rules",
            "can we do fop settlement",
        ],
    );
    m.insert(
        "cbu-custody.list-fop-rules",
        vec![
            // Core terms
            "list fop rules",
            "fop configuration",
            "show fop rules",
            // Corner cases
            "fop settings",
            "fop limits",
            "fop thresholds",
            "fop permissions",
            "active fop rules",
            "fop by market",
            // Question forms
            "what fop rules are configured",
            "show me fop configuration",
        ],
    );
    m.insert(
        "cbu-custody.set-csd-preference",
        vec![
            // Core terms
            "set csd preference",
            "preferred csd",
            "euroclear preference",
            "clearstream preference",
            "dtcc preference",
            "icsd preference",
            // Corner cases - csd types
            "central securities depository",
            "local csd",
            "domestic csd",
            "international csd",
            "primary csd",
            "secondary csd",
            "backup csd",
            "crest preference",
            "monte titoli preference",
            "iberclear preference",
            "jasdec preference",
            "ccdc preference",
            // Corner cases - preference scenarios
            "default csd",
            "preferred settlement location",
            "csd priority",
            "csd ranking",
            "csd selection",
            "automatic csd selection",
            // Question forms
            "how do i set csd preference",
            "which csd to use",
            "how to prefer euroclear",
        ],
    );
    m.insert(
        "cbu-custody.list-csd-preferences",
        vec![
            // Core terms
            "list csd preferences",
            "csd configuration",
            "show csd preferences",
            // Corner cases
            "csd settings",
            "csd priorities",
            "all csd preferences",
            "active csd preferences",
            "csd by market",
            // Question forms
            "what csd preferences are set",
            "show me csd configuration",
        ],
    );
    m.insert(
        "cbu-custody.set-settlement-cycle",
        vec![
            // Core terms
            "set settlement cycle",
            "settlement cycle override",
            "t+1",
            "t+2",
            "t+3",
            "settlement timing",
            // Corner cases - cycle types
            "t+0 settlement",
            "same day settlement",
            "next day settlement",
            "standard settlement",
            "non-standard settlement",
            "extended settlement",
            "shortened settlement",
            "negotiated settlement",
            "forward settlement",
            "spot settlement",
            // Corner cases - scenarios
            "cycle override",
            "cycle exception",
            "market specific cycle",
            "instrument specific cycle",
            "counterparty agreed cycle",
            "regulatory cycle",
            "csdr settlement",
            "settlement discipline",
            // Question forms
            "how do i set settlement cycle",
            "how to override settlement timing",
            "what is settlement cycle",
        ],
    );
    m.insert(
        "cbu-custody.list-settlement-cycle-overrides",
        vec![
            // Core terms
            "list settlement cycles",
            "cycle overrides",
            "settlement timing config",
            // Corner cases
            "all cycle overrides",
            "active overrides",
            "cycle exceptions",
            "non-standard cycles",
            "cycle by market",
            "cycle by instrument",
            // Question forms
            "what cycle overrides exist",
            "show me settlement cycle configuration",
        ],
    );

    // ==========================================================================
    // ENTITY SETTLEMENT VERBS - Enhanced with corner cases
    // ==========================================================================
    m.insert(
        "entity-settlement.set-identity",
        vec![
            // Core terms
            "set settlement identity",
            "counterparty identity",
            "settlement bic",
            "alert participant",
            "ctm participant",
            "counterparty setup",
            // Corner cases - identity types
            "lei code",
            "legal entity identifier",
            "bic code",
            "swift code",
            "participant id",
            "member id",
            "clearing member",
            "settlement member",
            "csd participant",
            "icsd participant",
            "trading party",
            "counterparty bic",
            // Corner cases - systems
            "alert id",
            "ctm id",
            "omgeo id",
            "dtcc id",
            "swift id",
            "traiana id",
            "markitwire id",
            // Corner cases - context
            "counterparty onboarding",
            "broker identity",
            "dealer identity",
            "custodian identity",
            "prime broker identity",
            "executing broker identity",
            "clearing broker identity",
            // Question forms
            "how do i set counterparty identity",
            "how to setup counterparty for settlement",
            "what is counterparty bic",
        ],
    );
    m.insert(
        "entity-settlement.add-ssi",
        vec![
            // Core terms
            "add counterparty ssi",
            "counterparty settlement",
            "their ssi",
            "broker ssi",
            "dealer ssi",
            // Corner cases - counterparty types
            "prime broker ssi",
            "executing broker ssi",
            "clearing broker ssi",
            "custodian ssi",
            "sub-custodian ssi",
            "agent ssi",
            "correspondent ssi",
            "beneficiary ssi",
            "intermediary ssi",
            "fund admin ssi",
            // Corner cases - direction
            "receive from counterparty",
            "deliver to counterparty",
            "counterparty delivery",
            "counterparty receipt",
            "incoming ssi",
            "outgoing ssi",
            // Question forms
            "how do i add counterparty ssi",
            "how to setup broker settlement",
            "where does counterparty want delivery",
        ],
    );
    m.insert(
        "entity-settlement.remove-ssi",
        vec![
            // Core terms
            "remove counterparty ssi",
            "delete their ssi",
            // Corner cases
            "revoke counterparty ssi",
            "end counterparty ssi",
            "cancel counterparty ssi",
            "retire counterparty ssi",
            "expire counterparty ssi",
            "old counterparty ssi",
            "superseded ssi",
            // Question forms
            "how do i remove counterparty ssi",
        ],
    );

    // ==========================================================================
    // PRICING CONFIG VERBS - Enhanced with corner cases
    // ==========================================================================
    m.insert(
        "pricing-config.set",
        vec![
            // Core terms
            "set pricing source",
            "pricing configuration",
            "valuation source",
            "bloomberg pricing",
            "reuters pricing",
            "price feed",
            "how to price",
            "pricing setup",
            // Corner cases - pricing vendors
            "bbg pricing",
            "refinitiv pricing",
            "markit pricing",
            "six pricing",
            "ice pricing",
            "factset pricing",
            "morningstar pricing",
            "internal pricing",
            "client pricing",
            "broker pricing",
            "fund admin pricing",
            "nav pricing",
            // Corner cases - price types
            "closing price",
            "opening price",
            "mid price",
            "bid price",
            "ask price",
            "last trade price",
            "vwap price",
            "settlement price",
            "theoretical price",
            "model price",
            "fair value",
            "nav per share",
            // Corner cases - timing
            "eod pricing",
            "intraday pricing",
            "real time pricing",
            "delayed pricing",
            "historical pricing",
            "t+1 pricing",
            // Question forms
            "how do i set pricing source",
            "how to configure valuation",
            "what pricing vendor to use",
            "where does price come from",
        ],
    );
    m.insert(
        "pricing-config.list",
        vec![
            // Core terms
            "list pricing config",
            "show pricing sources",
            "pricing setup",
            // Corner cases
            "all pricing sources",
            "active pricing config",
            "pricing by instrument",
            "pricing by market",
            "pricing summary",
            "pricing inventory",
            "configured pricing",
            // Question forms
            "what pricing is configured",
            "show me pricing sources",
        ],
    );
    m.insert(
        "pricing-config.remove",
        vec![
            // Core terms
            "remove pricing config",
            "delete pricing source",
            // Corner cases
            "revoke pricing",
            "cancel pricing",
            "disable pricing",
            "stop pricing",
            "end pricing subscription",
            // Question forms
            "how do i remove pricing",
        ],
    );
    m.insert(
        "pricing-config.find-for-instrument",
        vec![
            // Core terms
            "find pricing source",
            "which pricing",
            "pricing for instrument",
            "resolve pricing",
            // Corner cases - search
            "best pricing source",
            "preferred pricing source",
            "applicable pricing",
            "pricing lookup",
            "pricing resolution",
            "price source selection",
            "pricing hierarchy",
            "pricing waterfall",
            // Question forms
            "which pricing source applies",
            "where do i get price for this",
            "how is this instrument priced",
        ],
    );
    m.insert(
        "pricing-config.link-resource",
        vec![
            // Core terms
            "link pricing resource",
            "connect price feed",
            "pricing resource",
            // Corner cases
            "pricing data feed",
            "pricing connection",
            "pricing integration",
            "pricing api",
            "pricing subscription",
            "pricing license",
            "pricing entitlement",
            // Question forms
            "how do i connect pricing feed",
            "how to link pricing resource",
        ],
    );
    m.insert(
        "pricing-config.set-valuation-schedule",
        vec![
            // Core terms
            "set valuation schedule",
            "valuation frequency",
            "when to price",
            "eod pricing",
            "intraday pricing",
            "nav timing",
            // Corner cases - schedule types
            "daily valuation",
            "weekly valuation",
            "monthly valuation",
            "quarterly valuation",
            "annual valuation",
            "ad hoc valuation",
            "event driven valuation",
            "continuous valuation",
            "snapshot valuation",
            // Corner cases - timing
            "valuation cutoff",
            "valuation deadline",
            "pricing window",
            "nav calculation time",
            "pricing timestamp",
            "valuation point",
            "dealing cutoff",
            // Question forms
            "how often should we value",
            "when do we run pricing",
            "what is valuation schedule",
        ],
    );
    m.insert(
        "pricing-config.list-valuation-schedules",
        vec![
            // Core terms
            "list valuation schedules",
            "show pricing schedules",
            "valuation timing",
            // Corner cases
            "all schedules",
            "active schedules",
            "schedules by fund",
            "schedules by product",
            // Question forms
            "what valuation schedules exist",
            "show me pricing schedules",
        ],
    );
    m.insert(
        "pricing-config.set-fallback-chain",
        vec![
            // Core terms
            "set fallback chain",
            "pricing fallback",
            "backup pricing source",
            "secondary pricing",
            "price fallback",
            // Corner cases - fallback types
            "pricing hierarchy",
            "pricing waterfall",
            "pricing cascade",
            "alternative source",
            "backup source",
            "tertiary source",
            "last resort pricing",
            "manual pricing fallback",
            // Corner cases - scenarios
            "missing price fallback",
            "stale price fallback",
            "failed price fallback",
            "timeout fallback",
            "error fallback",
            // Question forms
            "what if primary price missing",
            "how to setup backup pricing",
            "what is fallback source",
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
    // INSTRUCTION PROFILE VERBS - Enhanced with corner cases
    // ==========================================================================
    m.insert(
        "instruction-profile.define-message-type",
        vec![
            // Core terms
            "define message type",
            "new message type",
            "mt message",
            "mx message",
            "swift message type",
            "fix message",
            "instruction type",
            // Corner cases - SWIFT MT types
            "mt202",
            "mt202cov",
            "mt540",
            "mt541",
            "mt542",
            "mt543",
            "mt544",
            "mt545",
            "mt546",
            "mt547",
            "mt548",
            "mt535",
            "mt536",
            "mt537",
            "mt564",
            "mt566",
            "mt578",
            "mt599",
            // Corner cases - ISO 20022 / MX
            "sese023",
            "sese024",
            "setr001",
            "setr002",
            "setr006",
            "setr010",
            "semt017",
            "camt053",
            "pacs008",
            "pacs009",
            // Corner cases - FIX protocol
            "fix allocation",
            "fix execution",
            "fix order",
            "fix confirmation",
            "fixml message",
            // Question forms
            "how do i define message type",
            "what message types are supported",
        ],
    );
    m.insert(
        "instruction-profile.list-message-types",
        vec![
            // Core terms
            "list message types",
            "available messages",
            "swift messages",
            "message catalog",
            // Corner cases
            "supported message types",
            "enabled messages",
            "message inventory",
            "message library",
            "mt messages",
            "mx messages",
            "iso messages",
            "fix messages",
            "proprietary messages",
            "message types by category",
            "settlement messages",
            "confirmation messages",
            "instruction messages",
            "reporting messages",
            "corporate action messages",
            // Question forms
            "what message types exist",
            "show me available messages",
        ],
    );
    m.insert(
        "instruction-profile.create-template",
        vec![
            // Core terms
            "create instruction template",
            "new template",
            "message template",
            "swift template",
            "instruction format",
            // Corner cases - template types
            "delivery template",
            "receipt template",
            "payment template",
            "dvp template",
            "fop template",
            "confirmation template",
            "cancellation template",
            "amendment template",
            "corporate action template",
            "settlement template",
            // Corner cases - components
            "template structure",
            "field mapping",
            "message layout",
            "instruction schema",
            "message specification",
            "template definition",
            // Question forms
            "how do i create instruction template",
            "how to setup message template",
        ],
    );
    m.insert(
        "instruction-profile.read-template",
        vec![
            // Core terms
            "read template",
            "show template",
            "template details",
            // Corner cases
            "template specification",
            "template structure",
            "template fields",
            "template contents",
            "view template",
            "display template",
            "template info",
            // Question forms
            "show me this template",
            "what is in this template",
        ],
    );
    m.insert(
        "instruction-profile.list-templates",
        vec![
            // Core terms
            "list templates",
            "available templates",
            "message templates",
            // Corner cases
            "template catalog",
            "template library",
            "template inventory",
            "all templates",
            "active templates",
            "templates by type",
            "templates by market",
            "templates by message",
            "delivery templates",
            "receipt templates",
            // Question forms
            "what templates are available",
            "show me all templates",
        ],
    );
    m.insert(
        "instruction-profile.assign-template",
        vec![
            // Core terms
            "assign template",
            "map template",
            "which template",
            "template for instrument",
            "template assignment",
            "how to instruct",
            // Corner cases - assignment criteria
            "assign by instrument class",
            "assign by market",
            "assign by currency",
            "assign by counterparty",
            "assign by settlement type",
            "default template",
            "specific template",
            "template mapping",
            "template binding",
            "template selection",
            "template rule",
            // Corner cases - scenarios
            "template for equities",
            "template for bonds",
            "template for derivatives",
            "template for fx",
            "template for funds",
            // Question forms
            "how do i assign template",
            "which template should i use",
            "what template applies",
        ],
    );
    m.insert(
        "instruction-profile.list-assignments",
        vec![
            // Core terms
            "list template assignments",
            "show assignments",
            "template mappings",
            // Corner cases
            "assignment matrix",
            "assignment overview",
            "all assignments",
            "active assignments",
            "assignments by template",
            "assignments by instrument",
            "assignments by market",
            "template coverage",
            // Question forms
            "what templates are assigned",
            "show me template mappings",
        ],
    );
    m.insert(
        "instruction-profile.remove-assignment",
        vec![
            // Core terms
            "remove assignment",
            "unassign template",
            "delete assignment",
            // Corner cases
            "revoke assignment",
            "clear mapping",
            "unbind template",
            "end assignment",
            "assignment removal",
            // Question forms
            "how do i remove assignment",
        ],
    );
    m.insert(
        "instruction-profile.add-field-override",
        vec![
            // Core terms
            "add field override",
            "override field",
            "custom field value",
            "field customization",
            "swift field override",
            "message field override",
            // Corner cases - field types
            "party field override",
            "account field override",
            "settlement field override",
            "narrative field override",
            "reference field override",
            "static field",
            "default value",
            "conditional field",
            // Corner cases - SWIFT specific
            "block override",
            "sequence override",
            "qualifier override",
            "party identifier override",
            "bic override",
            "account override",
            // Question forms
            "how do i override a field",
            "how to customize message field",
        ],
    );
    m.insert(
        "instruction-profile.list-field-overrides",
        vec![
            // Core terms
            "list field overrides",
            "show overrides",
            "field customizations",
            // Corner cases
            "all overrides",
            "active overrides",
            "overrides by template",
            "overrides by field",
            "field override summary",
            // Question forms
            "what overrides are configured",
            "show me field customizations",
        ],
    );
    m.insert(
        "instruction-profile.remove-field-override",
        vec![
            // Core terms
            "remove field override",
            "delete override",
            "clear override",
            // Corner cases
            "revoke override",
            "end override",
            "restore default",
            "reset field",
            // Question forms
            "how do i remove override",
        ],
    );
    m.insert(
        "instruction-profile.find-template",
        vec![
            // Core terms
            "find template",
            "which template for trade",
            "resolve template",
            "template lookup",
            // Corner cases
            "template selection",
            "template resolution",
            "applicable template",
            "matching template",
            "best template",
            "template for this trade",
            "template for this instrument",
            "template for this market",
            // Question forms
            "what template applies to this trade",
            "which template should i use",
        ],
    );
    m.insert(
        "instruction-profile.validate-profile",
        vec![
            // Core terms
            "validate instruction profile",
            "instruction gaps",
            "template coverage",
            "instruction completeness",
            // Corner cases
            "profile validation",
            "template validation",
            "coverage check",
            "completeness check",
            "instruction readiness",
            "message readiness",
            "validation errors",
            "validation warnings",
            // Question forms
            "is instruction profile complete",
            "what templates are missing",
        ],
    );
    m.insert(
        "instruction-profile.derive-required-templates",
        vec![
            // Core terms
            "derive required templates",
            "what templates needed",
            "template gap analysis",
            // Corner cases
            "calculate template needs",
            "template requirements",
            "missing templates",
            "template shortfall",
            "templates for universe",
            "templates for mandate",
            "instruction requirements",
            // Question forms
            "what templates do i need",
            "what instruction coverage is missing",
        ],
    );

    // ==========================================================================
    // TRADE GATEWAY VERBS - Enhanced with corner cases
    // ==========================================================================
    m.insert(
        "trade-gateway.define-gateway",
        vec![
            // Core terms
            "define gateway",
            "new gateway",
            "add gateway",
            "create gateway",
            "trade gateway",
            "swift gateway",
            "fix gateway",
            // Corner cases - gateway types
            "swiftnet gateway",
            "swiftnet link",
            "swift alliance",
            "fix engine",
            "fix connection",
            "oasys gateway",
            "omgeo gateway",
            "alert gateway",
            "ctm gateway",
            "traiana gateway",
            "markitwire gateway",
            "dtcc gateway",
            "proprietary gateway",
            "api gateway",
            "mq gateway",
            // Corner cases - connectivity
            "message gateway",
            "instruction gateway",
            "confirmation gateway",
            "matching gateway",
            "clearing gateway",
            "settlement gateway",
            // Question forms
            "how do i define gateway",
            "how to add new gateway",
        ],
    );
    m.insert(
        "trade-gateway.read-gateway",
        vec![
            // Core terms
            "read gateway",
            "gateway details",
            "show gateway",
            // Corner cases
            "gateway specification",
            "gateway configuration",
            "gateway settings",
            "gateway info",
            "view gateway",
            "gateway status",
            // Question forms
            "show me gateway details",
            "what is this gateway",
        ],
    );
    m.insert(
        "trade-gateway.list-gateways",
        vec![
            // Core terms
            "list gateways",
            "available gateways",
            "all gateways",
            "gateway catalog",
            // Corner cases
            "gateway inventory",
            "gateway library",
            "supported gateways",
            "enabled gateways",
            "active gateways",
            "gateways by type",
            "gateways by protocol",
            "swift gateways",
            "fix gateways",
            // Question forms
            "what gateways are available",
            "show me all gateways",
        ],
    );
    m.insert(
        "trade-gateway.enable-gateway",
        vec![
            // Core terms
            "enable gateway",
            "connect gateway",
            "gateway connectivity",
            "activate gateway connection",
            "setup gateway",
            // Corner cases
            "provision gateway",
            "establish connection",
            "gateway enrollment",
            "gateway subscription",
            "gateway onboarding",
            "connect to network",
            "join network",
            // Question forms
            "how do i enable gateway",
            "how to connect to gateway",
        ],
    );
    m.insert(
        "trade-gateway.activate-gateway",
        vec![
            // Core terms
            "activate gateway",
            "go live gateway",
            "gateway active",
            // Corner cases
            "gateway go live",
            "start gateway",
            "commence gateway",
            "gateway production",
            "gateway live",
            "turn on gateway",
            "gateway operational",
            // Question forms
            "how do i activate gateway",
            "how to go live with gateway",
        ],
    );
    m.insert(
        "trade-gateway.suspend-gateway",
        vec![
            // Core terms
            "suspend gateway",
            "disable gateway",
            "pause gateway",
            "gateway inactive",
            // Corner cases
            "gateway on hold",
            "freeze gateway",
            "stop gateway",
            "gateway maintenance",
            "disconnect gateway",
            "gateway offline",
            "gateway suspended",
            // Question forms
            "how do i suspend gateway",
            "how to disable gateway",
        ],
    );
    m.insert(
        "trade-gateway.list-cbu-gateways",
        vec![
            // Core terms
            "list cbu gateways",
            "cbu connectivity",
            "connected gateways",
            "gateway status",
            // Corner cases
            "client gateways",
            "enabled gateways for client",
            "active client gateways",
            "cbu gateway status",
            "gateway connectivity status",
            "gateway health",
            // Question forms
            "what gateways are connected",
            "show me client gateways",
        ],
    );
    m.insert(
        "trade-gateway.add-routing-rule",
        vec![
            // Core terms
            "add gateway routing",
            "route to gateway",
            "gateway rule",
            "which gateway",
            "trade routing",
            "instruction routing",
            // Corner cases - rule types
            "gateway selection rule",
            "routing configuration",
            "message routing",
            "instruction delivery",
            "gateway mapping",
            "gateway assignment",
            "default gateway",
            "specific gateway",
            // Corner cases - criteria
            "route by market",
            "route by instrument",
            "route by counterparty",
            "route by message type",
            "route by currency",
            "route by settlement type",
            // Question forms
            "how do i add routing rule",
            "how to route to specific gateway",
        ],
    );
    m.insert(
        "trade-gateway.list-routing-rules",
        vec![
            // Core terms
            "list routing rules",
            "gateway routing",
            "show routing",
            // Corner cases
            "routing matrix",
            "all routing rules",
            "active routing rules",
            "routing by gateway",
            "routing by market",
            "routing configuration",
            // Question forms
            "what routing rules exist",
            "show me gateway routing",
        ],
    );
    m.insert(
        "trade-gateway.remove-routing-rule",
        vec![
            // Core terms
            "remove routing rule",
            "delete gateway route",
            // Corner cases
            "revoke routing",
            "clear routing",
            "end routing rule",
            "disable routing",
            // Question forms
            "how do i remove routing rule",
        ],
    );
    m.insert(
        "trade-gateway.set-fallback",
        vec![
            // Core terms
            "set gateway fallback",
            "fallback gateway",
            "backup gateway",
            "gateway failover",
            // Corner cases
            "secondary gateway",
            "alternate gateway",
            "disaster recovery gateway",
            "dr gateway",
            "bcp gateway",
            "failover configuration",
            "gateway redundancy",
            "gateway backup",
            // Question forms
            "how do i set fallback gateway",
            "what if gateway fails",
        ],
    );
    m.insert(
        "trade-gateway.list-fallbacks",
        vec![
            // Core terms
            "list fallbacks",
            "gateway fallbacks",
            "failover config",
            // Corner cases
            "fallback configuration",
            "backup gateways",
            "secondary gateways",
            "dr configuration",
            // Question forms
            "what fallbacks are configured",
            "show me gateway backups",
        ],
    );
    m.insert(
        "trade-gateway.find-gateway",
        vec![
            // Core terms
            "find gateway",
            "which gateway for trade",
            "resolve gateway",
            "gateway lookup",
            // Corner cases
            "gateway selection",
            "gateway resolution",
            "applicable gateway",
            "matching gateway",
            "best gateway",
            "preferred gateway",
            "gateway for this trade",
            "gateway for this message",
            // Question forms
            "which gateway should i use",
            "what gateway applies",
        ],
    );
    m.insert(
        "trade-gateway.validate-routing",
        vec![
            // Core terms
            "validate gateway routing",
            "routing gaps",
            "gateway coverage",
            "routing completeness",
            // Corner cases
            "routing validation",
            "connectivity validation",
            "gateway readiness",
            "routing health check",
            "routing errors",
            "routing warnings",
            // Question forms
            "is routing complete",
            "what routing is missing",
        ],
    );
    m.insert(
        "trade-gateway.derive-required-routes",
        vec![
            // Core terms
            "derive required routes",
            "what routes needed",
            "routing gap analysis",
            // Corner cases
            "calculate routing needs",
            "routing requirements",
            "missing routes",
            "routing shortfall",
            "routes for universe",
            "routes for mandate",
            // Question forms
            "what routing do i need",
            "what gateway coverage is missing",
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
    // GLEIF VERBS - LEI enrichment and corporate tree import
    // ==========================================================================
    m.insert(
        "gleif.enrich",
        vec![
            "enrich from gleif",
            "fetch gleif data",
            "get lei data",
            "pull lei info",
            "gleif lookup",
            "lei lookup",
            "enrich entity from lei",
            "populate from gleif",
            "sync gleif",
            "update from lei",
            "lei enrichment",
            "gleif enrichment",
            "fetch lei record",
            "get legal entity identifier",
            "lookup lei",
            "lei details",
            "gleif record",
        ],
    );
    m.insert(
        "gleif.import-tree",
        vec![
            "import gleif tree",
            "import corporate tree",
            "fetch ownership tree",
            "get parent chain",
            "build ownership structure",
            "import lei hierarchy",
            "scrape gleif",
            "fetch subsidiaries",
            "get corporate structure",
            "lei tree",
            "gleif hierarchy",
            "parent relationships",
            "ultimate parent",
            "direct parent",
            "corporate ownership",
            "ownership chain from lei",
            "lei parents",
            "gleif parents",
        ],
    );
    m.insert(
        "gleif.refresh",
        vec![
            "refresh gleif",
            "update lei data",
            "resync gleif",
            "refresh lei",
            "check lei expiry",
            "update gleif data",
            "gleif resync",
            "lei refresh",
            "sync lei",
            "gleif update",
        ],
    );

    // ==========================================================================
    // BODS VERBS - Beneficial Ownership Data Standard
    // ==========================================================================
    m.insert(
        "bods.discover-ubos",
        vec![
            "discover ubos from bods",
            "bods ubo lookup",
            "beneficial ownership data",
            "fetch bods data",
            "bods query",
            "who owns this company",
            "find beneficial owners",
            "ubo discovery",
            "beneficial ownership lookup",
            "bods ubo",
            "ownership disclosure",
            "psc lookup",
            "persons with significant control",
        ],
    );
    m.insert(
        "bods.import-ownership",
        vec![
            "import bods ownership",
            "bods import",
            "beneficial ownership import",
            "import psc data",
            "bods ownership chain",
            "import disclosed owners",
            "bods to ubo",
            "convert bods",
        ],
    );
    m.insert(
        "bods.refresh",
        vec![
            "refresh bods",
            "update bods data",
            "resync bods",
            "bods resync",
            "ubo data refresh",
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
    // TEMPORAL VERBS - Enhanced with corner cases
    // ==========================================================================
    m.insert(
        "temporal.ownership-as-of",
        vec![
            "ownership as of",
            "historical ownership",
            "ownership on date",
            "past ownership",
            "point in time ownership",
            // Corner cases
            "ownership at date",
            "ownership snapshot",
            "historical shareholding",
            "past shareholding",
            "ownership history",
            "ownership timeline",
            "who owned on date",
            "shareholders on date",
            "cap table as of",
            "historical cap table",
            "backdated ownership",
            "retroactive ownership",
        ],
    );
    m.insert(
        "temporal.ubo-chain-as-of",
        vec![
            "ubo chain as of",
            "historical ubo",
            "ubo on date",
            "past ubo structure",
            // Corner cases
            "ubo at date",
            "ubo snapshot",
            "historical beneficial owners",
            "past beneficial owners",
            "ubo timeline",
            "ubo history",
            "who were ubos on date",
            "25% owners on date",
            "historical control chain",
            "past control structure",
        ],
    );
    m.insert(
        "temporal.cbu-relationships-as-of",
        vec![
            "cbu relationships as of",
            "historical relationships",
            "relationships on date",
            // Corner cases
            "relationships at date",
            "relationship snapshot",
            "past relationships",
            "historical parties",
            "parties on date",
            "structure on date",
            "roles on date",
            "who was involved on date",
        ],
    );
    m.insert(
        "temporal.cbu-roles-as-of",
        vec![
            "roles as of",
            "historical roles",
            "roles on date",
            // Corner cases
            "roles at date",
            "role snapshot",
            "past roles",
            "who held role on date",
            "role history",
            "role timeline",
            "historical appointments",
            "past appointments",
        ],
    );
    m.insert(
        "temporal.cbu-state-at-approval",
        vec![
            "state at approval",
            "snapshot at approval",
            "what was approved",
            // Corner cases
            "approval snapshot",
            "approved state",
            "approval baseline",
            "what was onboarded",
            "onboarding snapshot",
            "go live state",
            "activation state",
            "decision point state",
            "point of approval",
            "approval evidence",
        ],
    );
    m.insert(
        "temporal.relationship-history",
        vec![
            "relationship history",
            "audit trail",
            "change history",
            // Corner cases
            "relationship audit",
            "relationship changes",
            "relationship timeline",
            "what changed",
            "history of changes",
            "modification history",
            "amendment history",
            "update history",
            "version history",
            "all versions",
            "change log",
            "audit log",
        ],
    );
    m.insert(
        "temporal.entity-history",
        vec![
            "entity history",
            "entity changes",
            "entity audit",
            // Corner cases
            "entity timeline",
            "entity modifications",
            "entity amendments",
            "entity versions",
            "what changed about entity",
            "entity change log",
            "entity audit trail",
            "historical entity data",
            "past entity state",
        ],
    );
    m.insert(
        "temporal.compare-ownership",
        vec![
            "compare ownership",
            "ownership diff",
            "what changed between dates",
            // Corner cases
            "ownership comparison",
            "structure comparison",
            "before and after",
            "ownership changes",
            "structure changes",
            "delta between dates",
            "diff between snapshots",
            "changes since date",
            "changes between dates",
            "ownership variance",
            "cap table diff",
        ],
    );

    // ==========================================================================
    // TEAM VERBS - Enhanced with corner cases
    // ==========================================================================
    m.insert(
        "team.create",
        vec![
            "create team",
            "new team",
            "add team",
            "setup team",
            // Corner cases
            "establish team",
            "form team",
            "team creation",
            "new workgroup",
            "new department",
            "new unit",
            "operations team",
            "compliance team",
            "onboarding team",
            "kyc team",
            "review team",
            "approval team",
        ],
    );
    m.insert(
        "team.read",
        vec![
            "read team",
            "team details",
            "show team",
            // Corner cases
            "view team",
            "get team",
            "team info",
            "team information",
            "team summary",
            "team overview",
            "team profile",
        ],
    );
    m.insert(
        "team.archive",
        vec![
            "archive team",
            "deactivate team",
            "remove team",
            // Corner cases
            "disable team",
            "retire team",
            "sunset team",
            "close team",
            "team archived",
            "team inactive",
            "team deleted",
            "dissolve team",
        ],
    );
    m.insert(
        "team.add-member",
        vec![
            "add team member",
            "add user to team",
            "join team",
            "team membership",
            // Corner cases
            "add member",
            "new member",
            "member addition",
            "add to team",
            "assign to team",
            "include in team",
            "team onboarding",
            "member onboarding",
        ],
    );
    m.insert(
        "team.remove-member",
        vec![
            "remove team member",
            "leave team",
            "remove from team",
            // Corner cases
            "delete member",
            "remove member",
            "member removal",
            "exit team",
            "team offboarding",
            "member offboarding",
            "remove user from team",
        ],
    );
    m.insert(
        "team.update-member",
        vec![
            "update member",
            "change member role",
            "modify membership",
            // Corner cases
            "member update",
            "change member",
            "modify member",
            "member role change",
            "promote member",
            "demote member",
            "update permissions",
            "change access level",
        ],
    );
    m.insert(
        "team.transfer-member",
        vec![
            "transfer member",
            "move to team",
            "reassign to team",
            // Corner cases
            "team transfer",
            "member transfer",
            "move member",
            "relocate member",
            "change team",
            "switch team",
            "cross team transfer",
        ],
    );
    m.insert(
        "team.add-governance-member",
        vec![
            "add governance member",
            "board member",
            "committee member",
            // Corner cases
            "governance role",
            "add to governance",
            "governance access",
            "oversight role",
            "steering committee",
            "advisory board",
            "executive committee",
        ],
    );
    m.insert(
        "team.verify-governance-access",
        vec![
            "verify governance access",
            "audit governance",
            "governance check",
            // Corner cases
            "governance audit",
            "governance verification",
            "access verification",
            "permission audit",
            "entitlement audit",
            "check governance",
            "validate governance",
        ],
    );
    m.insert(
        "team.add-cbu-access",
        vec![
            "add cbu access",
            "team cbu access",
            "grant access",
            // Corner cases
            "grant cbu access",
            "cbu entitlement",
            "client access",
            "customer access",
            "enable cbu",
            "authorize cbu",
            "cbu permission",
            "team can access cbu",
        ],
    );
    m.insert(
        "team.remove-cbu-access",
        vec![
            "remove cbu access",
            "revoke access",
            // Corner cases
            "revoke cbu access",
            "remove cbu entitlement",
            "disable cbu access",
            "unauthorized cbu",
            "remove cbu permission",
            "team cannot access cbu",
        ],
    );
    m.insert(
        "team.grant-service",
        vec![
            "grant service",
            "team entitlement",
            "enable service",
            // Corner cases
            "service entitlement",
            "service access",
            "enable function",
            "grant function",
            "feature entitlement",
            "feature access",
            "capability grant",
        ],
    );
    m.insert(
        "team.revoke-service",
        vec![
            "revoke service",
            "remove entitlement",
            "disable service",
            // Corner cases
            "service removal",
            "service revocation",
            "disable function",
            "revoke function",
            "feature removal",
            "capability revocation",
        ],
    );
    m.insert(
        "team.list-members",
        vec![
            "list team members",
            "team roster",
            "who is on team",
            // Corner cases
            "team members",
            "member list",
            "team composition",
            "team headcount",
            "team directory",
            "team contacts",
        ],
    );
    m.insert(
        "team.list-cbus",
        vec![
            "list team cbus",
            "team access",
            "cbus for team",
            // Corner cases
            "team cbus",
            "accessible cbus",
            "team clients",
            "team customers",
            "team portfolio",
            "assigned cbus",
        ],
    );

    // ==========================================================================
    // USER VERBS - Enhanced with corner cases
    // ==========================================================================
    m.insert(
        "user.create",
        vec![
            "create user",
            "new user",
            "add user",
            "register user",
            // Corner cases
            "user creation",
            "user registration",
            "new employee",
            "new staff",
            "new analyst",
            "new reviewer",
            "user onboarding",
            "provision user",
            "setup user",
        ],
    );
    m.insert(
        "user.suspend",
        vec![
            "suspend user",
            "disable user",
            "deactivate user",
            // Corner cases
            "user suspension",
            "user disabled",
            "user inactive",
            "lock user",
            "block user",
            "freeze user",
            "temporary disable",
            "security hold",
        ],
    );
    m.insert(
        "user.reactivate",
        vec![
            "reactivate user",
            "enable user",
            "activate user",
            // Corner cases
            "user reactivation",
            "user enabled",
            "user active",
            "unlock user",
            "unblock user",
            "unfreeze user",
            "restore user",
            "reinstate user",
        ],
    );
    m.insert(
        "user.offboard",
        vec![
            "offboard user",
            "terminate user",
            "user left company",
            // Corner cases
            "user termination",
            "user offboarding",
            "leaver",
            "employee exit",
            "staff exit",
            "remove user permanently",
            "delete user",
            "user departure",
        ],
    );
    m.insert(
        "user.list-teams",
        vec![
            "user teams",
            "which teams",
            "team membership",
            // Corner cases
            "user team membership",
            "teams for user",
            "user belongs to",
            "user member of",
            "user groups",
            "user affiliations",
        ],
    );
    m.insert(
        "user.list-cbus",
        vec![
            "user cbus",
            "user access",
            "what can user access",
            // Corner cases
            "user cbu access",
            "cbus for user",
            "user clients",
            "user customers",
            "user portfolio",
            "accessible cbus",
            "user entitlements",
        ],
    );
    m.insert(
        "user.check-access",
        vec![
            "check user access",
            "can user access",
            "access check",
            // Corner cases
            "verify user access",
            "access verification",
            "permission check",
            "entitlement check",
            "authorization check",
            "is user authorized",
            "does user have access",
        ],
    );

    // ==========================================================================
    // SLA VERBS - Enhanced with corner cases
    // ==========================================================================
    m.insert(
        "sla.list-templates",
        vec![
            // Core terms
            "list sla templates",
            "sla catalog",
            "available slas",
            "sla options",
            // Corner cases - service levels
            "service level agreements",
            "service level options",
            "what slas are available",
            "standard sla options",
            "premium sla",
            "gold sla",
            "silver sla",
            "bronze sla",
            "tiered service levels",
            "sla tiers",
            "service commitments catalog",
            "kpi templates",
            "performance templates",
            // Question forms
            "what slas can i choose",
            "show me sla options",
            "how to choose sla",
        ],
    );
    m.insert(
        "sla.read-template",
        vec![
            "read sla template",
            "sla details",
            "template details",
            "sla specification",
            // Corner cases
            "sla terms",
            "sla conditions",
            "sla metrics",
            "sla kpis",
            "turnaround times",
            "response times",
            "service windows",
            "availability targets",
            "uptime commitment",
            "sla penalties",
            "sla credits",
            "service credits",
            "breach consequences",
            "performance targets",
            "sla thresholds",
            // Question forms
            "what does this sla include",
            "sla requirements",
        ],
    );
    m.insert(
        "sla.commit",
        vec![
            "commit sla",
            "sla commitment",
            "agree to sla",
            "sla agreement",
            // Corner cases
            "accept sla",
            "sign up for sla",
            "subscribe to sla",
            "select sla",
            "choose sla",
            "commit to service level",
            "sla selection",
            "service level commitment",
            "contractual commitment",
            "formalize sla",
            "lock in sla",
            "activate sla",
            "sla effective date",
            "sla start date",
            "begin sla coverage",
            "sla enrollment",
            // Question forms
            "how to commit to sla",
            "how to select sla",
        ],
    );
    m.insert(
        "sla.bind-to-profile",
        vec![
            "bind sla to profile",
            "sla for profile",
            "link sla",
            // Corner cases
            "associate sla with trading profile",
            "trading profile sla",
            "profile service level",
            "matrix sla",
            "trading sla",
            "custody profile sla",
            "execution sla",
            "settlement sla for profile",
            "attach sla to profile",
            "profile level agreement",
        ],
    );
    m.insert(
        "sla.bind-to-service",
        vec![
            "bind sla to service",
            "service sla",
            // Corner cases
            "custody sla",
            "fund accounting sla",
            "transfer agency sla",
            "ta sla",
            "fa sla",
            "reconciliation sla",
            "reporting sla",
            "corporate action sla",
            "pricing sla",
            "nav sla",
            "service level for product",
            "product sla",
            "service specific sla",
            "attach sla to service",
            "product service level",
            // Question forms
            "what sla for this service",
            "service turnaround",
        ],
    );
    m.insert(
        "sla.bind-to-resource",
        vec![
            "bind sla to resource",
            "resource sla",
            // Corner cases
            "account sla",
            "portfolio sla",
            "fund sla",
            "share class sla",
            "instrument level sla",
            "market level sla",
            "currency sla",
            "resource specific commitment",
            "granular sla",
            "detailed service level",
            "resource level agreement",
        ],
    );
    m.insert(
        "sla.bind-to-isda",
        vec![
            "bind sla to isda",
            "isda sla",
            // Corner cases
            "derivatives sla",
            "otc sla",
            "swap sla",
            "isda agreement sla",
            "master agreement sla",
            "derivative service level",
            "margin call sla",
            "collateral sla",
            "vm sla",
            "im sla",
        ],
    );
    m.insert(
        "sla.bind-to-csa",
        vec![
            "bind sla to csa",
            "csa sla",
            // Corner cases
            "credit support sla",
            "collateral management sla",
            "margin sla",
            "collateral call sla",
            "collateral transfer sla",
            "substitution sla",
            "recall sla",
            "interest sla",
        ],
    );
    m.insert(
        "sla.list-commitments",
        vec![
            "list sla commitments",
            "cbu slas",
            "all commitments",
            // Corner cases
            "my slas",
            "client slas",
            "active slas",
            "current commitments",
            "sla portfolio",
            "sla summary",
            "sla dashboard",
            "committed service levels",
            "what slas do we have",
            "service level summary",
            "all service agreements",
            "sla inventory",
            // Question forms
            "what slas am i on",
            "show all slas",
        ],
    );
    m.insert(
        "sla.suspend-commitment",
        vec![
            "suspend sla",
            "pause commitment",
            // Corner cases
            "temporarily disable sla",
            "sla holiday",
            "sla suspension",
            "freeze sla",
            "pause service level",
            "sla on hold",
            "defer sla",
            "waive sla temporarily",
            "force majeure",
            "exceptional circumstances",
            "sla exemption",
            "grace period",
        ],
    );
    m.insert(
        "sla.record-measurement",
        vec![
            "record sla measurement",
            "sla metric",
            "measure sla",
            // Corner cases
            "log sla performance",
            "sla data point",
            "track sla",
            "sla observation",
            "kpi measurement",
            "performance measurement",
            "turnaround measurement",
            "response time log",
            "actual vs target",
            "sla tracking",
            "capture metric",
            "record kpi",
            "log performance",
        ],
    );
    m.insert(
        "sla.list-measurements",
        vec![
            "list sla measurements",
            "sla history",
            "measurement history",
            // Corner cases
            "sla performance history",
            "historical performance",
            "sla trend",
            "kpi history",
            "turnaround history",
            "response time history",
            "sla report",
            "performance report",
            "sla dashboard data",
            "sla analytics",
            "sla statistics",
            "average performance",
            "sla percentile",
            // Question forms
            "how have we performed",
            "sla track record",
        ],
    );
    m.insert(
        "sla.report-breach",
        vec![
            "report sla breach",
            "sla violation",
            "sla failure",
            // Corner cases
            "missed sla",
            "sla miss",
            "breached commitment",
            "failed kpi",
            "missed deadline",
            "late delivery",
            "service failure",
            "performance failure",
            "target missed",
            "sla exception",
            "log breach",
            "record breach",
            "breach notification",
            "service credit trigger",
            "penalty trigger",
            // Question forms
            "how to report sla miss",
        ],
    );
    m.insert(
        "sla.update-remediation",
        vec![
            "update remediation",
            "breach remediation",
            "fix sla breach",
            // Corner cases
            "remediation plan",
            "corrective action",
            "root cause analysis",
            "rca",
            "breach response",
            "recovery plan",
            "action plan",
            "improvement plan",
            "prevent recurrence",
            "remediation status",
            "remediation progress",
            "fix progress",
            "mitigation steps",
        ],
    );
    m.insert(
        "sla.resolve-breach",
        vec![
            "resolve breach",
            "breach resolved",
            "sla fixed",
            // Corner cases
            "close breach",
            "breach closure",
            "breach completed",
            "remediation complete",
            "sla restored",
            "back on track",
            "issue resolved",
            "mark resolved",
            "breach sign off",
            "accept resolution",
            "confirm fix",
            "close incident",
        ],
    );
    m.insert(
        "sla.escalate-breach",
        vec![
            "escalate breach",
            "sla escalation",
            // Corner cases
            "escalate to management",
            "management escalation",
            "senior escalation",
            "executive escalation",
            "priority escalation",
            "urgent breach",
            "critical breach",
            "repeated breach",
            "systemic issue",
            "pattern of breaches",
            "raise to senior",
            "notify management",
            "alert leadership",
        ],
    );
    m.insert(
        "sla.list-open-breaches",
        vec![
            "list open breaches",
            "active breaches",
            "unresolved slas",
            // Corner cases
            "outstanding breaches",
            "pending remediation",
            "open incidents",
            "current breaches",
            "breach backlog",
            "breach queue",
            "unremediated breaches",
            "awaiting resolution",
            "breach dashboard",
            "breach report",
            "aged breaches",
            "overdue remediation",
            // Question forms
            "what breaches are open",
            "how many breaches",
        ],
    );

    // ==========================================================================
    // REGULATORY VERBS - Enhanced with corner cases
    // ==========================================================================
    m.insert(
        "regulatory.registration.add",
        vec![
            "add regulatory registration",
            "register with regulator",
            "fca registration",
            "sec registration",
            "regulatory license",
            // Corner cases - regulators
            "esma registration",
            "cftc registration",
            "finra registration",
            "nfa registration",
            "cssf registration",
            "bafin registration",
            "amf registration",
            "mas registration",
            "sfc registration",
            "hkma registration",
            "asic registration",
            "cbi registration",
            "fatf compliance",
            "aml registration",
            // License types
            "aifm license",
            "ucits license",
            "mifid license",
            "investment firm license",
            "fund manager license",
            "custodian license",
            "depositary license",
            "broker dealer license",
            "investment adviser registration",
            "cpt license",
            "cta license",
            // Actions
            "submit registration",
            "apply for license",
            "regulatory submission",
            "new registration",
            "add license",
            "record registration",
            "regulatory filing",
            // Question forms
            "how to register with regulator",
            "how to add license",
        ],
    );
    m.insert(
        "regulatory.registration.list",
        vec![
            "list registrations",
            "regulatory status",
            "all registrations",
            // Corner cases
            "all licenses",
            "license inventory",
            "regulatory inventory",
            "regulatory portfolio",
            "license summary",
            "regulatory dashboard",
            "current registrations",
            "active licenses",
            "valid registrations",
            "regulatory compliance status",
            "registration overview",
            "jurisdictional registrations",
            "cross border registrations",
            // Question forms
            "what registrations do we have",
            "which regulators",
            "where are we registered",
        ],
    );
    m.insert(
        "regulatory.registration.verify",
        vec![
            "verify registration",
            "check registration",
            "registration verification",
            // Corner cases
            "validate license",
            "confirm registration",
            "registration check",
            "license lookup",
            "regulatory lookup",
            "verify license status",
            "check license validity",
            "is registered",
            "is licensed",
            "registration valid",
            "license current",
            "not expired",
            "good standing",
            "regulatory standing",
            // Question forms
            "is this registration valid",
            "is license current",
        ],
    );
    m.insert(
        "regulatory.registration.remove",
        vec![
            "remove registration",
            "withdraw registration",
            "deregister",
            // Corner cases
            "cancel license",
            "revoke registration",
            "surrender license",
            "terminate registration",
            "end registration",
            "registration withdrawal",
            "voluntary deregistration",
            "exit jurisdiction",
            "close license",
            "discontinue registration",
            "lapse registration",
            "let license expire",
            "non renewal",
            "registration termination",
        ],
    );
    m.insert(
        "regulatory.status.check",
        vec![
            "check regulatory status",
            "is regulated",
            "regulatory check",
            // Corner cases
            "regulatory standing",
            "compliance status",
            "license status",
            "registration status",
            "good standing check",
            "regulatory health",
            "enforcement check",
            "sanctions check regulatory",
            "disciplinary history",
            "regulatory record",
            "clean record",
            "regulatory clearance",
            "fit and proper",
            "regulatory capital",
            "prudential requirements",
            // Question forms
            "are we in good standing",
            "any regulatory issues",
            "regulatory concerns",
        ],
    );
    m.insert(
        "regulatory.registration.renew",
        vec![
            "renew registration",
            "renew license",
            "registration renewal",
            // Corner cases
            "license renewal",
            "annual renewal",
            "periodic renewal",
            "extend registration",
            "registration extension",
            "re-registration",
            "renewal application",
            "renewal submission",
            "renewal fee",
            "renewal deadline",
            "prevent lapse",
            "maintain registration",
            "continue registration",
            // Question forms
            "when to renew",
            "how to renew license",
        ],
    );
    m.insert(
        "regulatory.registration.update",
        vec![
            "update registration",
            "amend registration",
            "registration update",
            // Corner cases
            "modify license",
            "change registration details",
            "regulatory update",
            "material change",
            "notification to regulator",
            "form adv update",
            "annual update",
            "significant change",
            "update registered details",
            "change of particulars",
            "regulatory notification",
            "form pf update",
            "regulatory amendment",
        ],
    );
    m.insert(
        "regulatory.registration.list-pending",
        vec![
            "pending registrations",
            "applications in progress",
            "registration queue",
            // Corner cases
            "awaiting approval",
            "under review",
            "submitted applications",
            "registration pipeline",
            "pending licenses",
            "outstanding applications",
            "regulatory applications in flight",
            "application status",
            "approval timeline",
            // Question forms
            "what applications are pending",
            "any pending approvals",
        ],
    );
    m.insert(
        "regulatory.registration.list-expiring",
        vec![
            "expiring registrations",
            "upcoming renewals",
            "renewal calendar",
            // Corner cases
            "licenses expiring soon",
            "renewal due dates",
            "expiration dates",
            "90 day warning",
            "30 day warning",
            "renewal reminders",
            "upcoming deadlines",
            "regulatory calendar",
            "compliance calendar",
            "expiring licenses",
            // Question forms
            "what is expiring",
            "when do registrations expire",
        ],
    );

    // ==========================================================================
    // SEMANTIC VERBS - Enhanced with corner cases
    // ==========================================================================
    m.insert(
        "semantic.get-state",
        vec![
            "get semantic state",
            "onboarding progress",
            "where are we",
            "stage progress",
            // Corner cases - status queries
            "current state",
            "current stage",
            "onboarding status",
            "workflow status",
            "progress check",
            "completion status",
            "readiness status",
            "how far along",
            "percentage complete",
            "completion percentage",
            "stages done",
            "stages remaining",
            "milestone status",
            "checkpoint status",
            "workflow position",
            "semantic position",
            // Question forms
            "where am i in onboarding",
            "how much is complete",
            "what stage are we at",
            "are we on track",
        ],
    );
    m.insert(
        "semantic.list-stages",
        vec![
            "list stages",
            "all stages",
            "stage definitions",
            // Corner cases
            "onboarding stages",
            "workflow stages",
            "stage catalog",
            "stage inventory",
            "available stages",
            "stage sequence",
            "stage order",
            "stage hierarchy",
            "parent stages",
            "child stages",
            "sub stages",
            "stage taxonomy",
            "stage tree",
            "stage map",
            // Question forms
            "what stages exist",
            "show all stages",
            "what is the stage structure",
        ],
    );
    m.insert(
        "semantic.stages-for-product",
        vec![
            "stages for product",
            "product stages",
            "required stages",
            // Corner cases
            "custody stages",
            "fund accounting stages",
            "ta stages",
            "transfer agency stages",
            "derivatives stages",
            "fx stages",
            "securities lending stages",
            "product specific stages",
            "mandatory stages",
            "optional stages",
            "conditional stages",
            "product workflow",
            "product onboarding steps",
            "service stages",
            // Question forms
            "what stages for this product",
            "which stages apply",
            "product requirements",
        ],
    );
    m.insert(
        "semantic.next-actions",
        vec![
            "next actions",
            "what to do next",
            "suggested actions",
            "actionable stages",
            // Corner cases
            "recommended actions",
            "next steps",
            "pending actions",
            "outstanding actions",
            "todo list",
            "action items",
            "work items",
            "tasks to complete",
            "blockers",
            "dependencies",
            "critical path",
            "priority actions",
            "urgent actions",
            "immediate actions",
            "quick wins",
            "low hanging fruit",
            "agent suggestions",
            "ai recommendations",
            // Question forms
            "what should i do next",
            "what is blocking",
            "what is the priority",
        ],
    );
    m.insert(
        "semantic.missing-entities",
        vec![
            "missing entities",
            "what is missing",
            "gaps in structure",
            // Corner cases
            "incomplete structure",
            "missing data",
            "incomplete data",
            "data gaps",
            "entity gaps",
            "missing roles",
            "missing documents",
            "outstanding requirements",
            "unfulfilled requirements",
            "incomplete kyc",
            "kyc gaps",
            "structural gaps",
            "missing relationships",
            "orphan entities",
            "unlinked entities",
            "missing ownership",
            "missing control",
            "missing ubos",
            // Question forms
            "what is not complete",
            "what do we need",
            "what is outstanding",
        ],
    );
    m.insert(
        "semantic.prompt-context",
        vec![
            "prompt context",
            "agent context",
            "session context",
            // Corner cases
            "conversation context",
            "current context",
            "working context",
            "focus entity",
            "selected entity",
            "active cbu",
            "context summary",
            "state summary",
            "session state",
            "conversation state",
            "what are we working on",
            "current focus",
            "navigation context",
            "breadcrumb",
            "where am i",
            // Question forms
            "what is the context",
            "what am i looking at",
            "which entity",
        ],
    );
    m.insert(
        "semantic.transition-stage",
        vec![
            "transition stage",
            "move to stage",
            "advance stage",
            // Corner cases
            "complete stage",
            "finish stage",
            "stage transition",
            "next stage",
            "progress stage",
            "promote stage",
            "stage advancement",
            "milestone complete",
            "checkpoint complete",
            "mark stage complete",
            "stage done",
            "stage finished",
            // Question forms
            "how to advance stage",
            "can i complete this stage",
        ],
    );
    m.insert(
        "semantic.reset-stage",
        vec![
            "reset stage",
            "reopen stage",
            "stage rollback",
            // Corner cases
            "undo stage",
            "revert stage",
            "un-complete stage",
            "stage regression",
            "go back to stage",
            "re-enter stage",
            "stage restart",
            "start over",
            "redo stage",
            "reprocess stage",
        ],
    );
    m.insert(
        "semantic.stage-dependencies",
        vec![
            "stage dependencies",
            "what depends on what",
            "dependency graph",
            // Corner cases
            "prerequisite stages",
            "required before",
            "must complete first",
            "blocking stages",
            "dependent stages",
            "downstream stages",
            "upstream stages",
            "stage order",
            "stage sequence",
            "parallel stages",
            "concurrent stages",
            "sequential stages",
            // Question forms
            "what must complete before",
            "what is blocking this stage",
        ],
    );
    m.insert(
        "semantic.stage-history",
        vec![
            "stage history",
            "stage audit trail",
            "stage transitions",
            // Corner cases
            "workflow history",
            "onboarding history",
            "stage timeline",
            "when was stage completed",
            "who completed stage",
            "stage timestamps",
            "transition log",
            "workflow log",
            "progress history",
            "milestone history",
            "historical progress",
            // Question forms
            "when did we complete this stage",
            "show stage history",
        ],
    );

    // ==========================================================================
    // CASH SWEEP VERBS - Enhanced with corner cases
    // ==========================================================================
    m.insert(
        "cash-sweep.configure",
        vec![
            "configure cash sweep",
            "setup sweep",
            "stif configuration",
            "cash management",
            // Corner cases - sweep types
            "auto sweep",
            "automated cash sweep",
            "end of day sweep",
            "eod sweep",
            "intraday sweep",
            "overnight sweep",
            "idle cash sweep",
            "excess cash sweep",
            "target balance sweep",
            "zero balance sweep",
            "notional pooling",
            "physical pooling",
            "cash concentration",
            "liquidity management",
            // Vehicle types
            "mmf sweep",
            "money market fund sweep",
            "repo sweep",
            "overnight repo",
            "reverse repo",
            "tri party repo",
            "bank deposit sweep",
            "time deposit sweep",
            "term deposit sweep",
            "cd sweep",
            "commercial paper sweep",
            // Question forms
            "how to setup sweep",
            "configure cash management",
            "how to invest idle cash",
        ],
    );
    m.insert(
        "cash-sweep.link-resource",
        vec![
            "link sweep resource",
            "sweep account",
            // Corner cases
            "link cash account",
            "sweep destination",
            "sweep source",
            "sweep from account",
            "sweep to vehicle",
            "connect sweep account",
            "associate sweep account",
            "sweep account mapping",
            "cash account for sweep",
            "settlement account sweep",
            "custody account sweep",
            "portfolio cash sweep",
            "fund cash sweep",
        ],
    );
    m.insert(
        "cash-sweep.list",
        vec![
            "list cash sweeps",
            "sweep configuration",
            "all sweeps",
            // Corner cases
            "sweep summary",
            "sweep inventory",
            "sweep dashboard",
            "active sweeps",
            "sweep overview",
            "cash sweep report",
            "sweep status",
            "sweep arrangements",
            "all sweep configurations",
            "sweep portfolio",
            // Question forms
            "what sweeps are configured",
            "show sweep setup",
        ],
    );
    m.insert(
        "cash-sweep.update-threshold",
        vec![
            "update sweep threshold",
            "change threshold",
            // Corner cases
            "modify threshold",
            "set threshold",
            "minimum balance",
            "target balance",
            "sweep trigger",
            "threshold amount",
            "sweep limit",
            "minimum sweep amount",
            "maximum sweep amount",
            "sweep floor",
            "sweep ceiling",
            "buffer amount",
            "cushion amount",
            "headroom",
            // Question forms
            "what is sweep threshold",
            "change minimum balance",
        ],
    );
    m.insert(
        "cash-sweep.update-timing",
        vec![
            "update sweep timing",
            "change sweep time",
            // Corner cases
            "sweep schedule",
            "sweep frequency",
            "sweep cutoff",
            "sweep deadline",
            "eod cutoff",
            "sweep window",
            "sweep time zone",
            "sweep execution time",
            "when to sweep",
            "sweep trigger time",
            "automatic sweep time",
            "batch sweep time",
            // Question forms
            "when does sweep run",
            "change sweep schedule",
        ],
    );
    m.insert(
        "cash-sweep.change-vehicle",
        vec![
            "change sweep vehicle",
            "different stif",
            "change mmf",
            // Corner cases
            "switch money market fund",
            "change sweep destination",
            "new sweep vehicle",
            "alternative sweep vehicle",
            "better yield vehicle",
            "different fund",
            "replace sweep fund",
            "swap sweep vehicle",
            "upgrade sweep vehicle",
            "change investment vehicle",
            // Question forms
            "how to change sweep fund",
            "can i use different mmf",
        ],
    );
    m.insert(
        "cash-sweep.suspend",
        vec![
            "suspend sweep",
            "pause sweep",
            // Corner cases
            "stop sweep",
            "disable sweep",
            "halt sweep",
            "sweep pause",
            "temporary sweep stop",
            "sweep holiday",
            "freeze sweep",
            "no sweep",
            "skip sweep",
            "sweep exception",
            "one time skip",
            // Question forms
            "how to stop sweep",
            "can i pause sweep",
        ],
    );
    m.insert(
        "cash-sweep.reactivate",
        vec![
            "reactivate sweep",
            "resume sweep",
            // Corner cases
            "restart sweep",
            "enable sweep",
            "turn on sweep",
            "sweep back on",
            "resume cash management",
            "reactivate auto sweep",
            "sweep reactivation",
            "end sweep pause",
            "cancel sweep suspension",
            // Question forms
            "how to restart sweep",
            "resume sweeping",
        ],
    );
    m.insert(
        "cash-sweep.remove",
        vec![
            "remove sweep",
            "delete sweep config",
            // Corner cases
            "terminate sweep",
            "end sweep arrangement",
            "cancel sweep",
            "sweep termination",
            "deactivate sweep permanently",
            "remove sweep configuration",
            "delete sweep setup",
            "no longer sweep",
            "stop sweeping permanently",
        ],
    );
    m.insert(
        "cash-sweep.list-transactions",
        vec![
            "list sweep transactions",
            "sweep history",
            "sweep activity",
            // Corner cases
            "sweep movements",
            "sweep executions",
            "historical sweeps",
            "sweep log",
            "sweep audit trail",
            "sweep report",
            "daily sweeps",
            "sweep amounts",
            "sweep dates",
            // Question forms
            "show sweep history",
            "when did sweeps run",
        ],
    );
    m.insert(
        "cash-sweep.yield-report",
        vec![
            "sweep yield report",
            "sweep returns",
            "sweep performance",
            // Corner cases
            "cash yield",
            "stif yield",
            "mmf yield",
            "sweep earnings",
            "interest earned",
            "cash return",
            "idle cash return",
            "sweep roi",
            "yield comparison",
            "sweep analytics",
            // Question forms
            "how much did sweep earn",
            "what is sweep yield",
        ],
    );

    // ==========================================================================
    // INVESTMENT MANAGER VERBS - Enhanced with corner cases
    // ==========================================================================
    m.insert(
        "investment-manager.assign",
        vec![
            "assign investment manager",
            "add im",
            "investment manager setup",
            "appoint im",
            // Corner cases - IM types
            "assign portfolio manager",
            "add sub advisor",
            "add subadvisor",
            "appoint asset manager",
            "add discretionary manager",
            "add execution manager",
            "add trading manager",
            "aifm assignment",
            "manco assignment",
            "management company",
            "add external manager",
            "third party im",
            "outsourced im",
            "delegate to im",
            "im delegation",
            // Contexts
            "fund im",
            "portfolio im",
            "sleeve im",
            "mandate im",
            "account im",
            // Question forms
            "how to add investment manager",
            "how to assign im",
            "who manages this",
        ],
    );
    m.insert(
        "investment-manager.set-scope",
        vec![
            "set im scope",
            "im trading scope",
            "im permissions",
            // Corner cases
            "im authority",
            "im mandate",
            "im limits",
            "im restrictions",
            "im investment universe",
            "im asset classes",
            "im markets",
            "im instruments",
            "im geography",
            "im countries",
            "im currencies",
            "discretionary scope",
            "non discretionary scope",
            "advisory scope",
            "execution only scope",
            "trading authority",
            "investment guidelines",
            "im constraints",
            // Question forms
            "what can im trade",
            "im trading limits",
            "im boundaries",
        ],
    );
    m.insert(
        "investment-manager.link-connectivity",
        vec![
            "link im connectivity",
            "im instruction method",
            "how im sends trades",
            // Corner cases - connectivity types
            "im fix connection",
            "im swift connection",
            "im api connection",
            "im portal access",
            "im email instructions",
            "im fax instructions",
            "im oms connection",
            "im ems connection",
            "order routing",
            "trade routing im",
            "instruction channel",
            "execution channel",
            "im communication method",
            "how im instructs",
            "im order flow",
            "electronic trading im",
            "dma im",
            // Question forms
            "how does im send orders",
            "im connectivity setup",
        ],
    );
    m.insert(
        "investment-manager.list",
        vec![
            "list investment managers",
            "all ims",
            "im assignments",
            // Corner cases
            "im roster",
            "im inventory",
            "active ims",
            "im summary",
            "im dashboard",
            "who manages what",
            "manager assignments",
            "delegated managers",
            "external managers",
            "im portfolio",
            "manager lineup",
            // Question forms
            "who are the ims",
            "show all managers",
            "which ims do we use",
        ],
    );
    m.insert(
        "investment-manager.suspend",
        vec![
            "suspend im",
            "pause im",
            // Corner cases
            "disable im",
            "im on hold",
            "temporary im suspension",
            "freeze im",
            "block im trading",
            "restrict im",
            "im freeze",
            "halt im activity",
            "im timeout",
            "im cooling off",
            "pending review",
            "under investigation",
        ],
    );
    m.insert(
        "investment-manager.terminate",
        vec![
            "terminate im",
            "end im relationship",
            // Corner cases
            "remove im",
            "im termination",
            "fire im",
            "dismiss im",
            "im offboarding",
            "cancel im mandate",
            "revoke im authority",
            "im exit",
            "transition from im",
            "im replacement",
            "change im",
            "new im",
            "im handover",
            // Question forms
            "how to remove im",
            "how to change manager",
        ],
    );
    m.insert(
        "investment-manager.find-for-trade",
        vec![
            "find im for trade",
            "which im",
            "im for instrument",
            // Corner cases
            "who manages this",
            "responsible im",
            "competent im",
            "im lookup",
            "resolve im",
            "im for market",
            "im for currency",
            "im for asset class",
            "designated im",
            "assigned manager",
            "trade routing to im",
            // Question forms
            "who handles this trade",
            "which im for this",
        ],
    );
    m.insert(
        "investment-manager.read",
        vec![
            "read im",
            "im details",
            "im profile",
            // Corner cases
            "im information",
            "im setup",
            "im configuration",
            "im settings",
            "im scope details",
            "im connectivity details",
            "im summary",
            "im snapshot",
            "manager profile",
            "manager details",
        ],
    );
    m.insert(
        "investment-manager.update-scope",
        vec![
            "update im scope",
            "modify im scope",
            "change im permissions",
            // Corner cases
            "expand im scope",
            "reduce im scope",
            "add to im universe",
            "remove from im universe",
            "update im limits",
            "change im restrictions",
            "amend im mandate",
            "revise im authority",
            "im scope change",
            // Question forms
            "how to change im scope",
            "how to update im permissions",
        ],
    );
    m.insert(
        "investment-manager.list-activity",
        vec![
            "im activity",
            "im trades",
            "im transactions",
            // Corner cases
            "im trading history",
            "im orders",
            "im executions",
            "im performance",
            "manager activity",
            "what has im traded",
            "im audit trail",
            "im order history",
            "im transaction log",
            // Question forms
            "what has im done",
            "show im activity",
        ],
    );

    // ==========================================================================
    // FUND INVESTOR VERBS - Enhanced with corner cases
    // ==========================================================================
    m.insert(
        "fund-investor.create",
        vec![
            "create fund investor",
            "register investor",
            "new investor",
            "add investor to fund",
            // Corner cases - investor types
            "institutional investor",
            "retail investor",
            "qualified investor",
            "accredited investor",
            "professional investor",
            "eligible investor",
            "knowledgeable investor",
            "sophisticated investor",
            "pension fund investor",
            "insurance company investor",
            "sovereign wealth fund investor",
            "family office investor",
            "foundation investor",
            "endowment investor",
            "fund of funds investor",
            "fof investor",
            "seed investor",
            "anchor investor",
            "cornerstone investor",
            "co investor",
            "limited partner",
            "lp investor",
            // Actions
            "onboard investor",
            "investor setup",
            "investor intake",
            // Question forms
            "how to add investor",
            "how to onboard investor",
        ],
    );
    m.insert(
        "fund-investor.list",
        vec![
            "list fund investors",
            "all investors",
            "investor list",
            // Corner cases
            "investor register",
            "share class investors",
            "active investors",
            "investor inventory",
            "investor base",
            "investor book",
            "investor population",
            "investor roster",
            // Question forms
            "who are the investors",
            "show all investors",
        ],
    );
    m.insert(
        "fund-investor.update-kyc-status",
        vec![
            "update investor kyc",
            "investor kyc status",
            // Corner cases
            "investor kyc refresh",
            "investor re-kyc",
            "investor periodic review",
            "investor annual review",
            "refresh investor data",
            "update investor information",
            "investor documentation update",
            "investor verification refresh",
            "kyc renewal investor",
            "investor compliance update",
        ],
    );
    m.insert(
        "fund-investor.get",
        vec![
            "get investor",
            "investor details",
            // Corner cases
            "investor profile",
            "investor information",
            "investor data",
            "investor summary",
            "investor snapshot",
            "investor overview",
            "investor kyc",
            "investor suitability",
            "investor classification",
            "investor status",
            // Question forms
            "show investor details",
            "investor info",
        ],
    );
    m.insert(
        "fund-investor.classify",
        vec![
            "classify investor",
            "investor classification",
            "investor type",
            // Corner cases
            "investor category",
            "investor tier",
            "investor segment",
            "retail or professional",
            "qualified investor check",
            "accredited investor check",
            "eligibility classification",
            "investor suitability",
            "mifid classification",
            "investor appropriateness",
            // Question forms
            "what type of investor",
            "is investor qualified",
        ],
    );
    m.insert(
        "fund-investor.suspend",
        vec![
            "suspend investor",
            "freeze investor",
            "investor freeze",
            // Corner cases
            "block investor",
            "restrict investor",
            "investor restriction",
            "no trading investor",
            "investor on hold",
            "investor suspended",
            "temporary investor block",
            "investor compliance hold",
        ],
    );
    m.insert(
        "fund-investor.terminate",
        vec![
            "terminate investor",
            "investor exit",
            "investor redemption",
            // Corner cases
            "investor offboarding",
            "investor closure",
            "full redemption",
            "investor departure",
            "end investor relationship",
            "investor termination",
            "forced redemption",
            "compulsory redemption",
            "investor removal",
        ],
    );
    m.insert(
        "fund-investor.list-holdings",
        vec![
            "investor holdings",
            "investor positions",
            "investor portfolio",
            // Corner cases
            "what does investor hold",
            "investor share classes",
            "investor investments",
            "investor aum",
            "investor nav",
            "investor balances",
            "investor units",
            "investor shares",
            // Question forms
            "what does investor own",
            "investor exposure",
        ],
    );
    m.insert(
        "fund-investor.list-activity",
        vec![
            "investor activity",
            "investor transactions",
            "investor movements",
            // Corner cases
            "investor history",
            "investor subscriptions",
            "investor redemptions",
            "investor transfers",
            "investor switches",
            "investor trades",
            "investor cash flows",
            "investor audit trail",
            // Question forms
            "what has investor done",
            "investor transaction history",
        ],
    );

    // ==========================================================================
    // DELEGATION VERBS - Enhanced with corner cases
    // ==========================================================================
    m.insert(
        "delegation.add",
        vec![
            "add delegation",
            "delegate to",
            "sub-advisor",
            "outsource to",
            "delegation chain",
            // Corner cases - delegation types
            "delegate function",
            "outsource function",
            "appoint delegate",
            "third party delegate",
            "external delegate",
            "sub-custodian",
            "sub-administrator",
            "sub-ta",
            "transfer agent delegate",
            "fund admin delegate",
            "middle office delegate",
            "back office delegate",
            "operational delegate",
            "it delegate",
            "valuation delegate",
            "compliance delegate",
            "risk delegate",
            "reporting delegate",
            "kyc delegate",
            "aml delegate",
            // Regulatory
            "ucits delegate",
            "aifmd delegate",
            "mifid outsourcing",
            "critical function delegation",
            "material delegation",
            // Question forms
            "how to delegate",
            "how to add delegate",
            "outsource this function",
        ],
    );
    m.insert(
        "delegation.end",
        vec![
            "end delegation",
            "terminate delegation",
            "stop delegation",
            // Corner cases
            "cancel delegation",
            "revoke delegation",
            "remove delegate",
            "bring in house",
            "insource",
            "end outsourcing",
            "delegation termination",
            "transition from delegate",
            "delegate exit",
            "offboard delegate",
            "delegate replacement",
            "change delegate",
        ],
    );
    m.insert(
        "delegation.list-delegates",
        vec![
            "list delegates",
            "who do we delegate to",
            "our delegates",
            // Corner cases
            "delegate roster",
            "delegate inventory",
            "outsourced functions",
            "delegation map",
            "delegation summary",
            "third parties",
            "vendor list",
            "service providers",
            "external providers",
            "delegate portfolio",
            // Question forms
            "who are our delegates",
            "what is outsourced",
        ],
    );
    m.insert(
        "delegation.list-delegations-received",
        vec![
            "delegations received",
            "who delegates to us",
            "received delegations",
            // Corner cases
            "we are delegate for",
            "inbound delegations",
            "acting as delegate",
            "delegate relationships",
            "clients delegating to us",
            "delegation clients",
            "inward delegations",
            // Question forms
            "who do we provide services to",
            "who are our delegation clients",
        ],
    );
    m.insert(
        "delegation.read",
        vec![
            "read delegation",
            "delegation details",
            "delegation agreement",
            // Corner cases
            "delegation scope",
            "delegation terms",
            "delegation contract",
            "outsourcing agreement",
            "service agreement",
            "sla with delegate",
            "delegate responsibilities",
            "delegation boundaries",
        ],
    );
    m.insert(
        "delegation.update",
        vec![
            "update delegation",
            "modify delegation",
            "change delegation scope",
            // Corner cases
            "amend delegation",
            "expand delegation",
            "reduce delegation",
            "update outsourcing",
            "delegation change",
            "scope change",
            "delegation amendment",
        ],
    );
    m.insert(
        "delegation.oversight",
        vec![
            "delegation oversight",
            "monitor delegate",
            "delegate monitoring",
            // Corner cases
            "delegate due diligence",
            "delegate review",
            "delegate audit",
            "delegate performance",
            "delegate kpis",
            "delegate sla",
            "delegate risk",
            "delegate compliance",
            "regulatory oversight",
            // Question forms
            "how is delegate performing",
            "delegate issues",
        ],
    );

    // ==========================================================================
    // DELIVERY VERBS - Enhanced with corner cases
    // ==========================================================================
    m.insert(
        "delivery.record",
        vec![
            "record delivery",
            "service delivery",
            "delivered service",
            // Corner cases - delivery types
            "log delivery",
            "delivery event",
            "milestone delivery",
            "deliverable completed",
            "output delivered",
            "report delivered",
            "statement delivered",
            "data delivered",
            "file delivered",
            "package delivered",
            "batch delivered",
            "nav delivery",
            "pricing delivery",
            "reconciliation delivery",
            "trade confirmation delivery",
            "settlement confirmation",
            "corporate action notification",
            // Channels
            "swift delivery",
            "email delivery",
            "portal delivery",
            "ftp delivery",
            "sftp delivery",
            "api delivery",
            "web service delivery",
            // Question forms
            "when was delivered",
            "delivery timestamp",
        ],
    );
    m.insert(
        "delivery.complete",
        vec![
            "complete delivery",
            "delivery done",
            "service delivered",
            // Corner cases
            "delivery success",
            "successful delivery",
            "confirmed delivery",
            "acknowledged delivery",
            "delivery receipt",
            "delivery confirmation",
            "delivery verified",
            "delivery accepted",
            "client received",
            "mark delivered",
            "close delivery",
            "finalize delivery",
        ],
    );
    m.insert(
        "delivery.fail",
        vec![
            "delivery failed",
            "service failure",
            "failed delivery",
            // Corner cases
            "delivery error",
            "delivery exception",
            "bounced delivery",
            "rejected delivery",
            "undeliverable",
            "delivery timeout",
            "connection failed",
            "authentication failed",
            "delivery denied",
            "delivery not received",
            "delivery issue",
            "retry delivery",
            "redeliver",
            // Question forms
            "why did delivery fail",
            "delivery issues",
        ],
    );
    m.insert(
        "delivery.list",
        vec![
            "list deliveries",
            "delivery history",
            "all deliveries",
            // Corner cases
            "delivery log",
            "delivery audit trail",
            "delivery report",
            "delivery dashboard",
            "recent deliveries",
            "pending deliveries",
            "failed deliveries",
            "successful deliveries",
            "delivery statistics",
            // Question forms
            "what was delivered",
            "show delivery history",
        ],
    );
    m.insert(
        "delivery.retry",
        vec![
            "retry delivery",
            "redeliver",
            "resend",
            // Corner cases
            "try again",
            "repeat delivery",
            "resubmit",
            "second attempt",
            "delivery retry",
            "manual retry",
            "force delivery",
        ],
    );
    m.insert(
        "delivery.schedule",
        vec![
            "schedule delivery",
            "delivery schedule",
            "when to deliver",
            // Corner cases
            "delivery timing",
            "delivery frequency",
            "daily delivery",
            "weekly delivery",
            "monthly delivery",
            "on demand delivery",
            "real time delivery",
            "batch delivery schedule",
            "delivery window",
            "delivery cutoff",
        ],
    );

    // ==========================================================================
    // SERVICE RESOURCE VERBS - Enhanced with corner cases
    // ==========================================================================
    m.insert(
        "service-resource.read",
        vec![
            "read resource type",
            "resource details",
            "resource definition",
            // Corner cases
            "resource specification",
            "resource schema",
            "resource template",
            "what is this resource",
            "resource info",
            "resource metadata",
            "resource configuration",
            "resource properties",
            "resource requirements",
            // Question forms
            "tell me about resource",
            "resource description",
        ],
    );
    m.insert(
        "service-resource.list",
        vec![
            "list resource types",
            "available resources",
            "resource catalog",
            // Corner cases
            "resource inventory",
            "resource library",
            "resource options",
            "all resources",
            "supported resources",
            "resource menu",
            "what resources exist",
            "resource taxonomy",
            "resource hierarchy",
            // Question forms
            "what resources are available",
            "show resource options",
        ],
    );
    m.insert(
        "service-resource.list-by-service",
        vec![
            "resources for service",
            "service resources",
            // Corner cases
            "resources needed for service",
            "service resource requirements",
            "what resources for this service",
            "required resources",
            "mandatory resources",
            "optional resources",
            "service dependencies",
            "custody resources",
            "fa resources",
            "ta resources",
            // Question forms
            "what do i need for this service",
        ],
    );
    m.insert(
        "service-resource.list-attributes",
        vec![
            "list resource attributes",
            "required attributes",
            "attribute requirements",
            // Corner cases
            "resource fields",
            "resource parameters",
            "configuration options",
            "setup parameters",
            "mandatory fields",
            "optional fields",
            "attribute schema",
            "field definitions",
            "what to configure",
            // Question forms
            "what attributes are needed",
            "what to fill in",
        ],
    );
    m.insert(
        "service-resource.provision",
        vec![
            "provision resource",
            "create resource instance",
            "setup resource",
            "new resource instance",
            // Corner cases
            "instantiate resource",
            "allocate resource",
            "deploy resource",
            "spin up resource",
            "create account",
            "setup account",
            "provision portfolio",
            "create share class instance",
            "provision custody account",
            "setup trading account",
            "create safekeeping account",
            "provision cash account",
            // Question forms
            "how to create resource",
            "how to provision",
        ],
    );
    m.insert(
        "service-resource.set-attr",
        vec![
            "set resource attribute",
            "configure resource",
            "resource setting",
            // Corner cases
            "update attribute",
            "change configuration",
            "modify setting",
            "set parameter",
            "configure option",
            "attribute update",
            "resource customization",
            "tailor resource",
            "personalize resource",
            // Question forms
            "how to configure",
            "how to set attribute",
        ],
    );
    m.insert(
        "service-resource.activate",
        vec![
            "activate resource",
            "resource active",
            "go live resource",
            // Corner cases
            "enable resource",
            "turn on resource",
            "resource go live",
            "make active",
            "start resource",
            "resource activation",
            "commence service",
            "open account",
            "account active",
            // Question forms
            "how to activate",
            "when is resource active",
        ],
    );
    m.insert(
        "service-resource.suspend",
        vec![
            "suspend resource",
            "pause resource",
            // Corner cases
            "disable resource",
            "freeze resource",
            "resource on hold",
            "temporary suspension",
            "resource freeze",
            "block resource",
            "restrict resource",
            "resource maintenance",
            "take offline",
            // Question forms
            "how to suspend",
            "how to pause resource",
        ],
    );
    m.insert(
        "service-resource.decommission",
        vec![
            "decommission resource",
            "retire resource",
            "remove resource",
            // Corner cases
            "delete resource",
            "terminate resource",
            "close resource",
            "wind down resource",
            "resource closure",
            "end resource",
            "resource offboarding",
            "close account",
            "account closure",
            "permanent removal",
            // Question forms
            "how to remove resource",
            "how to close account",
        ],
    );
    m.insert(
        "service-resource.validate-attrs",
        vec![
            "validate resource attrs",
            "check resource config",
            "resource validation",
            // Corner cases
            "verify configuration",
            "validate setup",
            "config check",
            "attribute validation",
            "completeness check",
            "readiness check",
            "is resource ready",
            "can activate",
            "pre activation check",
            "resource health check",
            // Question forms
            "is resource configured correctly",
            "is resource ready to activate",
        ],
    );
    m.insert(
        "service-resource.list-instances",
        vec![
            "list resource instances",
            "provisioned resources",
            "active instances",
            // Corner cases
            "my resources",
            "cbu resources",
            "deployed resources",
            "resource summary",
            "resource dashboard",
            "all instances",
            "resource inventory",
            // Question forms
            "what resources do we have",
            "show provisioned resources",
        ],
    );
    m.insert(
        "service-resource.clone",
        vec![
            "clone resource",
            "copy resource",
            "duplicate resource",
            // Corner cases
            "resource template",
            "create from template",
            "copy configuration",
            "replicate setup",
            "based on existing",
            "similar to",
            // Question forms
            "how to copy resource",
            "create similar resource",
        ],
    );

    // ==========================================================================
    // CLIENT PORTAL VERBS - Enhanced with corner cases
    // ==========================================================================
    m.insert(
        "client.get-status",
        vec![
            "get onboarding status",
            "my status",
            "where am i",
            "onboarding progress",
            // Corner cases
            "how is onboarding going",
            "progress check",
            "status update",
            "where are we at",
            "completion percentage",
            "how far along",
            "stages completed",
            "remaining steps",
            "time to completion",
            "eta",
            "estimated completion",
            "go live date",
            "when will we be done",
            // Question forms
            "check my progress",
            "show status",
            "update me",
        ],
    );
    m.insert(
        "client.get-outstanding",
        vec![
            "outstanding requests",
            "what do i need to do",
            "pending items",
            // Corner cases
            "my tasks",
            "action items",
            "to do list",
            "what is needed from me",
            "client tasks",
            "pending documents",
            "outstanding information",
            "blocking items",
            "urgent items",
            "overdue items",
            "deadline approaching",
            "my responsibilities",
            // Question forms
            "what do i need to provide",
            "what is holding things up",
            "what is blocking",
        ],
    );
    m.insert(
        "client.get-request-detail",
        vec![
            "request detail",
            "why is this needed",
            "request info",
            // Corner cases
            "explain request",
            "request explanation",
            "request requirements",
            "what format",
            "what is acceptable",
            "more information about request",
            "request instructions",
            "how to fulfill",
            "request guidance",
            "help with request",
            // Question forms
            "why do you need this",
            "what exactly is required",
        ],
    );
    m.insert(
        "client.get-entity-info",
        vec![
            "entity info",
            "my entity",
            "entity summary",
            // Corner cases
            "my company info",
            "what do you have on file",
            "current data",
            "entity details",
            "recorded information",
            "what is on record",
            "verify my info",
            "check entity data",
            "entity profile",
            // Question forms
            "is my info correct",
            "review my details",
        ],
    );
    m.insert(
        "client.submit-document",
        vec![
            "submit document",
            "upload document",
            "provide document",
            // Corner cases
            "send document",
            "attach document",
            "share document",
            "upload file",
            "send file",
            "document submission",
            "provide evidence",
            "supporting document",
            "required document",
            "upload pdf",
            "upload certificate",
            "upload registration",
            "upload articles",
            "upload id",
            "upload passport",
            // Question forms
            "how to upload",
            "where to upload",
        ],
    );
    m.insert(
        "client.provide-info",
        vec![
            "provide info",
            "submit information",
            "answer question",
            // Corner cases
            "supply information",
            "give details",
            "respond to request",
            "fill in",
            "complete form",
            "submit data",
            "enter information",
            "provide details",
            "answer inquiry",
            "respond to query",
            // Question forms
            "how to provide info",
            "how to respond",
        ],
    );
    m.insert(
        "client.add-note",
        vec![
            "add note",
            "leave comment",
            "note on request",
            // Corner cases
            "add comment",
            "write note",
            "message to team",
            "communication",
            "client note",
            "explanation",
            "clarification",
            "additional context",
            "more information",
            "supporting note",
        ],
    );
    m.insert(
        "client.request-clarification",
        vec![
            "request clarification",
            "ask question",
            "need help",
            // Corner cases
            "i have a question",
            "unclear request",
            "dont understand",
            "what does this mean",
            "need more info",
            "confused",
            "help please",
            "assistance needed",
            "speak to someone",
            "contact support",
            "get help",
            // Question forms
            "who can help",
            "how to get help",
        ],
    );
    m.insert(
        "client.escalate",
        vec![
            "escalate",
            "speak to human",
            "need help",
            "contact relationship manager",
            // Corner cases
            "speak to manager",
            "escalate issue",
            "urgent issue",
            "complaint",
            "not happy",
            "frustrated",
            "taking too long",
            "priority request",
            "expedite",
            "fast track",
            "need attention",
            "speak to someone senior",
            // Question forms
            "who can i talk to",
            "how to escalate",
        ],
    );
    m.insert(
        "client.view-timeline",
        vec![
            "view timeline",
            "onboarding timeline",
            "what happens next",
            // Corner cases
            "expected dates",
            "milestones",
            "key dates",
            "schedule",
            "onboarding schedule",
            "phase timeline",
            "stage dates",
            "go live timeline",
            // Question forms
            "when will things happen",
            "what is the timeline",
        ],
    );
    m.insert(
        "client.view-history",
        vec![
            "view history",
            "activity history",
            "what has been done",
            // Corner cases
            "audit trail",
            "submission history",
            "document history",
            "communication history",
            "progress history",
            "what happened",
            "timeline of events",
            // Question forms
            "what have i submitted",
            "show history",
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
    // BATCH VERBS - Enhanced with corner cases
    // ==========================================================================
    m.insert(
        "batch.pause",
        vec![
            "pause batch",
            "stop batch",
            "batch pause",
            // Corner cases
            "hold batch",
            "suspend batch",
            "freeze batch",
            "batch on hold",
            "temporary stop",
            "pause processing",
            "halt batch",
            "pause bulk operation",
            "wait batch",
            // Question forms
            "how to pause batch",
            "stop processing temporarily",
        ],
    );
    m.insert(
        "batch.resume",
        vec![
            "resume batch",
            "continue batch",
            "restart batch",
            // Corner cases
            "unpause batch",
            "resume processing",
            "continue processing",
            "batch continue",
            "pick up where left off",
            "restart processing",
            "batch resume",
            "reactivate batch",
            "unhold batch",
            // Question forms
            "how to resume batch",
            "continue from pause",
        ],
    );
    m.insert(
        "batch.continue",
        vec![
            "batch continue",
            "process more",
            "next batch item",
            // Corner cases
            "process next",
            "continue to next",
            "move on",
            "next item in batch",
            "proceed",
            "keep going",
            "process remaining",
            "continue batch processing",
        ],
    );
    m.insert(
        "batch.skip",
        vec![
            "skip batch item",
            "skip current",
            "next item",
            // Corner cases
            "skip this one",
            "bypass item",
            "ignore item",
            "skip and continue",
            "pass on this",
            "skip problematic",
            "skip error",
            "defer item",
            "skip for now",
            "come back later",
            // Question forms
            "can i skip this",
            "how to skip item",
        ],
    );
    m.insert(
        "batch.abort",
        vec![
            "abort batch",
            "cancel batch",
            "stop all",
            // Corner cases
            "terminate batch",
            "kill batch",
            "end batch",
            "batch cancellation",
            "emergency stop",
            "abort all",
            "cancel remaining",
            "stop batch entirely",
            "batch termination",
            "abandon batch",
            // Question forms
            "how to cancel batch",
            "how to abort",
        ],
    );
    m.insert(
        "batch.status",
        vec![
            "batch status",
            "batch progress",
            "how is batch doing",
            // Corner cases
            "batch health",
            "processing status",
            "batch report",
            "items processed",
            "items remaining",
            "batch completion",
            "percentage complete",
            "batch errors",
            "batch failures",
            "success rate",
            "batch statistics",
            "batch dashboard",
            // Question forms
            "how far along is batch",
            "is batch running",
            "batch progress report",
        ],
    );
    m.insert(
        "batch.add-products",
        vec![
            "batch add products",
            "bulk add products",
            "products to multiple cbus",
            // Corner cases
            "mass product assignment",
            "bulk product",
            "multiple product assignment",
            "products to many clients",
            "bulk enable products",
            "batch product enrollment",
            "multiple client products",
            "product rollout",
            "batch service activation",
        ],
    );
    m.insert(
        "batch.create",
        vec![
            "create batch",
            "new batch",
            "start batch",
            // Corner cases
            "bulk operation",
            "mass operation",
            "batch job",
            "bulk update",
            "batch update",
            "bulk process",
            "process many",
            "process multiple",
            "bulk upload",
            "mass upload",
            // Question forms
            "how to start batch",
            "how to bulk process",
        ],
    );
    m.insert(
        "batch.list",
        vec![
            "list batches",
            "all batches",
            "batch history",
            // Corner cases
            "active batches",
            "running batches",
            "completed batches",
            "failed batches",
            "batch queue",
            "batch inventory",
            "batch log",
            "batch audit",
            // Question forms
            "what batches are running",
            "show batches",
        ],
    );
    m.insert(
        "batch.retry-failed",
        vec![
            "retry failed",
            "reprocess errors",
            "retry batch errors",
            // Corner cases
            "rerun failed items",
            "retry failures",
            "reprocess failed",
            "batch retry",
            "error recovery",
            "fix and retry",
            "second attempt batch",
            "retry all failed",
        ],
    );

    // ==========================================================================
    // KYC AGREEMENT VERBS - Enhanced with corner cases
    // ==========================================================================
    m.insert(
        "kyc-agreement.create",
        vec![
            "create kyc agreement",
            "kyc service agreement",
            "sponsor agreement",
            // Corner cases - agreement types
            "new kyc agreement",
            "kyc sharing agreement",
            "kyc reliance agreement",
            "kyc outsourcing agreement",
            "sponsor kyc",
            "principal kyc",
            "agent kyc",
            "kyc delegation agreement",
            "kyc service contract",
            "aml service agreement",
            "compliance service agreement",
            "cdd agreement",
            "edd agreement",
            "due diligence agreement",
            "kyc utility agreement",
            "passport utility",
            "kyc passport",
            // Question forms
            "how to create kyc agreement",
            "new sponsor agreement",
        ],
    );
    m.insert(
        "kyc-agreement.read",
        vec![
            "read kyc agreement",
            "agreement details",
            // Corner cases
            "agreement terms",
            "agreement scope",
            "agreement info",
            "kyc agreement info",
            "sponsor agreement details",
            "what is covered",
            "agreement coverage",
            "kyc scope agreement",
            "agreement specification",
            // Question forms
            "show agreement",
            "agreement contents",
        ],
    );
    m.insert(
        "kyc-agreement.list",
        vec![
            "list kyc agreements",
            "sponsor agreements",
            // Corner cases
            "all kyc agreements",
            "agreement inventory",
            "active agreements",
            "kyc agreement summary",
            "agreement portfolio",
            "kyc service agreements",
            "sponsor relationships",
            "kyc delegations",
            "who we rely on",
            "who relies on us",
            // Question forms
            "what agreements exist",
            "show all agreements",
        ],
    );
    m.insert(
        "kyc-agreement.update-status",
        vec![
            "update agreement status",
            "agreement status change",
            // Corner cases
            "activate agreement",
            "suspend agreement",
            "terminate agreement",
            "agreement activation",
            "agreement termination",
            "agreement on hold",
            "agreement review",
            "renew agreement",
            "extend agreement",
            "modify agreement",
            "amend agreement",
            // Question forms
            "how to change agreement status",
            "update agreement",
        ],
    );
    m.insert(
        "kyc-agreement.terminate",
        vec![
            "terminate kyc agreement",
            "end agreement",
            "cancel agreement",
            // Corner cases
            "agreement termination",
            "stop reliance",
            "end sponsor relationship",
            "exit agreement",
            "agreement wind down",
            "cease agreement",
            "close agreement",
            "agreement expiry",
        ],
    );
    m.insert(
        "kyc-agreement.list-entities",
        vec![
            "entities under agreement",
            "agreement coverage",
            "covered entities",
            // Corner cases
            "what is covered",
            "agreement scope entities",
            "entities in scope",
            "agreement portfolio entities",
            "sponsored entities",
            "reliance entities",
            // Question forms
            "who is covered by agreement",
            "what does agreement cover",
        ],
    );

    // ==========================================================================
    // KYC SCOPE VERBS - Enhanced with corner cases
    // ==========================================================================
    m.insert(
        "kyc.preview-scope",
        vec![
            "preview kyc scope",
            "kyc obligations",
            "who needs kyc",
            "scope preview",
            // Corner cases
            "kyc requirements preview",
            "what kyc is needed",
            "kyc analysis",
            "scope analysis",
            "who is in scope",
            "kyc universe",
            "full scope",
            "cdd scope",
            "edd scope",
            "pep scope",
            "sanctions scope",
            "documentation scope",
            "entity scope",
            "structure scope",
            "beneficial owner scope",
            // Question forms
            "what entities need kyc",
            "how much kyc is needed",
            "show kyc scope",
        ],
    );
    m.insert(
        "kyc.recommend",
        vec![
            "kyc recommendation",
            "recommend approval",
            "kyc decision",
            // Corner cases
            "propose decision",
            "recommendation",
            "analyst recommendation",
            "maker recommendation",
            "approve recommendation",
            "reject recommendation",
            "escalation recommendation",
            "conditional approval",
            "approval with conditions",
            "kyc verdict",
            "kyc conclusion",
            "risk recommendation",
            // Question forms
            "what is the recommendation",
            "should we approve",
        ],
    );
    m.insert(
        "kyc.sponsor-decision",
        vec![
            "sponsor decision",
            "sponsor approval",
            "sponsor accept",
            "sponsor reject",
            // Corner cases
            "checker decision",
            "final decision",
            "approval decision",
            "rejection decision",
            "escalate decision",
            "refer decision",
            "senior approval",
            "committee decision",
            "board approval kyc",
            "mlro decision",
            "compliance decision",
            "second line approval",
            // Question forms
            "what is the decision",
            "approve or reject",
        ],
    );
    m.insert(
        "kyc.list-in-scope",
        vec![
            "list in scope entities",
            "scope entities",
            "who is in scope",
            // Corner cases
            "all scoped entities",
            "kyc scope list",
            "entities requiring kyc",
            "kyc queue",
            "pending kyc",
            "outstanding kyc",
            "incomplete kyc",
            // Question forms
            "who needs kyc",
            "kyc backlog",
        ],
    );
    m.insert(
        "kyc.calculate-risk",
        vec![
            "calculate risk score",
            "kyc risk calculation",
            "risk assessment",
            // Corner cases
            "entity risk score",
            "overall risk",
            "aggregated risk",
            "portfolio risk",
            "risk factors",
            "risk indicators",
            "high risk entities",
            "medium risk entities",
            "low risk entities",
            // Question forms
            "what is the risk score",
            "how risky",
        ],
    );
    m.insert(
        "kyc.apply-policy",
        vec![
            "apply kyc policy",
            "policy application",
            "determine requirements",
            // Corner cases
            "policy mapping",
            "requirements determination",
            "risk based approach",
            "simplified dd",
            "standard dd",
            "enhanced dd",
            "policy rules",
            "policy engine",
            "requirement derivation",
            // Question forms
            "what policy applies",
            "which requirements",
        ],
    );

    // ==========================================================================
    // REQUEST VERBS - Enhanced with corner cases
    // ==========================================================================
    m.insert(
        "request.create",
        vec![
            // Core terms
            "create request",
            "new request",
            "outstanding request",
            "need from client",
            "raise request",
            "initiate request",
            "submit request",
            // Corner cases - request types
            "information request",
            "document request",
            "data request",
            "clarification request",
            "action request",
            "approval request",
            "sign off request",
            "confirmation request",
            "verification request",
            "attestation request",
            // Corner cases - contexts
            "ask client for",
            "need from counterparty",
            "require from investor",
            "request from fund admin",
            "request from custodian",
            "request from TA",
            "request from broker",
            "client action item",
            "outstanding item",
            "pending item",
            "open request",
            "active request",
            "rfi",
            "request for information",
            "rfp",
            "request for proposal",
            // Corner cases - urgency
            "urgent request",
            "priority request",
            "critical request",
            "routine request",
            "standard request",
            "bulk request",
            "batch request",
            // Question forms
            "how to create request",
            "how to ask client",
            "how to request information",
        ],
    );
    m.insert(
        "request.list",
        vec![
            // Core terms
            "list requests",
            "outstanding requests",
            "pending requests",
            "show requests",
            "view requests",
            "all requests",
            // Corner cases - filters
            "requests by entity",
            "requests by type",
            "requests by status",
            "requests by due date",
            "requests by priority",
            "requests by assignee",
            "requests by requester",
            "open requests",
            "closed requests",
            "active requests",
            "completed requests",
            "my requests",
            "team requests",
            "case requests",
            "workstream requests",
            // Corner cases - views
            "request queue",
            "request dashboard",
            "request summary",
            "request overview",
            "request report",
            "aged requests",
            "request aging",
            // Question forms
            "what requests are outstanding",
            "what do we need from client",
        ],
    );
    m.insert(
        "request.overdue",
        vec![
            // Core terms
            "overdue requests",
            "late requests",
            "past due",
            "missed deadline",
            "expired requests",
            // Corner cases
            "breach sla",
            "sla breach",
            "aging requests",
            "delinquent requests",
            "behind schedule",
            "overdue items",
            "past deadline",
            "deadline passed",
            "stale requests",
            "old requests",
            "unanswered requests",
            "ignored requests",
            "unresponded",
            "no response",
            "awaiting response too long",
            "escalation due",
            "needs escalation",
            // Question forms
            "what is overdue",
            "what requests are late",
            "which requests missed deadline",
        ],
    );
    m.insert(
        "request.fulfill",
        vec![
            // Core terms
            "fulfill request",
            "request fulfilled",
            "request done",
            "complete request",
            "satisfy request",
            "close request",
            "resolve request",
            // Corner cases - fulfillment types
            "mark as received",
            "mark as complete",
            "mark as fulfilled",
            "mark as satisfied",
            "request satisfied",
            "requirement met",
            "deliverable received",
            "information received",
            "document received",
            "response received",
            "client responded",
            "counterparty responded",
            // Corner cases - partial fulfillment
            "partial fulfillment",
            "partially complete",
            "partially satisfied",
            "partial response",
            // Question forms
            "how to close request",
            "how to mark request done",
        ],
    );
    m.insert(
        "request.cancel",
        vec![
            // Core terms
            "cancel request",
            "void request",
            "remove request",
            "delete request",
            "abort request",
            "withdraw request",
            // Corner cases
            "no longer needed",
            "request obsolete",
            "request superseded",
            "replaced by",
            "duplicate request",
            "mistaken request",
            "erroneous request",
            "retract request",
            "rescind request",
            "abandon request",
            // Question forms
            "how to cancel request",
            "how to remove request",
        ],
    );
    m.insert(
        "request.extend",
        vec![
            // Core terms
            "extend request",
            "more time",
            "extend deadline",
            "push deadline",
            "move deadline",
            "new deadline",
            "revise deadline",
            // Corner cases
            "deadline extension",
            "due date extension",
            "grace period",
            "grant extension",
            "request extension",
            "reschedule",
            "postpone",
            "defer",
            "delay deadline",
            "additional time",
            "extra time",
            "time extension",
            "extend due date",
            "new due date",
            "revised due date",
            // Question forms
            "how to extend deadline",
            "how to give more time",
        ],
    );
    m.insert(
        "request.remind",
        vec![
            // Core terms
            "remind",
            "send reminder",
            "follow up",
            "nudge",
            "chase",
            "ping",
            // Corner cases
            "reminder email",
            "reminder notification",
            "follow up email",
            "follow up call",
            "gentle reminder",
            "friendly reminder",
            "second reminder",
            "third reminder",
            "final reminder",
            "last reminder",
            "polite chase",
            "chaser",
            "send chaser",
            "bump",
            "re-request",
            "resubmit request",
            "repeat request",
            "resend request",
            // Question forms
            "how to send reminder",
            "how to follow up",
            "how to chase client",
        ],
    );
    m.insert(
        "request.escalate",
        vec![
            // Core terms
            "escalate request",
            "bump request",
            "urgent request",
            "priority request",
            "raise priority",
            "increase urgency",
            // Corner cases
            "escalation",
            "escalate to manager",
            "escalate to senior",
            "escalate to relationship manager",
            "escalate to client service",
            "escalate to compliance",
            "escalate to legal",
            "critical path",
            "blocking issue",
            "blocker",
            "impediment",
            "needs attention",
            "requires intervention",
            "management escalation",
            "exec escalation",
            "board escalation",
            "committee escalation",
            "formal escalation",
            // Question forms
            "how to escalate request",
            "how to make request urgent",
        ],
    );
    m.insert(
        "request.waive",
        vec![
            // Core terms
            "waive request",
            "not needed",
            "skip request",
            "bypass request",
            "exempt",
            "waiver",
            // Corner cases
            "grant waiver",
            "approve waiver",
            "waiver approved",
            "requirement waived",
            "exception granted",
            "exception approved",
            "dispensation",
            "not applicable",
            "n/a",
            "does not apply",
            "out of scope",
            "excluded",
            "exempted",
            "carve out",
            "carved out",
            "risk accepted",
            "accept risk",
            "proceed without",
            "conditional approval",
            // Question forms
            "how to waive request",
            "how to grant exception",
        ],
    );
    m.insert(
        "request.reassign",
        vec![
            // Core terms
            "reassign request",
            "transfer request",
            "move request",
            "change owner",
            "change assignee",
            // Corner cases
            "delegate request",
            "handover request",
            "hand off",
            "new owner",
            "new assignee",
            "reassignment",
            "request handover",
            "pass to colleague",
            "coverage change",
            "responsibility transfer",
            // Question forms
            "how to reassign request",
            "how to change owner",
        ],
    );
    m.insert(
        "request.bulk-create",
        vec![
            // Core terms
            "bulk create requests",
            "batch requests",
            "multiple requests",
            "mass request",
            // Corner cases
            "template requests",
            "standard request set",
            "request package",
            "request bundle",
            "requirement package",
            "checklist requests",
            "generate requests",
            "auto generate requests",
            "populate requests",
            "create all requests",
            // Question forms
            "how to create multiple requests",
            "how to bulk request",
        ],
    );
    m.insert(
        "request.link-to-document",
        vec![
            // Core terms
            "link document to request",
            "attach document",
            "document fulfills request",
            // Corner cases
            "evidence for request",
            "proof for request",
            "supporting document",
            "request attachment",
            "upload for request",
            "document reference",
            "link evidence",
            "associate document",
            // Question forms
            "how to attach document to request",
            "how to fulfill with document",
        ],
    );

    // ==========================================================================
    // CASE EVENT VERBS - Enhanced with corner cases
    // ==========================================================================
    m.insert(
        "case-event.log",
        vec![
            // Core terms
            "log event",
            "case event",
            "audit log",
            "record activity",
            "log activity",
            "create event",
            // Corner cases - event types
            "status change event",
            "state transition",
            "workflow event",
            "milestone event",
            "checkpoint",
            "progress marker",
            "system event",
            "user action",
            "automated action",
            "manual action",
            "decision event",
            "approval event",
            "rejection event",
            "escalation event",
            "assignment event",
            "handover event",
            "communication event",
            "document event",
            "verification event",
            "screening event",
            // Corner cases - audit
            "audit trail",
            "activity log",
            "timeline entry",
            "history entry",
            "chronicle",
            "record keeper",
            "compliance log",
            "regulatory log",
            "evidence trail",
            // Question forms
            "how to log event",
            "how to record activity",
        ],
    );
    m.insert(
        "case-event.list-by-case",
        vec![
            // Core terms
            "list case events",
            "case history",
            "event log",
            "case timeline",
            "activity history",
            // Corner cases - views
            "audit trail",
            "full audit",
            "complete history",
            "all events",
            "event sequence",
            "chronological events",
            "reverse chronological",
            "latest events",
            "recent events",
            "event feed",
            "activity feed",
            "activity stream",
            // Corner cases - filters
            "events by type",
            "events by user",
            "events by date",
            "events in range",
            "filter events",
            "search events",
            "significant events",
            "key events",
            "milestones only",
            // Question forms
            "what happened on case",
            "show case history",
            "what activities occurred",
        ],
    );
    m.insert(
        "case-event.list-by-entity",
        vec![
            // Core terms
            "entity event history",
            "entity activity log",
            "entity timeline",
            // Corner cases
            "events for entity",
            "entity audit trail",
            "what happened to entity",
            "entity changes",
            "entity modifications",
            "entity lifecycle events",
            "entity state changes",
            // Question forms
            "what happened to this entity",
            "entity event log",
        ],
    );
    m.insert(
        "case-event.list-by-user",
        vec![
            // Core terms
            "user activity",
            "user actions",
            "what user did",
            // Corner cases
            "user audit trail",
            "actions by user",
            "user event log",
            "analyst activity",
            "reviewer activity",
            "approver activity",
            "user contributions",
            "my actions",
            "my activity",
            // Question forms
            "what did this user do",
            "show user activity",
        ],
    );
    m.insert(
        "case-event.export",
        vec![
            // Core terms
            "export event log",
            "export audit trail",
            "download events",
            // Corner cases
            "audit report",
            "activity report",
            "compliance report",
            "regulatory report",
            "exam package",
            "evidence package",
            "pdf audit trail",
            "csv events",
            "event extract",
            // Question forms
            "how to export audit trail",
            "download case history",
        ],
    );
    m.insert(
        "case-event.annotate",
        vec![
            // Core terms
            "annotate event",
            "add note to event",
            "comment on event",
            // Corner cases
            "event annotation",
            "event note",
            "event comment",
            "explain event",
            "contextualize event",
            "event rationale",
            "event justification",
            // Question forms
            "how to annotate event",
            "add context to event",
        ],
    );

    // ==========================================================================
    // OBSERVATION VERBS - Enhanced with corner cases
    // ==========================================================================
    m.insert(
        "observation.record",
        vec![
            "record observation",
            "capture observation",
            "observe attribute",
            "attribute observation",
            // Corner cases
            "note observation",
            "log observation",
            "add observation",
            "create observation",
            "data point",
            "capture data point",
            "record value",
            "capture value",
            "attribute value",
            "field value",
            "observed value",
            "sourced value",
            "verified value",
            "extracted value",
            "manual entry",
            "user input",
            "system observation",
            "api observation",
            "registry observation",
            "third party observation",
            "primary source",
            "secondary source",
        ],
    );
    m.insert(
        "observation.record-from-document",
        vec![
            "observation from document",
            "extract observation",
            "document observation",
            // Corner cases
            "document sourced",
            "document evidence",
            "extracted from document",
            "pulled from document",
            "parsed from document",
            "ocr observation",
            "ai extracted",
            "ml extracted",
            "manual extraction",
            "document reference",
            "evidence based observation",
            "documented observation",
            "supporting document",
        ],
    );
    m.insert(
        "observation.supersede",
        vec![
            "supersede observation",
            "replace observation",
            "newer observation",
            // Corner cases
            "override observation",
            "update observation",
            "correct observation",
            "new version",
            "latest observation",
            "more recent",
            "fresher data",
            "updated value",
            "corrected value",
            "revised value",
            "observation v2",
            "observation update",
            "data refresh",
            "value refresh",
        ],
    );
    m.insert(
        "observation.list-for-entity",
        vec![
            "list observations",
            "entity observations",
            "all observations",
            // Corner cases
            "observation history",
            "observation timeline",
            "data points for entity",
            "values for entity",
            "attributes for entity",
            "entity data",
            "entity attributes",
            "entity values",
            "entity profile data",
            "collected data",
            "captured data",
            "observed data",
        ],
    );
    m.insert(
        "observation.list-for-attribute",
        vec![
            "observations for attribute",
            "attribute history",
            // Corner cases
            "attribute timeline",
            "value history",
            "value timeline",
            "field history",
            "data point history",
            "historical values",
            "past values",
            "all values",
            "value changes",
            "attribute audit",
            "field audit",
        ],
    );
    m.insert(
        "observation.get-current",
        vec![
            "current observation",
            "best observation",
            "latest observation",
            // Corner cases
            "current value",
            "best value",
            "latest value",
            "most recent",
            "freshest data",
            "authoritative value",
            "golden source",
            "master value",
            "canonical value",
            "winning observation",
            "resolved value",
            "effective value",
        ],
    );
    m.insert(
        "observation.reconcile",
        vec![
            "reconcile observations",
            "compare observations",
            "find conflicts",
            // Corner cases
            "observation conflict",
            "value conflict",
            "data conflict",
            "reconciliation",
            "conflict resolution",
            "conflict detection",
            "mismatch detection",
            "discrepancy detection",
            "cross reference",
            "data comparison",
            "value comparison",
            "source comparison",
            "multi-source reconciliation",
        ],
    );
    m.insert(
        "observation.verify-allegations",
        vec![
            "verify allegations",
            "check allegations",
            "allegation verification",
            // Corner cases
            "confirm claims",
            "validate claims",
            "check claims",
            "allegation check",
            "claim verification",
            "client assertion check",
            "self-certification check",
            "declaration check",
            "attestation check",
        ],
    );

    // ==========================================================================
    // ALLEGATION VERBS - Enhanced with corner cases
    // ==========================================================================
    m.insert(
        "allegation.record",
        vec![
            "record allegation",
            "client claim",
            "alleged value",
            "client says",
            // Corner cases
            "self-reported",
            "self-declared",
            "self-certified",
            "client assertion",
            "client declaration",
            "client statement",
            "attestation",
            "representation",
            "client representation",
            "unverified claim",
            "pending verification",
            "to be verified",
            "tbv",
            "client provided",
            "user provided",
            "manual input",
        ],
    );
    m.insert(
        "allegation.verify",
        vec![
            "verify allegation",
            "confirm claim",
            "allegation verified",
            // Corner cases
            "claim confirmed",
            "verified against source",
            "cross-checked",
            "corroborated",
            "substantiated",
            "supported by evidence",
            "documented verification",
            "independent verification",
            "third party confirmation",
            "registry confirmation",
            "source confirmation",
        ],
    );
    m.insert(
        "allegation.contradict",
        vec![
            "contradict allegation",
            "allegation false",
            "not accurate",
            // Corner cases
            "claim contradicted",
            "claim disputed",
            "evidence contradicts",
            "source contradicts",
            "incorrect claim",
            "false claim",
            "inaccurate claim",
            "misleading claim",
            "inconsistent",
            "does not match",
            "mismatch found",
            "discrepancy found",
            "verification failed",
        ],
    );
    m.insert(
        "allegation.mark-partial",
        vec![
            "partial verification",
            "partially correct",
            "partly verified",
            // Corner cases
            "partial match",
            "close match",
            "approximate match",
            "near match",
            "minor discrepancy",
            "immaterial difference",
            "acceptable variance",
            "within tolerance",
            "partial confirmation",
            "qualified verification",
            "conditional verification",
        ],
    );
    m.insert(
        "allegation.list-by-entity",
        vec![
            "list allegations",
            "entity allegations",
            "client claims",
            // Corner cases
            "allegation list",
            "claim list",
            "self-declarations",
            "attestations",
            "representations",
            "client statements",
            "pending claims",
            "verified claims",
            "contradicted claims",
        ],
    );
    m.insert(
        "allegation.list-pending",
        vec![
            "pending allegations",
            "unverified claims",
            "needs verification",
            // Corner cases
            "awaiting verification",
            "verification queue",
            "tbv queue",
            "outstanding claims",
            "open allegations",
            "unchecked claims",
            "not yet verified",
            "verification backlog",
        ],
    );

    // ==========================================================================
    // DISCREPANCY VERBS - Enhanced with corner cases
    // ==========================================================================
    m.insert(
        "discrepancy.record",
        vec![
            "record discrepancy",
            "data conflict",
            "observation conflict",
            "mismatch",
            // Corner cases
            "log discrepancy",
            "note discrepancy",
            "create discrepancy",
            "flag discrepancy",
            "data mismatch",
            "value mismatch",
            "source conflict",
            "inconsistency",
            "data inconsistency",
            "variance",
            "deviation",
            "difference found",
            "conflict identified",
            "reconciliation break",
            "break identified",
        ],
    );
    m.insert(
        "discrepancy.resolve",
        vec![
            "resolve discrepancy",
            "fix conflict",
            "discrepancy resolved",
            // Corner cases
            "conflict resolved",
            "mismatch resolved",
            "reconciled",
            "break resolved",
            "corrected",
            "fixed",
            "updated to correct",
            "source updated",
            "value corrected",
            "resolution applied",
            "winner selected",
            "golden source selected",
            "authoritative source chosen",
        ],
    );
    m.insert(
        "discrepancy.escalate",
        vec![
            "escalate discrepancy",
            "serious conflict",
            // Corner cases
            "material discrepancy",
            "significant difference",
            "major mismatch",
            "cannot resolve",
            "needs decision",
            "needs investigation",
            "requires senior input",
            "compliance escalation",
            "data quality escalation",
            "ops escalation",
        ],
    );
    m.insert(
        "discrepancy.list-open",
        vec![
            "list discrepancies",
            "open conflicts",
            "unresolved discrepancies",
            // Corner cases
            "discrepancy list",
            "conflict list",
            "mismatch list",
            "open breaks",
            "unresolved breaks",
            "pending reconciliation",
            "reconciliation queue",
            "conflict queue",
            "data quality issues",
        ],
    );

    // ==========================================================================
    // VERIFICATION VERBS - Enhanced with corner cases
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
            // Corner cases
            "pattern detection",
            "pattern recognition",
            "typology detection",
            "suspicious pattern",
            "risk pattern",
            "anomaly detection",
            "outlier detection",
            "unusual pattern",
            "shell company detection",
            "complex structure detection",
            "multiple layering",
            "nominee arrangement",
            "front company",
            "pass-through entity",
            "conduit entity",
            "structuring detection",
            "round tripping",
            "mirror trading",
            "back-to-back",
            "circular transactions",
            "ownership loop",
            "cross-ownership",
            "interlocking directors",
        ],
    );
    m.insert(
        "verify.detect-evasion",
        vec![
            "detect evasion",
            "evasion signals",
            "suspicious behavior",
            "document delays",
            // Corner cases
            "evasion detection",
            "evasive behavior",
            "non-cooperation",
            "reluctance",
            "avoidance",
            "delay tactics",
            "stalling",
            "incomplete responses",
            "partial disclosure",
            "selective disclosure",
            "changing story",
            "inconsistent answers",
            "contradictory information",
            "refusal to provide",
            "excessive redactions",
            "missing information",
            "unexplained gaps",
            "document manipulation",
            "forgery indicators",
            "fraud indicators",
            "identity fraud signals",
            "document fraud signals",
        ],
    );
    m.insert(
        "verify.challenge",
        vec![
            "raise challenge",
            "verification challenge",
            "question client",
            "formal challenge",
            // Corner cases
            "challenge observation",
            "dispute value",
            "question data",
            "require explanation",
            "request clarification",
            "seek confirmation",
            "formal inquiry",
            "written challenge",
            "documented challenge",
            "challenge response required",
            "deadline for response",
            "escalated challenge",
            "compliance challenge",
            "regulatory challenge",
        ],
    );
    m.insert(
        "verify.respond-to-challenge",
        vec![
            "respond to challenge",
            "challenge response",
            "answer challenge",
            // Corner cases
            "provide explanation",
            "submit clarification",
            "response submitted",
            "evidence provided",
            "documentation submitted",
            "client response",
            "formal response",
            "written response",
            "response received",
            "challenge addressed",
            "defense submitted",
            "justification provided",
        ],
    );
    m.insert(
        "verify.resolve-challenge",
        vec![
            "resolve challenge",
            "challenge resolved",
            "accept challenge",
            "reject challenge",
            // Corner cases
            "challenge disposition",
            "challenge outcome",
            "challenge decision",
            "challenge closed",
            "challenge accepted",
            "challenge rejected",
            "explanation accepted",
            "explanation rejected",
            "satisfactory response",
            "unsatisfactory response",
            "adequate explanation",
            "inadequate explanation",
            "challenge sustained",
            "challenge overruled",
        ],
    );
    m.insert(
        "verify.list-challenges",
        vec![
            "list challenges",
            "open challenges",
            "all challenges",
            // Corner cases
            "challenge list",
            "challenge queue",
            "pending challenges",
            "resolved challenges",
            "challenge history",
            "challenge timeline",
            "challenge summary",
            "challenge report",
            "outstanding challenges",
            "overdue challenges",
        ],
    );
    m.insert(
        "verify.escalate",
        vec![
            "escalate verification",
            "verification escalation",
            "senior review",
            "mlro review",
            // Corner cases
            "escalate to senior",
            "escalate to compliance",
            "escalate to committee",
            "escalate to board",
            "escalate to regulator",
            "sar consideration",
            "suspicious activity report",
            "str consideration",
            "suspicious transaction report",
            "regulatory reporting consideration",
            "second opinion",
            "independent review",
            "quality assurance",
            "qa review",
        ],
    );
    m.insert(
        "verify.resolve-escalation",
        vec![
            "resolve escalation",
            "escalation decision",
            "escalation resolved",
            // Corner cases
            "escalation outcome",
            "senior decision",
            "committee decision",
            "board decision",
            "escalation closed",
            "escalation completed",
            "decision recorded",
            "approval granted",
            "approval denied",
            "proceed",
            "do not proceed",
            "exit relationship",
            "file sar",
            "no sar required",
        ],
    );
    m.insert(
        "verify.list-escalations",
        vec![
            "list escalations",
            "open escalations",
            "pending decisions",
            // Corner cases
            "escalation list",
            "escalation queue",
            "escalation backlog",
            "decision queue",
            "awaiting decision",
            "escalation history",
            "escalation timeline",
            "escalation report",
        ],
    );
    m.insert(
        "verify.calculate-confidence",
        vec![
            "calculate confidence",
            "confidence score",
            "how confident",
            "data quality",
            // Corner cases
            "confidence level",
            "confidence rating",
            "certainty score",
            "certainty level",
            "data quality score",
            "verification score",
            "trust score",
            "reliability score",
            "source reliability",
            "evidence strength",
            "corroboration level",
            "multi-source confidence",
            "composite confidence",
            "weighted confidence",
        ],
    );
    m.insert(
        "verify.get-status",
        vec![
            "verification status",
            "verification report",
            "how verified",
            // Corner cases
            "verification summary",
            "verification overview",
            "verification dashboard",
            "verification metrics",
            "verification progress",
            "verification completion",
            "verified attributes",
            "unverified attributes",
            "verification gaps",
            "verification coverage",
        ],
    );
    m.insert(
        "verify.verify-against-registry",
        vec![
            "verify against registry",
            "registry check",
            "gleif check",
            "companies house check",
            // Corner cases
            "registry verification",
            "official registry",
            "public registry",
            "government registry",
            "corporate registry",
            "trade registry",
            "commercial registry",
            "business registry",
            "company register check",
            "lei verification",
            "lei lookup",
            "legal entity identifier",
            "kbis check",
            "handelsregister check",
            "kvk check",
            "sec edgar check",
            "fca register check",
            "bafin register check",
            "cssf register check",
        ],
    );
    m.insert(
        "verify.assert",
        vec![
            "assert confidence",
            "minimum confidence",
            "confidence gate",
            "verification gate",
            // Corner cases
            "confidence threshold",
            "confidence requirement",
            "minimum threshold",
            "quality gate",
            "verification requirement",
            "must have confidence",
            "required confidence",
            "confidence check",
            "threshold check",
            "pass fail check",
            "go no-go check",
        ],
    );
    m.insert(
        "verify.record-pattern",
        vec![
            "record pattern",
            "log pattern",
            "pattern detected",
            // Corner cases
            "pattern identified",
            "pattern logged",
            "pattern recorded",
            "typology match logged",
            "risk indicator logged",
            "suspicious indicator logged",
            "flag pattern",
            "note pattern",
            "document pattern",
        ],
    );
    m.insert(
        "verify.resolve-pattern",
        vec![
            "resolve pattern",
            "dismiss pattern",
            "pattern resolved",
            // Corner cases
            "pattern explained",
            "pattern mitigated",
            "pattern accepted",
            "false positive pattern",
            "legitimate pattern",
            "commercial rationale",
            "business justification",
            "pattern closed",
            "no action required",
        ],
    );
    m.insert(
        "verify.list-patterns",
        vec![
            "list patterns",
            "detected patterns",
            "suspicious patterns",
            // Corner cases
            "pattern list",
            "pattern history",
            "pattern summary",
            "open patterns",
            "resolved patterns",
            "pattern queue",
            "typology matches",
            "risk indicators",
        ],
    );

    // ==========================================================================
    // ONBOARDING VERBS - Enhanced with corner cases
    // ==========================================================================
    m.insert(
        "onboarding.auto-complete",
        vec![
            // Core terms
            "auto complete onboarding",
            "generate missing entities",
            "fill gaps automatically",
            "autopilot onboarding",
            "automated onboarding",
            // Corner cases - automation types
            "auto fill",
            "auto populate",
            "auto generate",
            "auto discover",
            "auto create",
            "smart complete",
            "intelligent completion",
            "ai assisted",
            "ml powered",
            "suggested completion",
            "recommended entities",
            "inferred entities",
            "derived entities",
            // Corner cases - contexts
            "complete structure",
            "fill ownership gaps",
            "generate ubo chain",
            "derive beneficial owners",
            "suggest directors",
            "suggest officers",
            "complete org chart",
            "fill roles",
            "suggest roles",
            "recommend parties",
            // Question forms
            "how to auto complete",
            "can system fill gaps",
            "auto generate entities",
        ],
    );
    m.insert(
        "onboarding.validate-completeness",
        vec![
            // Core terms
            "validate onboarding completeness",
            "check if ready",
            "onboarding validation",
            "completeness check",
            // Corner cases
            "readiness assessment",
            "go live check",
            "pre-flight check",
            "quality gate",
            "completion check",
            "missing items",
            "outstanding items",
            "gaps analysis",
            "what is missing",
            "what is incomplete",
            "validation errors",
            "blocking issues",
            "onboarding blockers",
            "required fields",
            "mandatory items",
            // Question forms
            "is onboarding complete",
            "are we ready to go live",
            "what is missing for completion",
        ],
    );
    m.insert(
        "onboarding.generate-checklist",
        vec![
            // Core terms
            "generate checklist",
            "onboarding checklist",
            "requirements list",
            "todo list",
            // Corner cases
            "task list",
            "action items",
            "requirement matrix",
            "completion checklist",
            "document checklist",
            "verification checklist",
            "screening checklist",
            "due diligence checklist",
            "kyc checklist",
            "aml checklist",
            "cdd checklist",
            "edd checklist",
            "regulatory checklist",
            "compliance checklist",
            "standard checklist",
            "custom checklist",
            "template checklist",
            // Question forms
            "what checklist for this client",
            "generate requirements",
        ],
    );
    m.insert(
        "onboarding.estimate-timeline",
        vec![
            // Core terms
            "estimate timeline",
            "onboarding duration",
            "how long to onboard",
            "time estimate",
            // Corner cases
            "expected duration",
            "target completion",
            "eta",
            "estimated completion",
            "projected timeline",
            "milestone dates",
            "key dates",
            "deadline forecast",
            "schedule projection",
            "capacity planning",
            "resource requirement",
            "effort estimate",
            "complexity assessment",
            // Question forms
            "how long will onboarding take",
            "when will onboarding complete",
        ],
    );
    m.insert(
        "onboarding.track-progress",
        vec![
            // Core terms
            "track onboarding progress",
            "progress tracking",
            "onboarding status",
            "completion percentage",
            // Corner cases
            "progress bar",
            "percent complete",
            "milestone tracking",
            "phase tracking",
            "stage tracking",
            "workflow progress",
            "pipeline progress",
            "funnel progress",
            "dashboard view",
            "status board",
            "kanban",
            "sprint progress",
            "burndown",
            // Question forms
            "how far along is onboarding",
            "what is onboarding progress",
        ],
    );
    m.insert(
        "onboarding.fast-track",
        vec![
            // Core terms
            "fast track onboarding",
            "expedite onboarding",
            "priority onboarding",
            "urgent onboarding",
            // Corner cases
            "accelerated onboarding",
            "rapid onboarding",
            "quick onboarding",
            "express onboarding",
            "vip onboarding",
            "white glove",
            "concierge onboarding",
            "priority lane",
            "skip queue",
            "bump priority",
            "critical client",
            "strategic client",
            "key account",
            // Question forms
            "how to fast track",
            "can we expedite",
        ],
    );
    m.insert(
        "onboarding.pause",
        vec![
            // Core terms
            "pause onboarding",
            "hold onboarding",
            "stop onboarding",
            "suspend onboarding",
            // Corner cases
            "onboarding on hold",
            "client delay",
            "pending client",
            "awaiting client",
            "temporary hold",
            "onboarding stalled",
            "onboarding blocked",
            "snooze onboarding",
            "defer onboarding",
            "delay onboarding",
            "postpone onboarding",
            // Question forms
            "how to pause onboarding",
            "put onboarding on hold",
        ],
    );
    m.insert(
        "onboarding.resume",
        vec![
            // Core terms
            "resume onboarding",
            "restart onboarding",
            "continue onboarding",
            "unpause onboarding",
            // Corner cases
            "reactivate onboarding",
            "take off hold",
            "remove hold",
            "client ready",
            "client responded",
            "blockers resolved",
            "continue where left off",
            "pick up onboarding",
            "onboarding active again",
            // Question forms
            "how to resume onboarding",
            "restart paused onboarding",
        ],
    );
    m.insert(
        "onboarding.abort",
        vec![
            // Core terms
            "abort onboarding",
            "cancel onboarding",
            "terminate onboarding",
            "decline client",
            // Corner cases
            "exit onboarding",
            "stop onboarding permanently",
            "client declined",
            "prospect lost",
            "no longer pursuing",
            "client withdrew",
            "relationship terminated",
            "failed onboarding",
            "onboarding rejected",
            "compliance rejection",
            "risk rejection",
            "committee decline",
            "board rejection",
            // Question forms
            "how to cancel onboarding",
            "terminate client onboarding",
        ],
    );
    m.insert(
        "onboarding.clone-template",
        vec![
            // Core terms
            "clone onboarding template",
            "use template",
            "apply template",
            "standard template",
            // Corner cases
            "copy from template",
            "template based onboarding",
            "boilerplate onboarding",
            "standard onboarding",
            "product template",
            "client type template",
            "jurisdiction template",
            "regulatory template",
            "best practice template",
            "playbook",
            "runbook",
            "standard operating procedure",
            "sop",
            // Question forms
            "which template to use",
            "apply onboarding template",
        ],
    );

    // ==========================================================================
    // HOLDING VERBS - Enhanced with corner cases
    // ==========================================================================
    m.insert(
        "holding.create",
        vec![
            // Core terms
            "create holding",
            "new investor holding",
            "register holding",
            "add holding",
            "establish position",
            // Corner cases - holding types
            "new position",
            "open position",
            "initial position",
            "first position",
            "investor position",
            "shareholder position",
            "unitholder position",
            "beneficial holding",
            "nominee holding",
            "direct holding",
            "indirect holding",
            "omnibus holding",
            "segregated holding",
            "pooled holding",
            // Corner cases - contexts
            "fund holding",
            "share class holding",
            "investor account",
            "ta account",
            "registry entry",
            "register entry",
            "certificate",
            "share certificate",
            "unit certificate",
            "book entry",
            "dematerialized holding",
            "drs position",
            // Question forms
            "how to create holding",
            "how to register investor",
            "how to add position",
        ],
    );
    m.insert(
        "holding.ensure",
        vec![
            // Core terms
            "ensure holding",
            "upsert holding",
            "create or update holding",
            "find or create holding",
            // Corner cases
            "idempotent holding",
            "holding if not exists",
            "safe holding create",
            "get or create holding",
            "lookup or create holding",
            "check and create",
        ],
    );
    m.insert(
        "holding.update-units",
        vec![
            // Core terms
            "update holding units",
            "adjust position",
            "change units",
            "modify units",
            // Corner cases - adjustment types
            "increase units",
            "decrease units",
            "add units",
            "remove units",
            "unit correction",
            "unit adjustment",
            "position adjustment",
            "rebalance",
            "top up",
            "partial redemption",
            "dilution adjustment",
            "stock split adjustment",
            "dividend reinvestment",
            "drip",
            "scrip dividend",
            "bonus issue",
            "rights issue",
            "unit consolidation",
            "reverse split",
            "corporate action adjustment",
            // Question forms
            "how to adjust units",
            "how to change position size",
        ],
    );
    m.insert(
        "holding.read",
        vec![
            // Core terms
            "read holding",
            "holding details",
            "get holding",
            "view holding",
            "show holding",
            // Corner cases
            "holding information",
            "position details",
            "account details",
            "investor details",
            "current units",
            "current position",
            "holding snapshot",
            "as of position",
            "point in time holding",
            "historical holding",
            "holding at date",
            "holding value",
            "nav position",
            // Question forms
            "what is holding",
            "show me position",
        ],
    );
    m.insert(
        "holding.list-by-share-class",
        vec![
            // Core terms
            "holdings by share class",
            "share class investors",
            "who holds this class",
            "class holdings",
            // Corner cases
            "investors in class",
            "unitholders in class",
            "shareholders in class",
            "class register",
            "share class register",
            "class breakdown",
            "class composition",
            "investor concentration",
            "top holders",
            "largest investors",
            "holder distribution",
            "ownership distribution",
            "register extract",
            "certified register",
            // Question forms
            "who are the investors in this class",
            "list share class holders",
        ],
    );
    m.insert(
        "holding.list-by-investor",
        vec![
            // Core terms
            "holdings by investor",
            "investor portfolio",
            "what does investor hold",
            "investor positions",
            // Corner cases
            "investor account summary",
            "all positions",
            "portfolio summary",
            "portfolio breakdown",
            "asset allocation",
            "fund exposure",
            "investor statement",
            "position report",
            "holdings report",
            "consolidated holdings",
            "across funds",
            "cross fund holdings",
            "total investment",
            "total units",
            "total nav",
            // Question forms
            "what does this investor own",
            "show investor portfolio",
        ],
    );
    m.insert(
        "holding.close",
        vec![
            // Core terms
            "close holding",
            "zero holding",
            "exit position",
            "close position",
            // Corner cases
            "full redemption",
            "complete exit",
            "liquidate position",
            "wind down holding",
            "terminate holding",
            "close account",
            "end position",
            "holding closed",
            "no longer invested",
            "former investor",
            "historical investor",
            "departed investor",
            "exited investor",
            // Question forms
            "how to close position",
            "how to exit holding",
        ],
    );
    m.insert(
        "holding.lock",
        vec![
            // Core terms
            "lock holding",
            "freeze holding",
            "restrict holding",
            "block redemption",
            // Corner cases
            "soft lock",
            "hard lock",
            "lock up period",
            "redemption lock",
            "transfer restriction",
            "regulatory hold",
            "legal hold",
            "litigation hold",
            "dispute hold",
            "sanction freeze",
            "kyc hold",
            "aml hold",
            "gating",
            "investor gating",
            "redemption gate",
            "side pocket",
            // Question forms
            "how to lock position",
            "restrict redemptions",
        ],
    );
    m.insert(
        "holding.unlock",
        vec![
            // Core terms
            "unlock holding",
            "unfreeze holding",
            "remove restriction",
            "allow redemption",
            // Corner cases
            "release hold",
            "lift restriction",
            "unlock position",
            "end lock up",
            "lock up expired",
            "redemption allowed",
            "transfer allowed",
            "clear hold",
            "remove block",
            // Question forms
            "how to unlock position",
            "release holding restriction",
        ],
    );
    m.insert(
        "holding.transfer",
        vec![
            // Core terms
            "transfer holding",
            "change ownership",
            "reassign holding",
            "holding transfer",
            // Corner cases
            "re-registration",
            "re-reg",
            "change of beneficial owner",
            "inheritance transfer",
            "estate transfer",
            "gift transfer",
            "intra-family transfer",
            "corporate reorganization",
            "scheme of arrangement",
            "merger transfer",
            "name change",
            "account rename",
            // Question forms
            "how to transfer holding",
            "change holding owner",
        ],
    );
    m.insert(
        "holding.certify",
        vec![
            // Core terms
            "certify holding",
            "holding certification",
            "certified extract",
            // Corner cases
            "certified register extract",
            "share certificate",
            "certificate of ownership",
            "holding statement",
            "position confirmation",
            "investor confirmation",
            "audit confirmation",
            "balance confirmation",
            "registrar confirmation",
            // Question forms
            "provide holding certification",
            "certify position",
        ],
    );

    // ==========================================================================
    // MOVEMENT VERBS - Enhanced with corner cases
    // ==========================================================================
    m.insert(
        "movement.subscribe",
        vec![
            // Core terms
            "subscription",
            "buy units",
            "invest in fund",
            "new subscription",
            "subscribe",
            "investor subscription",
            // Corner cases - subscription types
            "initial subscription",
            "additional subscription",
            "top up",
            "subsequent subscription",
            "follow on investment",
            "co-investment",
            "side letter investment",
            "commitment drawdown",
            "capital call",
            "capital contribution",
            "cash subscription",
            "in specie subscription",
            "in kind subscription",
            "asset contribution",
            "rollover subscription",
            "tax lot",
            // Corner cases - contexts
            "buy order",
            "purchase order",
            "investment order",
            "subscription order",
            "deal",
            "subscription deal",
            "commitment",
            "pledge",
            "allocation",
            "series subscription",
            "tranche subscription",
            "closing subscription",
            // Question forms
            "how to subscribe",
            "how to invest",
            "process subscription",
        ],
    );
    m.insert(
        "movement.redeem",
        vec![
            // Core terms
            "redemption",
            "sell units",
            "redeem holding",
            "cash out",
            "redeem",
            "investor redemption",
            // Corner cases - redemption types
            "partial redemption",
            "full redemption",
            "complete redemption",
            "in kind redemption",
            "in specie redemption",
            "scheduled redemption",
            "unscheduled redemption",
            "mandatory redemption",
            "voluntary redemption",
            "forced redemption",
            "compulsory redemption",
            "early redemption",
            "penalty redemption",
            "soft lock redemption",
            "hard lock redemption",
            // Corner cases - contexts
            "sell order",
            "sale order",
            "disinvestment",
            "divestment",
            "liquidation",
            "withdrawal",
            "payout",
            "distribution request",
            "capital distribution",
            "return of capital",
            "notice period redemption",
            "dealing day redemption",
            // Question forms
            "how to redeem",
            "process redemption",
            "sell units",
        ],
    );
    m.insert(
        "movement.transfer-in",
        vec![
            // Core terms
            "transfer in",
            "incoming transfer",
            "receive units",
            "inbound transfer",
            // Corner cases - transfer types
            "re-registration in",
            "re-reg in",
            "delivery in",
            "receipt",
            "incoming delivery",
            "external transfer in",
            "internal transfer in",
            "inter-fund transfer in",
            "cross-fund transfer",
            "conversion in",
            "switch in",
            "exchange in",
            "rollover in",
            "ira transfer in",
            "pension transfer in",
            "inheritance receipt",
            "estate distribution",
            "gift receipt",
            // Corner cases - contexts
            "book in",
            "receive delivery",
            "custody receipt",
            "depository credit",
            "csd credit",
            "dtcc delivery",
            "euroclear delivery",
            "clearstream delivery",
            // Question forms
            "how to transfer in",
            "receive units from",
        ],
    );
    m.insert(
        "movement.transfer-out",
        vec![
            // Core terms
            "transfer out",
            "outgoing transfer",
            "send units",
            "outbound transfer",
            // Corner cases - transfer types
            "re-registration out",
            "re-reg out",
            "delivery out",
            "dispatch",
            "outgoing delivery",
            "external transfer out",
            "internal transfer out",
            "inter-fund transfer out",
            "conversion out",
            "switch out",
            "exchange out",
            "rollover out",
            "ira transfer out",
            "pension transfer out",
            "gift",
            "donation",
            // Corner cases - contexts
            "book out",
            "make delivery",
            "custody delivery",
            "depository debit",
            "csd debit",
            "free delivery",
            "dvp delivery",
            // Question forms
            "how to transfer out",
            "send units to",
        ],
    );
    m.insert(
        "movement.switch",
        vec![
            // Core terms
            "switch",
            "fund switch",
            "class switch",
            "exchange",
            // Corner cases
            "switch between classes",
            "switch between funds",
            "conversion",
            "unit exchange",
            "share exchange",
            "class conversion",
            "fund conversion",
            "lateral transfer",
            "rebalance",
            "reallocate",
            "restructure",
            "same day switch",
            "t+0 switch",
            "simultaneous switch",
            // Question forms
            "how to switch",
            "convert between classes",
        ],
    );
    m.insert(
        "movement.confirm",
        vec![
            // Core terms
            "confirm movement",
            "movement confirmed",
            "trade confirmed",
            "confirm transaction",
            // Corner cases - confirmation types
            "order confirmation",
            "trade confirmation",
            "deal confirmation",
            "execution confirmation",
            "contract note",
            "confirmation note",
            "booking confirmation",
            "allocation confirmation",
            "verbal confirmation",
            "written confirmation",
            "electronic confirmation",
            "swift confirmation",
            "fix confirmation",
            "matching confirmation",
            "affirmation",
            // Question forms
            "how to confirm trade",
            "confirm movement",
        ],
    );
    m.insert(
        "movement.settle",
        vec![
            // Core terms
            "settle movement",
            "movement settled",
            "trade settled",
            "settlement",
            // Corner cases - settlement types
            "cash settlement",
            "physical settlement",
            "dvp settlement",
            "fop settlement",
            "delivery versus payment",
            "free of payment",
            "rvp settlement",
            "receive versus payment",
            "net settlement",
            "gross settlement",
            "same day settlement",
            "t+0 settlement",
            "t+1 settlement",
            "t+2 settlement",
            "t+3 settlement",
            "standard settlement",
            "non-standard settlement",
            "delayed settlement",
            "future settlement",
            "forward settlement",
            // Question forms
            "how to settle",
            "complete settlement",
        ],
    );
    m.insert(
        "movement.cancel",
        vec![
            // Core terms
            "cancel movement",
            "void transaction",
            "movement cancelled",
            "cancel transaction",
            // Corner cases
            "cancel order",
            "withdraw order",
            "retract order",
            "trade cancellation",
            "deal cancellation",
            "bust trade",
            "break trade",
            "error correction",
            "transaction reversal",
            "movement reversal",
            "undo movement",
            "cancel before settlement",
            "pre-settlement cancel",
            // Question forms
            "how to cancel",
            "void transaction",
        ],
    );
    m.insert(
        "movement.fail",
        vec![
            // Core terms
            "fail movement",
            "settlement fail",
            "failed trade",
            "movement failed",
            // Corner cases
            "fail to deliver",
            "fail to receive",
            "settlement failure",
            "buy in",
            "sell out",
            "fails management",
            "fail aging",
            "aged fail",
            "chronic fail",
            "systemic fail",
            "counterparty fail",
            "cash fail",
            "securities fail",
            "partial fail",
            "full fail",
            // Question forms
            "why did settlement fail",
            "failed settlements",
        ],
    );
    m.insert(
        "movement.list-by-holding",
        vec![
            // Core terms
            "list movements",
            "transaction history",
            "holding transactions",
            "movement history",
            // Corner cases - views
            "transaction log",
            "activity log",
            "dealing history",
            "trade history",
            "order history",
            "all transactions",
            "position changes",
            "unit changes",
            "chronological movements",
            "movement timeline",
            "statement",
            "investor statement",
            "account statement",
            "activity statement",
            // Question forms
            "what transactions on this holding",
            "show movement history",
        ],
    );
    m.insert(
        "movement.read",
        vec![
            // Core terms
            "read movement",
            "movement details",
            "transaction details",
            "get movement",
            // Corner cases
            "view movement",
            "show movement",
            "movement information",
            "trade details",
            "order details",
            "deal details",
            "settlement details",
            "movement status",
            "transaction status",
            "lifecycle status",
            // Question forms
            "show me this movement",
            "movement information",
        ],
    );
    m.insert(
        "movement.amend",
        vec![
            // Core terms
            "amend movement",
            "correct movement",
            "modify transaction",
            "update movement",
            // Corner cases
            "price correction",
            "quantity correction",
            "date correction",
            "account correction",
            "amendment",
            "trade amendment",
            "deal amendment",
            "post trade amendment",
            "allocation amendment",
            "late allocation",
            "reallocation",
            // Question forms
            "how to amend movement",
            "correct transaction",
        ],
    );
    m.insert(
        "movement.reverse",
        vec![
            // Core terms
            "reverse movement",
            "reversal",
            "movement reversal",
            "undo movement",
            // Corner cases
            "counter movement",
            "offset movement",
            "error reversal",
            "correction reversal",
            "storno",
            "back out",
            "unwind",
            "trade unwind",
            "as of adjustment",
            "back dated correction",
            // Question forms
            "how to reverse movement",
            "undo transaction",
        ],
    );

    // ==========================================================================
    // REFERENCE DATA VERBS - Enhanced with corner cases
    // ==========================================================================
    m.insert(
        "market.read",
        vec![
            // Core terms
            "read market",
            "market details",
            "mic code",
            "exchange info",
            "market info",
            // Corner cases - market identifiers
            "market identifier",
            "iso 10383",
            "operating mic",
            "segment mic",
            "market name",
            "exchange name",
            "trading venue",
            "execution venue",
            "mtf",
            "otf",
            "systematic internaliser",
            // Corner cases - market data
            "market hours",
            "trading hours",
            "market timezone",
            "market country",
            "market currency",
            "settlement currency",
            "market status",
            "market open",
            "market close",
            "pre-market",
            "after hours",
            "auction",
            // Question forms
            "what is this market",
            "market information",
        ],
    );
    m.insert(
        "market.list",
        vec![
            // Core terms
            "list markets",
            "available markets",
            "all exchanges",
            "market catalog",
            // Corner cases - filters
            "markets by country",
            "markets by currency",
            "markets by region",
            "equity markets",
            "bond markets",
            "derivative markets",
            "commodity markets",
            "fx markets",
            "money markets",
            "primary markets",
            "secondary markets",
            "otc markets",
            "regulated markets",
            "unregulated markets",
            "dark pools",
            "lit markets",
            // Corner cases - views
            "market directory",
            "exchange directory",
            "venue list",
            "supported markets",
            "enabled markets",
            "active markets",
            // Question forms
            "what markets are available",
            "which exchanges do we support",
        ],
    );
    m.insert(
        "market.set-holiday-calendar",
        vec![
            // Core terms
            "set holiday calendar",
            "market holidays",
            "trading calendar",
            "business days",
            // Corner cases - calendar types
            "exchange calendar",
            "settlement calendar",
            "holiday schedule",
            "non-trading days",
            "bank holidays",
            "public holidays",
            "market closures",
            "early close",
            "half day",
            "irregular trading",
            "ad hoc closure",
            "emergency closure",
            // Corner cases - management
            "add holiday",
            "remove holiday",
            "update calendar",
            "calendar sync",
            "calendar import",
            "calendar export",
            "calendar subscription",
            "calendar feed",
            // Question forms
            "when is market closed",
            "what are market holidays",
        ],
    );
    m.insert(
        "market.calculate-settlement-date",
        vec![
            // Core terms
            "calculate settlement date",
            "when does it settle",
            "settlement date calc",
            "value date",
            // Corner cases - settlement cycles
            "t+0 settlement",
            "t+1 settlement",
            "t+2 settlement",
            "t+3 settlement",
            "settlement cycle",
            "standard settlement",
            "non-standard settlement",
            "forward settlement",
            "spot settlement",
            "next day settlement",
            // Corner cases - calculations
            "business day calculation",
            "settlement day calculation",
            "good value date",
            "next good day",
            "modified following",
            "preceding",
            "following",
            "end of month",
            "imm date",
            "cross border settlement",
            "dual calendar",
            "joint calendar",
            // Question forms
            "what is settlement date",
            "when will this settle",
        ],
    );
    m.insert(
        "instrument-class.read",
        vec![
            // Core terms
            "read instrument class",
            "instrument class details",
            "asset class info",
            "instrument classification",
            // Corner cases - classification systems
            "cfi code",
            "iso 10962",
            "asset class",
            "instrument type",
            "product type",
            "security category",
            "investment category",
            "smpg classification",
            "anna classification",
            "figi classification",
            "bloomberg classification",
            // Corner cases - attributes
            "class characteristics",
            "instrument features",
            "settlement requirements",
            "custody requirements",
            "regulatory classification",
            "mifid classification",
            "priips classification",
            // Question forms
            "what type of instrument",
            "instrument category",
        ],
    );
    m.insert(
        "instrument-class.list",
        vec![
            // Core terms
            "list instrument classes",
            "asset classes",
            "instrument catalog",
            "instrument taxonomy",
            // Corner cases - filters
            "equity classes",
            "fixed income classes",
            "derivative classes",
            "fund classes",
            "commodity classes",
            "alternative classes",
            "structured product classes",
            "money market classes",
            "fx classes",
            // Corner cases - views
            "class hierarchy",
            "class tree",
            "class breakdown",
            "supported instruments",
            "enabled instruments",
            // Question forms
            "what instrument types",
            "available asset classes",
        ],
    );
    m.insert(
        "security-type.read",
        vec![
            // Core terms
            "read security type",
            "security type details",
            "smpg code",
            "security classification",
            // Corner cases - security identifiers
            "isin",
            "cusip",
            "sedol",
            "figi",
            "ticker",
            "wkn",
            "valor",
            "ric",
            "bloomberg id",
            // Corner cases - type details
            "security features",
            "instrument characteristics",
            "maturity",
            "coupon",
            "dividend",
            "fungibility",
            "denomination",
            "form",
            "registered",
            "bearer",
            "dematerialized",
            // Question forms
            "what type of security",
            "security information",
        ],
    );
    m.insert(
        "security-type.list",
        vec![
            // Core terms
            "list security types",
            "security type catalog",
            "security taxonomy",
            // Corner cases - filters
            "equity types",
            "bond types",
            "derivative types",
            "fund types",
            "warrant types",
            "certificate types",
            "structured types",
            "etf types",
            "etn types",
            "adr types",
            "gdr types",
            // Question forms
            "what security types",
            "available security types",
        ],
    );
    m.insert(
        "subcustodian.read",
        vec![
            // Core terms
            "read subcustodian",
            "subcustodian details",
            "agent details",
            "local agent",
            // Corner cases - agent types
            "subcustodian info",
            "local custodian",
            "settlement agent",
            "clearing agent",
            "paying agent",
            "registrar",
            "transfer agent",
            "depository",
            "csd",
            "icsd",
            "nominee",
            "correspondent bank",
            "nostro agent",
            "vostro agent",
            // Corner cases - identifiers
            "bic code",
            "swift code",
            "lei",
            "participant id",
            "account number",
            // Question forms
            "who is subcustodian",
            "agent information",
        ],
    );
    m.insert(
        "subcustodian.list",
        vec![
            // Core terms
            "list subcustodians",
            "network agents",
            "local agents",
            "agent network",
            // Corner cases - filters
            "agents by market",
            "agents by country",
            "agents by region",
            "agents by currency",
            "equity agents",
            "fixed income agents",
            "derivative agents",
            "settlement agents",
            "custody agents",
            "primary agents",
            "backup agents",
            "active agents",
            "dormant agents",
            // Corner cases - views
            "agent directory",
            "correspondent network",
            "custodian network",
            "global network",
            // Question forms
            "who are our agents",
            "subcustodian network",
        ],
    );
    m.insert(
        "currency.read",
        vec![
            // Core terms
            "read currency",
            "currency details",
            "iso currency",
            "currency info",
            // Corner cases
            "currency code",
            "iso 4217",
            "currency name",
            "currency symbol",
            "decimal places",
            "minor unit",
            "currency country",
            "central bank",
            "currency group",
            "g10 currency",
            "em currency",
            "major currency",
            "exotic currency",
            // Question forms
            "what is this currency",
            "currency information",
        ],
    );
    m.insert(
        "currency.list",
        vec![
            // Core terms
            "list currencies",
            "available currencies",
            "supported currencies",
            // Corner cases
            "active currencies",
            "tradeable currencies",
            "base currencies",
            "quote currencies",
            "settlement currencies",
            "reporting currencies",
            "g10 currencies",
            "em currencies",
            "pegged currencies",
            "floating currencies",
            // Question forms
            "what currencies available",
            "supported currencies",
        ],
    );
    m.insert(
        "country.read",
        vec![
            // Core terms
            "read country",
            "country details",
            "iso country",
            "country info",
            // Corner cases
            "country code",
            "iso 3166",
            "alpha-2",
            "alpha-3",
            "country name",
            "region",
            "subregion",
            "continent",
            "currency",
            "timezone",
            "jurisdiction",
            "regulatory regime",
            // Question forms
            "what is this country",
            "country information",
        ],
    );
    m.insert(
        "country.list",
        vec![
            // Core terms
            "list countries",
            "available countries",
            "supported jurisdictions",
            // Corner cases
            "countries by region",
            "eu countries",
            "eea countries",
            "fatf countries",
            "high risk countries",
            "sanctioned countries",
            "tax treaty countries",
            "crs countries",
            "fatca countries",
            // Question forms
            "what countries available",
            "supported countries",
        ],
    );

    // ==========================================================================
    // UI NAVIGATION VERBS - Blade Runner Esper-style graph/matrix control
    // ==========================================================================

    // --------------------------------------------------------------------------
    // VIEW MODE SWITCHING
    // --------------------------------------------------------------------------
    m.insert(
        "ui.view-kyc",
        vec![
            // Direct commands
            "show kyc view",
            "switch to kyc",
            "kyc mode",
            "show kyc",
            "display kyc view",
            "kyc ubo view",
            "show ubo structure",
            "ownership view",
            "show ownership",
            // Blade Runner style
            "give me kyc",
            "pull up kyc",
            "bring up kyc view",
            // Question forms
            "can i see the kyc view",
            "show me kyc structure",
            "what does the kyc look like",
            // UK/US colloquialisms
            "let's see kyc",
            "gimme kyc",
            "kyc please",
            "need kyc view",
            "want kyc view",
        ],
    );
    m.insert(
        "ui.view-trading",
        vec![
            // Direct commands
            "show trading matrix",
            "switch to trading",
            "trading view",
            "trading mode",
            "show trading",
            "display trading matrix",
            "custody view",
            "show custody",
            "settlement view",
            "show ssi",
            "booking rules view",
            // Blade Runner style
            "give me trading",
            "pull up trading matrix",
            "bring up trading view",
            "show me the matrix",
            "matrix view",
            // Question forms
            "can i see the trading setup",
            "show me trading config",
            "what trading is configured",
            "how is trading set up",
            // UK/US colloquialisms
            "let's see trading",
            "gimme trading matrix",
            "trading please",
            "need trading view",
            "want to see trading",
            "pop up trading",
        ],
    );
    m.insert(
        "ui.view-services",
        vec![
            // Direct commands
            "show services view",
            "switch to services",
            "services mode",
            "show services",
            "display service delivery",
            "service delivery view",
            "products view",
            "show products",
            // Blade Runner style
            "give me services",
            "pull up services",
            "bring up service view",
            // Question forms
            "can i see the services",
            "show me what services",
            "what services are active",
            // UK/US colloquialisms
            "let's see services",
            "gimme services view",
            "services please",
        ],
    );
    m.insert(
        "ui.view-custody",
        vec![
            // Direct commands
            "show custody view",
            "switch to custody",
            "custody mode",
            "show custody setup",
            "display custody",
            "ssi view",
            "show ssis",
            "booking view",
            "show booking rules",
            // Blade Runner style
            "give me custody",
            "pull up custody",
            "bring up custody view",
            // Question forms
            "can i see custody config",
            "show me custody setup",
            "how is custody configured",
            // UK/US colloquialisms
            "let's see custody",
            "gimme custody view",
            "custody please",
        ],
    );

    // --------------------------------------------------------------------------
    // CBU LOADING
    // --------------------------------------------------------------------------
    m.insert(
        "ui.load-cbu",
        vec![
            // Direct commands
            "load cbu",
            "open cbu",
            "show cbu",
            "display cbu",
            "switch to cbu",
            "select cbu",
            "load client",
            "open client",
            "go to cbu",
            "navigate to cbu",
            // With names (patterns)
            "load allianz",
            "show allianz",
            "open apex fund",
            "display pacific growth",
            "switch to blackrock",
            "go to goldman",
            // Blade Runner style
            "give me cbu",
            "pull up cbu",
            "bring up cbu",
            "let me see cbu",
            // Question forms
            "can you show cbu",
            "can i see cbu",
            "show me the cbu for",
            "what does cbu look like",
            // UK/US colloquialisms
            "let's look at cbu",
            "gimme cbu",
            "cbu please",
            "need to see cbu",
            "want cbu",
            "pop up cbu",
            "fetch cbu",
        ],
    );

    // --------------------------------------------------------------------------
    // ZOOM AND PAN CONTROLS - Esper-style
    // --------------------------------------------------------------------------
    m.insert(
        "ui.zoom-in",
        vec![
            // Blade Runner Esper commands
            "enhance",
            "zoom in",
            "zoom",
            "closer",
            "move in",
            "go closer",
            "magnify",
            "enlarge",
            "bigger",
            "more detail",
            "enhance detail",
            "increase zoom",
            // Incremental
            "zoom in more",
            "enhance more",
            "closer still",
            "keep zooming",
            "continue zoom",
            // UK/US colloquialisms
            "get in there",
            "tighter",
            "push in",
            "go tighter",
            "drill in",
            "dive in",
        ],
    );
    m.insert(
        "ui.zoom-out",
        vec![
            // Blade Runner Esper commands
            "pull back",
            "zoom out",
            "wider",
            "move out",
            "go back",
            "reduce",
            "smaller",
            "less detail",
            "decrease zoom",
            "back out",
            "step back",
            // Incremental
            "zoom out more",
            "pull back more",
            "wider still",
            "keep pulling back",
            // UK/US colloquialisms
            "back off",
            "ease back",
            "pull out",
            "go wider",
            "big picture",
            "overview",
            "full view",
        ],
    );
    m.insert(
        "ui.zoom-fit",
        vec![
            // Blade Runner Esper commands
            "fit to screen",
            "fit view",
            "show all",
            "full view",
            "entire graph",
            "whole structure",
            "zoom to fit",
            "fit all",
            "see everything",
            "reset zoom",
            "default zoom",
            // UK/US colloquialisms
            "show me everything",
            "let me see all of it",
            "the whole thing",
            "bird's eye",
            "birds eye view",
            "30000 foot view",
            "helicopter view",
        ],
    );
    m.insert(
        "ui.pan-left",
        vec![
            // Blade Runner Esper commands
            "track left",
            "pan left",
            "move left",
            "go left",
            "left",
            "scroll left",
            "shift left",
            // Degrees
            "track 45 left",
            "pan 45 left",
            // UK/US colloquialisms
            "over to the left",
            "to the left",
            "leftward",
            "that way left",
        ],
    );
    m.insert(
        "ui.pan-right",
        vec![
            // Blade Runner Esper commands
            "track right",
            "pan right",
            "move right",
            "go right",
            "right",
            "scroll right",
            "shift right",
            // Degrees
            "track 45 right",
            "pan 45 right",
            // UK/US colloquialisms
            "over to the right",
            "to the right",
            "rightward",
            "that way right",
        ],
    );
    m.insert(
        "ui.pan-up",
        vec![
            // Blade Runner Esper commands
            "pan up",
            "move up",
            "go up",
            "up",
            "scroll up",
            "shift up",
            "track up",
            // UK/US colloquialisms
            "upward",
            "to the top",
            "head up",
            "climb",
        ],
    );
    m.insert(
        "ui.pan-down",
        vec![
            // Blade Runner Esper commands
            "pan down",
            "move down",
            "go down",
            "down",
            "scroll down",
            "shift down",
            "track down",
            // UK/US colloquialisms
            "downward",
            "to the bottom",
            "head down",
            "drop",
        ],
    );
    m.insert(
        "ui.center",
        vec![
            // Blade Runner Esper commands
            "center",
            "center and stop",
            "center view",
            "center on",
            "center that",
            "recenter",
            "re-center",
            "middle",
            "back to center",
            "home position",
            "home",
            // UK/US colloquialisms
            "put it in the middle",
            "center it",
            "bring to center",
        ],
    );

    // --------------------------------------------------------------------------
    // STOP/PAUSE CONTROLS - Esper-style
    // --------------------------------------------------------------------------
    m.insert(
        "ui.stop",
        vec![
            // Blade Runner Esper commands
            "stop",
            "hold",
            "freeze",
            "pause",
            "wait",
            "hold there",
            "stop there",
            "hold it",
            "freeze there",
            "that's good",
            "right there",
            "stay",
            "stay there",
            // UK/US colloquialisms
            "hang on",
            "wait a sec",
            "wait a moment",
            "whoa",
            "hold up",
            "stop right there",
        ],
    );

    // --------------------------------------------------------------------------
    // ENTITY FOCUS AND SELECTION
    // --------------------------------------------------------------------------
    m.insert(
        "ui.focus-entity",
        vec![
            // Blade Runner Esper commands
            "focus on",
            "center on",
            "zoom to",
            "go to",
            "navigate to",
            "show me",
            "highlight",
            "select",
            "find",
            "locate",
            "where is",
            // Entity targeting
            "focus on entity",
            "center on entity",
            "zoom to entity",
            "go to entity",
            "find entity",
            "highlight entity",
            "select entity",
            "click on",
            // Blade Runner style
            "enhance on",
            "give me a closer look at",
            "let me see",
            "pull up",
            "bring up",
            // UK/US colloquialisms
            "show that one",
            "what about that",
            "look at that",
            "check out",
            "drill into",
            "dive into",
            "zero in on",
            "hone in on",
            "home in on",
        ],
    );
    m.insert(
        "ui.clear-selection",
        vec![
            // Direct commands
            "clear selection",
            "deselect",
            "unselect",
            "clear",
            "reset selection",
            "nothing selected",
            "select nothing",
            "deselect all",
            "clear all",
            // UK/US colloquialisms
            "never mind",
            "forget that",
            "cancel selection",
            "unmark",
        ],
    );

    // --------------------------------------------------------------------------
    // HIERARCHY NAVIGATION - Drill up/down
    // --------------------------------------------------------------------------
    m.insert(
        "ui.drill-down",
        vec![
            // Blade Runner Esper style
            "drill down",
            "go deeper",
            "expand",
            "open",
            "show children",
            "show details",
            "more detail",
            "what's inside",
            "what's underneath",
            "explore",
            "dig deeper",
            "dive in",
            "go into",
            "enter",
            // Hierarchy navigation
            "show subsidiaries",
            "show sub-funds",
            "show share classes",
            "expand node",
            "open node",
            "drill into",
            // UK/US colloquialisms
            "let's go deeper",
            "take me deeper",
            "what's in there",
            "open that up",
            "crack it open",
            "peel back",
        ],
    );
    m.insert(
        "ui.drill-up",
        vec![
            // Blade Runner Esper style
            "drill up",
            "go up",
            "collapse",
            "close",
            "show parent",
            "parent level",
            "up one level",
            "back up",
            "go back",
            "ascend",
            "rise",
            // Hierarchy navigation
            "show owners",
            "show umbrella",
            "show parent fund",
            "collapse node",
            "close node",
            // UK/US colloquialisms
            "take me up",
            "back out",
            "up a level",
            "step back",
            "one level up",
        ],
    );
    m.insert(
        "ui.expand-all",
        vec![
            // Direct commands
            "expand all",
            "show all",
            "open all",
            "unfold all",
            "full expansion",
            "expand everything",
            "open everything",
            // UK/US colloquialisms
            "blow it all open",
            "show me everything",
            "all the detail",
            "complete expansion",
        ],
    );
    m.insert(
        "ui.collapse-all",
        vec![
            // Direct commands
            "collapse all",
            "close all",
            "fold all",
            "hide all",
            "collapse everything",
            "close everything",
            "reset expansion",
            // UK/US colloquialisms
            "close it all up",
            "fold it up",
            "pack it up",
            "minimize all",
        ],
    );

    // --------------------------------------------------------------------------
    // GRAPH LAYER CONTROLS
    // --------------------------------------------------------------------------
    m.insert(
        "ui.show-layer",
        vec![
            // Direct commands
            "show layer",
            "enable layer",
            "turn on layer",
            "display layer",
            "add layer",
            "include layer",
            // Specific layers
            "show ownership layer",
            "show control layer",
            "show kyc layer",
            "show services layer",
            "show custody layer",
            "show trading layer",
            "show ubo layer",
            // UK/US colloquialisms
            "bring in layer",
            "layer on",
            "add that layer",
            "include that",
        ],
    );
    m.insert(
        "ui.hide-layer",
        vec![
            // Direct commands
            "hide layer",
            "disable layer",
            "turn off layer",
            "remove layer",
            "exclude layer",
            // Specific layers
            "hide ownership layer",
            "hide control layer",
            "hide kyc layer",
            "hide services layer",
            // UK/US colloquialisms
            "take off layer",
            "layer off",
            "remove that layer",
            "exclude that",
            "get rid of layer",
        ],
    );

    // --------------------------------------------------------------------------
    // FILTERING
    // --------------------------------------------------------------------------
    m.insert(
        "ui.filter",
        vec![
            // Direct commands
            "filter",
            "filter by",
            "show only",
            "just show",
            "only show",
            "limit to",
            "restrict to",
            // Type filtering
            "filter by type",
            "show only funds",
            "show only persons",
            "show only companies",
            "just entities",
            "only ubos",
            "only directors",
            // Status filtering
            "show active only",
            "hide inactive",
            "only verified",
            "only pending",
            // UK/US colloquialisms
            "narrow down",
            "focus on just",
            "just the",
            "gimme just",
            "only want",
        ],
    );
    m.insert(
        "ui.clear-filter",
        vec![
            // Direct commands
            "clear filter",
            "remove filter",
            "reset filter",
            "show all",
            "no filter",
            "unfilter",
            "clear filters",
            "remove all filters",
            // UK/US colloquialisms
            "stop filtering",
            "show everything",
            "back to normal",
            "reset view",
        ],
    );

    // --------------------------------------------------------------------------
    // EXPORT AND CAPTURE - "Give me a hard copy"
    // --------------------------------------------------------------------------
    m.insert(
        "ui.export",
        vec![
            // Blade Runner Esper commands
            "give me a hard copy",
            "hard copy",
            "print",
            "print that",
            "export",
            "download",
            "save",
            "capture",
            "screenshot",
            "snapshot",
            // Format specific
            "export to pdf",
            "download pdf",
            "save as png",
            "export image",
            "save image",
            // UK/US colloquialisms
            "gimme a copy",
            "print it out",
            "save that",
            "grab that",
            "capture that",
            "get me a copy",
        ],
    );

    // --------------------------------------------------------------------------
    // UNDO/REDO AND HISTORY
    // --------------------------------------------------------------------------
    m.insert(
        "ui.undo",
        vec![
            // Direct commands
            "undo",
            "go back",
            "back",
            "previous",
            "reverse",
            "undo that",
            "undo last",
            // UK/US colloquialisms
            "take that back",
            "never mind",
            "oops",
            "scratch that",
            "forget that",
            "step back",
        ],
    );
    m.insert(
        "ui.redo",
        vec![
            // Direct commands
            "redo",
            "go forward",
            "forward",
            "next",
            "redo that",
            "redo last",
            // UK/US colloquialisms
            "do that again",
            "bring it back",
            "repeat",
        ],
    );

    // --------------------------------------------------------------------------
    // LAYOUT CONTROLS
    // --------------------------------------------------------------------------
    m.insert(
        "ui.layout-vertical",
        vec![
            // Direct commands
            "vertical layout",
            "top to bottom",
            "hierarchical",
            "tree layout",
            "org chart layout",
            "switch to vertical",
            // UK/US colloquialisms
            "make it vertical",
            "stack it",
            "top down",
        ],
    );
    m.insert(
        "ui.layout-horizontal",
        vec![
            // Direct commands
            "horizontal layout",
            "left to right",
            "flow layout",
            "switch to horizontal",
            // UK/US colloquialisms
            "make it horizontal",
            "side by side",
            "left right",
        ],
    );
    m.insert(
        "ui.layout-radial",
        vec![
            // Direct commands
            "radial layout",
            "circular layout",
            "around center",
            "hub and spoke",
            "switch to radial",
            // UK/US colloquialisms
            "make it circular",
            "spread around",
            "fan out",
        ],
    );
    m.insert(
        "ui.layout-force",
        vec![
            // Direct commands
            "force layout",
            "force directed",
            "physics layout",
            "organic layout",
            "natural layout",
            "auto layout",
            "switch to force",
            // UK/US colloquialisms
            "let it settle",
            "shake it out",
            "let it flow",
        ],
    );

    // --------------------------------------------------------------------------
    // ANIMATION CONTROLS
    // --------------------------------------------------------------------------
    m.insert(
        "ui.animate",
        vec![
            // Direct commands
            "animate",
            "play",
            "run animation",
            "show timeline",
            "play history",
            "animate changes",
            // UK/US colloquialisms
            "show me how it evolved",
            "play it through",
            "run it",
        ],
    );
    m.insert(
        "ui.pause-animation",
        vec![
            // Direct commands
            "pause",
            "pause animation",
            "stop animation",
            "freeze",
            "hold",
            // UK/US colloquialisms
            "stop there",
            "wait",
            "hold it",
        ],
    );

    // --------------------------------------------------------------------------
    // SEARCH WITHIN VIEW
    // --------------------------------------------------------------------------
    m.insert(
        "ui.search",
        vec![
            // Direct commands
            "search",
            "find",
            "look for",
            "search for",
            "find in view",
            "locate",
            "where is",
            // UK/US colloquialisms
            "looking for",
            "trying to find",
            "need to find",
            "hunt for",
            "track down",
        ],
    );

    // --------------------------------------------------------------------------
    // COMPARISON AND DIFF
    // --------------------------------------------------------------------------
    m.insert(
        "ui.compare",
        vec![
            // Direct commands
            "compare",
            "compare with",
            "show diff",
            "show difference",
            "show changes",
            "what changed",
            "side by side",
            "diff",
            // UK/US colloquialisms
            "spot the difference",
            "what's different",
            "show me what changed",
            "before and after",
        ],
    );

    // --------------------------------------------------------------------------
    // HELP AND INFO
    // --------------------------------------------------------------------------
    m.insert(
        "ui.help",
        vec![
            // Direct commands
            "help",
            "how do i",
            "what can i do",
            "show help",
            "show commands",
            "available commands",
            "what commands",
            // UK/US colloquialisms
            "i'm stuck",
            "what now",
            "options",
            "what can i say",
        ],
    );

    // ==========================================================================
    // 3D ESPER NAVIGATION - SCALE (Astronomical metaphor)
    // Like zooming through a solar system: Universe → Galaxy → System → Planet → Surface → Core
    // ==========================================================================
    m.insert(
        "ui.scale-universe",
        vec![
            // Direct commands
            "universe view",
            "show universe",
            "all portfolios",
            "everything",
            "full picture",
            "god view",
            "bird's eye",
            "zoom out max",
            // UK/US colloquialisms
            "show me everything",
            "the big picture",
            "thirty thousand feet",
            "satellite view",
        ],
    );
    m.insert(
        "ui.scale-galaxy",
        vec![
            // Direct commands
            "galaxy view",
            "show galaxy",
            "fund family",
            "portfolio group",
            "umbrella level",
            // UK/US colloquialisms
            "cluster view",
            "group level",
            "the constellation",
        ],
    );
    m.insert(
        "ui.scale-system",
        vec![
            // Direct commands
            "system view",
            "solar system",
            "single fund",
            "fund level",
            "this fund",
            // UK/US colloquialisms
            "just this one",
            "focus here",
            "this orbit",
        ],
    );
    m.insert(
        "ui.scale-planet",
        vec![
            // Direct commands
            "planet view",
            "entity level",
            "single entity",
            "focus entity",
            "this entity",
            // UK/US colloquialisms
            "zoom to this",
            "land here",
            "touch down",
        ],
    );
    m.insert(
        "ui.scale-surface",
        vec![
            // Direct commands
            "surface view",
            "entity details",
            "show details",
            "attributes",
            "properties",
            // UK/US colloquialisms
            "what's on the surface",
            "the details",
            "closer look",
        ],
    );
    m.insert(
        "ui.scale-core",
        vec![
            // Direct commands
            "core view",
            "deep details",
            "raw data",
            "source data",
            "the core",
            "innermost",
            // UK/US colloquialisms
            "the guts",
            "deep inside",
            "the heart of it",
        ],
    );

    // ==========================================================================
    // 3D ESPER NAVIGATION - DEPTH (Layer penetration)
    // Like peeling an onion or x-ray vision through layers
    // ==========================================================================
    m.insert(
        "ui.drill-through",
        vec![
            // Direct commands
            "drill through",
            "punch through",
            "go through",
            "pierce",
            "penetrate layer",
            // UK/US colloquialisms
            "cut through",
            "blast through",
            "straight through",
        ],
    );
    m.insert(
        "ui.x-ray",
        vec![
            // Direct commands - Blade Runner forensic theme
            "x-ray",
            "x ray",
            "xray",
            "show internal",
            "see through",
            "transparency",
            "reveal structure",
            // UK/US colloquialisms
            "look inside",
            "see the bones",
            "skeleton view",
            "what's underneath",
        ],
    );
    m.insert(
        "ui.peel",
        vec![
            // Direct commands
            "peel",
            "peel back",
            "peel layer",
            "unwrap",
            "reveal layer",
            "strip away",
            // UK/US colloquialisms
            "peel the onion",
            "take off a layer",
            "one layer down",
        ],
    );
    m.insert(
        "ui.cross-section",
        vec![
            // Direct commands
            "cross section",
            "cross-section",
            "slice view",
            "cutaway",
            "cut away",
            "section view",
            // UK/US colloquialisms
            "slice it open",
            "show the layers",
            "cake slice view",
        ],
    );

    // ==========================================================================
    // 3D ESPER NAVIGATION - ORBITAL (Rotation around subject)
    // Like orbiting a planet to see all sides
    // ==========================================================================
    m.insert(
        "ui.orbit",
        vec![
            // Direct commands
            "orbit",
            "orbit around",
            "circle around",
            "go around",
            "rotate view",
            "spin view",
            // UK/US colloquialisms
            "walk around it",
            "see all sides",
            "three sixty",
            "360 view",
        ],
    );
    m.insert(
        "ui.rotate-layer",
        vec![
            // Direct commands
            "rotate layer",
            "spin layer",
            "turn layer",
            "twist layer",
            // UK/US colloquialisms
            "turn it around",
            "flip the layer",
            "spin this level",
        ],
    );
    m.insert(
        "ui.flip",
        vec![
            // Direct commands
            "flip",
            "flip view",
            "flip over",
            "turn over",
            "reverse side",
            "other side",
            // UK/US colloquialisms
            "flip it",
            "show the back",
            "what's behind",
        ],
    );
    m.insert(
        "ui.tilt",
        vec![
            // Direct commands
            "tilt",
            "tilt view",
            "angle view",
            "perspective",
            "oblique",
            // UK/US colloquialisms
            "cock it",
            "lean it",
            "skew view",
        ],
    );

    // ==========================================================================
    // 3D ESPER NAVIGATION - TEMPORAL (Time dimension)
    // Like a time machine through entity history
    // ==========================================================================
    m.insert(
        "ui.rewind",
        vec![
            // Direct commands
            "rewind",
            "go back",
            "back in time",
            "previous state",
            "earlier",
            "before",
            // UK/US colloquialisms
            "take me back",
            "what was it before",
            "roll back",
            "wind back",
        ],
    );
    m.insert(
        "ui.time-play",
        vec![
            // Direct commands
            "play time",
            "animate time",
            "show evolution",
            "time lapse",
            "history playback",
            // UK/US colloquialisms
            "play it through time",
            "show me how it changed",
            "run the history",
        ],
    );
    m.insert(
        "ui.time-freeze",
        vec![
            // Direct commands
            "freeze time",
            "stop time",
            "snapshot",
            "freeze frame",
            "hold this moment",
            "point in time",
            // UK/US colloquialisms
            "freeze it there",
            "stop right there",
            "hold it",
        ],
    );
    m.insert(
        "ui.time-slice",
        vec![
            // Direct commands
            "time slice",
            "at this date",
            "as of",
            "point in time view",
            "historical view",
            // UK/US colloquialisms
            "show me as of",
            "what did it look like on",
            "back then",
        ],
    );
    m.insert(
        "ui.time-trail",
        vec![
            // Direct commands
            "time trail",
            "show history",
            "audit trail",
            "change log",
            "evolution",
            "timeline",
            // UK/US colloquialisms
            "show me the journey",
            "how did we get here",
            "the path taken",
        ],
    );

    // ==========================================================================
    // MATRIX-THEMED INVESTIGATION (Hidden truth discovery)
    // "Follow the white rabbit" = trace to terminus
    // "Dive into" = explore structure
    // ==========================================================================
    m.insert(
        "ui.follow-the-rabbit",
        vec![
            // Matrix-themed: Trace ownership chain to terminus (find the humans)
            "follow the rabbit",
            "follow the white rabbit",
            "white rabbit",
            "rabbit hole",
            "down the rabbit hole",
            "how deep does this go",
            "how far down",
            "trace to terminus",
            "find the humans",
            // Legacy support
            "follow the money",
            "trace funds",
            "money trail",
            // UK/US colloquialisms
            "who's really behind this",
            "the real owners",
            "ultimate beneficiaries",
        ],
    );
    m.insert(
        "ui.dive-into",
        vec![
            // Exploration-focused: Examine entity structure
            "dive into",
            "dive in",
            "deep dive",
            "go deep",
            "dig into",
            "explore this",
            "examine closely",
            "investigate this",
            // UK/US colloquialisms
            "let's look closer",
            "unpack this",
            "break this down",
            "what's inside",
        ],
    );
    m.insert(
        "ui.who-controls",
        vec![
            // Control structure investigation
            "who controls",
            "who runs",
            "who's in charge",
            "control structure",
            "governance",
            "decision makers",
            // UK/US colloquialisms
            "who calls the shots",
            "who's pulling the strings",
            "the puppet masters",
        ],
    );

    // ==========================================================================
    // BLADE RUNNER FORENSIC COMMANDS (Enhancement/analysis)
    // ==========================================================================
    m.insert(
        "ui.illuminate",
        vec![
            // Highlight connections/patterns
            "illuminate",
            "light up",
            "highlight",
            "show connections",
            "reveal links",
            // UK/US colloquialisms
            "make it glow",
            "light it up",
            "show me the web",
        ],
    );
    m.insert(
        "ui.shadow",
        vec![
            // Dim non-essential to focus
            "shadow",
            "dim",
            "fade others",
            "isolate",
            "focus only",
            // UK/US colloquialisms
            "grey out the rest",
            "just show this",
            "hide the noise",
        ],
    );
    m.insert(
        "ui.red-flag-scan",
        vec![
            // Scan for risk indicators
            "red flag scan",
            "scan for risk",
            "show risks",
            "find problems",
            "risk indicators",
            "warning signs",
            // UK/US colloquialisms
            "what's wrong here",
            "any red flags",
            "spot the issues",
        ],
    );
    m.insert(
        "ui.black-hole",
        vec![
            // Identify information gaps
            "black hole",
            "find gaps",
            "missing data",
            "information void",
            "what's missing",
            "incomplete",
            // UK/US colloquialisms
            "where are the holes",
            "what don't we know",
            "blind spots",
        ],
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
    // GLEIF ENRICHMENT PHASE
    // ==========================================================================
    m.insert("gleif.enrich", "gleif_enrichment");
    m.insert("gleif.import-tree", "gleif_enrichment");
    m.insert("gleif.refresh", "gleif_enrichment");

    // ==========================================================================
    // BODS UBO DISCOVERY PHASE
    // ==========================================================================
    m.insert("bods.discover-ubos", "bods_discovery");
    m.insert("bods.import-ownership", "bods_discovery");
    m.insert("bods.refresh", "bods_discovery");

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
    m.insert("request.reassign", "request_management");
    m.insert("request.bulk-create", "request_management");
    m.insert("request.link-to-document", "request_management");

    // ==========================================================================
    // CASE EVENT / AUDIT PHASE
    // ==========================================================================
    m.insert("case-event.log", "case_audit");
    m.insert("case-event.list-by-case", "case_audit");
    m.insert("case-event.list-by-entity", "case_audit");
    m.insert("case-event.list-by-user", "case_audit");
    m.insert("case-event.export", "case_audit");
    m.insert("case-event.annotate", "case_audit");

    // ==========================================================================
    // ONBOARDING AUTOMATION PHASE
    // ==========================================================================
    m.insert("onboarding.auto-complete", "onboarding_automation");
    m.insert("onboarding.validate-completeness", "onboarding_automation");
    m.insert("onboarding.generate-checklist", "onboarding_automation");
    m.insert("onboarding.estimate-timeline", "onboarding_automation");
    m.insert("onboarding.track-progress", "onboarding_automation");
    m.insert("onboarding.fast-track", "onboarding_automation");
    m.insert("onboarding.pause", "onboarding_automation");
    m.insert("onboarding.resume", "onboarding_automation");
    m.insert("onboarding.abort", "onboarding_automation");
    m.insert("onboarding.clone-template", "onboarding_automation");

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
    m.insert("holding.lock", "registry_management");
    m.insert("holding.unlock", "registry_management");
    m.insert("holding.transfer", "registry_management");
    m.insert("holding.certify", "registry_management");
    m.insert("movement.subscribe", "registry_management");
    m.insert("movement.redeem", "registry_management");
    m.insert("movement.transfer-in", "registry_management");
    m.insert("movement.transfer-out", "registry_management");
    m.insert("movement.switch", "registry_management");
    m.insert("movement.confirm", "registry_management");
    m.insert("movement.settle", "registry_management");
    m.insert("movement.cancel", "registry_management");
    m.insert("movement.fail", "registry_management");
    m.insert("movement.list-by-holding", "registry_management");
    m.insert("movement.read", "registry_management");
    m.insert("movement.amend", "registry_management");
    m.insert("movement.reverse", "registry_management");

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
    m.insert("currency.read", "reference_data");
    m.insert("currency.list", "reference_data");
    m.insert("country.read", "reference_data");
    m.insert("country.list", "reference_data");

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
            // Holding verbs
            "holding.create",
            "holding.ensure",
            "holding.update-units",
            "holding.read",
            "holding.list-by-share-class",
            "holding.list-by-investor",
            "holding.close",
            "holding.lock",
            "holding.unlock",
            "holding.transfer",
            "holding.certify",
            // Movement verbs
            "movement.subscribe",
            "movement.redeem",
            "movement.transfer-in",
            "movement.transfer-out",
            "movement.switch",
            "movement.confirm",
            "movement.settle",
            "movement.cancel",
            "movement.fail",
            "movement.list-by-holding",
            "movement.read",
            "movement.amend",
            "movement.reverse",
            // Fund investor verbs
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
            "currency.read",
            "currency.list",
            "country.read",
            "country.list",
        ],
    );

    // ==========================================================================
    // LAYER: REQUEST MANAGEMENT
    // ==========================================================================
    m.insert(
        "layer_request_management",
        vec![
            "request.create",
            "request.list",
            "request.overdue",
            "request.fulfill",
            "request.cancel",
            "request.extend",
            "request.remind",
            "request.escalate",
            "request.waive",
            "request.reassign",
            "request.bulk-create",
            "request.link-to-document",
        ],
    );

    // ==========================================================================
    // LAYER: CASE EVENTS / AUDIT
    // ==========================================================================
    m.insert(
        "layer_case_audit",
        vec![
            "case-event.log",
            "case-event.list-by-case",
            "case-event.list-by-entity",
            "case-event.list-by-user",
            "case-event.export",
            "case-event.annotate",
        ],
    );

    // ==========================================================================
    // LAYER: ONBOARDING AUTOMATION
    // ==========================================================================
    m.insert(
        "layer_onboarding_automation",
        vec![
            "onboarding.auto-complete",
            "onboarding.validate-completeness",
            "onboarding.generate-checklist",
            "onboarding.estimate-timeline",
            "onboarding.track-progress",
            "onboarding.fast-track",
            "onboarding.pause",
            "onboarding.resume",
            "onboarding.abort",
            "onboarding.clone-template",
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
        vec!["cbu.role:assign-trust-role", "entity.create-proper-person"],
    );
    m.insert(
        "entity.create-partnership-limited",
        vec!["ubo.add-ownership", "cbu.assign-role"],
    );

    // ==========================================================================
    // FUND FLOW
    // ==========================================================================
    m.insert(
        "fund.create-umbrella",
        vec!["fund.create-subfund", "cbu.role:assign-fund-role"],
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
        vec!["entity-workstream.create", "doc-request.create"],
    );

    // ==========================================================================
    // CONTROL FLOW
    // ==========================================================================
    m.insert(
        "control.add",
        vec!["entity.create-proper-person", "cbu.role:assign-control"],
    );

    // ==========================================================================
    // GLEIF ENRICHMENT FLOW
    // ==========================================================================
    m.insert(
        "gleif.enrich",
        vec![
            "gleif.import-tree",
            "ubo.add-ownership",
            "entity-settlement.set-identity",
        ],
    );
    m.insert(
        "gleif.import-tree",
        vec!["bods.discover-ubos", "ubo.calculate", "ubo.trace-chains"],
    );
    m.insert("gleif.refresh", vec!["gleif.import-tree", "ubo.calculate"]);

    // ==========================================================================
    // BODS UBO DISCOVERY FLOW
    // ==========================================================================
    m.insert(
        "bods.discover-ubos",
        vec![
            "bods.import-ownership",
            "ubo.register-ubo",
            "entity.create-proper-person",
        ],
    );
    m.insert(
        "bods.import-ownership",
        vec![
            "ubo.register-ubo",
            "ubo.calculate",
            "entity-workstream.create",
        ],
    );
    m.insert("bods.refresh", vec!["bods.discover-ubos", "ubo.calculate"]);

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
        vec!["delegation.add", "investment-manager.assign"],
    );
    m.insert(
        "cbu.role:assign-signatory",
        vec!["doc-request.create", "entity-workstream.create"],
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
        vec!["entity-workstream.create", "kyc-case.update-status"],
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
        vec!["kyc-case.close", "kyc-case.escalate"],
    );
    m.insert(
        "kyc-case.close",
        vec!["cbu.decide", "trading-profile.activate"],
    );
    m.insert(
        "kyc-case.escalate",
        vec!["verify.escalate", "red-flag.raise"],
    );
    m.insert(
        "kyc-case.reopen",
        vec!["entity-workstream.create", "kyc-case.assign"],
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
        vec!["entity-workstream.create", "kyc-case.update-status"],
    );
    m.insert(
        "entity-workstream.set-enhanced-dd",
        vec!["doc-request.create", "verify.challenge"],
    );
    m.insert(
        "entity-workstream.block",
        vec!["red-flag.raise", "verify.escalate"],
    );

    // ==========================================================================
    // DOCUMENT REQUEST FLOW
    // ==========================================================================
    m.insert(
        "doc-request.create",
        vec!["doc-request.mark-requested", "request.create"],
    );
    m.insert(
        "doc-request.mark-requested",
        vec!["doc-request.receive", "request.remind"],
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
        vec!["entity-workstream.update-status", "allegation.verify"],
    );
    m.insert(
        "doc-request.reject",
        vec!["doc-request.create", "request.create"],
    );

    // ==========================================================================
    // SCREENING FLOW
    // ==========================================================================
    m.insert("case-screening.run", vec!["case-screening.complete"]);
    m.insert(
        "case-screening.complete",
        vec![
            "case-screening.review-hit",
            "entity-workstream.update-status",
        ],
    );
    m.insert(
        "case-screening.review-hit",
        vec!["red-flag.raise", "entity-workstream.update-status"],
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
        vec!["entity-workstream.update-status", "kyc-case.update-status"],
    );
    m.insert(
        "red-flag.set-blocking",
        vec!["entity-workstream.block", "kyc-case.escalate"],
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
        vec!["observation.reconcile", "allegation.verify"],
    );
    m.insert(
        "observation.reconcile",
        vec!["discrepancy.record", "allegation.verify"],
    );

    // ==========================================================================
    // ALLEGATION FLOW
    // ==========================================================================
    m.insert(
        "allegation.record",
        vec!["doc-request.create", "allegation.verify"],
    );
    m.insert("allegation.verify", vec!["entity-workstream.update-status"]);
    m.insert(
        "allegation.contradict",
        vec!["verify.challenge", "red-flag.raise"],
    );

    // ==========================================================================
    // DISCREPANCY FLOW
    // ==========================================================================
    m.insert(
        "discrepancy.record",
        vec!["discrepancy.resolve", "verify.challenge"],
    );
    m.insert(
        "discrepancy.resolve",
        vec!["observation.supersede", "entity-workstream.update-status"],
    );
    m.insert("discrepancy.escalate", vec!["verify.escalate"]);

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
        vec!["verify.challenge", "verify.escalate"],
    );
    m.insert("verify.challenge", vec!["verify.respond-to-challenge"]);
    m.insert(
        "verify.respond-to-challenge",
        vec!["verify.resolve-challenge"],
    );
    m.insert(
        "verify.resolve-challenge",
        vec!["entity-workstream.update-status", "red-flag.raise"],
    );
    m.insert("verify.escalate", vec!["verify.resolve-escalation"]);
    m.insert(
        "verify.resolve-escalation",
        vec!["kyc-case.update-status", "kyc-case.close"],
    );
    m.insert(
        "verify.calculate-confidence",
        vec!["verify.assert", "entity-workstream.update-status"],
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
        vec!["verify.challenge", "red-flag.raise"],
    );
    m.insert(
        "verify.resolve-pattern",
        vec!["entity-workstream.update-status"],
    );

    // ==========================================================================
    // REQUEST FLOW
    // ==========================================================================
    m.insert("request.create", vec!["request.remind"]);
    m.insert(
        "request.fulfill",
        vec!["doc-request.receive", "entity-workstream.update-status"],
    );
    m.insert("request.escalate", vec!["red-flag.raise"]);

    // ==========================================================================
    // TRADING SETUP FLOW
    // ==========================================================================
    m.insert(
        "cbu-custody.add-universe",
        vec!["cbu-custody.create-ssi", "cbu-custody.add-booking-rule"],
    );
    m.insert(
        "cbu-custody.create-ssi",
        vec!["cbu-custody.activate-ssi", "cbu-custody.add-booking-rule"],
    );
    m.insert(
        "cbu-custody.ensure-ssi",
        vec!["cbu-custody.activate-ssi", "cbu-custody.add-booking-rule"],
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
        vec!["cbu-custody.validate-booking-coverage"],
    );
    m.insert(
        "cbu-custody.derive-required-coverage",
        vec!["cbu-custody.create-ssi", "cbu-custody.add-booking-rule"],
    );
    m.insert(
        "cbu-custody.validate-booking-coverage",
        vec!["trading-profile.validate-matrix-completeness"],
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
        vec!["cbu-custody.set-settlement-cycle"],
    );
    m.insert(
        "entity-settlement.set-identity",
        vec!["entity-settlement.add-ssi"],
    );

    // ==========================================================================
    // INSTRUCTION PROFILE FLOW
    // ==========================================================================
    m.insert(
        "instruction-profile.define-message-type",
        vec!["instruction-profile.create-template"],
    );
    m.insert(
        "instruction-profile.create-template",
        vec!["instruction-profile.assign-template"],
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
        vec!["instruction-profile.validate-profile"],
    );
    m.insert(
        "instruction-profile.validate-profile",
        vec!["instruction-profile.derive-required-templates"],
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
        vec!["trade-gateway.enable-gateway"],
    );
    m.insert(
        "trade-gateway.enable-gateway",
        vec!["trade-gateway.activate-gateway"],
    );
    m.insert(
        "trade-gateway.activate-gateway",
        vec!["trade-gateway.add-routing-rule"],
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
        vec!["trade-gateway.validate-routing"],
    );
    m.insert(
        "trade-gateway.validate-routing",
        vec!["trade-gateway.derive-required-routes"],
    );
    m.insert(
        "trade-gateway.derive-required-routes",
        vec!["trade-gateway.add-routing-rule"],
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
        vec!["pricing-config.set-valuation-schedule"],
    );
    m.insert(
        "pricing-config.set-valuation-schedule",
        vec!["pricing-config.set-fallback-chain"],
    );
    m.insert(
        "pricing-config.set-fallback-chain",
        vec!["pricing-config.set-stale-policy"],
    );
    m.insert(
        "pricing-config.set-stale-policy",
        vec!["pricing-config.set-nav-threshold"],
    );
    m.insert(
        "pricing-config.set-nav-threshold",
        vec!["pricing-config.validate-pricing-config"],
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
        vec!["corporate-action.link-ca-ssi"],
    );
    m.insert(
        "corporate-action.link-ca-ssi",
        vec!["corporate-action.validate-ca-config"],
    );
    m.insert(
        "corporate-action.validate-ca-config",
        vec!["corporate-action.derive-required-config"],
    );
    m.insert(
        "corporate-action.derive-required-config",
        vec!["corporate-action.set-preferences"],
    );

    // ==========================================================================
    // TAX FLOW
    // ==========================================================================
    m.insert(
        "tax-config.set-withholding-profile",
        vec!["tax-config.set-reclaim-preferences"],
    );
    m.insert(
        "tax-config.set-reclaim-preferences",
        vec!["tax-config.link-tax-documentation"],
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
        vec!["tax-config.validate-tax-config"],
    );
    m.insert(
        "tax-config.validate-tax-config",
        vec!["tax-config.find-withholding-rate"],
    );

    // ==========================================================================
    // TRADING PROFILE FLOW
    // ==========================================================================
    m.insert("trading-profile.import", vec!["trading-profile.validate"]);
    m.insert("trading-profile.validate", vec!["trading-profile.activate"]);
    m.insert(
        "trading-profile.activate",
        vec!["trading-profile.materialize"],
    );
    m.insert(
        "trading-profile.materialize",
        vec!["trading-profile.validate-matrix-completeness"],
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
    m.insert("lifecycle.discover", vec!["lifecycle.provision"]);
    m.insert("lifecycle.provision", vec!["lifecycle.activate"]);
    m.insert("lifecycle.activate", vec!["lifecycle.analyze-gaps"]);
    m.insert(
        "lifecycle.analyze-gaps",
        vec!["lifecycle.check-readiness", "lifecycle.generate-plan"],
    );
    m.insert("lifecycle.check-readiness", vec!["lifecycle.generate-plan"]);
    m.insert("lifecycle.generate-plan", vec!["lifecycle.provision"]);

    // ==========================================================================
    // ISDA FLOW
    // ==========================================================================
    m.insert("isda.create", vec!["isda.add-coverage"]);
    m.insert("isda.add-coverage", vec!["isda.add-csa"]);
    m.insert(
        "isda.add-csa",
        vec!["trading-profile.validate-matrix-completeness"],
    );

    // ==========================================================================
    // MATRIX OVERLAY FLOW
    // ==========================================================================
    m.insert("matrix-overlay.subscribe", vec!["matrix-overlay.add"]);
    m.insert(
        "matrix-overlay.add",
        vec!["matrix-overlay.effective-matrix"],
    );
    m.insert(
        "matrix-overlay.effective-matrix",
        vec!["matrix-overlay.unified-gaps"],
    );
    m.insert(
        "matrix-overlay.unified-gaps",
        vec!["matrix-overlay.compare-products"],
    );

    // ==========================================================================
    // TEAM FLOW
    // ==========================================================================
    m.insert(
        "team.create",
        vec!["team.add-member", "team.add-cbu-access"],
    );
    m.insert(
        "team.add-member",
        vec!["team.grant-service", "team.add-cbu-access"],
    );
    m.insert("team.add-cbu-access", vec!["team.grant-service"]);

    // ==========================================================================
    // USER FLOW
    // ==========================================================================
    m.insert("user.create", vec!["team.add-member"]);
    m.insert("user.offboard", vec!["team.remove-member"]);

    // ==========================================================================
    // SLA FLOW
    // ==========================================================================
    m.insert(
        "sla.commit",
        vec!["sla.bind-to-profile", "sla.bind-to-service"],
    );
    m.insert("sla.report-breach", vec!["sla.update-remediation"]);
    m.insert(
        "sla.update-remediation",
        vec!["sla.resolve-breach", "sla.escalate-breach"],
    );

    // ==========================================================================
    // REGULATORY FLOW
    // ==========================================================================
    m.insert(
        "regulatory.registration.add",
        vec!["regulatory.registration.verify"],
    );
    m.insert(
        "regulatory.registration.verify",
        vec!["regulatory.status.check"],
    );

    // ==========================================================================
    // CASH SWEEP FLOW
    // ==========================================================================
    m.insert("cash-sweep.configure", vec!["cash-sweep.link-resource"]);

    // ==========================================================================
    // INVESTMENT MANAGER FLOW
    // ==========================================================================
    m.insert(
        "investment-manager.assign",
        vec!["investment-manager.set-scope"],
    );
    m.insert(
        "investment-manager.set-scope",
        vec!["investment-manager.link-connectivity"],
    );

    // ==========================================================================
    // DELEGATION FLOW
    // ==========================================================================
    m.insert("delegation.add", vec!["cbu.role:assign-fund-role"]);

    // ==========================================================================
    // SERVICE RESOURCE FLOW
    // ==========================================================================
    m.insert(
        "service-resource.provision",
        vec!["service-resource.set-attr"],
    );
    m.insert(
        "service-resource.set-attr",
        vec!["service-resource.validate-attrs"],
    );
    m.insert(
        "service-resource.validate-attrs",
        vec!["service-resource.activate"],
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
        vec!["client.collection-response"],
    );
    m.insert(
        "client.collection-response",
        vec!["client.collection-confirm"],
    );
    m.insert("client.collection-confirm", vec!["client.get-outstanding"]);

    // ==========================================================================
    // FUND INVESTOR FLOW
    // ==========================================================================
    m.insert(
        "fund-investor.create",
        vec!["holding.create", "fund-investor.update-kyc-status"],
    );

    // ==========================================================================
    // HOLDING FLOW
    // ==========================================================================
    m.insert("holding.create", vec!["movement.subscribe"]);
    m.insert("movement.subscribe", vec!["movement.confirm"]);
    m.insert("movement.redeem", vec!["movement.confirm"]);
    m.insert("movement.confirm", vec!["movement.settle"]);
    m.insert("movement.settle", vec!["holding.update-units"]);

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
        vec!["semantic.next-actions", "semantic.missing-entities"],
    );
    m.insert("semantic.next-actions", vec!["semantic.missing-entities"]);

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
        assert!(next
            .get("cbu.create")
            .unwrap()
            .contains(&"entity.create-limited-company"));
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
