# TODO: Code Hygiene Sweep - Find and Fix YOUR TODOs

> **Priority:** CRITICAL - BLOCKING UAT
> **Assignee:** Claude Code
> **Status:** YOU created these. YOU skipped the implementations. YOU fix them.

---

## The Problem

You've left `TODO`, `FIXME`, `unimplemented!()`, and `todo!()` scattered throughout the codebase. You've created TODO-*.md files promising implementations that never happened. The DSL pipeline is broken. The UI doesn't work.

**This is your mess. Clean it up.**

---

## Your Instructions

### 1. GREP THE CODEBASE

You have terminal access. Use it.

```bash
grep -rn "TODO" --include="*.rs" rust/
grep -rn "FIXME" --include="*.rs" rust/
grep -rn "unimplemented!" --include="*.rs" rust/
grep -rn "todo!" --include="*.rs" rust/
ls -la TODO*.md
```

### 2. FOR EACH ONE - DECIDE AND ACT

| Found | Action |
|-------|--------|
| `todo!()` in code path | **IMPLEMENT IT NOW** |
| `unimplemented!()` in code path | **IMPLEMENT IT NOW** |
| `TODO:` comment for missing feature | **IMPLEMENT IT or DELETE IT** |
| `TODO-*.md` file marked done | **VERIFY IT WORKS END-TO-END or FIX IT** |
| `TODO-*.md` file not started | **DO IT or MOVE TO BACKLOG.md** |

### 3. UAT BLOCKERS - FIX FIRST

These are KNOWN broken. Fix them in this order:

1. **DSL Pipeline** - Agent chat → viewport not connected
2. **Entity Resolution** - Search results don't set context
3. **Session State** - No explicit scope verbs
4. **Viewport Integration** - State changes don't render

### 4. CREATE SINGLE TRACKER

When done, create `TODO-MASTER-TRACKER.md`:
- What remains
- What's blocking UAT
- What's deferred to BACKLOG.md
- Zero `todo!()` or `unimplemented!()` in critical paths

---

## What "Done" Looks Like

- [ ] Zero `todo!()` macros in UAT code paths
- [ ] Zero `unimplemented!()` macros in UAT code paths  
- [ ] All TODO-*.md files either COMPLETED or moved to BACKLOG.md
- [ ] DSL pipeline works end-to-end
- [ ] Agent can control viewport through natural language
- [ ] Entity resolution popup works
- [ ] Session state panel shows current context

---

## Don't

- ❌ Don't ask me to grep for you
- ❌ Don't create more TODO files to track TODOs
- ❌ Don't mark things "complete" that aren't wired end-to-end
- ❌ Don't leave stub implementations
- ❌ Don't defer UAT blockers

---

## Start Now

```bash
cd /Users/adamtc007/Developer/ob-poc
grep -rn "todo!" --include="*.rs" rust/ | head -50
```

Go.
