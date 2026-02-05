# Conceptual Data Model

**Document Version:** 1.0  
**Last Updated:** 2026-02-05  
**Audience:** Engineering Team, Business Analysts, Product  

---

## Why This Model Exists

We provide **custody and fund administration services** to investment funds. To do this, we must answer fundamental questions:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        THE QUESTIONS WE MUST ANSWER                          │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  HOW did we win this business?          →  Deal Record & Fee Billing        │
│                                                                              │
│  WHO owns/controls this structure?      →  Ownership & Control Model        │
│                                                                              │
│  WHAT services do we provide?           →  Products & Subscriptions         │
│                                                                              │
│  WHAT can they trade?                   →  Trading Matrix                   │
│                                                                              │
│  WHERE do we settle?                    →  Settlement Infrastructure        │
│                                                                              │
│  WHO invests in the fund?               →  Investor Register                │
│                                                                              │
│  ARE they compliant?                    →  KYC/AML Model                    │
│                                                                              │
│  WHAT documents prove this?             →  Document Library                 │
│                                                                              │
│  WHAT attributes must we collect?       →  Attribute Dictionary             │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## The Central Concept: CBU (Client Business Unit)

**Everything revolves around the CBU.** It's our atomic unit of service delivery.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                                                                              │
│                           WHAT IS A CBU?                                     │
│                                                                              │
│    A CBU is the "thing" we service. It represents a single trading/         │
│    investment unit that needs custody, administration, and compliance.      │
│                                                                              │
│    ┌─────────────────┐   ┌─────────────────┐   ┌─────────────────┐          │
│    │                 │   │                 │   │                 │          │
│    │   UCITS Fund    │   │   PE Fund       │   │   Segregated    │          │
│    │                 │   │                 │   │   Mandate       │          │
│    │  "Allianz Euro  │   │  "Blackstone    │   │                 │          │
│    │   High Yield"   │   │   Capital VII"  │   │  "Swiss Pension │          │
│    │                 │   │                 │   │   Scheme"       │          │
│    └─────────────────┘   └─────────────────┘   └─────────────────┘          │
│           │                     │                     │                     │
│           └─────────────────────┼─────────────────────┘                     │
│                                 │                                            │
│                                 ▼                                            │
│                    ┌───────────────────────┐                                │
│                    │                       │                                │
│                    │    All are CBUs       │                                │
│                    │                       │                                │
│                    │  • Same service model │                                │
│                    │  • Same data model    │                                │
│                    │  • Same compliance    │                                │
│                    │                       │                                │
│                    └───────────────────────┘                                │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Domain Model Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                                                                              │
│                        HIGH-LEVEL DOMAIN MODEL                               │
│                                                                              │
│                                                                              │
│         ┌──────────────────────────────────────────────────────┐            │
│         │                                                      │            │
│         │              GROUP / CLIENT HIERARCHY                │            │
│         │                                                      │            │
│         │    "Allianz" owns 50 funds across 5 jurisdictions   │            │
│         │                                                      │            │
│         └──────────────────────────┬───────────────────────────┘            │
│                                    │                                         │
│                                    │ contains                                │
│                                    ▼                                         │
│    ┌───────────────────────────────────────────────────────────────┐        │
│    │                                                               │        │
│    │                    ┌─────────────────┐                        │        │
│    │                    │                 │                        │        │
│    │                    │      CBU        │                        │        │
│    │                    │   (The Fund)    │                        │        │
│    │                    │                 │                        │        │
│    │                    └────────┬────────┘                        │        │
│    │                             │                                 │        │
│    │           ┌─────────────────┼─────────────────┐               │        │
│    │           │                 │                 │               │        │
│    │           ▼                 ▼                 ▼               │        │
│    │   ┌──────────────┐  ┌──────────────┐  ┌──────────────┐       │        │
│    │   │   PARTIES    │  │   PRODUCTS   │  │  INVESTORS   │       │        │
│    │   │              │  │              │  │              │       │        │
│    │   │ Who's        │  │ What services│  │ Who owns     │       │        │
│    │   │ involved?    │  │ do we sell?  │  │ shares?      │       │        │
│    │   │              │  │              │  │              │       │        │
│    │   └──────────────┘  └──────────────┘  └──────────────┘       │        │
│    │           │                 │                 │               │        │
│    │           ▼                 ▼                 ▼               │        │
│    │   ┌──────────────┐  ┌──────────────┐  ┌──────────────┐       │        │
│    │   │     KYC      │  │   TRADING    │  │   HOLDINGS   │       │        │
│    │   │              │  │   MATRIX     │  │              │       │        │
│    │   │ Are they     │  │ What can     │  │ How much     │       │        │
│    │   │ compliant?   │  │ they trade?  │  │ do they own? │       │        │
│    │   │              │  │              │  │              │       │        │
│    │   └──────────────┘  └──────────────┘  └──────────────┘       │        │
│    │                                                               │        │
│    └───────────────────────────────────────────────────────────────┘        │
│                                                                              │
│    ┌───────────────────────────────────────────────────────────────┐        │
│    │                     CROSS-CUTTING CONCERNS                    │        │
│    │                                                               │        │
│    │   ┌─────────────┐    ┌─────────────┐    ┌─────────────┐      │        │
│    │   │  DOCUMENTS  │    │ ATTRIBUTES  │    │   EVENTS    │      │        │
│    │   │             │    │             │    │             │      │        │
│    │   │  Evidence   │    │  What data  │    │  What       │      │        │
│    │   │  & proof    │    │  do we need?│    │  happened?  │      │        │
│    │   └─────────────┘    └─────────────┘    └─────────────┘      │        │
│    └───────────────────────────────────────────────────────────────┘        │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 1. Parties & Roles: WHO is involved?

