# Tunnel Navigation: The Experiential Brief

## The Feeling

You are the squiddy. You're not looking at a org chart. You're not clicking through pages. You're **swimming through the data structure**.

The taxonomy isn't a diagram - it's a tunnel system. You're inside it. The walls are made of the current context. Ahead of you are branches. Behind you is where you came from. You steer. You have momentum.

---

## Core Experience Principles

### 1. You Are Always IN the Space

There is no "overview mode" that pulls you outside the data. Even at the universe level, you're inside the universe - floating among the clusters, not looking down at them from above.

```
WRONG mental model:         RIGHT mental model:

    [User]                      ════════════════════
       │                        │                  │
       ▼ clicks                 │   You are HERE   │
  ┌─────────┐                   │       ↓          │
  │ Universe│                   │      ◉          │
  │  page   │                   │    ╱   ╲        │
  └─────────┘                   │  [LU]  [IE]     │
       │                        │                  │
       ▼ clicks                 ════════════════════
  ┌─────────┐                   
  │ Cluster │                   The tunnels extend in all directions
  │  page   │                   You see what's ahead and beside you
  └─────────┘                   Not a page. A place.
```

### 2. Movement is Continuous

You don't teleport. You **travel**. When you "dive into" a cluster, the camera doesn't cut - it flies. The cluster grows as you approach. Other clusters recede into peripheral blur. The tunnel walls form around you as you enter.

```
Frame 0:              Frame 15:             Frame 30:

   [LU]  [IE]            [LU]                  ═══════════
     ╲  ╱               ╱    ╲                 │   [LU]  │
      ◉                ◉      ·                │  ╱ │ ╲ │
     you              you    (IE fading)       │[A][B][C]│
                                               │    ◉    │
                                               ═══════════
                                               you're inside LU now
```

The transition takes 400-600ms. You feel the movement. You could abort mid-flight if you changed your mind.

### 3. Forks Present Themselves

When you approach a node with children, you don't see a list. You see **the fork in the tunnel**.

```
Approaching a CBU with 3 key entities:

                    ╱─── [ManCo] ───→ (deeper: directors, UBOs)
                   ╱
You ──────→ [CBU] ────── [IM] ───→ (deeper: portfolio managers)
                   ╲
                    ╲─── [Custodian] ───→ (deeper: sub-custodians)
```

