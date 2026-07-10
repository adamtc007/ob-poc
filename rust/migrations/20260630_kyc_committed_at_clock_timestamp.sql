-- EOP-DD-KYCUBO-002 §5/B1 correctness fix — committed_at must be the recovery axis.
--
-- Supersedes the `DEFAULT now()` (and the "monotonic with seq by construction"
-- claim) in 20260630_kyc_intent_events.sql.
--
-- WHY: committed_at is the transaction-time recovery axis (B1/D1/K-33) —
-- recovery folds the `committed_at <= T` prefix. For that prefix to be a TRUE
-- prefix (never a holey fold), committed_at must be monotonic non-decreasing
-- with seq. `now()` is **transaction-START** time (statement-stable), so a
-- transaction that BEGINs early, then blocks on the per-subject FOR UPDATE lock,
-- is assigned a LATE seq but stamps an EARLY committed_at → non-monotonic →
-- `committed_at <= T` can return a hole.
--
-- `clock_timestamp()` is the real wall-clock at the moment of the INSERT, which
-- executes UNDER the serializing per-subject lock AFTER seq allocation. Because
-- the lock strictly orders the critical section, the insert wall-clock advances
-- with seq → committed_at is monotonic with seq. (The recovery query is also
-- written defensively as a true seq-prefix, so it stays correct even under a
-- rare wall-clock regression.)
--
-- Safe: additive default change; no rows depend on the old default.

ALTER TABLE "ob-poc".kyc_intent_events
    ALTER COLUMN committed_at SET DEFAULT clock_timestamp();