**Business Problem:** A fund has many participants - managers, directors, custodians, auditors. We need to know WHO they are and WHAT ROLE they play.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                                                                              │
│                         PARTIES & ROLES MODEL                                │
│                                                                              │
│                                                                              │
│                         ┌─────────────────┐                                 │
│                         │                 │                                 │
│                         │      CBU        │                                 │
│                         │  "Alpha Fund"   │                                 │
│                         │                 │                                 │
│                         └────────┬────────┘                                 │
│                                  │                                          │
│              ┌───────────────────┼───────────────────┐                      │
│              │                   │                   │                      │
│              ▼                   ▼                   ▼                      │
│    ┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐             │
│    │                 │ │                 │ │                 │             │
│    │   MANAGEMENT    │ │   GOVERNANCE    │ │   SERVICES      │             │
│    │                 │ │                 │ │                 │             │
│    └────────┬────────┘ └────────┬────────┘ └────────┬────────┘             │
│             │                   │                   │                      │
│    ┌────────┴────────┐ ┌────────┴────────┐ ┌────────┴────────┐             │
│    │ • ManCo         │ │ • Directors     │ │ • Depositary    │             │
│    │ • Inv. Manager  │ │ • Chairman      │ │ • Custodian     │             │
│    │ • Sub-Advisor   │ │ • Cond. Officer │ │ • Administrator │             │
│    │ • Portfolio Mgr │ │ • Co. Secretary │ │ • Transfer Agent│             │
│    └─────────────────┘ └─────────────────┘ │ • Auditor       │             │
│                                            │ • Legal Counsel │             │
│                                            └─────────────────┘             │
│                                                                              │
│    ┌─────────────────────────────────────────────────────────────────┐      │
│    │                                                                 │      │
│    │  Each PARTY is an ENTITY (person or company) playing a ROLE    │      │
│    │                                                                 │      │
│    │  ┌──────────────┐         ┌──────────────┐                     │      │
│    │  │              │  plays  │              │                     │      │
│    │  │    ENTITY    │────────►│     ROLE     │                     │      │
│    │  │  "John Smith"│         │  "Director"  │                     │      │
│    │  │              │         │              │                     │      │
│    │  └──────────────┘         └──────────────┘                     │      │
│    │                                                                 │      │
│    │  Same entity can play multiple roles in multiple CBUs          │      │
│    │                                                                 │      │
│    └─────────────────────────────────────────────────────────────────┘      │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 2. Ownership & Control: WHO owns/controls?

**Business Problem:** Regulations (AML, UBO) require us to identify who OWNS and who CONTROLS the fund. These are different concepts.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                                                                              │
│                      OWNERSHIP vs CONTROL MODEL                              │
│                                                                              │
│                                                                              │
│     OWNERSHIP (Follow the Money)        CONTROL (Follow the Power)          │
│     ════════════════════════════        ════════════════════════            │
│                                                                              │
│     Who gets the economic benefit?      Who makes decisions?                │
│                                                                              │
│                                                                              │
│          ┌─────────┐                         ┌─────────┐                    │
│          │  UBO 1  │ 40%                     │ Director│                    │
│          │ Person  │                         │ Person  │                    │
│          └────┬────┘                         └────┬────┘                    │
│               │                                   │                         │
│               ▼                                   ▼                         │
│          ┌─────────┐                         ┌─────────┐                    │
│          │Holding  │                         │  ManCo  │                    │
│          │Company  │                         │ Company │                    │
│          └────┬────┘                         └────┬────┘                    │
│               │ 100%                              │ manages                 │
│               ▼                                   ▼                         │
│          ┌─────────┐                         ┌─────────┐                    │
│          │   CBU   │                         │   CBU   │                    │
│          │  Fund   │                         │  Fund   │                    │
│          └─────────┘                         └─────────┘                    │
│                                                                              │
│                                                                              │
│    ┌─────────────────────────────────────────────────────────────────┐      │
│    │                                                                 │      │
│    │   WHY TWO CHAINS?                                              │      │
│    │                                                                 │      │
│    │   • A fund might be owned by Investor A (ownership chain)      │      │
│    │   • But managed by ManCo B with Director C (control chain)     │      │
│    │   • Regulations require identifying BOTH chains                │      │
│    │   • UBO = anyone with ≥25% ownership OR significant control    │      │
│    │                                                                 │      │
│    └─────────────────────────────────────────────────────────────────┘      │
│                                                                              │
│                                                                              │
│    ┌─────────────────────────────────────────────────────────────────┐      │
│    │                                                                 │      │
│    │   ROLE CATEGORIES FOR CHAINS:                                  │      │
│    │                                                                 │      │
│    │   OWNERSHIP_CHAIN        │  CONTROL_CHAIN                      │      │
│    │   ───────────────        │  ─────────────                      │      │
│    │   • Shareholder          │  • Director                         │      │
│    │   • Limited Partner      │  • Chairman                         │      │
│    │   • General Partner      │  • Controlling Person               │      │
│    │   • Beneficial Owner     │  • Chief Executive                  │      │
│    │   • Holding Company      │  • Conducting Officer               │      │
│    │   • UBO (terminus)       │  • Trustee (for trusts)             │      │
│    │                          │                                      │      │
│    └─────────────────────────────────────────────────────────────────┘      │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 3. Group Structure: Commercial Clients

**Business Problem:** We don't onboard one fund at a time. Clients like "Allianz" have 50+ funds. We need to model the GROUP.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                                                                              │
│                         CLIENT GROUP MODEL                                   │
│                                                                              │
│                                                                              │
│                         ┌─────────────────┐                                 │
│                         │                 │                                 │
│                         │  CLIENT GROUP   │                                 │
│                         │   "Allianz"     │                                 │
│                         │                 │                                 │
│                         └────────┬────────┘                                 │
│                                  │                                          │
│                    ┌─────────────┼─────────────┐                            │
│                    │             │             │                            │
│                    ▼             ▼             ▼                            │
│           ┌──────────────┐ ┌──────────────┐ ┌──────────────┐               │
│           │              │ │              │ │              │               │
│           │  Allianz SE  │ │  AGI GmbH    │ │ AGI Ireland  │               │
│           │ (Ultimate    │ │ (Governance  │ │ (Operating   │               │
│           │  Parent)     │ │  Controller) │ │  Entity)     │               │
│           │              │ │              │ │              │               │
│           └──────────────┘ └──────────────┘ └──────────────┘               │
│                                  │                                          │
│                    ┌─────────────┼─────────────┐                            │
│                    │             │             │                            │
│                    ▼             ▼             ▼                            │
│           ┌──────────────┐ ┌──────────────┐ ┌──────────────┐               │
│           │     CBU      │ │     CBU      │ │     CBU      │               │
│           │  LU Fund 1   │ │  LU Fund 2   │ │  IE Fund 1   │               │
│           └──────────────┘ └──────────────┘ └──────────────┘               │
│                                                                              │
│                                                                              │
│    ┌─────────────────────────────────────────────────────────────────┐      │
│    │                                                                 │      │
│    │   ANCHOR ROLES (how we navigate the group):                    │      │
│    │                                                                 │      │
│    │   ┌─────────────────┬─────────────────────────────────────┐    │      │
│    │   │ Role            │ Purpose                             │    │      │
│    │   ├─────────────────┼─────────────────────────────────────┤    │      │
│    │   │ ULTIMATE_PARENT │ Top of corporate hierarchy (UBO)    │    │      │
│    │   │ GOVERNANCE_CTRL │ Session scope, CBU loading          │    │      │
│    │   │ BOOK_CONTROLLER │ Regional operations                 │    │      │
│    │   │ OPERATING_CTRL  │ Day-to-day management               │    │      │
│    │   │ REGULATORY_ANCHOR│ Compliance contact point           │    │      │
│    │   └─────────────────┴─────────────────────────────────────┘    │      │
│    │                                                                 │      │
│    │   "Load the Allianz book" → Finds GOVERNANCE_CONTROLLER        │      │
│    │                           → Returns all CBUs under that entity │      │
│    │                                                                 │      │
│    └─────────────────────────────────────────────────────────────────┘      │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 4. Trading Matrix: WHAT can they trade?

