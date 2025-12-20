# The Gap: Top of House vs The Shop Floor

*Captured: 2024-12-20*
*Context: BNY's public Microsoft/OpenAI "transformation" messaging vs internal reality*

---

## The Two Realities

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  TOP OF HOUSE (Public messaging, earnings calls, press releases)           │
│                                                                             │
│  "Strategic partnership with Microsoft"                                    │
│  "AI-powered transformation"                                               │
│  "Reimagining the future of finance"                                       │
│  "Leading the industry in innovation"                                      │
│                                                                             │
│  Audience: Investors, regulators, board, LinkedIn                          │
│  Purpose: Stock price, recruitment, competitive positioning                │
│  Reality: Aspirational vision with budget attached                         │
│                                                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│  THE SHOP FLOOR (Engineering, middle management, delivery teams)           │
│                                                                             │
│  "We're a Java shop"                                                       │
│  "Who approved this?"                                                      │
│  "That's not how we do things here"                                        │
│  "My team will maintain this how exactly?"                                 │
│  "This threatens my project/budget/headcount"                              │
│                                                                             │
│  Audience: Each other, their managers, HR                                  │
│  Purpose: Self-preservation, career protection, risk avoidance             │
│  Reality: Terror and backstabbing                                          │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## The Organizational Dynamics

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                                                                             │
│  CEO/Board                                                                 │
│     │                                                                      │
│     │  "We need AI transformation"                                         │
│     │  "Microsoft partnership is strategic"                                │
│     │  "Why aren't we seeing results?"                                     │
│     ▼                                                                      │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  SENIOR BUSINESS LEADERS (MD, SVP, Business Line Heads)             │   │
│  │                                                                     │   │
│  │  • Pressure from above: "Deliver AI transformation"                 │   │
│  │  • Resistance from below: "We need 2 more years and 50 people"     │   │
│  │  • Budget responsibility: "This has to show ROI"                   │   │
│  │  • Career risk: "If this fails, it's my head"                      │   │
│  │                                                                     │   │
│  │  ══════════════════════════════════════════════════════════════════│   │
│  │  THIS IS WHERE DECISIONS GET MADE                                  │   │
│  │  These people are DESPERATE enough to take risks                   │   │
│  │  They have AUTHORITY to bypass "how we do things"                  │   │
│  │  They are NOT in engineering                                       │   │
│  │  ══════════════════════════════════════════════════════════════════│   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│     │                                                                      │
│     │  "Make it happen"                                                    │
│     │  "I don't care how"                                                  │
│     │  "Show me results"                                                   │
│     ▼                                                                      │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  MIDDLE MANAGEMENT (Directors, VPs, Team Leads)                     │   │
│  │                                                                     │   │
│  │  • Squeezed from both sides                                        │   │
│  │  • Protecting existing projects/teams/budgets                      │   │
│  │  • "Not invented here" syndrome                                    │   │
│  │  • Fear of being made obsolete                                     │   │
│  │  • Backstabbing and politics                                       │   │
│  │                                                                     │   │
│  │  DEFAULT RESPONSE: Delay, committee, study, pilot-to-nowhere       │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│     │                                                                      │
│     ▼                                                                      │
│  Engineering / Delivery Teams                                              │
│     │                                                                      │
│     │  "Just tell me what to build"                                        │
│     │  "We've always done it this way"                                     │
│     │  "Who's going to support this?"                                      │
│     ▼                                                                      │
│  [Nothing changes]                                                         │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## The Target: Desperate + Authority

The winning combination:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  THE DECISION MAKER YOU NEED                                               │
│                                                                             │
│  ✓ Senior enough to bypass "how we do things here"                        │
│  ✓ Business line responsibility (P&L, not just cost center)               │
│  ✓ Under pressure to deliver "transformation"                              │
│  ✓ Frustrated with 2-year Java project proposals                          │
│  ✓ Willing to take career risk for career reward                          │
│  ✓ NOT in engineering (engineering protects engineering)                   │
│                                                                             │
│  Title patterns:                                                           │
│  • MD, Global Head of [Business Line]                                      │
│  • SVP, Chief [Something] Officer                                          │
│  • Head of [Product/Service] for [Region/Segment]                         │
│  • Managing Director, [Revenue-Generating Function]                        │
│                                                                             │
│  NOT:                                                                      │
│  • VP Engineering                                                          │
│  • Chief Architect                                                         │
│  • Head of Platform                                                        │
│  • Director of Software Development                                        │
│                                                                             │
│  Why not engineering? They're optimizing for:                              │
│  • Team size (bigger = more important)                                     │
│  • Technology consistency (Java everywhere)                                │
│  • Risk avoidance (nobody got fired for choosing Java)                     │
│  • Project longevity (2 years = job security)                             │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## The Pitch Changes by Audience

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  TO ENGINEERING                                                            │
│                                                                             │
│  What you'd say:  "Compile-time entity resolution, DAG execution..."      │
│  What they hear:  "Threat to my Java expertise and project budget"        │
│  Response:        "Interesting, let's form a working group to evaluate"   │
│  Outcome:         Death by committee                                       │
│                                                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│  TO DESPERATE BUSINESS LEADER                                              │
│                                                                             │
│  What you'd say:  "3 months to working demo. Your KYC backlog cleared.    │
│                    AI that actually works. Auditors can read the output.   │
│                    I'll show you Tuesday."                                 │
│                                                                             │
│  What they hear:  "Solution to my problem. Fast. Demoable. Low risk."     │
│  Response:        "Show me Tuesday. If it works, you have air cover."     │
│  Outcome:         Sponsored pilot that bypasses normal process            │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## The Real Blockers

