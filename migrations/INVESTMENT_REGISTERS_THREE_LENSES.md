# Investment Registers — Three Lenses on Shared Capital

**Version:** 1.0  
**Date:** 2026-02-11  
**Status:** For Peer Review  
**Audience:** Engineering, Product, Domain Architects, Legal & Compliance  
> **Mermaid diagrams** render in GitHub, VS Code, and any CommonMark renderer with mermaid support.

---

## 1. Purpose

This paper describes the **three investment register types** in our system, how they share the same capital structure vocabulary (`share_classes`, `holdings`, `issuance_events`) but answer fundamentally different questions, and how the ownership graph flows between them.

**Key thesis:** A single share register underpins three distinct analytical lenses. The same holding of 1,000 shares simultaneously represents a voting right (control register), an economic entitlement (investor register), and an underlying exposure (fund-of-fund register). The registers are not separate databases — they are **projections** of the same capital structure, differentiated by the question being asked.

---

## 2. The Three Registers

```mermaid
graph TB
    subgraph CAPITAL["Shared Capital Structure"]
        SC["share_classes<br/><i>instrument_kind, voting_rights_per_share,<br/>economic_per_unit, liquidation_rank</i>"]
        H["holdings<br/><i>units, investor_entity_id,<br/>usage_type: TA | UBO</i>"]
        IE["issuance_events<br/><i>Append-only supply ledger</i>"]
        SCS["share_class_supply<br/><i>Materialized supply state</i>"]
    end

    subgraph R1["Register 1: Control / Voting"]
        CR["Who controls?<br/><i>Voting rights → UBO discovery</i>"]
    end

    subgraph R2["Register 2: Economic / Investor"]
        IR["Who benefits?<br/><i>Economic rights → NAV allocation</i>"]
    end

    subgraph R3["Register 3: Fund-of-Fund / Look-Through"]
        FF["What's the underlying exposure?<br/><i>Multi-layer fund chains → asset aggregation</i>"]
    end

    SC --> CR
    SC --> IR
    SC --> FF
    H --> CR
    H --> IR
    H --> FF

    style CAPITAL fill:#e8f0fe,stroke:#4a90d9
    style R1 fill:#fde8e8,stroke:#d0021b
    style R2 fill:#e8f4e8,stroke:#50b848
    style R3 fill:#fef3e8,stroke:#f5a623
```

| Register | Question | Input | Output | Regulatory Driver |
|----------|----------|-------|--------|-------------------|
| **Control/Voting** | Who controls this entity? | Voting rights per share, board appointments, special instruments | UBO chain, PSC register, board controller | 4AMLD/5AMLD, UK PSC, BODS |
| **Economic/Investor** | Who has economic interest? | Economic rights per unit, NAV per share, distributions | Investor register, tax reporting, FATCA/CRS | MiFID II, AIFMD, UCITS, FATCA/CRS |
| **Fund-of-Fund** | What's the actual underlying exposure? | Holdings in other funds, look-through chains | Aggregated asset exposure, concentration risk | UCITS look-through, Solvency II, AIFMD Art. 7 |

---

## 3. The Shared Vocabulary: Share Classes

Share classes are the atomic unit that all three registers operate on. A single share class carries **both** voting rights and economic rights — the register lens determines which dimension matters.

### 3.1 Share Class Taxonomy

```mermaid
graph TD
    ROOT["Share Class<br/>(kyc.share_classes)"]

    ROOT --> IK["instrument_kind"]
    IK --> IK1["ORDINARY_EQUITY<br/><i>Standard voting + economic</i>"]
    IK --> IK2["PREFERENCE_EQUITY<br/><i>Senior economics, often non-voting</i>"]
    IK --> IK3["DEFERRED_EQUITY<br/><i>Subordinated, last to receive</i>"]
    IK --> IK4["FUND_UNIT / FUND_SHARE<br/><i>Open-ended fund participation</i>"]
    IK --> IK5["LP_INTEREST / GP_INTEREST<br/><i>Partnership structures</i>"]
    IK --> IK6["CONVERTIBLE / WARRANT<br/><i>Dilutive instruments</i>"]

    ROOT --> VR["Voting Dimension"]
    VR --> VR1["votes_per_unit<br/><i>default 1.0</i>"]
    VR --> VR2["voting_cap_pct<br/><i>max voting %, e.g. 10%</i>"]
    VR --> VR3["voting_threshold_pct<br/><i>min holding to vote</i>"]

    ROOT --> ER["Economic Dimension"]
    ER --> ER1["economic_per_unit<br/><i>default 1.0</i>"]
    ER --> ER2["dividend_rate"]
    ER --> ER3["liquidation_preference"]
    ER --> ER4["liquidation_rank<br/><i>lower = more senior</i>"]

    style ROOT fill:#2d6da4,color:#fff
    style IK fill:#4a90d9,color:#fff
    style VR fill:#d0021b,color:#fff
    style ER fill:#50b848,color:#fff
```

### 3.2 Key Insight: Voting vs Economic Divergence

The critical design insight is that **voting power and economic interest can diverge**:

| Scenario | Voting Rights | Economic Rights | Example |
|----------|---------------|-----------------|---------|
| Ordinary shares | 1 vote per share | 1x economic per share | Standard equity |
| Preference shares | 0 votes | 1.5x liquidation preference | Non-voting prefs |
| Dual-class (Class A) | 10 votes per share | 1x economic | Founder shares |
| Dual-class (Class B) | 1 vote per share | 1x economic | Public shares |
| LP interest | 0 votes | Full economic | Limited partners |
| GP interest | Full control | Carried interest only | General partner |
| Fund units | 0 votes (typically) | Full NAV participation | UCITS units |

This divergence is why we need **separate registers** — a 5% economic holder might have 0% voting power (preference shares), while a 2% holder might have 20% voting control (dual-class).

### 3.3 Share Class Storage