**Business Problem:** Each fund has specific permissions - what instruments, which markets, what currencies. This is the "trading passport."

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                                                                              │
│                         TRADING MATRIX CONCEPT                               │
│                                                                              │
│                                                                              │
│    The Trading Matrix is a 3D permission cube:                              │
│                                                                              │
│                                                                              │
│                        CURRENCIES                                            │
│                       ┌─────────────────────────────┐                       │
│                      /│ GBP │ USD │ EUR │ JPY │ CHF│                        │
│                     / └─────┴─────┴─────┴─────┴────┘                        │
│                    /                                                         │
│         MARKETS   /    ┌─────┬─────┬─────┬─────┬────┐                       │
│                  /     │  ✓  │  ✓  │  ✓  │     │    │  XLON                 │
│    ┌────────────/      ├─────┼─────┼─────┼─────┼────┤                       │
│    │ XLON      │       │  ✓  │  ✓  │  ✓  │  ✓  │    │  XNYS                 │
│    │ XNYS      │       ├─────┼─────┼─────┼─────┼────┤                       │
│    │ XPAR      │       │     │     │  ✓  │     │    │  XPAR                 │
│    │ XTKS      │       ├─────┼─────┼─────┼─────┼────┤                       │
│    │ ...       │       │     │     │     │  ✓  │    │  XTKS                 │
│    └───────────┘       └─────┴─────┴─────┴─────┴────┘                       │
│         │                                                                    │
│         │                                                                    │
│         │              INSTRUMENT CLASSES                                    │
│         │              ══════════════════                                    │
│         └─────────►    EQUITY │ FIXED_INCOME │ OTC_IRS │ FX                 │
│                                                                              │
│                                                                              │
│    ┌─────────────────────────────────────────────────────────────────┐      │
│    │                                                                 │      │
│    │   For OTC derivatives, add a 4th dimension: COUNTERPARTY       │      │
│    │                                                                 │      │
│    │   "Can we trade IRS in USD with Goldman Sachs?"                │      │
│    │                                                                 │      │
│    │   Requires: ✓ OTC_IRS in universe                              │      │
│    │            ✓ USD permitted                                     │      │
│    │            ✓ Goldman in counterparty list                      │      │
│    │            ✓ Active ISDA agreement                             │      │
│    │            ✓ CSA for collateral                                │      │
│    │                                                                 │      │
│    └─────────────────────────────────────────────────────────────────┘      │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 5. Products, Services & Resources: The Delivery Hierarchy