The branches fan out ahead of you. You see where each one leads (preview labels). You steer toward one. As you commit to a direction, the others collapse behind you (but you remember they're there - you can reverse).

### 4. Steering, Not Clicking

Input should feel like **steering a vehicle**, not clicking buttons.

| Input | Squiddy Equivalent | Result |
|-------|-------------------|--------|
| Mouse hover | Turning to look | Highlights branch, shows preview |
| Mouse toward edge | Banking | Camera pans in that direction |
| Click | Thrust forward | Commit to highlighted branch |
| Right-click / Back | Reverse thrust | Back up toward previous fork |
| Scroll | Throttle | Speed up / slow down approach |
| Escape | Full stop | Halt movement, stay in place |

Voice commands are **navigation orders**, not button labels:
- "Go deeper" - thrust toward focused branch
- "Back up" - reverse
- "Show me the ManCo" - turn to face that branch
- "What's down there?" - enhance preview without committing

### 5. Momentum and Inertia

Movement doesn't start and stop instantly. There's:
- **Acceleration** - movement starts slow, builds speed
- **Coasting** - release input, you drift a bit
- **Braking** - explicit stop takes a moment
- **Overshoot** - if you brake hard, slight wobble/settle

This creates **commitment**. You can't nervous-click through the data. Each movement is a small decision. This forces intentionality and creates the feeling of exploration.

```
Velocity curve for "dive into cluster":

Speed
  │
  │        ╭────╮
  │       ╱      ╲
  │      ╱        ╲
  │     ╱          ╲____  ← coast/settle
  │    ╱
  │___╱
  └─────────────────────→ Time
     │   │        │   │
   start│     brake arrive
      peak
```

### 6. Peripheral Awareness

You see more than just your focus. The tunnel has **peripheral vision**.

```
Your view at any moment:

     ┌─────────────────────────────────────────┐
     │ · · ·                           · · ·   │  ← distant: fog/blur
     │        ╲                     ╱          │
     │         ╲                   ╱           │
     │    [Sibling1]    [FOCUS]    [Sibling2]  │  ← same depth: clear
     │                   ╱ │ ╲                 │
     │                  ╱  │  ╲                │
     │              [A]   [B]   [C]            │  ← children: approaching
     │                     │                   │
     │                     ▼                   │  ← deeper: preview
     │                   · · ·                 │
     └─────────────────────────────────────────┘
```

- **Focus** is crisp, full detail
- **Siblings** (same depth, other branches) are visible but muted
- **Children** (branches ahead) are visible, preview detail on hover
- **Parent** (behind) is felt more than seen - you know it's there
- **Distant** (2+ levels away) is fog/blur - structure visible, no detail

### 7. Depth is Physical

Deeper in the taxonomy should **feel** deeper. This is achieved through:

**Visual cues:**
- Tunnel walls narrow as you go deeper (more focused = more constrained)
- Lighting shifts - universe is bright/open, deep nodes are focused/intimate
- Ambient particles increase with depth (like dust in a tunnel)
- Color temperature shifts - warm at surface, cooler at depth

**Audio cues (if/when added):**
- Universe: open, reverberant, spacious
- Deep: close, intimate, focused
- Movement: whoosh/rush sounds scale with speed

**Structural cues:**
- Branching factor decreases with depth (fewer choices = more specific)
- Node size increases with depth (closer = bigger)
- Edge lengths shorten with depth (tighter connections)

---

## Navigation Scenarios

### Scenario 1: Exploring Unknown Territory

You've been asked: "Find who controls the Luxembourg funds for Allianz."

```
[Start: Universe level, floating among jurisdiction clusters]

You see: Glowing orbs for LU, IE, DE, FR, UK, CH scattered around you.
         LU is largest (most CBUs). Each pulses with aggregate risk color.

Voice: "Luxembourg"
→ You turn to face LU. It highlights. Preview shows: "177 CBUs, 23 clients"

You drift toward it... closer... the orb grows...
→ Other clusters fade to peripheral blur
→ The orb's surface becomes visible - you see CBU-dots orbiting inside

Voice: "Enter"
→ You punch through the surface. Brief transition. Now you're INSIDE Luxembourg.
→ The "tunnel" forms - Luxembourg's CBUs arranged around you
→ Grouped by client. You see the Allianz cluster ahead.

Voice: "Allianz"
→ You turn toward the Allianz grouping. 47 CBUs. Shared entities visible as connectors.
→ The ManCo (Allianz GI GmbH) appears at a hub - lines to 35 CBUs

You drift toward the ManCo...
→ Its branches become visible: Directors, UBOs, connected CBUs

Voice: "Who owns this?"
→ The ownership branch highlights. You see the chain extending upward:
   ManCo → Allianz Asset Management → Allianz SE (Ultimate)
→ Without diving in, you see the answer. You can go deeper or move on.

Voice: "Back up. Show me the high-risk funds."
→ You reverse. ManCo recedes. You're back in the Allianz cluster.
→ Risk filter applies. 3 CBUs glow red. Others dim.
→ You steer toward one...
```

### Scenario 2: Known Destination

You know exactly where you're going: CBU "Allianz Lux Fund Alpha"

```
Voice: "Take me to Allianz Lux Fund Alpha"
→ System plots course: Universe → LU → Allianz → Fund Alpha
→ You see the path light up - a tunnel forming through the structure
→ Automatic pilot engages

You fly through:
→ Universe blurs → LU looms → enters → Allianz cluster → Fund Alpha
→ 2 seconds total. You skip the exploration. Direct route.

You arrive inside Fund Alpha's graph.
→ Now you explore at your leisure.
```

### Scenario 3: Breadcrumb Navigation

You're deep in an entity's ownership chain. You need to go back to the cluster level.

```
Current position: 
  Universe → LU → Allianz → Fund Alpha → ManCo → Director → UBO

The breadcrumb isn't a list of links. It's a **trail behind you**.
You can feel the depth - you're 6 levels deep.

Voice: "Surface to cluster"
→ You start rising. The UBO recedes. Director recedes. ManCo recedes.
→ Fund Alpha shrinks to a node. Other Allianz funds become visible.
→ You're back at cluster level (depth 2), seeing all Allianz CBUs.

The ascent takes ~800ms. You feel the layers peeling away.
Fast, but not instant. You could interrupt mid-ascent.
```

---

## Fork Presentation Patterns

### Pattern A: Fan-Out (Few Children)

When a node has 2-5 children, they fan out ahead of you:

```
              [Child 1]
             ╱
[Parent] ──────[Child 2]
             ╲
              [Child 3]

Fan angle: ~30° per child
Children visible simultaneously
Hover any to see preview
```

### Pattern B: Carousel (Medium Children)

When a node has 6-15 children, carousel around you:

```
        [C3]    [C4]    [C5]
          ╲      │      ╱
    [C2]─────[Parent]─────[C6]
          ╱      │      ╲
        [C1]    [C8]    [C7]

Track left/right to rotate carousel
Focused child is ahead of you
Others wrap around periphery
```

### Pattern C: Tunnel Grid (Many Children)

When a node has 15+ children, they form a tunnel grid:

```
    ┌───┬───┬───┬───┬───┐
    │ · │ · │ · │ · │ · │  ← row 3 (distant)
    ├───┼───┼───┼───┼───┤
    │ C │ D │ E │ F │ G │  ← row 2
    ├───┼───┼───┼───┼───┤
    │ A │ B │[*]│ H │ I │  ← row 1 (closest), [*] = current focus
    └───┴───┴───┴───┴───┘
          You →

Arrow keys / WASD to navigate grid
Scroll/zoom to see more rows
Focus on one to see preview, click to dive
```

### Pattern D: Nested Preview (Inline Expansion)

For "show me more" without navigation:

```
Before "enhance":              After "enhance":

   [ManCo]                        [ManCo]
                                     │
                                     ├── [Director: J.Smith]
                                     ├── [Director: M.Jones]
                                     ├── [UBO: Person A (25%)]
                                     └── [Parent: Allianz AM]

The branch extends inline. You haven't moved.
You're seeing deeper without going deeper.
"Pull back" retracts the preview.
"Dive in" to Director commits to navigation.
```

---

## Camera Behavior

### Following vs. Leading

The camera should **lead**, not follow. It knows where you're going before you get there.

```
You're at A. You click to go to B.

WRONG (following):
  Frame 0:  Camera on A
  Frame 10: You start moving toward B
  Frame 20: Camera starts following you
  Frame 30: You arrive at B
  Frame 40: Camera catches up
  → Feels sluggish, reactive

RIGHT (leading):
  Frame 0:  Camera on A
  Frame 5:  Camera starts moving toward B
  Frame 10: You start moving toward B
  Frame 20: Camera arrives, prepares framing
  Frame 30: You arrive at B, perfectly framed
  → Feels responsive, cinematic
```

### Framing Anticipation

Camera should frame the **destination**, not the current position:

```
Diving from cluster into CBU:

Frame 0:   Cluster in center. CBU visible as small node.
           Camera: centered on cluster

Frame 10:  You're moving toward CBU.
           Camera: panning to put CBU in center-bottom, 
                   leaving room above for CBU's children

Frame 20:  CBU fills center. Its children becoming visible above.
           Camera: CBU centered, children in frame

Frame 30:  You're inside CBU. Children spread around you.
           Camera: slight pull back to show full layout
```

### Depth-Based Zoom

Zoom level correlates with depth:

| Depth | Zoom | Field of View | Detail Level |
|-------|------|---------------|--------------|
| 0 (Universe) | 0.3x | Very wide | Clusters only |
| 1 (Cluster) | 0.6x | Wide | CBU cards |
| 2 (CBU) | 1.0x | Normal | Entity nodes |
| 3 (Entity) | 1.5x | Narrow | Full attributes |
| 4+ (Deep) | 2.0x+ | Very narrow | Document-level |

Zooming happens automatically as you navigate depth.
Manual zoom override available but snaps back on navigation.

---

## The Tunnel Walls

What are the "walls" of the tunnel? They're the **context boundaries**.

When you're inside "Luxembourg cluster":
- The walls are made of "Luxembourg-ness"
- Everything you see belongs to Luxembourg
- The walls fade to show other jurisdictions exist (peripheral blur)
- But you can't see Ireland's details until you exit and enter Ireland

When you're inside "Fund Alpha":
- The walls are made of "Fund Alpha membership"
- Everything you see has a role in Fund Alpha
- Shared entities show connections to other CBUs (tunnels branching off)
- But those other CBUs are glimpsed, not detailed

The walls create **focus**. They answer the question: "What context am I in?"

```
Visual representation of tunnel walls:

    ═══════════════════════════════════════════
    ║                                         ║
    ║  Context: Luxembourg > Allianz > Fund A ║
    ║  ─────────────────────────────────────  ║
    ║                                         ║
    ║       [ManCo]────[IM]────[Custodian]   ║
    ║          │                              ║
    ║       [Director]                        ║
    ║          │                              ║
    ║        [UBO]                            ║
    ║                                         ║
    ═══════════════════════════════════════════
              │
              │ (tunnel continues to related CBUs)
              ▼
         · · · · · (peripheral glimpse of Fund B sharing the ManCo)
```

---

## Interruption and Undo

Navigation must be interruptible. You're piloting, not executing transactions.

### Mid-Flight Abort

If you're flying toward a destination and change your mind:
- Press Escape / right-click / "stop"
- Momentum decays (don't instant stop)
- You halt between origin and destination
- Can then steer elsewhere or reverse

### Undo Stack

Every navigation move is undoable:
- "Back" reverses the last move
- "Back back back" (or "surface") reverses multiple
- Undo is also animated (you fly backwards)
- Redo exists if you undo then want to go forward again

### Bookmark / Anchor

You can drop an anchor:
- "Mark this spot" - creates a saved position
- "Return to mark" - flies back instantly
- Useful for comparing two distant locations

---

## Summary: What Makes It Feel Like Piloting

| Principle | Implementation |
|-----------|----------------|
| **Continuous space** | No page loads. Camera flies. Everything exists in one 3D space. |
| **Steering input** | Mouse position = direction. Click = thrust. Scroll = speed. |
| **Momentum** | Acceleration, coasting, braking. Movement has physics. |
| **Peripheral vision** | Siblings visible but muted. Depth creates focus gradient. |
| **Anticipatory camera** | Camera leads movement. Frames destination before arrival. |
| **Interruptible** | Any move can abort. Undo is animated. You're always in control. |
| **Forks present choices** | Branches fan out ahead. You see before you commit. |
| **Depth is physical** | Deeper = narrower tunnel, closer nodes, warmer light. |
| **Context walls** | Current scope constrains visibility. Focus has boundaries. |

The user shouldn't feel like they're "using an application."
They should feel like they're **flying through information.**

---

## Implementation Notes

This brief doesn't replace the technical docs. It sits above them.

- `GALAXY-NODE-EDGE-TAXONOMY.md` - What the nodes/edges ARE
- `NATURAL-TREE-TRAVERSAL.md` - How animations WORK
- `ESPER-NAVIGATION-MODEL.md` - What commands DO
- **This doc** - How it should FEEL

When implementing, ask: "Does this feel like piloting through tunnels?"
If not, the mechanics are right but the feel is wrong.