```sql
CREATE TABLE kyc.share_classes (
    id                      UUID PRIMARY KEY,
    cbu_id                  UUID REFERENCES cbus(cbu_id),        -- Which CBU issued this
    issuer_entity_id        UUID REFERENCES entities(entity_id), -- Legal issuer
    
    -- Classification
    instrument_kind         VARCHAR(30),   -- ORDINARY_EQUITY, FUND_UNIT, LP_INTEREST, ...
    instrument_type         VARCHAR(30),   -- UNITS, SHARES, LP_INTEREST
    
    -- Voting dimension (→ Control Register)
    votes_per_unit          NUMERIC(10,4) DEFAULT 1.0,
    voting_cap_pct          NUMERIC(5,2),
    voting_threshold_pct    NUMERIC(5,2),
    
    -- Economic dimension (→ Investor Register)
    economic_per_unit       NUMERIC(10,4) DEFAULT 1.0,
    dividend_rate           NUMERIC(10,4),
    liquidation_preference  DECIMAL(20,2),
    liquidation_rank        INTEGER DEFAULT 100,  -- Lower = more senior
    
    -- Supply
    authorized_shares       NUMERIC(20,6),
    issued_shares           NUMERIC(20,6),
    nav_per_share           DECIMAL(20,6),
    nav_date                DATE,
    
    -- Conversion (→ Dilution)
    converts_to_share_class_id UUID REFERENCES kyc.share_classes(id),
    conversion_ratio_num    NUMERIC(10,4),
    
    -- Fund context
    compartment_id          UUID REFERENCES kyc.fund_compartments(id),
    fund_type               VARCHAR(50),
    fund_structure          VARCHAR(50)
);
```

---

## 4. Register 1: Control / Voting — "Who Controls?"

The control register traces **voting rights and control mechanisms** through ownership chains to identify Ultimate Beneficial Owners (UBOs). It answers: "Who can direct the decisions of this entity?"

### 4.1 Architecture

```mermaid
graph TB
    subgraph INPUT["Input Sources"]
        H["holdings<br/><i>(units x votes_per_unit)</i>"]
        CE["control_edges<br/><i>HOLDS_VOTING_RIGHTS,<br/>APPOINTS_BOARD,<br/>IS_TRUSTEE, ...</i>"]
        BODS["BODS statements<br/><i>ownership_statements,<br/>person_statements</i>"]
        GLEIF["GLEIF hierarchy<br/><i>parent relationships</i>"]
        PSC["UK PSC register"]
    end

    subgraph ANALYSIS["Analysis Engine"]
        FN1["fn_holder_control_position()<br/><i>Per-holder voting % + economic %</i>"]
        FN2["control.analyze<br/><i>Routes to appropriate analyzer</i>"]
        FN3["control.identify-ubos<br/><i>Across 4 vectors</i>"]
        FN4["ubo.trace-chains<br/><i>Recursive graph traversal</i>"]
    end

    subgraph OUTPUT["Output"]
        UBO["entity_ubos<br/><i>UBO determinations</i>"]
        BC["board_controller<br/><i>Method + confidence + score</i>"]
        OS["ownership_snapshots<br/><i>Point-in-time records</i>"]
    end

    H --> FN1
    CE --> FN2
    BODS --> FN3
    GLEIF --> FN3
    PSC --> FN3
    FN1 --> OS
    FN2 --> BC
    FN3 --> UBO
    FN4 --> UBO

    style INPUT fill:#e8f0fe,stroke:#4a90d9
    style ANALYSIS fill:#fef3e8,stroke:#f5a623
    style OUTPUT fill:#fde8e8,stroke:#d0021b
```

### 4.2 Control Edge Types (Standards-Aligned)

The `control_edges` table captures 16 edge types aligned to three international standards:

```mermaid
graph LR
    subgraph OWNERSHIP["Ownership / Voting"]
        E1["HOLDS_SHARES"]
        E2["HOLDS_VOTING_RIGHTS"]
    end

    subgraph BOARD["Board Control"]
        E3["APPOINTS_BOARD"]
        E4["EXERCISES_INFLUENCE"]
        E5["IS_SENIOR_MANAGER"]
    end

    subgraph TRUST["Trust Arrangements"]
        E6["IS_SETTLOR"]
        E7["IS_TRUSTEE"]
        E8["IS_PROTECTOR"]
        E9["IS_BENEFICIARY"]
    end

    subgraph ECONOMIC["Economic Rights"]
        E10["HAS_DISSOLUTION_RIGHTS"]
        E11["HAS_PROFIT_RIGHTS"]
    end

    subgraph HIERARCHY["Corporate Hierarchy"]
        E12["CONSOLIDATED_BY"]
        E13["ULTIMATELY_CONSOLIDATED_BY"]
        E14["MANAGED_BY"]
        E15["SUBFUND_OF"]
        E16["FEEDS_INTO"]
    end

    style OWNERSHIP fill:#d0021b,color:#fff
    style BOARD fill:#f5a623,color:#fff
    style TRUST fill:#8e44ad,color:#fff
    style ECONOMIC fill:#50b848,color:#fff
    style HIERARCHY fill:#4a90d9,color:#fff
```

Each edge carries cross-references to **BODS** (`bods_interest_type`), **GLEIF** (`gleif_relationship_type`), and **UK PSC** (`psc_category`) for regulatory reporting.

### 4.3 UBO Discovery Pipeline

```mermaid
sequenceDiagram
    participant UI as User / Agent
    participant UBO as ubo.calculate
    participant GLEIF as GLEIF RR
    participant BODS as BODS Statements
    participant CE as control_edges
    participant OUT as entity_ubos

    UI->>UBO: Calculate UBOs for Entity X
    UBO->>CE: Load control edges (voting, board, trust)
    UBO->>GLEIF: Check GLEIF exceptions
    
    alt GLEIF exception = NO_KNOWN_PERSON
        UBO->>OUT: UBO type = PUBLIC_FLOAT
    else GLEIF exception = STATE_OWNED
        UBO->>OUT: UBO type = STATE_OWNED
    else Normal entity
        UBO->>UBO: Recursive graph traversal (default 25% threshold)
        loop Each ownership chain
            UBO->>BODS: Query person statements for terminal entity
            alt Natural person found
                UBO->>OUT: UBO type = NATURAL_PERSON
            else No natural person
                UBO->>UBO: Continue traversal to ultimate parent
            end
        end
    end
    
    UBO->>OUT: Store UBO determinations with confidence + chain
```

### 4.4 Board Controller Identification

Three rules determine who controls the board:

| Rule | Method | Example |
|------|--------|---------|
| **Rule A** | Board appointment rights | Shareholder agreement grants appointment of 3/5 directors |
| **Rule B** | Voting rights majority | Holder with >50% voting shares |
| **Rule C** | Special instrument | GP of LP, trustee of trust, golden share holder |

```rust
pub enum BoardControlMethod {
    BoardAppointmentRights,  // Rule A
    VotingRightsMajority,    // Rule B
    SpecialInstrument,       // Rule C
    Mixed,                   // Multiple rules apply
    NoSingleController,      // Widely held
}
```

### 4.5 Disclosure Thresholds (Configurable Per Issuer)

```mermaid
graph LR
    T1["5%<br/>Disclosure"] --> T2["10%<br/>Material"]
    T2 --> T3["25%<br/>Significant<br/>Influence"]
    T3 --> T4["50%<br/>Control"]

    style T1 fill:#4a90d9,color:#fff
    style T2 fill:#f5a623,color:#fff
    style T3 fill:#e67e22,color:#fff
    style T4 fill:#d0021b,color:#fff
```

| Threshold | Default | Meaning | UI Treatment |
|-----------|---------|---------|--------------|
| `disclosure_pct` | 5% | Individual node in visualization | Shown as named holder |
| `material_pct` | 10% | Highlighted holding | Bold border |
| `significant_pct` | 25% | Blocking minority, UBO candidate | Warning indicator |
| `control_pct` | 50% | Majority control | Control badge |

Stored in `kyc.issuer_control_config` — per-issuer overrides for jurisdictions with different thresholds (e.g., Luxembourg requires 25% for UBO, UK uses 25% for PSC, some jurisdictions use 10%).

### 4.6 Key Verbs — Control Domain

| Verb | Purpose |
|------|---------|
| `control.add` | Add control edge with type + percentage |
| `control.end` | Terminate control relationship |
| `control.analyze` | Run control analysis (routes by entity type) |
| `control.build-graph` | Full control graph with nodes + edges |
| `control.identify-ubos` | UBO identification across 4 vectors |
| `control.trace-chain` | Trace specific control chain |
| `control.reconcile-ownership` | Verify voting shares sum to 100% |
| `control.show-board-controller` | Show current board controller determination |
| `control.import-psc-register` | Import UK PSC data |
| `control.import-gleif-control` | Import GLEIF relationship data |

### 4.7 Key Verbs — UBO Domain

| Verb | Purpose |
|------|---------|
| `ubo.add-ownership` | Record ownership interest |
| `ubo.add-control` | Record control mechanism |
| `ubo.add-trust-role` | Record trust arrangement |
| `ubo.calculate` | Recursive UBO calculation (25% threshold) |
| `ubo.trace-chains` | Trace all ownership chains |
| `ubo.list-ubos` | List determined UBOs |
| `ubo.mark-terminus` | Mark entity as terminal (LISTED_COMPANY, GOVT, etc.) |
| `ubo.mark-deceased` | Mark UBO as deceased |

---

## 5. Register 2: Economic / Investor — "Who Benefits?"

The investor register tracks **economic interest holders** — who is entitled to NAV participation, distributions, and redemption rights. It answers: "Who benefits financially from this entity?"

### 5.1 Architecture

```mermaid
graph TB
    subgraph INPUT["Input Sources"]
        H["holdings<br/><i>(units x economic_per_unit)</i>"]
        INV["investors<br/><i>Full lifecycle: ENQUIRY → OFFBOARDED</i>"]
        MOV["movements<br/><i>Transaction ledger</i>"]
        PROV["Provider feeds<br/><i>Clearstream, Euroclear, CSV, API</i>"]
    end

    subgraph ANALYSIS["Analysis Engine"]
        VR["v_investor_register<br/><i>Provider-agnostic view</i>"]
        FN1["fn_holder_control_position()<br/><i>Economic % calculation</i>"]
        OS["fn_derive_ownership_snapshots()<br/><i>Register → snapshot bridge</i>"]
    end

    subgraph OUTPUT["Output"]
        REG["Investor Register<br/><i>Per-share-class position book</i>"]
        SNAP["ownership_snapshots<br/><i>basis = ECONOMIC</i>"]
        TAX["Tax reporting<br/><i>FATCA, CRS, DAC6</i>"]
    end

    H --> VR
    INV --> VR
    MOV --> VR
    PROV --> H
    VR --> REG
    FN1 --> SNAP
    OS --> SNAP
    REG --> TAX

    style INPUT fill:#e8f0fe,stroke:#4a90d9
    style ANALYSIS fill:#fef3e8,stroke:#f5a623
    style OUTPUT fill:#e8f4e8,stroke:#50b848
```

### 5.2 Investor Lifecycle State Machine

```mermaid
stateDiagram-v2
    [*] --> ENQUIRY
    ENQUIRY --> PENDING_DOCUMENTS : request-documents
    PENDING_DOCUMENTS --> KYC_IN_PROGRESS : start-kyc
    KYC_IN_PROGRESS --> KYC_APPROVED : approve-kyc
    KYC_IN_PROGRESS --> KYC_REJECTED : reject-kyc
    KYC_APPROVED --> ELIGIBLE_TO_SUBSCRIBE : mark-eligible
    ELIGIBLE_TO_SUBSCRIBE --> SUBSCRIBED : record-subscription
    SUBSCRIBED --> ACTIVE_HOLDER : activate
    ACTIVE_HOLDER --> REDEEMING : start-redemption
    REDEEMING --> OFFBOARDED : complete-redemption
    
    ACTIVE_HOLDER --> SUSPENDED : suspend
    SUSPENDED --> ACTIVE_HOLDER : reinstate
    
    KYC_REJECTED --> PENDING_DOCUMENTS : re-submit
    
    ENQUIRY --> OFFBOARDED : offboard
    ACTIVE_HOLDER --> OFFBOARDED : offboard
```

### 5.3 Dual-Purpose Holdings

The `holdings` table serves two modes via the `usage_type` discriminator:

| Mode | `usage_type` | Linked To | Purpose |
|------|-------------|-----------|---------|
| **Transfer Agency** | `TA` | `investor_id` (kyc.investors) | Client investor positions with full lifecycle |
| **UBO Tracking** | `UBO` | `investor_entity_id` only | Intra-group ownership for control analysis |