**Business Problem:** We sell services to clients, but delivering them requires a clear hierarchy from commercial contracts down to actual system endpoints. This three-tier model maps what we SELL to HOW we DELIVER it.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                                                                              │
│                  THE PRODUCT → SERVICE → RESOURCE HIERARCHY                  │
│                                                                              │
│                                                                              │
│    ┌─────────────────────────────────────────────────────────────────┐      │
│    │                                                                 │      │
│    │   TIER 1: PRODUCTS (Commercial / Contractable)                 │      │
│    │   ════════════════════════════════════════════                 │      │
│    │                                                                 │      │
│    │   What we SELL. These appear on contracts and invoices.        │      │
│    │   Client subscribes CBUs to Products.                          │      │
│    │                                                                 │      │
│    │   ┌─────────────┐ ┌─────────────┐ ┌─────────────┐              │      │
│    │   │   CUSTODY   │ │  FUND ADMIN │ │ TRANSFER AG │              │      │
│    │   │             │ │             │ │             │              │      │
│    │   │ Safekeeping │ │ NAV & Acctg │ │ Shareholder │              │      │
│    │   │ of assets   │ │ services    │ │ servicing   │              │      │
│    │   └──────┬──────┘ └──────┬──────┘ └──────┬──────┘              │      │
│    │          │               │               │                      │      │
│    └──────────┼───────────────┼───────────────┼──────────────────────┘      │
│               │               │               │                              │
│               │  decomposes   │               │                              │
│               │  into         │               │                              │
│               ▼               ▼               ▼                              │
│    ┌─────────────────────────────────────────────────────────────────┐      │
│    │                                                                 │      │
│    │   TIER 2: SERVICES (Business-Generic Capabilities)             │      │
│    │   ════════════════════════════════════════════════             │      │
│    │                                                                 │      │
│    │   Standard industry terminology. What we actually DO.          │      │
│    │   One Product maps to multiple Services.                       │      │
│    │                                                                 │      │
│    │   CUSTODY Product includes:                                    │      │
│    │   ┌────────────┐ ┌────────────┐ ┌────────────┐ ┌────────────┐ │      │
│    │   │SAFEKEEPING │ │ SETTLEMENT │ │CORP ACTIONS│ │  INCOME    │ │      │
│    │   │            │ │            │ │            │ │ COLLECTION │ │      │
│    │   │ Hold assets│ │ DVP/FOP    │ │ Dividends, │ │            │ │      │
│    │   │ securely   │ │ processing │ │ elections  │ │ Coupons    │ │      │
│    │   └─────┬──────┘ └─────┬──────┘ └─────┬──────┘ └─────┬──────┘ │      │
│    │         │              │              │              │         │      │
│    │   FUND ADMIN Product includes:                                 │      │
│    │   ┌────────────┐ ┌────────────┐ ┌────────────┐ ┌────────────┐ │      │
│    │   │  NAV CALC  │ │FUND ACCTG  │ │  EXPENSE   │ │PERFORMANCE │ │      │
│    │   │            │ │            │ │    MGMT    │ │ MEASUREMENT│ │      │
│    │   │ Daily NAV  │ │ GL entries │ │ Accruals   │ │ Attribution│ │      │
│    │   └─────┬──────┘ └─────┬──────┘ └─────┬──────┘ └─────┬──────┘ │      │
│    │         │              │              │              │         │      │
│    └─────────┼──────────────┼──────────────┼──────────────┼─────────┘      │
│              │              │              │              │                  │
│              │  implemented │              │              │                  │
│              │  by          │              │              │                  │
│              ▼              ▼              ▼              ▼                  │
│    ┌─────────────────────────────────────────────────────────────────┐      │
│    │                                                                 │      │
│    │   TIER 3: RESOURCES (BNY Proprietary Delivery Endpoints)       │      │
│    │   ══════════════════════════════════════════════════════       │      │
│    │                                                                 │      │
│    │   BNY-specific applications and systems. The actual endpoints  │      │
│    │   where work gets done. Usually provisioned as accounts.       │      │
│    │                                                                 │      │
│    │   ┌────────────┐ ┌────────────┐ ┌────────────┐ ┌────────────┐ │      │
│    │   │  CUSTODY   │ │ SETTLEMENT │ │    SWIFT   │ │    DTCC    │ │      │
│    │   │  ACCOUNT   │ │  ACCOUNT   │ │ CONNECTION │ │  SYSTEM    │ │      │
│    │   │            │ │            │ │            │ │            │ │      │
│    │   │ Per-CBU    │ │ Per-CSD    │ │ Messaging  │ │ US Settle  │ │      │
│    │   │ asset acct │ │ cash acct  │ │ gateway    │ │ gateway    │ │      │
│    │   └────────────┘ └────────────┘ └────────────┘ └────────────┘ │      │
│    │                                                                 │      │
│    │   ┌────────────┐ ┌────────────┐ ┌────────────┐ ┌────────────┐ │      │
│    │   │    NAV     │ │  INVESTOR  │ │    IBOR    │ │ CORP ACTION│ │      │
│    │   │   ENGINE   │ │   LEDGER   │ │   SYSTEM   │ │  PLATFORM  │ │      │
│    │   │            │ │            │ │            │ │            │ │      │
│    │   │ Pricing &  │ │ Shareholder│ │ Position   │ │ Event      │ │      │
│    │   │ valuation  │ │ register   │ │ tracking   │ │ processing │ │      │
│    │   └────────────┘ └────────────┘ └────────────┘ └────────────┘ │      │
│    │                                                                 │      │
│    └─────────────────────────────────────────────────────────────────┘      │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Why Three Tiers?**

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                                                                              │
│    Tier 1: PRODUCT     │  Commercial language (what Sales sells)            │
│    ════════════════    │  "We're buying Custody and Fund Admin"             │
│                        │                                                     │
│    Tier 2: SERVICE     │  Operations language (what we deliver)             │
│    ════════════════    │  "We need Settlement, NAV Calc, Corp Actions"      │
│                        │                                                     │
│    Tier 3: RESOURCE    │  Technology language (where it runs)               │
│    ═══════════════     │  "Provision IBOR account, SWIFT connection"        │
│                                                                              │
│    ┌────────────────────────────────────────────────────────────────────┐   │
│    │                                                                    │   │
│    │   This separation allows:                                         │   │
│    │                                                                    │   │
│    │   • Sales to talk Products without knowing system details         │   │
│    │   • Ops to plan Services without knowing commercial pricing       │   │
│    │   • Tech to provision Resources without knowing client context    │   │
│    │                                                                    │   │
│    │   Each tier is independently maintainable and auditable.          │   │
│    │                                                                    │   │
│    └────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Subscription & Onboarding Flow:**

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                                                                              │
│    CONTRACT → PRODUCT → CBU SUBSCRIPTION → SERVICE ENABLEMENT → RESOURCES   │
│                                                                              │
│                                                                              │
│    ┌──────────────┐         ┌──────────────┐         ┌──────────────┐      │
│    │              │         │              │         │              │      │
│    │   CONTRACT   │────────►│   PRODUCT    │◄────────│     CBU      │      │
│    │              │ covers  │              │ subscribes             │      │
│    │  "MSA-2024"  │         │  "CUSTODY"   │    to    │ "Alpha Fund"│      │
│    │              │         │              │         │              │      │
│    └──────────────┘         └──────┬───────┘         └──────────────┘      │
│                                    │                                        │
│                                    │ includes                               │
│                                    ▼                                        │
│                     ┌──────────────────────────────┐                       │
│                     │         SERVICES             │                       │
│                     │                              │                       │
│                     │  • SAFEKEEPING               │                       │
│                     │  • SETTLEMENT                │                       │
│                     │  • CORP_ACTIONS              │                       │
│                     │  • INCOME_COLLECTION         │                       │
│                     │                              │                       │
│                     └──────────────┬───────────────┘                       │
│                                    │                                        │
│                                    │ provisioned via                        │
│                                    ▼                                        │
│                     ┌──────────────────────────────┐                       │
│                     │   CBU RESOURCE INSTANCES     │                       │
│                     │                              │                       │
│                     │  • Custody Account: 12345678 │                       │
│                     │  • Settlement Acct: 87654321 │                       │
│                     │  • SWIFT BIC: IRVTUS3NXXX    │                       │
│                     │  • IBOR ID: ALPHA-IBOR-001   │                       │
│                     │                              │                       │
│                     └──────────────────────────────┘                       │
│                                                                              │
│                                                                              │
│    The ONBOARDING GATE:                                                     │
│    ═══════════════════                                                      │
│                                                                              │
│    • Contract must cover the Product                                        │
│    • CBU subscribes via the Contract                                        │
│    • Subscription triggers Service enablement                               │
│    • Services require Resource provisioning                                 │
│    • No subscription = no service delivery possible                         │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Products vs Trading Matrix - Key Distinction:**

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                                                                              │
│    PRODUCTS = What services we PROVIDE to the CBU                           │
│               (Custody, Fund Admin, Transfer Agency)                        │
│               "Can we safekeep their assets?"                               │
│                                                                              │
│    TRADING MATRIX = What the CBU can TRADE                                  │
│                     (Equities on XLON, OTC IRS with Goldman)                │
│                     "What instruments/markets are permitted?"               │
│                                                                              │
│    ┌────────────────────────────────────────────────────────────────────┐   │
│    │                                                                    │   │
│    │   Example:                                                        │   │
│    │                                                                    │   │
│    │   Alpha Fund subscribes to CUSTODY product                        │   │
│    │   → Gets SAFEKEEPING, SETTLEMENT, CORP_ACTIONS services           │   │
│    │   → Resources provisioned (accounts, connections)                 │   │
│    │                                                                    │   │
│    │   Alpha Fund's Trading Matrix permits:                            │   │
│    │   → EQUITY on XLON, XNYS in USD, GBP, EUR                        │   │
│    │   → FIXED_INCOME on all markets                                   │   │
│    │                                                                    │   │
│    │   These are INDEPENDENT. You need BOTH:                           │   │
│    │   - Product subscription (to use the service)                     │   │
│    │   - Trading Matrix permission (to trade that instrument)          │   │
│    │                                                                    │   │
│    └────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 6. Settlement: WHERE does it settle?