It's not technical. It's organizational:

| Blocker | Nature | How to Bypass |
|---------|--------|---------------|
| "We're a Java shop" | Identity/tribal | Executive sponsor who doesn't care |
| "Who maintains this?" | Fear of unknown | "My team. Or I'll train yours." |
| "Not invented here" | Ego/territory | Make them look good, give them credit |
| "Security review" | Legitimate but weaponized | Get sponsor to expedite |
| "Architecture board" | Gatekeeping | Sponsor bypasses or you wait 18 months |
| "Resource allocation" | Budget politics | Sponsor provides budget directly |
| "Risk assessment" | CYA culture | "POC with no production exposure" |

**Every blocker has a bypass. The bypass is executive sponsorship.**

---

## The Access Problem

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  THE CHALLENGE                                                             │
│                                                                             │
│  You're in: Solution Architecture                                          │
│  You need:  MD-level Business Sponsor                                      │
│                                                                             │
│  Layers between:                                                           │
│  • Your manager                                                            │
│  • Their manager                                                           │
│  • Director level                                                          │
│  • VP level                                                                │
│  • SVP level                                                               │
│  • MD level                                                                │
│                                                                             │
│  Each layer:                                                               │
│  • Filters what goes up                                                    │
│  • Protects their position                                                 │
│  • Adds delay                                                              │
│  • Dilutes the message                                                     │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Access Strategies

### Strategy 1: Find the Back Door

```
Who do you know who knows someone senior?
• Previous colleagues who moved up
• People you've helped in the past
• Cross-functional contacts (compliance, ops, business)
• External network (conferences, LinkedIn, industry groups)

The ask: "Can you get me 30 minutes with [Name]?"
Not: "Can you advocate for my project?"
```

### Strategy 2: Create Visible Success

```
Build something that gets noticed:
• Demo that someone screenshots and shares
• Solution to a problem that's publicly embarrassing
• Result that makes someone look good

Let it travel up organically:
"Have you seen what Adam built?"
```

### Strategy 3: Attach to a Burning Platform

```
Find the crisis:
• Regulatory deadline with no solution
• Failed project that needs rescue
• Competitor threat that's causing panic
• Audit finding that needs response

Position as: "I can help with [crisis]"
Not: "I have this cool technology"
```

### Strategy 4: The Demo Ambush

```
Get invited to any meeting with senior people:
• Town halls with Q&A
• Strategy sessions
• Innovation showcases
• "Lunch and learn" with executives

Have the demo ready. Wait for the opening.
"Actually, I built something that addresses that..."
```

### Strategy 5: The External Validator

```
Get external credibility:
• Conference talk
• Published article
• Industry recognition
• Vendor partnership (Anthropic? Microsoft?)

Internal people dismiss internal people.
External validation creates internal meetings.
```

---

## The Conversation You Need

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  THE 30 MINUTES THAT MATTER                                                │
│                                                                             │
│  Senior Leader: "I'm told you have something to show me."                 │
│                                                                             │
│  You: "You're under pressure to deliver AI transformation.                │
│        Your teams are proposing 2-year Java projects.                     │
│        I have a working demo of AI-powered KYC onboarding.                │
│        It took 3 months. It works now. Can I show you?"                   │
│                                                                             │
│  [Demo: 5 minutes. Allianz scenario. AI resolves entities. DSL executes.] │
│                                                                             │
│  Senior Leader: "That's interesting. What do you need?"                   │
│                                                                             │
│  You: "Air cover. A real use case. 90 days.                               │
│        If it works, you have a story for [CEO/Board].                     │
│        If it doesn't, you've lost nothing."                               │
│                                                                             │
│  Senior Leader: "Who's blocking this?"                                    │
│                                                                             │
│  You: [Don't name names. Name patterns.]                                  │
│       "The normal process. Architecture review. Resource allocation.      │
│        By the time that clears, the Microsoft announcement is old news."  │
│                                                                             │
│  Senior Leader: "Let me make some calls."                                 │
│                                                                             │
│  [YOU'RE IN]                                                               │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## What Changes With Sponsorship

| Without Sponsor | With Sponsor |
|-----------------|--------------|
| "We need to evaluate this" | "Make it happen" |
| "Architecture board review" | "Skip it, I'll handle the politics" |
| "No budget allocation" | "Use my budget" |
| "Who's going to maintain it?" | "Figure it out, that's your job" |
| "Security review: 6 months" | "Expedite it, call [Name]" |
| "We're a Java shop" | "I don't care, show me results" |

**Sponsorship doesn't remove obstacles. It removes the need to argue about them.**

---

## The Uncomfortable Truth

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                                                                             │
│  Technical excellence is necessary but not sufficient.                     │
│                                                                             │
│  The best solution, without sponsorship, loses to                          │
│  the mediocre solution with sponsorship.                                   │
│                                                                             │
│  Every. Single. Time.                                                      │
│                                                                             │
│  This is not fair. This is not right. This is how it works.               │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Key Quotes

> "Back in the 'shop' terror and backstabbing prevail."

> "If I can get in front of senior people who are desperate enough to take a risk, then I'm in."

> "And that's NOT engineering."

---

## Next Actions

1. **Identify the desperate** - Who's under pressure? Who's failing visibly?
2. **Map the access** - Who do you know who knows them?
3. **Perfect the demo** - 5 minutes, visual, undeniable
4. **Craft the ask** - "Air cover. Real use case. 90 days."
5. **Be ready** - When the door opens, walk through it

---

*The technology is ready. The organization isn't. That's a people problem, not a code problem.*