```sql
-- TA mode: full investor lifecycle, KYC tracking, tax reporting
SELECT * FROM kyc.holdings WHERE usage_type = 'TA';

-- UBO mode: ownership tracking for control register
SELECT * FROM kyc.holdings WHERE usage_type = 'UBO';
```

### 5.4 Movement Types (Transaction Ledger)

```mermaid
graph TD
    subgraph STANDARD["Standard Fund Movements"]
        M1["subscription<br/><i>initial, additional</i>"]
        M2["redemption<br/><i>partial, full</i>"]
        M3["transfer_in / transfer_out"]
        M4["dividend"]
        M5["adjustment"]
    end

    subgraph PE_VC["PE/VC Movements"]
        M6["commitment"]
        M7["capital_call<br/><i>call_number tracking</i>"]
        M8["distribution<br/><i>INCOME, CAPITAL,<br/>RETURN_OF_CAPITAL,<br/>RECALLABLE</i>"]
    end

    subgraph CORPORATE["Corporate Events"]
        M9["stock_split"]
        M10["merger"]
        M11["spinoff"]
    end

    style STANDARD fill:#e8f4e8,stroke:#50b848
    style PE_VC fill:#fef3e8,stroke:#f5a623
    style CORPORATE fill:#e8f0fe,stroke:#4a90d9
```

### 5.5 Provider-Agnostic Design

The investor register supports multiple data providers:

| Provider | Integration | Use Case |
|----------|------------|----------|
| `MANUAL` | Direct DSL entry | Small funds, ad-hoc |
| `CLEARSTREAM` | Automated feed | Luxembourg domiciled |
| `EUROCLEAR` | Automated feed | Belgian/European |
| `CSV_IMPORT` | Batch file | Migration, reconciliation |
| `API_FEED` | REST webhook | Real-time updates |

Each holding tracks `provider`, `provider_reference`, and `provider_sync_at` for reconciliation.

### 5.6 Investor Register View

The `v_investor_register` view presents a provider-agnostic, denormalized register:

| Field | Source | Purpose |
|-------|--------|---------|
| `investor_name` | entities | Display name |
| `investor_type` | investors | RETAIL, PROFESSIONAL, INSTITUTIONAL |
| `lifecycle_state` | investors | Current state in lifecycle |
| `kyc_status` | investors | KYC approval status |
| `holding_quantity` | holdings | Units held |
| `market_value` | units × NAV | Current value |
| `ownership_percentage` | units / total | Economic stake |
| `provider` | holdings | Data source |

### 5.7 Key Verbs — Investor Domain

| Verb | Purpose |
|------|---------|
| `investor.create` | Register new investor (starts at ENQUIRY) |
| `investor.request-documents` | ENQUIRY → PENDING_DOCUMENTS |
| `investor.approve-kyc` | KYC_IN_PROGRESS → KYC_APPROVED |
| `investor.mark-eligible` | KYC_APPROVED → ELIGIBLE_TO_SUBSCRIBE |
| `investor.record-subscription` | Record subscription event |
| `investor.activate` | SUBSCRIBED → ACTIVE_HOLDER |
| `investor.start-redemption` | Begin redemption process |
| `investor.suspend` / `reinstate` | Freeze/unfreeze investor |

### 5.8 Key Verbs — Holding Domain

| Verb | Purpose |
|------|---------|
| `holding.create` | Create holding (raw, entity-only) |
| `holding.create-for-investor` | Create holding linked to investor record |
| `holding.update-units` | Adjust position (with movement recording) |
| `holding.list-by-share-class` | All holders of a share class |
| `holding.list-by-investor` | All holdings for an investor |
| `holding.close` | Close position (zero units) |

---

## 6. Register 3: Fund-of-Fund / Look-Through — "What's the Exposure?"

The fund-of-fund register traces **economic ownership through multi-layer fund structures** to identify ultimate asset exposure. It answers: "If this investor holds Fund A, and Fund A holds Fund B, what is the investor's real exposure?"

### 6.1 Architecture

```mermaid
graph TB
    subgraph INPUT["Input Sources"]
        H["holdings<br/><i>(units x economic_per_unit)</i>"]
        FV["fund_vehicles<br/><i>Vehicle type taxonomy</i>"]
        IRP["investor_role_profiles<br/><i>lookthrough_policy per holder</i>"]
        OS["ownership_snapshots<br/><i>basis = ECONOMIC</i>"]
    end

    subgraph ENGINE["Look-Through Engine"]
        FN["fn_compute_economic_exposure()<br/><i>Bounded recursive CTE</i><br/><i>max_depth=6, min_pct=0.01%</i><br/><i>Cycle detection, role-aware stops</i>"]
    end

    subgraph OUTPUT["Output"]
        EXP["Economic exposure paths<br/><i>root → leaf with cumulative %</i>"]
        SUM["Aggregated exposure<br/><i>By investor type, jurisdiction</i>"]
        RISK["Concentration analysis<br/><i>Single-name, sector, geography</i>"]
    end

    H --> FN
    FV --> FN
    IRP --> FN
    OS --> FN
    FN --> EXP
    FN --> SUM
    EXP --> RISK

    style INPUT fill:#e8f0fe,stroke:#4a90d9
    style ENGINE fill:#fef3e8,stroke:#f5a623
    style OUTPUT fill:#e8f4e8,stroke:#50b848
```

### 6.2 The Look-Through Problem

Consider a three-layer fund structure:

```mermaid
graph TD
    INV["Pension Fund<br/>(investor)"]
    FOF["Fund of Funds<br/>(INTERMEDIARY_FOF)"]
    UND1["US Equity Fund<br/>(END_INVESTOR leaf)"]
    UND2["EU Bond Fund<br/>(END_INVESTOR leaf)"]

    INV -->|"30% holding"| FOF
    FOF -->|"60% holding"| UND1
    FOF -->|"40% holding"| UND2

    INV -.->|"effective: 18%<br/>(30% x 60%)"| UND1
    INV -.->|"effective: 12%<br/>(30% x 40%)"| UND2

    style INV fill:#2d6da4,color:#fff
    style FOF fill:#f5a623,color:#fff
    style UND1 fill:#50b848,color:#fff
    style UND2 fill:#50b848,color:#fff
```