**Business Problem:** When a trade happens, we need to know WHERE to deliver securities and WHERE to pay/receive cash.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                                                                              │
│                         SETTLEMENT MODEL                                     │
│                                                                              │
│                                                                              │
│    A TRADE needs SETTLEMENT INSTRUCTIONS:                                   │
│                                                                              │
│                                                                              │
│       ┌─────────────────┐                                                   │
│       │                 │                                                   │
│       │     TRADE       │   Buy 10,000 shares of Vodafone                   │
│       │                 │   on London Stock Exchange                        │
│       │                 │   settling DVP in GBP                             │
│       │                 │                                                   │
│       └────────┬────────┘                                                   │
│                │                                                            │
│                │  Which SSI?                                                │
│                ▼                                                            │
│       ┌─────────────────────────────────────────────────────────┐          │
│       │                                                         │          │
│       │   SSI BOOKING RULES (priority ordered)                  │          │
│       │                                                         │          │
│       │   Priority 10: XLON + GBP + DVP → SSI-XLON-GBP-DVP     │          │
│       │   Priority 20: XLON + GBP       → SSI-XLON-GBP         │          │
│       │   Priority 30: XLON             → SSI-XLON-DEFAULT      │          │
│       │   Priority 99: (fallback)       → SSI-DEFAULT           │          │
│       │                                                         │          │
│       └─────────────────────────────────────────────────────────┘          │
│                │                                                            │
│                │  Most specific match wins                                  │
│                ▼                                                            │
│       ┌─────────────────────────────────────────────────────────┐          │
│       │                                                         │          │
│       │   SSI: SSI-XLON-GBP-DVP                                 │          │
│       │                                                         │          │
│       │   Securities Account: 12345678                          │          │
│       │   Custodian BIC:      SBOSUS3NLND                       │          │
│       │   Cash Account:       87654321                          │          │
│       │   Cash BIC:           BABORB2L                          │          │
│       │   Place of Settlement: CREST (CRSTGB2L)                 │          │
│       │                                                         │          │
│       └─────────────────────────────────────────────────────────┘          │
│                                                                              │
│                                                                              │
│    For complex routes, we use SETTLEMENT CHAINS:                            │
│                                                                              │
│       ┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐         │
│       │          │    │          │    │          │    │          │         │
│       │   CBU    │───►│ Custodian│───►│   Sub-   │───►│   CSD    │         │
│       │          │    │          │    │ Custodian│    │          │         │
│       │          │    │ (Hop 1)  │    │ (Hop 2)  │    │ (Hop 3)  │         │
│       └──────────┘    └──────────┘    └──────────┘    └──────────┘         │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 7. Investor Register: WHO invests?

**Business Problem:** Funds have investors who own shares. We track WHO they are, WHAT they own, and their KYC status.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                                                                              │
│                         INVESTOR REGISTER MODEL                              │
│                                                                              │
│                                                                              │
│                         ┌─────────────────┐                                 │
│                         │                 │                                 │
│                         │      CBU        │                                 │
│                         │  "Alpha Fund"   │                                 │
│                         │                 │                                 │
│                         └────────┬────────┘                                 │
│                                  │                                          │
│                    ┌─────────────┼─────────────┐                            │
│                    │             │             │                            │
│                    ▼             ▼             ▼                            │
│           ┌──────────────┐ ┌──────────────┐ ┌──────────────┐               │
│           │ SHARE CLASS  │ │ SHARE CLASS  │ │ SHARE CLASS  │               │
│           │              │ │              │ │              │               │
│           │  Class A     │ │  Class B     │ │  Class I     │               │
│           │  EUR Acc     │ │  USD Dist    │ │  GBP Inst    │               │
│           │              │ │              │ │              │               │
│           │  ISIN: LU... │ │  ISIN: LU... │ │  ISIN: LU... │               │
│           │  NAV: €100   │ │  NAV: $98    │ │  NAV: £102   │               │
│           │              │ │              │ │              │               │
│           └──────┬───────┘ └──────────────┘ └──────────────┘               │
│                  │                                                          │
│        ┌─────────┼─────────┐                                               │
│        │         │         │                                               │
│        ▼         ▼         ▼                                               │
│    ┌────────┐ ┌────────┐ ┌────────┐                                        │
│    │HOLDING │ │HOLDING │ │HOLDING │                                        │
│    │        │ │        │ │        │                                        │
│    │10,000  │ │50,000  │ │25,000  │                                        │
│    │ units  │ │ units  │ │ units  │                                        │
│    └───┬────┘ └───┬────┘ └───┬────┘                                        │
│        │          │          │                                             │
│        ▼          ▼          ▼                                             │
│    ┌────────┐ ┌────────┐ ┌────────┐                                        │
│    │INVESTOR│ │INVESTOR│ │INVESTOR│                                        │
│    │        │ │        │ │        │                                        │
│    │ Pension│ │ HNW    │ │ Family │                                        │
│    │ Fund   │ │ Client │ │ Office │                                        │
│    │        │ │        │ │        │                                        │
│    │ KYC: ✓ │ │ KYC: ✓ │ │ KYC: ⏳│                                        │
│    └───┬────┘ └───┬────┘ └───┬────┘                                        │
│        │          │          │                                             │
│        ▼          ▼          ▼                                             │
│    ┌────────┐ ┌────────┐ ┌────────┐                                        │
│    │ ENTITY │ │ ENTITY │ │ ENTITY │                                        │
│    │        │ │        │ │        │                                        │
│    │"Swiss  │ │"John   │ │"Smith  │                                        │
│    │Pension"│ │ Doe"   │ │Family" │                                        │
│    └────────┘ └────────┘ └────────┘                                        │
│                                                                              │
│                                                                              │
│    KEY CONCEPT: INVESTOR wraps ENTITY with fund-specific context           │
│    (KYC status, tax status, investor category, eligible fund types)        │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 8. KYC/AML Model: Compliance

**Business Problem:** Every party must be screened and verified. Different roles have different KYC obligations.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                                                                              │
│                           KYC MODEL                                          │
│                                                                              │
│                                                                              │
│    ROLE determines KYC OBLIGATION:                                          │
│                                                                              │
│    ┌─────────────────────┬─────────────────────────────────────────────┐    │
│    │ KYC Obligation      │ What it means                               │    │
│    ├─────────────────────┼─────────────────────────────────────────────┤    │
│    │ FULL_KYC            │ Complete due diligence, ID verification,    │    │
│    │                     │ source of wealth, UBO identification        │    │
│    ├─────────────────────┼─────────────────────────────────────────────┤    │
│    │ SIMPLIFIED          │ Basic checks, regulated entity exemptions   │    │
│    ├─────────────────────┼─────────────────────────────────────────────┤    │
│    │ SCREEN_AND_ID       │ Sanctions screening + ID verification       │    │
│    ├─────────────────────┼─────────────────────────────────────────────┤    │
│    │ SCREEN_ONLY         │ Sanctions/PEP screening only                │    │
│    ├─────────────────────┼─────────────────────────────────────────────┤    │
│    │ RECORD_ONLY         │ Just record their details, no verification  │    │
│    └─────────────────────┴─────────────────────────────────────────────┘    │
│                                                                              │
│                                                                              │
│    KYC CASE LIFECYCLE:                                                      │
│                                                                              │
│    ┌───────┐    ┌───────┐    ┌───────┐    ┌───────┐    ┌───────┐          │
│    │       │    │       │    │       │    │       │    │       │          │
│    │ OPEN  │───►│ DATA  │───►│SCREEN │───►│REVIEW │───►│APPROVED│          │
│    │       │    │COLLECT│    │  ING  │    │       │    │       │          │
│    │       │    │       │    │       │    │       │    │       │          │
│    └───────┘    └───────┘    └───────┘    └───┬───┘    └───────┘          │
│                                               │                            │
│                                               │ issues found               │
│                                               ▼                            │
│                                          ┌───────┐                         │
│                                          │       │                         │
│                                          │REJECTED│                        │
│                                          │       │                         │
│                                          └───────┘                         │
│                                                                              │
│                                                                              │
│    UBO TREATMENT (how to handle in ownership chain):                        │
│                                                                              │
│    ┌─────────────────────┬─────────────────────────────────────────────┐    │
│    │ Treatment           │ Meaning                                     │    │
│    ├─────────────────────┼─────────────────────────────────────────────┤    │
│    │ TERMINUS            │ This IS the UBO, stop here                  │    │
│    │ LOOK_THROUGH        │ Must identify who's behind this entity      │    │
│    │ BY_PERCENTAGE       │ UBO if ownership ≥ 25%                      │    │
│    │ CONTROL_PRONG       │ Control = UBO regardless of %               │    │
│    │ EXEMPT              │ Exempt entity (sovereign, listed)           │    │
│    │ NOT_APPLICABLE      │ Role doesn't affect UBO determination       │    │
│    └─────────────────────┴─────────────────────────────────────────────┘    │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 9. Document Library: Evidence & Proof

**Business Problem:** Everything we know about parties must be backed by documents. Documents are requested, received, and verified.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                                                                              │
│                         DOCUMENT MODEL                                       │
│                                                                              │
│                                                                              │
│    Three-Layer Model:                                                       │
│                                                                              │
│    ┌─────────────────────────────────────────────────────────────────┐      │
│    │                                                                 │      │
│    │   LAYER A: REQUIREMENT                                         │      │
│    │   ════════════════════                                         │      │
│    │   "What we NEED"                                               │      │
│    │                                                                 │      │
│    │   • Passport required for Director John Smith                  │      │
│    │   • Proof of Address required for UBO Jane Doe                 │      │
│    │   • Certificate of Incorporation for Holding Co                │      │
│    │                                                                 │      │
│    │   Status: MISSING → REQUESTED → RECEIVED → VERIFIED            │      │
│    │                                                                 │      │
│    └─────────────────────────────────────────────────────────────────┘      │
│                          │                                                  │
│                          │ fulfilled by                                     │
│                          ▼                                                  │
│    ┌─────────────────────────────────────────────────────────────────┐      │
│    │                                                                 │      │
│    │   LAYER B: DOCUMENT                                            │      │
│    │   ══════════════════                                           │      │
│    │   "The logical identity"                                       │      │
│    │                                                                 │      │
│    │   • John Smith's Passport (stable reference)                   │      │
│    │   • May have multiple versions over time                       │      │
│    │                                                                 │      │
│    └─────────────────────────────────────────────────────────────────┘      │
│                          │                                                  │
│                          │ has versions                                     │
│                          ▼                                                  │
│    ┌─────────────────────────────────────────────────────────────────┐      │
│    │                                                                 │      │
│    │   LAYER C: VERSION                                             │      │
│    │   ═══════════════                                              │      │
│    │   "Each submission"                                            │      │
│    │                                                                 │      │
│    │   • Version 1: Uploaded 2024-01-15, REJECTED (blurry)         │      │
│    │   • Version 2: Uploaded 2024-01-16, VERIFIED ✓                │      │
│    │                                                                 │      │
│    │   Immutable - never modified, only superseded                  │      │
│    │                                                                 │      │
│    └─────────────────────────────────────────────────────────────────┘      │
│                                                                              │
│                                                                              │
│    Document Types:                                                          │
│                                                                              │
│    ┌──────────────────┬────────────────────────────────────────────┐        │
│    │ Category         │ Types                                      │        │
│    ├──────────────────┼────────────────────────────────────────────┤        │
│    │ Identity         │ Passport, National ID, Driver's License    │        │
│    │ Address          │ Utility Bill, Bank Statement, Tax Bill     │        │
│    │ Corporate        │ Certificate of Inc., Articles, Registers   │        │
│    │ Ownership        │ Share Register, Org Chart, UBO Declaration │        │
│    │ Financial        │ Financial Statements, Source of Wealth     │        │
│    │ Regulatory       │ Licenses, Registrations, Authorizations    │        │
│    │ Contractual      │ ISDA, CSA, Custody Agreement, IMA          │        │
│    └──────────────────┴────────────────────────────────────────────┘        │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 10. Attribute Dictionary: What Data Do We Need?

**Business Problem:** Different CBU types, roles, and products require different attributes. We need a flexible way to define requirements.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                                                                              │
│                      ATTRIBUTE DICTIONARY MODEL                              │
│                                                                              │
│                                                                              │
│    The Attribute Dictionary defines WHAT DATA we need to collect:           │
│                                                                              │
│                                                                              │
│    ┌─────────────────────────────────────────────────────────────────┐      │
│    │                                                                 │      │
│    │   ATTRIBUTE REGISTRY                                           │      │
│    │                                                                 │      │
│    │   ┌─────────────────────────────────────────────────────────┐  │      │
│    │   │ Attribute        │ Type    │ Required For               │  │      │
│    │   ├───────────────────┼─────────┼────────────────────────────┤  │      │
│    │   │ legal_name        │ string  │ All entities               │  │      │
│    │   │ date_of_birth     │ date    │ Natural persons            │  │      │
│    │   │ nationality       │ enum    │ Natural persons            │  │      │
│    │   │ incorporation_date│ date    │ Legal entities             │  │      │
│    │   │ lei               │ string  │ Legal entities (optional)  │  │      │
│    │   │ tax_id            │ string  │ All (jurisdiction-specific)│  │      │
│    │   │ pep_status        │ boolean │ All persons                │  │      │
│    │   │ source_of_wealth  │ text    │ UBOs, HNW investors        │  │      │
│    │   │ aum               │ decimal │ Institutional investors    │  │      │
│    │   └───────────────────┴─────────┴────────────────────────────┘  │      │
│    │                                                                 │      │
│    └─────────────────────────────────────────────────────────────────┘      │
│                                                                              │
│                                                                              │
│    Requirements cascade from multiple sources:                              │
│                                                                              │
│    ┌─────────────┐   ┌─────────────┐   ┌─────────────┐                     │
│    │             │   │             │   │             │                     │
│    │  CBU TYPE   │   │   PRODUCT   │   │    ROLE     │                     │
│    │             │   │             │   │             │                     │
│    │ UCITS fund  │   │  CUSTODY    │   │  DIRECTOR   │                     │
│    │ requires X  │   │ requires Y  │   │ requires Z  │                     │
│    │             │   │             │   │             │                     │
│    └──────┬──────┘   └──────┬──────┘   └──────┬──────┘                     │
│           │                 │                 │                            │
│           └─────────────────┼─────────────────┘                            │
│                             │                                              │
│                             ▼                                              │
│                   ┌───────────────────┐                                    │
│                   │                   │                                    │
│                   │  UNIFIED ATTRIBUTE│                                    │
│                   │   REQUIREMENTS    │                                    │
│                   │                   │                                    │
│                   │  X ∪ Y ∪ Z        │                                    │
│                   │                   │                                    │
│                   └───────────────────┘                                    │
│                                                                              │
│                                                                              │
│    Gaps Analysis View:                                                      │
│                                                                              │
│    ┌─────────────────────────────────────────────────────────────────┐      │
│    │                                                                 │      │
│    │   CBU: Alpha Fund                                              │      │
│    │   ────────────────                                             │      │
│    │                                                                 │      │
│    │   Director: John Smith                                         │      │
│    │   ✓ legal_name: "John Smith"                                   │      │
│    │   ✓ date_of_birth: 1965-03-15                                  │      │
│    │   ✗ nationality: MISSING                    ← Gap!             │      │
│    │   ✓ passport_number: AB123456                                  │      │
│    │   ✗ proof_of_address: MISSING               ← Gap!             │      │
│    │                                                                 │      │
│    │   Completeness: 60%                                            │      │
│    │                                                                 │      │
│    └─────────────────────────────────────────────────────────────────┘      │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 11. Deal Record & Fee Billing: HOW did we win this business?