**Without look-through:** Pension Fund has a 30% position in Fund of Funds. That's all we know.

**With look-through:** Pension Fund has effective 18% exposure to US equities and 12% exposure to EU bonds.

### 6.3 Holder Role Profiles (The Look-Through Switch)

The `investor_role_profiles` table determines **whether and how** to look through each holder:

| Role Type | Look-Through | Rationale |
|-----------|-------------|-----------|
| `END_INVESTOR` | Never | Terminal node — UBO candidate |
| `NOMINEE` | Always if data | Holds on behalf, must look through |
| `OMNIBUS` | Always if data | Aggregated position, must disaggregate |
| `INTERMEDIARY_FOF` | Per policy | Fund-of-fund — look through to underlying |
| `MASTER_POOL` | Per policy | Master fund in master/feeder |
| `INTRA_GROUP_POOL` | Per policy | Internal group vehicle |
| `TREASURY` | Never | Self-held shares |
| `CUSTODIAN` | Always if data | Holds in custody, not beneficial |

**Look-Through Policies:**

| Policy | Behavior |
|--------|----------|
| `NONE` | Treat as leaf — stop here |
| `ON_DEMAND` | Only look through when explicitly requested |
| `AUTO_IF_DATA` | Automatically look through if beneficial owner data is available |
| `ALWAYS` | Always attempt look-through regardless of data |

### 6.4 Bounded Recursive Look-Through

The `fn_compute_economic_exposure()` function implements bounded recursive traversal:

```mermaid
graph TD
    START["Start: Root entity"]
    
    START --> LOAD["Load direct economic edges<br/>(v_economic_edges_direct)"]
    LOAD --> CHECK{"For each edge:<br/>check stop conditions"}
    
    CHECK -->|"CYCLE_DETECTED<br/>(entity in path)"| STOP1["Stop: report cycle"]
    CHECK -->|"MAX_DEPTH<br/>(depth > 6)"| STOP2["Stop: depth limit"]
    CHECK -->|"BELOW_MIN_PCT<br/>(cumulative < 0.01%)"| STOP3["Stop: de minimis"]
    CHECK -->|"END_INVESTOR<br/>(role = END_INVESTOR)"| STOP4["Stop: terminal holder"]
    CHECK -->|"POLICY_NONE<br/>(lookthrough = NONE)"| STOP5["Stop: policy block"]
    CHECK -->|"NO_BO_DATA<br/>(no data available)"| STOP6["Stop: data gap"]
    CHECK -->|"Continue"| RECURSE["Recurse: multiply cumulative %<br/>and traverse next level"]
    
    RECURSE --> LOAD

    style START fill:#2d6da4,color:#fff
    style STOP1 fill:#d0021b,color:#fff
    style STOP2 fill:#d0021b,color:#fff
    style STOP3 fill:#d0021b,color:#fff
    style STOP4 fill:#50b848,color:#fff
    style STOP5 fill:#f5a623,color:#fff
    style STOP6 fill:#f5a623,color:#fff
    style RECURSE fill:#4a90d9,color:#fff
```

**Parameters:**

| Parameter | Default | Purpose |
|-----------|---------|---------|
| `p_max_depth` | 6 | Maximum recursion depth |
| `p_min_pct` | 0.0001 (0.01%) | De minimis threshold |
| `p_max_rows` | 200 | Safety limit on result set |
| `p_stop_on_no_bo_data` | true | Stop when no BO data available |
| `p_stop_on_policy_none` | true | Respect role profile policy |

### 6.5 Fund Vehicle Taxonomy

```mermaid
graph TD
    ROOT["Fund Vehicles<br/>(kyc.fund_vehicles)"]

    ROOT --> LUX["Luxembourg"]
    LUX --> L1["SCSP<br/><i>Special Limited Partnership</i>"]
    LUX --> L2["SICAV_RAIF<br/><i>Reserved AIF</i>"]
    LUX --> L3["SICAV_SIF<br/><i>Specialized Investment Fund</i>"]
    LUX --> L4["SICAV_UCITS"]
    LUX --> L5["FCP<br/><i>Fonds Commun de Placement</i>"]

    ROOT --> ANGLO["Anglo-Saxon"]
    ANGLO --> A1["LLC"]
    ANGLO --> A2["LP"]
    ANGLO --> A3["TRUST"]
    ANGLO --> A4["OEIC"]

    ROOT --> OTHER["Other"]
    OTHER --> O1["ETF"]
    OTHER --> O2["REIT"]
    OTHER --> O3["BDC"]

    style ROOT fill:#2d6da4,color:#fff
    style LUX fill:#4a90d9,color:#fff
    style ANGLO fill:#50b848,color:#fff
    style OTHER fill:#f5a623,color:#fff
```

### 6.6 Umbrella / Compartment Structure

```mermaid
graph TD
    UMB["Umbrella Fund<br/>(is_umbrella = true)"]
    
    UMB --> C1["Compartment: US Equity<br/>(fund_compartments)"]
    UMB --> C2["Compartment: EU Bond<br/>(fund_compartments)"]
    UMB --> C3["Compartment: Asia Pacific<br/>(fund_compartments)"]

    C1 --> SC1a["Class A (Retail)"]
    C1 --> SC1b["Class I (Institutional)"]
    C2 --> SC2a["Class A (Retail)"]
    C3 --> SC3a["Class A (Retail)"]
    C3 --> SC3b["Class I (Institutional)"]

    style UMB fill:#2d6da4,color:#fff
    style C1 fill:#4a90d9,color:#fff
    style C2 fill:#4a90d9,color:#fff
    style C3 fill:#4a90d9,color:#fff
```

Compartments enable **ring-fencing** within umbrella funds — each compartment has its own share classes, NAV, and investor base, but shares the umbrella's legal identity.

### 6.7 Key Verbs — Economic Exposure Domain

| Verb | Purpose |
|------|---------|
| `economic-exposure.compute` | Bounded recursive look-through |
| `economic-exposure.summary` | Aggregated exposure by investor type |
| `issuer-control-config.upsert` | Set per-issuer thresholds |

### 6.8 Key Verbs — Ownership Domain (Register Bridge)