**Business Problem:** Before we can service a fund, we must WIN the business. Deals track the commercial origination lifecycle - from first conversation through signed contracts to billing.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                                                                              │
│                         DEAL LIFECYCLE                                       │
│                                                                              │
│   PROSPECT → QUALIFYING → NEGOTIATING → CONTRACTED → ONBOARDING → ACTIVE    │
│                                                          │                   │
│                                                          ▼                   │
│                                                    ┌──────────┐              │
│                                                    │  CBUs    │              │
│                                                    │ Created  │              │
│                                                    └──────────┘              │
│                                                                              │
│   ┌─────────────────────────────────────────────────────────────────┐       │
│   │                                                                 │       │
│   │                      DEAL STRUCTURE                             │       │
│   │                                                                 │       │
│   │                    ┌──────────────┐                             │       │
│   │                    │              │                             │       │
│   │                    │     DEAL     │ "Blackrock PB 2026"         │       │
│   │                    │              │                             │       │
│   │                    └──────┬───────┘                             │       │
│   │                           │                                     │       │
│   │    ┌──────────────────────┼──────────────────────┐             │       │
│   │    │                      │                      │             │       │
│   │    ▼                      ▼                      ▼             │       │
│   │ ┌─────────────┐    ┌─────────────┐       ┌─────────────┐       │       │
│   │ │ PARTICIPANTS│    │  RATE CARDS │       │  CONTRACTS  │       │       │
│   │ │             │    │             │       │             │       │       │
│   │ │ Blackrock   │    │ Custody:    │       │ MSA #1234   │       │       │
│   │ │ UK Ltd      │    │ 5 bps AUM   │       │             │       │       │
│   │ │ (Primary)   │    │             │       │ Schedule A  │       │       │
│   │ └─────────────┘    └─────────────┘       └─────────────┘       │       │
│   │                           │                                     │       │
│   │                           ▼                                     │       │
│   │                    ┌─────────────┐                             │       │
│   │                    │  ONBOARDING │                             │       │
│   │                    │  REQUESTS   │                             │       │
│   │                    │             │                             │       │
│   │                    │ Fund 1 ───► │ → CBU Created               │       │
│   │                    │ Fund 2 ───► │ → CBU Created               │       │
│   │                    └─────────────┘                             │       │
│   │                                                                 │       │
│   └─────────────────────────────────────────────────────────────────┘       │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### The Closed-Loop Billing Model

**The key insight:** The same rate agreed in the deal MUST match the rate we bill.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                                                                              │
│                     NEGOTIATION → BILLING AUDIT TRAIL                        │
│                                                                              │
│                                                                              │
│    DEAL PHASE                    BILLING PHASE                              │
│    ══════════                    ═════════════                              │
│                                                                              │
│    deal_rate_card_lines          fee_billing_account_targets                │
│    ┌────────────────────┐        ┌────────────────────┐                     │
│    │ line_id: ABC-123   │───────►│ rate_card_line_id: │                     │
│    │ fee_type: CUSTODY  │        │    ABC-123         │                     │
│    │ rate_bps: 5.0      │        │                    │                     │
│    │ status: AGREED     │        │ Billable Account   │                     │
│    └────────────────────┘        └─────────┬──────────┘                     │
│                                            │                                 │
│                                            ▼                                 │
│                                  fee_billing_period_lines                   │
│                                  ┌────────────────────┐                     │
│                                  │ rate_card_line_id: │                     │
│                                  │    ABC-123         │                     │
│                                  │                    │                     │
│                                  │ calculated_amount: │                     │
│                                  │    $50,000         │                     │
│                                  └────────────────────┘                     │
│                                                                              │
│    ═══════════════════════════════════════════════════════════════════      │
│                                                                              │
│    AUDIT PROOF: Line ABC-123 agreed at 5 bps, billed at 5 bps.             │
│                 Same line_id from negotiation through invoice.              │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Key Tables

| Table | Purpose |
|-------|---------|
| `deals` | Master deal record linked to client_group |
| `deal_participants` | Contracting parties (entities with roles) |
| `deal_rate_cards` | Pricing proposals (DRAFT → PROPOSED → AGREED) |
| `deal_rate_card_lines` | Individual fee schedules |
| `deal_contracts` | Links to legal_contracts |
| `deal_onboarding_requests` | Handoff to onboarding → CBU creation |
| `fee_billing_profiles` | Billing configuration per CBU + Product |
| `fee_billing_periods` | Monthly/quarterly billing cycles |
| `fee_billing_period_lines` | Calculated fees linked back to agreed rates |

### Pricing Models

```
┌────────────────────────────────────────────────────────────────┐
│   Pricing Model   │   Description           │   Required     │
├───────────────────┼─────────────────────────┼────────────────┤
│   FLAT            │   Fixed amount/period   │   flat_amount  │
│   BPS             │   Basis points on AUM   │   rate_bps,    │
│                   │                         │   fee_basis    │
│   TIERED          │   Tiered brackets       │   tier_brackets│
│   PER_TX          │   Per transaction       │   per_tx_rate  │
└───────────────────┴─────────────────────────┴────────────────┘
```

---

## 12. How It All Fits Together

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                                                                              │
│                    THE COMPLETE PICTURE                                      │
│                                                                              │
│                                                                              │
│                          ┌───────────────┐                                  │
│                          │               │                                  │
│                          │ CLIENT GROUP  │ "Allianz"                        │
│                          │               │                                  │
│                          └───────┬───────┘                                  │
│                                  │                                          │
│                    ┌─────────────┴─────────────┐                            │
│                    │                           │                            │
│                    ▼                           ▼                            │
│           ┌───────────────┐           ┌───────────────┐                    │
│           │               │           │               │                    │
│           │     CBU 1     │           │     CBU 2     │                    │
│           │  "LU Fund A"  │           │  "IE Fund B"  │                    │
│           │               │           │               │                    │
│           └───────┬───────┘           └───────────────┘                    │
│                   │                                                         │
│     ┌─────────────┼─────────────┬─────────────┬─────────────┐              │
│     │             │             │             │             │              │
│     ▼             ▼             ▼             ▼             ▼              │
│ ┌───────┐   ┌───────────┐ ┌───────────┐ ┌───────────┐ ┌───────────┐       │
│ │PARTIES│   │ PRODUCTS  │ │  TRADING  │ │ INVESTORS │ │    KYC    │       │
│ │& ROLES│   │           │ │  MATRIX   │ │           │ │           │       │
│ │       │   │           │ │           │ │           │ │           │       │
│ │ManCo  │   │✓ CUSTODY  │ │EQUITY:    │ │Pension Co │ │Case #123  │       │
│ │Director│  │✓ FUND_ACCT│ │ XLON,XNYS │ │HNW Client │ │Case #124  │       │
│ │Custodian│ │✓ TRANS_AG │ │OTC_IRS:   │ │Family Off │ │Case #125  │       │
│ │       │   │           │ │ Goldman   │ │           │ │           │       │
│ └───┬───┘   └─────┬─────┘ └─────┬─────┘ └─────┬─────┘ └─────┬─────┘       │
│     │             │             │             │             │              │
│     │             │             │             │             │              │
│     └─────────────┴─────────────┴─────────────┴─────────────┘              │
│                                     │                                       │
│                                     ▼                                       │
│                   ┌─────────────────────────────────┐                      │
│                   │                                 │                      │
│                   │   CROSS-CUTTING SERVICES        │                      │
│                   │                                 │                      │
│                   │   ┌─────────┐   ┌─────────┐    │                      │
│                   │   │DOCUMENTS│   │ATTRIBUTES│   │                      │
│                   │   │         │   │         │    │                      │
│                   │   │Passports│   │Required │    │                      │
│                   │   │Contracts│   │ fields  │    │                      │
│                   │   │Proofs   │   │per role │    │                      │
│                   │   └─────────┘   └─────────┘    │                      │
│                   │                                 │                      │
│                   └─────────────────────────────────┘                      │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Summary: Why We Built It This Way

| Business Need | Model Component | Why This Design |
|---------------|-----------------|-----------------|
| Track commercial origination | **Deal Record** | Full lifecycle from prospect to onboarded, linked to client_group |
| Audit fee billing | **Fee Billing + Rate Cards** | Same line_id from negotiation → invoice (closed-loop proof) |
| Service multiple fund types uniformly | **CBU** | Single abstraction for UCITS, AIF, PE, Mandates |
| Track who's involved | **Entity + Role** | Same person can be Director of Fund A and Investor in Fund B |
| Identify beneficial owners | **Ownership Chain** | Follow the money up the structure |
| Identify controllers | **Control Chain** | Follow the power (directors, trustees) |
| Manage client relationships | **Client Group** | "Allianz" has 50 funds, treat as one client |
| Gate service access | **Products → Services → Resources** | Three-tier: Commercial (Products) → Business (Services) → Delivery (Resources) |
| Control what can be traded | **Trading Matrix** | 3D permission cube: Instrument × Market × Currency |
| Route settlements correctly | **SSI + Booking Rules** | Priority-based routing to correct accounts |
| Track fund investors | **Investor Register** | Who owns what shares, KYC status |
| Ensure compliance | **KYC Model** | Role-based obligations, case management |
| Prove everything | **Document Library** | Evidence for every fact, versioned and verified |
| Know what data we need | **Attribute Dictionary** | Flexible requirements based on context |

---

## Key Design Principles

1. **CBU-Centric**: Everything connects to the CBU - it's the hub of the wheel
2. **Entity Reuse**: One entity, many roles across many CBUs
3. **Role-Based Obligations**: Your role determines what KYC you need
4. **Separation of Ownership & Control**: Different chains, different purposes
5. **Hierarchical Groups**: Model real corporate structures
6. **Evidence-Based**: Every claim backed by a document
7. **Flexible Requirements**: Attribute needs vary by context
8. **Priority-Based Routing**: Most specific rule wins

---

## Glossary

| Term | Definition |
|------|------------|
| **AML** | Anti-Money Laundering - regulatory compliance |
| **AUM** | Assets Under Management - total value of assets serviced |
| **BPS** | Basis Points - 1/100th of a percent (5 bps = 0.05%) |
| **CBU** | Client Business Unit - the fund/mandate we service |
| **Client Group** | Commercial client entity that owns multiple CBUs (e.g., "Allianz") |
| **CSA** | Credit Support Annex - collateral agreement under ISDA |
| **Deal** | A commercial opportunity being pursued, from prospect through onboarding |
| **Deal Participant** | An entity playing a role in the deal (primary contracting party, fee payer, etc.) |
| **Entity** | Any person (natural) or organization (legal) |
| **Fee Billing Account Target** | Links a CBU to billable products via the negotiated rate card line |
| **Fee Billing Period** | A billing cycle (monthly/quarterly) for generating invoices |
| **Fee Billing Profile** | Billing configuration for a CBU under a contract/product combination |
| **ISDA** | International Swaps and Derivatives Association master agreement |
| **KYC** | Know Your Customer - due diligence requirements |
| **ManCo** | Management Company - the entity that manages the fund |
| **Onboarding Request** | A request to create a CBU from a deal, triggers handoff to operations |
| **Product** | A commercial service we sell (Custody, Fund Admin, Transfer Agency) - appears on contracts |
| **Rate Card** | A set of fee schedules for a deal (DRAFT → PROPOSED → AGREED → SUPERSEDED) |
| **Rate Card Line** | A single fee schedule (e.g., "Custody at 5 bps on AUM") - flows through to billing |
| **Resource** | A BNY proprietary delivery endpoint (IBOR System, NAV Engine, Custody Account) - where work runs |
| **Role** | The capacity in which an entity participates (Director, Custodian, etc.) |
| **Service** | A business-generic capability (Safekeeping, Settlement, NAV Calc) - what we actually do |
| **SSI** | Standing Settlement Instructions - where to settle trades |
| **Subscription** | A CBU's enrollment in a Product via a Contract |
| **UBO** | Ultimate Beneficial Owner - the human who ultimately owns/controls |