| Verb | Purpose |
|------|---------|
| `ownership.compute` | Derive ownership snapshots from register holdings |
| `ownership.snapshot.list` | List historical snapshots |
| `ownership.control-positions` | Holder control positions |
| `ownership.who-controls` | Who controls this entity? |
| `ownership.reconcile` | Cross-check register vs control edges |
| `ownership.trace-chain` | Trace specific ownership chain |

---

## 7. The Intersection: How Registers Connect

### 7.1 The Ownership Snapshot Bridge

The `ownership_snapshots` table is the **bridge between registers**, recording point-in-time ownership from multiple bases:

```mermaid
graph TB
    subgraph SOURCES["Snapshot Sources"]
        REG["Register holdings<br/><i>derived_from = REGISTER</i>"]
        BODS_S["BODS statements<br/><i>derived_from = BODS</i>"]
        GLEIF_S["GLEIF relationships<br/><i>derived_from = GLEIF</i>"]
        PSC_S["PSC register<br/><i>derived_from = PSC</i>"]
        MAN["Manual entry<br/><i>derived_from = MANUAL</i>"]
    end

    subgraph BASES["Ownership Bases"]
        B1["UNITS<br/><i>Raw unit count</i>"]
        B2["VOTES<br/><i>Voting power</i>"]
        B3["ECONOMIC<br/><i>Economic interest</i>"]
        B4["CAPITAL<br/><i>Capital contribution</i>"]
        B5["DECLARED<br/><i>Self-declared (PSC/BODS)</i>"]
    end

    subgraph CONSUMERS["Register Consumers"]
        CR["Control Register<br/><i>basis = VOTES</i>"]
        IR["Investor Register<br/><i>basis = ECONOMIC</i>"]
        FF["Fund-of-Fund Register<br/><i>basis = ECONOMIC</i>"]
    end

    REG --> B1 & B2 & B3
    BODS_S --> B5
    GLEIF_S --> B3
    PSC_S --> B5
    MAN --> B2 & B3

    B2 --> CR
    B3 --> IR
    B3 --> FF

    style SOURCES fill:#e8f0fe,stroke:#4a90d9
    style BASES fill:#fef3e8,stroke:#f5a623
    style CONSUMERS fill:#e8f4e8,stroke:#50b848
```

### 7.2 Snapshot Schema

```sql
CREATE TABLE kyc.ownership_snapshots (
    snapshot_id       UUID PRIMARY KEY,
    issuer_entity_id  UUID NOT NULL,     -- Who is owned
    owner_entity_id   UUID NOT NULL,     -- Who owns
    share_class_id    UUID,              -- Which class (optional)
    
    as_of_date        DATE NOT NULL,     -- Point in time
    basis             VARCHAR(20),       -- UNITS, VOTES, ECONOMIC, CAPITAL, DECLARED
    
    -- The numbers
    units             NUMERIC(20,6),
    percentage        NUMERIC(8,4),
    percentage_min    NUMERIC(8,4),      -- Range (for BODS declared)
    percentage_max    NUMERIC(8,4),
    
    -- Denominator (for audit)
    numerator         NUMERIC(20,6),
    denominator       NUMERIC(20,6),
    
    -- Provenance
    derived_from      VARCHAR(20),       -- REGISTER, BODS, GLEIF, PSC, MANUAL, INFERRED
    is_direct         BOOLEAN DEFAULT true,
    confidence        VARCHAR(20) DEFAULT 'HIGH',
    
    -- Temporal versioning
    superseded_at     TIMESTAMPTZ,
    superseded_by     UUID
);
```

### 7.3 The UBO Sync Trigger

When a holding crosses the 25% threshold, an automatic trigger creates/updates UBO relationships:

```mermaid
sequenceDiagram
    participant H as holdings (INSERT/UPDATE)
    participant TRG as sync_holding_to_ubo_relationship()
    participant IRP as investor_role_profiles
    participant ER as entity_relationships
    participant CE as control_edges

    H->>TRG: Trigger fires
    TRG->>TRG: Calculate ownership %
    
    alt ownership >= 25%
        TRG->>IRP: Check role_type
        alt Role is NOMINEE, OMNIBUS, INTERMEDIARY_FOF, etc.
            TRG->>TRG: Skip (not a UBO candidate)
        else Role is END_INVESTOR or not classified
            TRG->>ER: Upsert entity_relationship (ownership type)
            TRG->>CE: Upsert control_edge (HOLDS_SHARES)
        end
    else ownership < 25%
        TRG->>ER: End-date relationship if exists
    end
```

This ensures the control register automatically stays in sync with the investor register when significant holdings change.

### 7.4 Reconciliation

The `ownership.reconcile` verb cross-checks the three sources:

| Check | Source 1 | Source 2 | Finding Type |
|-------|----------|----------|-------------|
| Register vs Control | Holdings-derived voting % | Control edges voting % | `VOTING_MISMATCH` |
| Register vs BODS | Holdings-derived economic % | BODS declared % | `DECLARATION_GAP` |
| Voting totals | Sum of all voting % | 100% | `VOTING_TOTAL_MISMATCH` |
| Economic totals | Sum of all economic % | 100% | `ECONOMIC_TOTAL_MISMATCH` |
| Missing UBO | >25% holder | entity_ubos | `UBO_CANDIDATE_NO_DETERMINATION` |

---

## 8. Supply Chain: Issuance and Dilution

### 8.1 Capital Supply Model

```mermaid
graph TB
    subgraph LEDGER["Append-Only Issuance Ledger"]
        IE["issuance_events<br/><i>INITIAL_ISSUE, NEW_ISSUE, STOCK_SPLIT,<br/>BONUS_ISSUE, CANCELLATION, BUYBACK,<br/>MERGER_IN, MERGER_OUT, CONVERSION</i>"]
    end

    subgraph SUPPLY["Materialized Supply State"]
        SCS["share_class_supply<br/><i>authorized, issued, outstanding,<br/>treasury, reserved</i>"]
    end

    subgraph DILUTION["Dilutive Instruments"]
        DI["dilution_instruments<br/><i>STOCK_OPTION, WARRANT,<br/>CONVERTIBLE_NOTE, SAFE,<br/>RSU, PHANTOM_STOCK</i>"]
    end

    subgraph CALC["Supply Functions"]
        FN1["fn_share_class_supply_at()<br/><i>Point-in-time supply</i>"]
        FN2["fn_diluted_supply_at()<br/><i>Fully diluted supply</i>"]
    end

    IE -->|"fold events"| SCS
    SCS --> FN1
    DI --> FN2
    FN1 --> FN2

    style LEDGER fill:#e8f0fe,stroke:#4a90d9
    style SUPPLY fill:#e8f4e8,stroke:#50b848
    style DILUTION fill:#fef3e8,stroke:#f5a623
    style CALC fill:#fde8e8,stroke:#d0021b
```

### 8.2 Dilution Instruments

For fully diluted ownership calculation, the system tracks instruments that **could** convert to equity:

| Instrument | Effect on Control | Effect on Economics |
|-----------|-------------------|---------------------|
| Stock options | Dilutes voting when exercised | Dilutes economic |
| Warrants | Same | Same |
| Convertible notes | Converts at valuation cap / discount | Adds new shares |
| SAFEs | Converts at next round | Adds new shares |
| RSUs | Vests into shares | Dilutes both |

**Fully diluted calculation:**

```
Outstanding shares: 1,000,000
+ Exercisable options: 100,000
+ In-the-money warrants: 50,000
+ Convertible note (at cap): 150,000
= Fully diluted: 1,300,000

Holder with 200,000 shares:
  Basic ownership: 200K / 1M = 20.0%
  Fully diluted: 200K / 1.3M = 15.4%
```

### 8.3 Special Rights (Beyond Shares)

Not all control comes from shares. The system tracks non-share rights:

| Right Type | Effect |
|-----------|--------|
| `BOARD_APPOINTMENT` | Appoints N directors |
| `BOARD_OBSERVER` | Observer seat (no vote) |
| `VETO_MA` | Veto on M&A transactions |
| `VETO_FUNDRAISE` | Veto on new fundraising |
| `VETO_DIVIDEND` | Veto on distributions |
| `ANTI_DILUTION` | Protection against dilution |
| `DRAG_ALONG` | Force sale of minority |
| `TAG_ALONG` | Right to sell alongside majority |
| `FIRST_REFUSAL` | Right of first refusal on transfers |
| `REDEMPTION` | Right to redeem shares |

These rights are stored per share class or per holder and feed into the control register's analysis.

---

## 9. Entity Relationship Diagram

```mermaid
erDiagram
    share_classes ||--o{ holdings : "has holders"
    share_classes ||--o{ issuance_events : "supply events"
    share_classes ||--o| share_class_supply : "current supply"
    share_classes ||--o{ dilution_instruments : "convertible to"
    share_classes }o--o| fund_compartments : "in compartment"

    holdings }o--|| investors : "TA mode"
    holdings }o--|| entities : "holder entity"
    holdings ||--o{ movements : "transaction history"

    investors }o--|| entities : "is entity"
    investors }o--o| cbus : "owned by CBU"

    ownership_snapshots }o--|| entities : "owner"
    ownership_snapshots }o--|| entities : "issuer"
    ownership_snapshots }o--o| share_classes : "of class"

    control_edges }o--|| entities : "from"
    control_edges }o--|| entities : "to"
    control_edges }o--o| share_classes : "via class"

    entity_ubos }o--|| entities : "of entity"

    investor_role_profiles }o--|| entities : "issuer"
    investor_role_profiles }o--|| entities : "holder"
    investor_role_profiles }o--o| share_classes : "for class"

    fund_vehicles }o--|| entities : "is entity"
    fund_vehicles }o--o| entities : "umbrella of"
    fund_compartments }o--|| entities : "in umbrella"

    bods_ownership_statements }o--o| bods_entity_statements : "subject"
    bods_ownership_statements }o--o| bods_person_statements : "interested party"
```

---

## 10. Worked Example: Three Lenses on Allianz

Consider an Allianz fund structure:

```mermaid
graph TD
    ASE["Allianz SE<br/><i>Listed company<br/>UBO terminus: PUBLIC_FLOAT</i>"]
    AGI["Allianz Global Investors<br/>Holdings GmbH<br/><i>100% owned by Allianz SE</i>"]

    FOF["Allianz Multi-Strategy Fund<br/><i>Fund-of-Fund (INTERMEDIARY_FOF)</i>"]
    
    EQ["Allianz IE ETF SICAV<br/><i>UCITS fund, compartmented</i>"]
    
    FI["Allianz Fixed Income Fund<br/><i>UCITS fund</i>"]

    PENSION["UK Pension Trust<br/><i>END_INVESTOR, 15% of FoF</i>"]
    RETAIL["Retail Investors<br/><i>Aggregated, 40% of FoF</i>"]

    ASE -->|"100% voting + economic"| AGI
    AGI -->|"100% control"| FOF
    AGI -->|"100% control"| EQ
    AGI -->|"100% control"| FI

    PENSION -->|"15%"| FOF
    RETAIL -->|"40%"| FOF
    
    FOF -->|"60% allocation"| EQ
    FOF -->|"40% allocation"| FI

    style ASE fill:#d0021b,color:#fff
    style AGI fill:#d0021b,color:#fff
    style FOF fill:#f5a623,color:#fff
    style EQ fill:#50b848,color:#fff
    style FI fill:#50b848,color:#fff
    style PENSION fill:#4a90d9,color:#fff
    style RETAIL fill:#4a90d9,color:#fff
```

**Lens 1 — Control Register:**
- Allianz SE → AGI Holdings (100% voting) → all funds (100% control)
- UBO determination: Allianz SE is PUBLIC_FLOAT (listed terminus)
- Board controller: AGI Holdings via Rule B (100% voting rights)

**Lens 2 — Investor Register:**
- Multi-Strategy Fund share classes: UK Pension Trust holds 15%, Retail 40%, AGI 45%
- UK Pension Trust lifecycle: ACTIVE_HOLDER, KYC_APPROVED, PROFESSIONAL investor
- NAV allocation: 15% × NAV = Pension Trust's economic entitlement

**Lens 3 — Fund-of-Fund Look-Through:**
- UK Pension Trust (15% of FoF) → FoF holds 60% EQ + 40% FI
- Effective exposure: 9% to EU equity ETF, 6% to fixed income
- Stop condition on Allianz SE: PUBLIC_FLOAT (no further look-through)

---

## 11. Completeness Summary

### 11.1 Table Coverage

| Domain | Tables | Key Tables |
|--------|--------|------------|
| Share Classes | 3 | `share_classes`, `share_class_supply`, `share_class_identifiers` |
| Holdings | 2 | `holdings`, `movements` |
| Investors | 1 | `investors` |
| Fund Structure | 2 | `fund_vehicles`, `fund_compartments` |
| Control | 2 | `control_edges`, `board_controller` (derived) |
| UBO | 1 | `entity_ubos` |
| BODS | 3 | `bods_entity_statements`, `bods_person_statements`, `bods_ownership_statements` |
| Ownership Bridge | 1 | `ownership_snapshots` |
| Role Profiles | 1 | `investor_role_profiles` |
| Dilution | 1 | `dilution_instruments` |
| Issuance | 1 | `issuance_events` |
| Config | 1 | `issuer_control_config` |
| **Total** | **19** | |

### 11.2 Verb Coverage

| Domain | Verb Count | Coverage |
|--------|-----------|----------|
| `investor` | 16 | Full lifecycle (ENQUIRY → OFFBOARDED) |
| `holding` | 11 | CRUD + dual-mode (TA/UBO) |
| `share-class` | 8 | CRUD + NAV updates + identifiers |
| `ubo` | 18 | Ownership, control, trust, lifecycle |
| `control` | 14 | Analysis, graph, reconciliation, import |
| `ownership` | 15 | Snapshots, reconciliation, special rights |
| `economic-exposure` | 3 | Compute, summary, config |
| **Total** | **85** | |

### 11.3 SQL Function Coverage

| Function | Purpose |
|----------|---------|
| `fn_holder_control_position()` | Per-holder voting + economic % |
| `fn_share_class_supply_at()` | Point-in-time supply (basic) |
| `fn_diluted_supply_at()` | Fully diluted supply including options/warrants |
| `fn_compute_economic_exposure()` | Bounded recursive look-through |
| `fn_economic_exposure_summary()` | Aggregated exposure by investor type |
| `fn_derive_ownership_snapshots()` | Register → snapshot bridge |
| `sync_holding_to_ubo_relationship()` | Auto-sync UBO trigger (25% threshold) |

### 11.4 Rust Type Coverage

| Type | Module | Purpose |
|------|--------|---------|
| `InvestorRegisterView` | `ob-poc-types` | Top-level register view |
| `ControlHolderNode` | `ob-poc-types` | Per-holder control analysis |
| `ThresholdConfig` | `ob-poc-types` | Configurable disclosure thresholds |
| `ControlEdge` | `ob-poc-types` | Standards-aligned edge |
| `ControlEdgeType` | `ob-poc-types` | 16 edge type enum |
| `BoardControlMethod` | `ob-poc-types` | 3-rule board controller |
| `BoardControllerEdge` | `ob-poc-types` | Board controller determination |
| `DiscoveredUbo` | `bods/ubo_discovery` | UBO with chain + confidence |
| `UboResult` | `bods/ubo_discovery` | NaturalPersons / PublicFloat / StateOwned / Unknown |
| `PscCategory` | `ob-poc-types` | UK PSC category enum |

---

## 12. Open Design Questions (For Peer Review)

1. **Temporal consistency across registers:** When a holding changes at T1, the ownership_snapshot is created at T1, but the UBO sync trigger fires immediately. Should the UBO determination use a point-in-time snapshot or always reflect real-time holdings?

2. **Look-through depth limits by jurisdiction:** UCITS requires full look-through for 10% concentration. AIFMD Art. 7 requires look-through for leverage. Should `fn_compute_economic_exposure()` accept jurisdiction-specific depth limits?

3. **Nominee chain length:** Some custody chains are Investor → Nominee → Global Custodian → Local Custodian → CSD. Should we have a maximum nominee chain depth separate from the fund-of-fund depth limit?

4. **Hybrid role profiles:** An entity can be an END_INVESTOR for one issuer and an INTERMEDIARY_FOF for another. The current `investor_role_profiles` supports this (scoped per issuer × holder × share_class), but should we have a default role type per entity?

5. **Real-time vs batch look-through:** The current implementation is on-demand (verb invocation). Should we add a materialized view for frequently-queried exposure paths (e.g., for the top 10 funds)?

6. **BODS vs Register reconciliation frequency:** The auto-sync trigger handles register → control, but BODS → register reconciliation is manual. Should this be automated on BODS import?

---

## Appendix A: Related Architecture Documents

| Document | Location | Coverage |
|----------|----------|----------|
| Instrument Matrix & Trading Universe | `migrations/INSTRUMENT_MATRIX_TRADING_UNIVERSE.md` | Trading profile, settlement routing, ISDA/CSA |
| Schema Entity Overview | `migrations/OB_POC_SCHEMA_ENTITY_OVERVIEW.md` | Full schema overview |
| Group Taxonomy & Ownership | `ai-thoughts/019-group-taxonomy-intra-company-ownership.md` | Group structure design |
| Investor Register Visualization | `ai-thoughts/018-investor-register-visualization.md` | UI visualization approach |
| Entity Resolution Wiring | `ai-thoughts/033-entity-resolution-wiring-plan.md` | Entity linking for register entities |

## Appendix B: Standards Cross-Reference

| Standard | Tables Used | Purpose |
|----------|-----------|---------|
| **BODS 0.4** | `bods_entity_statements`, `bods_person_statements`, `bods_ownership_statements` | Beneficial ownership data exchange |
| **GLEIF RR** | `entity_relationships`, `control_edges` | Legal entity relationship register |
| **UK PSC** | `control_edges.psc_category` | Persons of Significant Control |
| **4AMLD/5AMLD** | `entity_ubos`, `issuer_control_config` | EU UBO thresholds (25%) |
| **FATCA** | `investors.fatca_status` | US tax compliance |
| **CRS** | `investors.crs_status` | Common Reporting Standard |
| **UCITS** | `fund_vehicles`, `economic-exposure.compute` | Fund look-through requirements |
| **AIFMD** | `fund_vehicles`, `economic-exposure.compute` | AIF look-through for leverage |
